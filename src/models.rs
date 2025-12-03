use serde::{Deserialize, Serialize};
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