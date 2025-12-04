//! WebSocket-based large file upload handler
//!
//! This module handles large file uploads via WebSocket to bypass
//! Cloudflare's 100MB HTTP body size limit.

use axum::{
    extract::{
        ws::{Message, WebSocket},
        Query, State, WebSocketUpgrade,
    },
    response::Response,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::AppState;

/// Query parameters for WebSocket upload endpoint
#[derive(Deserialize)]
pub struct WsUploadQuery {
    /// Base64 encoded "username:password"
    pub auth: Option<String>,
}

/// Client to server messages
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMessage {
    /// Authentication message (alternative to query param)
    Auth { username: String, password: String },
    /// Initialize upload session
    Init {
        filename: String,
        size: u64,
        path: String,
    },
    /// Upload complete signal
    Complete,
    /// Cancel upload
    Cancel,
}

/// Server to client messages
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerMessage {
    /// Authentication required
    AuthRequired,
    /// Authentication successful
    AuthOk,
    /// Authentication failed
    AuthFailed { message: String },
    /// Upload session initialized
    InitOk { upload_id: String },
    /// Upload progress update
    Progress {
        received: u64,
        total: u64,
        percent: u8,
    },
    /// Upload completed successfully
    CompleteOk { path: String, size: u64 },
    /// Error occurred
    Error { code: String, message: String },
}

/// Upload session state
struct UploadSession {
    upload_id: String,
    filename: String,
    target_path: PathBuf,
    temp_path: PathBuf,
    total_size: u64,
    received_size: u64,
    file: Option<fs::File>,
}

/// WebSocket upload handler - upgrade HTTP to WebSocket
pub async fn ws_upload_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(query): Query<WsUploadQuery>,
) -> Response {
    // Try to authenticate from query parameter
    let auth_result = if let Some(auth) = &query.auth {
        validate_auth(auth, &state.username, &state.password)
    } else {
        None
    };

    ws.on_upgrade(move |socket| handle_upload(socket, state, auth_result))
}

/// Validate base64 encoded auth string
fn validate_auth(auth: &str, expected_user: &str, expected_pass: &str) -> Option<bool> {
    if let Ok(decoded) = BASE64.decode(auth) {
        if let Ok(credentials) = String::from_utf8(decoded) {
            if let Some((user, pass)) = credentials.split_once(':') {
                return Some(user == expected_user && pass == expected_pass);
            }
        }
    }
    Some(false)
}

/// Handle WebSocket upload connection
async fn handle_upload(mut socket: WebSocket, state: AppState, pre_auth: Option<bool>) {
    let mut authenticated = pre_auth.unwrap_or(false);
    let mut session: Option<UploadSession> = None;

    // If not pre-authenticated, request auth
    if !authenticated {
        if send_message(&mut socket, &ServerMessage::AuthRequired)
            .await
            .is_err()
        {
            return;
        }
    }

    // Main message loop
    while let Some(msg) = socket.recv().await {
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                warn!("WebSocket receive error: {}", e);
                break;
            }
        };

        match msg {
            Message::Text(text) => {
                // Parse JSON message
                let client_msg: ClientMessage = match serde_json::from_str(&text) {
                    Ok(m) => m,
                    Err(e) => {
                        let _ = send_message(
                            &mut socket,
                            &ServerMessage::Error {
                                code: "INVALID_MESSAGE".to_string(),
                                message: format!("Invalid JSON: {}", e),
                            },
                        )
                        .await;
                        continue;
                    }
                };

                match client_msg {
                    ClientMessage::Auth { username, password } => {
                        if username == state.username && password == state.password {
                            authenticated = true;
                            let _ = send_message(&mut socket, &ServerMessage::AuthOk).await;
                        } else {
                            let _ = send_message(
                                &mut socket,
                                &ServerMessage::AuthFailed {
                                    message: "Invalid credentials".to_string(),
                                },
                            )
                            .await;
                        }
                    }

                    ClientMessage::Init {
                        filename,
                        size,
                        path,
                    } => {
                        if !authenticated {
                            let _ = send_message(&mut socket, &ServerMessage::AuthRequired).await;
                            continue;
                        }

                        // Validate and create upload session
                        match create_upload_session(&state, &filename, size, &path).await {
                            Ok(s) => {
                                let upload_id = s.upload_id.clone();
                                session = Some(s);
                                let _ = send_message(
                                    &mut socket,
                                    &ServerMessage::InitOk { upload_id },
                                )
                                .await;
                            }
                            Err(e) => {
                                let _ = send_message(
                                    &mut socket,
                                    &ServerMessage::Error {
                                        code: "INIT_FAILED".to_string(),
                                        message: e,
                                    },
                                )
                                .await;
                            }
                        }
                    }

                    ClientMessage::Complete => {
                        info!("Received complete message");
                        if let Some(mut s) = session.take() {
                            info!("Processing complete for session: {}, received {} bytes", s.upload_id, s.received_size);
                            match complete_upload(&mut s).await {
                                Ok(()) => {
                                    info!(
                                        "Upload completed: {} ({} bytes)",
                                        s.target_path.display(),
                                        s.received_size
                                    );
                                    let _ = send_message(
                                        &mut socket,
                                        &ServerMessage::CompleteOk {
                                            path: format!("/{}", s.filename),
                                            size: s.received_size,
                                        },
                                    )
                                    .await;
                                }
                                Err(e) => {
                                    error!("Complete upload error: {}", e);
                                    let _ = send_message(
                                        &mut socket,
                                        &ServerMessage::Error {
                                            code: "COMPLETE_FAILED".to_string(),
                                            message: e,
                                        },
                                    )
                                    .await;
                                }
                            }
                        } else {
                            warn!("Complete received but no active session");
                        }
                        break;
                    }

                    ClientMessage::Cancel => {
                        if let Some(s) = session.take() {
                            // Clean up temp file
                            let _ = fs::remove_file(&s.temp_path).await;
                            info!("Upload cancelled: {}", s.filename);
                        }
                        break;
                    }
                }
            }

            Message::Binary(data) => {
                if !authenticated {
                    let _ = send_message(&mut socket, &ServerMessage::AuthRequired).await;
                    continue;
                }

                if let Some(ref mut s) = session {
                    match write_chunk(s, &data).await {
                        Ok(()) => {
                            // Send progress every 2MB - less frequent for better throughput
                            // Frequent progress messages can slow down the upload over high-latency connections
                            let progress_interval = 2 * 1024 * 1024; // 2MB
                            let prev_milestone = (s.received_size - data.len() as u64) / progress_interval;
                            let curr_milestone = s.received_size / progress_interval;
                            
                            if curr_milestone > prev_milestone || s.received_size == s.total_size {
                                let percent = if s.total_size > 0 {
                                    ((s.received_size as f64 / s.total_size as f64) * 100.0) as u8
                                } else {
                                    0
                                };
                                
                                let _ = send_message(
                                    &mut socket,
                                    &ServerMessage::Progress {
                                        received: s.received_size,
                                        total: s.total_size,
                                        percent,
                                    },
                                )
                                .await;
                            }
                        }
                        Err(e) => {
                            error!("Write chunk error: {}", e);
                            let _ = send_message(
                                &mut socket,
                                &ServerMessage::Error {
                                    code: "WRITE_FAILED".to_string(),
                                    message: e,
                                },
                            )
                            .await;
                            // Clean up
                            let _ = fs::remove_file(&s.temp_path).await;
                            break;
                        }
                    }
                } else {
                    let _ = send_message(
                        &mut socket,
                        &ServerMessage::Error {
                            code: "NO_SESSION".to_string(),
                            message: "Upload not initialized".to_string(),
                        },
                    )
                    .await;
                }
            }

            Message::Close(_) => {
                // Clean up if upload was in progress
                if let Some(s) = session.take() {
                    let _ = fs::remove_file(&s.temp_path).await;
                    warn!("Upload connection closed unexpectedly: {}", s.filename);
                }
                break;
            }

            _ => {}
        }
    }
}

/// Send a JSON message to the client
async fn send_message(socket: &mut WebSocket, msg: &ServerMessage) -> Result<(), String> {
    let json = serde_json::to_string(msg).map_err(|e| e.to_string())?;
    socket
        .send(Message::Text(json))
        .await
        .map_err(|e| e.to_string())
}

/// Create a new upload session
async fn create_upload_session(
    state: &AppState,
    filename: &str,
    size: u64,
    path: &str,
) -> Result<UploadSession, String> {
    // Validate path
    let normalized = path.trim_start_matches('/');
    let target_dir = if normalized.is_empty() {
        state.root_dir.clone()
    } else {
        state.root_dir.join(normalized)
    };

    // Security check - ensure path is under root
    let target_dir = target_dir
        .canonicalize()
        .unwrap_or_else(|_| target_dir.clone());
    if !target_dir.starts_with(&state.root_dir) {
        return Err("Invalid path: access denied".to_string());
    }

    // Ensure directory exists
    fs::create_dir_all(&target_dir)
        .await
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    // Generate upload ID and temp file path
    // Put temp file in same directory as target to enable fast rename (same filesystem)
    let upload_id = Uuid::new_v4().to_string();
    let temp_path = target_dir.join(format!(".upload_{}.tmp", upload_id));
    let target_path = target_dir.join(filename);

    // Create temp file
    let file = fs::File::create(&temp_path)
        .await
        .map_err(|e| format!("Failed to create temp file: {}", e))?;

    info!(
        "Upload session created: {} -> {} ({} bytes)",
        upload_id,
        target_path.display(),
        size
    );

    Ok(UploadSession {
        upload_id,
        filename: filename.to_string(),
        target_path,
        temp_path,
        total_size: size,
        received_size: 0,
        file: Some(file),
    })
}

/// Write a chunk of data to the upload file
async fn write_chunk(session: &mut UploadSession, data: &[u8]) -> Result<(), String> {
    if let Some(ref mut file) = session.file {
        file.write_all(data)
            .await
            .map_err(|e| format!("Write failed: {}", e))?;
        session.received_size += data.len() as u64;
        Ok(())
    } else {
        Err("File not open".to_string())
    }
}

/// Complete the upload - flush and move file to target location
async fn complete_upload(session: &mut UploadSession) -> Result<(), String> {
    // Flush and close the file
    if let Some(mut file) = session.file.take() {
        file.flush()
            .await
            .map_err(|e| format!("Flush failed: {}", e))?;
    }

    // Rename temp file to target (fast, same filesystem)
    fs::rename(&session.temp_path, &session.target_path)
        .await
        .map_err(|e| format!("Failed to move file: {}", e))?;

    Ok(())
}

