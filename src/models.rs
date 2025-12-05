use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// API 响应包装
#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(flatten)]
    pub data: Option<T>,
}
impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            error: None,
            data: Some(data),
        }
    }
    pub fn error(message: impl Into<String>) -> ApiResponse<()> {
        ApiResponse {
            success: false,
            error: Some(message.into()),
            data: None,
        }
    }
}
/// 文件信息
#[derive(Serialize, Clone)]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub file_type: String,
    pub size: u64,
    #[serde(rename = "sizeFormatted")]
    pub size_formatted: String,
    pub modified: String,
    pub created: String,
}
/// 文件列表响应
#[derive(Serialize)]
pub struct FilesResponse {
    pub path: String,
    pub files: Vec<FileInfo>,
}
/// 文件夹列表响应
#[derive(Serialize)]
pub struct FoldersResponse {
    pub folders: Vec<FolderItem>,
}
#[derive(Serialize)]
pub struct FolderItem {
    pub path: String,
    pub display: String,
}
/// 磁盘信息响应
#[derive(Serialize)]
pub struct DiskResponse {
    pub total: u64,
    pub used: u64,
    pub free: u64,
    #[serde(rename = "usedFormatted")]
    pub used_formatted: String,
}
/// 文件详情响应
#[derive(Serialize)]
pub struct InfoResponse {
    pub info: FileInfoDetail,
}
#[derive(Serialize)]
pub struct FileInfoDetail {
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub file_type: String,
    pub size: u64,
    #[serde(rename = "sizeFormatted")]
    pub size_formatted: String,
    pub modified: String,
    pub created: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<usize>,
}
/// 搜索结果响应
#[derive(Serialize)]
pub struct SearchResponse {
    pub results: Vec<FileInfo>,
}
/// 上传结果响应
#[derive(Serialize)]
pub struct UploadResponse {
    pub files: Vec<UploadedFile>,
}
#[derive(Serialize)]
pub struct UploadedFile {
    pub name: String,
    pub size: u64,
    pub path: String,
}
/// 操作结果响应
#[derive(Serialize)]
pub struct OperationResponse {
    pub message: String,
    #[serde(rename = "newPath", skip_serializing_if = "Option::is_none")]
    pub new_path: Option<String>,
}
// ========== 请求体 ==========
#[derive(Deserialize)]
pub struct CreateFolderRequest {
    pub path: String,
    pub name: String,
}
#[derive(Deserialize)]
pub struct RenameRequest {
    pub path: String,
    #[serde(rename = "newName")]
    pub new_name: String,
}
#[derive(Deserialize)]
pub struct MoveRequest {
    pub source: String,
    pub destination: String,
}
#[derive(Deserialize)]
pub struct CopyRequest {
    pub source: String,
    pub destination: String,
}
#[derive(Deserialize)]
pub struct DeleteRequest {
    pub path: String,
}
// ========== 查询参数 ==========
#[derive(Deserialize)]
pub struct PathQuery {
    pub path: Option<String>,
}
#[derive(Deserialize)]
pub struct SearchQuery {
    pub query: String,
    pub path: Option<String>,
}

// ========== Chunked Upload ==========

/// Chunked upload session info
#[derive(Clone)]
pub struct UploadSession {
    pub upload_id: String,
    pub filename: String,
    pub total_size: u64,
    pub total_chunks: u32,
    pub chunk_size: u64,
    pub upload_path: std::path::PathBuf,
    pub temp_dir: std::path::PathBuf,
    pub received_chunks: Vec<bool>,
    pub created_at: std::time::Instant,
}

/// Global upload sessions manager
pub type UploadSessions = Arc<RwLock<HashMap<String, UploadSession>>>;

/// Create a new upload sessions manager
pub fn new_upload_sessions() -> UploadSessions {
    Arc::new(RwLock::new(HashMap::new()))
}

/// Request to initialize chunked upload
#[derive(Deserialize)]
pub struct ChunkedUploadInitRequest {
    pub path: String,
    pub filename: String,
    #[serde(rename = "totalSize")]
    pub total_size: u64,
    #[serde(rename = "chunkSize")]
    pub chunk_size: u64,
    #[serde(rename = "totalChunks")]
    pub total_chunks: u32,
}

/// Response for chunked upload init
#[derive(Serialize)]
pub struct ChunkedUploadInitResponse {
    #[serde(rename = "uploadId")]
    pub upload_id: String,
    #[serde(rename = "chunkSize")]
    pub chunk_size: u64,
}

/// Query params for chunk upload
#[derive(Deserialize)]
pub struct ChunkUploadQuery {
    #[serde(rename = "uploadId")]
    pub upload_id: String,
    #[serde(rename = "chunkIndex")]
    pub chunk_index: u32,
}

/// Response for chunk upload
#[derive(Serialize)]
pub struct ChunkUploadResponse {
    #[serde(rename = "chunkIndex")]
    pub chunk_index: u32,
    pub received: bool,
}

/// Request to complete chunked upload
#[derive(Deserialize)]
pub struct ChunkedUploadCompleteRequest {
    #[serde(rename = "uploadId")]
    pub upload_id: String,
}

/// Response for chunked upload complete
#[derive(Serialize)]
pub struct ChunkedUploadCompleteResponse {
    pub name: String,
    pub size: u64,
    pub path: String,
}

/// Request to abort chunked upload
#[derive(Deserialize)]
pub struct ChunkedUploadAbortRequest {
    #[serde(rename = "uploadId")]
    pub upload_id: String,
}