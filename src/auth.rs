use axum::{
    body::Body,
    extract::State,
    http::{header, Request, StatusCode},
    middleware::Next,
    response::Response,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use crate::AppState;
/// HTTP Basic Authentication 中间件
pub async fn auth_middleware(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // 获取 Authorization header
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());
    match auth_header {
        Some(auth) if auth.starts_with("Basic ") => {
            let credentials = auth.trim_start_matches("Basic ");

            // 解码 Base64
            if let Ok(decoded) = STANDARD.decode(credentials) {
                if let Ok(credential_str) = String::from_utf8(decoded) {
                    // 分割用户名和密码
                    if let Some((username, password)) = credential_str.split_once(':') {
                        // 验证用户名和密码
                        if username == state.username && password == state.password {
                            return Ok(next.run(request).await);
                        }
                    }
                }
            }
        }
        _ => {}
    }
    // 认证失败，返回 401
    Ok(Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header(
            header::WWW_AUTHENTICATE,
            "Basic realm=\"File Manager\", charset=\"UTF-8\"",
        )
        .body(Body::from("Unauthorized"))
        .unwrap())
}