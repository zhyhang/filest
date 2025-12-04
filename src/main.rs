//! # Filest - 远程文件管理器
//!
//! 一个基于 Axum 的 Web 文件管理器，支持 HTTP Basic 认证
//!
//! ## 使用方法
//!
//! ```bash
//! # 编译
//! cargo build --release
//!
//! # 运行（使用默认配置）
//! ./filest
//!
//! # 自定义配置
//! ./filest --root /path/to/files --port 8080 --user admin --password secret
//! ```
mod auth;
mod handlers;
mod models;
mod ws_upload;
use axum::{
    body::Body,
    extract::DefaultBodyLimit,
    http::{header, Method, Response, StatusCode},
    middleware,
    routing::{delete, get, post, put},
    Router,
};
use clap::Parser;
use std::{net::SocketAddr, path::PathBuf};
use tower_http::cors::{Any, CorsLayer};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
/// 应用状态
#[derive(Clone)]
pub struct AppState {
    pub root_dir: PathBuf,
    pub username: String,
    pub password: String,
}
/// 命令行参数
#[derive(Parser, Debug)]
#[command(name = "filest")]
#[command(author = "File Manager")]
#[command(version = "1.0")]
#[command(about = "远程文件管理器 - Web UI + HTTP API", long_about = None)]
struct Args {
    /// 文件根目录
    #[arg(short, long, default_value = "./files")]
    root: PathBuf,
    /// 服务端口
    #[arg(short, long, default_value_t = 3000)]
    port: u16,
    /// 用户名
    #[arg(short, long, default_value = "admin")]
    user: String,
    /// 密码
    #[arg(short = 'P', long, default_value = "admin123")]
    password: String,
    /// 绑定地址
    #[arg(short, long, default_value = "0.0.0.0")]
    bind: String,
}
/// 嵌入的前端 HTML
const INDEX_HTML: &str = include_str!("../static/index.html");
/// 提供前端页面
async fn serve_index() -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(INDEX_HTML))
        .unwrap()
}
#[tokio::main]
async fn main() {
    // 初始化日志
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "filest=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    // 解析命令行参数
    let args = Args::parse();
    // 确保根目录存在
    let root_dir = args.root.canonicalize().unwrap_or_else(|_| {
        std::fs::create_dir_all(&args.root).expect("Failed to create root directory");
        args.root.canonicalize().expect("Failed to resolve root directory")
    });
    info!("文件根目录: {:?}", root_dir);
    // 创建应用状态
    let state = AppState {
        root_dir,
        username: args.user.clone(),
        password: args.password.clone(),
    };
    // CORS 配置
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers(Any);
    // API routes (require authentication)
    // Set upload limit to 500MB for large file uploads
    let api_routes = Router::new()
        .route("/files", get(handlers::get_files))
        .route("/folder", post(handlers::create_folder))
        .route("/upload", post(handlers::upload_files))
        .route("/download", get(handlers::download_file))
        .route("/rename", put(handlers::rename))
        .route("/move", put(handlers::move_file))
        .route("/copy", post(handlers::copy_file))
        .route("/delete", delete(handlers::delete_file))
        .route("/info", get(handlers::get_info))
        .route("/folders", get(handlers::get_folders))
        .route("/disk", get(handlers::get_disk_info))
        .route("/search", get(handlers::search_files))
        .layer(DefaultBodyLimit::max(500 * 1024 * 1024)) // 500MB limit
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ));

    // WebSocket routes (handle auth internally, no middleware)
    let ws_routes = Router::new()
        .route("/upload", get(ws_upload::ws_upload_handler));

    // Main routes - static resources don't require authentication
    let app = Router::new()
        .route("/", get(serve_index))
        .nest("/api", api_routes)
        .nest("/api/ws", ws_routes)
        .layer(cors)
        .with_state(state);
    // 启动服务器
    let addr: SocketAddr = format!("{}:{}", args.bind, args.port)
        .parse()
        .expect("Invalid address");
    println!(
        r#"
╔════════════════════════════════════════════════════════════════╗
║           Filest - 远程文件管理器 v1.0                          ║
╠════════════════════════════════════════════════════════════════╣
║  访问地址:  http://{}:{:<36}║
║  文件目录:  {:<50}║
║  用户名:    {:<50}║
║  密码:      {:<50}║
╠════════════════════════════════════════════════════════════════╣
║  使用 Ctrl+C 停止服务器                                         ║
╚════════════════════════════════════════════════════════════════╝
"#,
        if args.bind == "0.0.0.0" { "localhost" } else { &args.bind },
        args.port,
        args.root.display(),
        args.user,
        args.password
    );
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}