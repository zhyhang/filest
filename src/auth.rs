use axum::{
    body::Body,
    extract::State,
    http::{header, Request, StatusCode},
    middleware::Next,
    response::Response,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use crate::AppState;
/// HTTP Basic Authentication middleware
pub async fn auth_middleware(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Get Authorization header
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    // Track whether client attempted authentication
    let has_auth_header = auth_header.is_some();

    match auth_header {
        Some(auth) if auth.starts_with("Basic ") => {
            let credentials = auth.trim_start_matches("Basic ");

            // Decode Base64
            if let Ok(decoded) = STANDARD.decode(credentials) {
                if let Ok(credential_str) = String::from_utf8(decoded) {
                    // Split username and password
                    if let Some((username, password)) = credential_str.split_once(':') {
                        // Verify credentials
                        if username == state.username && password == state.password {
                            return Ok(next.run(request).await);
                        }
                    }
                }
            }
        }
        _ => {}
    }

    // Authentication failed, return 401
    // Only include WWW-Authenticate header if client didn't provide credentials
    // This prevents browser from showing built-in auth dialog when frontend handles auth
    let mut response = Response::builder()
        .status(StatusCode::UNAUTHORIZED);
    
    if !has_auth_header {
        // No auth header provided - include WWW-Authenticate for proper HTTP semantics
        response = response.header(
            header::WWW_AUTHENTICATE,
            "Basic realm=\"File Manager\", charset=\"UTF-8\"",
        );
    }
    // When auth header was provided but invalid, don't include WWW-Authenticate
    // This allows frontend to handle the error without browser interference

    Ok(response
        .body(Body::from("Unauthorized"))
        .unwrap())
}