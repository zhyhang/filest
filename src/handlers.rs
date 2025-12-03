use axum::{
    body::Body,
    extract::{Multipart, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Local};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use crate::models::*;
use crate::AppState;
// ========== 辅助函数 ==========
/// 格式化文件大小
fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if bytes == 0 {
        return "0 B".to_string();
    }
    let k = 1024_f64;
    let i = (bytes as f64).log(k).floor() as usize;
    let i = i.min(UNITS.len() - 1);
    format!("{:.2} {}", bytes as f64 / k.powi(i as i32), UNITS[i])
}
/// Result of safe_path containing both logical and actual paths
struct SafePathResult {
    /// The logical path (as user requested, relative to root)
    logical: PathBuf,
    /// The actual path on disk (symlinks resolved)
    actual: PathBuf,
}

/// Safe path check to prevent path traversal attacks
fn safe_path(root: &Path, user_path: &str) -> Result<SafePathResult, String> {
    // Normalize user path: remove leading slashes and handle .. components
    let normalized = user_path.trim_start_matches('/');
    
    // Build path without following symlinks first for security check
    let mut logical_path = root.to_path_buf();
    for component in normalized.split('/') {
        match component {
            "" | "." => continue,
            ".." => {
                // Don't allow going above root
                if logical_path == root {
                    return Err("Access denied: Invalid path".to_string());
                }
                logical_path.pop();
            }
            name => {
                logical_path.push(name);
            }
        }
    }
    
    // Verify logical path is under root
    if !logical_path.starts_with(root) {
        return Err("Access denied: Invalid path".to_string());
    }
    
    // Now get the actual path (following symlinks) for file operations
    // If the path doesn't exist yet, use the logical path
    let actual_path = if logical_path.exists() {
        logical_path.canonicalize().unwrap_or_else(|_| logical_path.clone())
    } else {
        logical_path.clone()
    };
    
    Ok(SafePathResult {
        logical: logical_path,
        actual: actual_path,
    })
}
/// 获取相对路径
fn relative_path(root: &Path, full_path: &Path) -> String {
    match full_path.strip_prefix(root) {
        Ok(rel) => {
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            if rel_str.is_empty() {
                "/".to_string()
            } else {
                format!("/{}", rel_str)
            }
        }
        Err(_) => "/".to_string(),
    }
}
/// 格式化时间
fn format_time(time: std::time::SystemTime) -> String {
    let datetime: DateTime<Local> = time.into();
    datetime.format("%Y-%m-%d %H:%M").to_string()
}
/// 获取文件信息
async fn get_file_info(root: &Path, path: &Path) -> Result<FileInfo, String> {
    let metadata = fs::metadata(path)
        .await
        .map_err(|e| format!("Failed to get metadata: {}", e))?;

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let file_type = if metadata.is_dir() { "folder" } else { "file" }.to_string();
    let size = metadata.len();

    let modified = metadata
        .modified()
        .map(format_time)
        .unwrap_or_else(|_| "-".to_string());

    let created = metadata
        .created()
        .map(format_time)
        .unwrap_or_else(|_| "-".to_string());

    Ok(FileInfo {
        name,
        path: relative_path(root, path),
        file_type,
        size,
        size_formatted: format_size(size),
        modified,
        created,
    })
}

/// Get file info using a logical base path for consistent path reporting
/// This is used when listing directory contents where the directory may be a symlink
async fn get_file_info_with_logical_base(root: &Path, logical_dir: &Path, actual_file: &Path) -> Result<FileInfo, String> {
    let metadata = fs::metadata(actual_file)
        .await
        .map_err(|e| format!("Failed to get metadata: {}", e))?;

    let name = actual_file
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let file_type = if metadata.is_dir() { "folder" } else { "file" }.to_string();
    let size = metadata.len();

    let modified = metadata
        .modified()
        .map(format_time)
        .unwrap_or_else(|_| "-".to_string());

    let created = metadata
        .created()
        .map(format_time)
        .unwrap_or_else(|_| "-".to_string());

    // Build the logical path by combining logical_dir with the file name
    let logical_file_path = logical_dir.join(&name);
    
    Ok(FileInfo {
        name,
        path: relative_path(root, &logical_file_path),
        file_type,
        size,
        size_formatted: format_size(size),
        modified,
        created,
    })
}

/// 递归获取目录大小
async fn get_dir_size(path: &Path) -> u64 {
    let mut size = 0u64;

    if let Ok(mut entries) = fs::read_dir(path).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let entry_path = entry.path();
            if let Ok(metadata) = fs::metadata(&entry_path).await {
                if metadata.is_dir() {
                    size += Box::pin(get_dir_size(&entry_path)).await;
                } else {
                    size += metadata.len();
                }
            }
        }
    }

    size
}
/// 递归复制目录
async fn copy_dir(src: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest)
        .await
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    let mut entries = fs::read_dir(src)
        .await
        .map_err(|e| format!("Failed to read directory: {}", e))?;

    while let Ok(Some(entry)) = entries.next_entry().await {
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if src_path.is_dir() {
            Box::pin(copy_dir(&src_path, &dest_path)).await?;
        } else {
            fs::copy(&src_path, &dest_path)
                .await
                .map_err(|e| format!("Failed to copy file: {}", e))?;
        }
    }

    Ok(())
}
// ========== API 处理函数 ==========
/// 获取目录内容
pub async fn get_files(
    State(state): State<AppState>,
    Query(query): Query<PathQuery>,
) -> impl IntoResponse {
    let user_path = query.path.unwrap_or_else(|| "/".to_string());

    let paths = match safe_path(&state.root_dir, &user_path) {
        Ok(p) => p,
        Err(e) => return Json(ApiResponse::<()>::error(e)).into_response(),
    };

    if !paths.actual.exists() {
        return Json(ApiResponse::<()>::error("目录不存在")).into_response();
    }

    if !paths.actual.is_dir() {
        return Json(ApiResponse::<()>::error("不是有效的目录")).into_response();
    }

    let mut files = Vec::new();

    match fs::read_dir(&paths.actual).await {
        Ok(mut entries) => {
            while let Ok(Some(entry)) = entries.next_entry().await {
                // Use logical path for file info to maintain consistent paths
                if let Ok(info) = get_file_info_with_logical_base(&state.root_dir, &paths.logical, &entry.path()).await {
                    files.push(info);
                }
            }
        }
        Err(e) => return Json(ApiResponse::<()>::error(format!("读取目录失败: {}", e))).into_response(),
    }

    // Return the logical path, not the actual (resolved) path
    Json(ApiResponse::success(FilesResponse {
        path: relative_path(&state.root_dir, &paths.logical),
        files,
    })).into_response()
}
/// 创建文件夹
pub async fn create_folder(
    State(state): State<AppState>,
    Json(req): Json<CreateFolderRequest>,
) -> impl IntoResponse {
    let parent = match safe_path(&state.root_dir, &req.path) {
        Ok(p) => p,
        Err(e) => return Json(ApiResponse::<()>::error(e)).into_response(),
    };

    let folder_path_actual = parent.actual.join(&req.name);
    let folder_path_logical = parent.logical.join(&req.name);

    if folder_path_actual.exists() {
        return Json(ApiResponse::<()>::error("文件夹已存在")).into_response();
    }

    match fs::create_dir_all(&folder_path_actual).await {
        Ok(_) => Json(ApiResponse::success(OperationResponse {
            message: "文件夹创建成功".to_string(),
            new_path: Some(relative_path(&state.root_dir, &folder_path_logical)),
        })).into_response(),
        Err(e) => Json(ApiResponse::<()>::error(format!("创建失败: {}", e))).into_response(),
    }
}
/// 上传文件
pub async fn upload_files(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut upload_path_actual = state.root_dir.clone();
    let mut upload_path_logical = state.root_dir.clone();
    let mut uploaded_files = Vec::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();

        if name == "path" {
            if let Ok(path_str) = field.text().await {
                let paths = match safe_path(&state.root_dir, &path_str) {
                    Ok(p) => p,
                    Err(e) => return Json(ApiResponse::<()>::error(e)).into_response(),
                };
                upload_path_actual = paths.actual;
                upload_path_logical = paths.logical;
            }
            continue;
        }

        if name == "files" {
            let filename = field
                .file_name()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "unknown".to_string());

            // 确保上传目录存在
            if let Err(e) = fs::create_dir_all(&upload_path_actual).await {
                return Json(ApiResponse::<()>::error(format!("创建目录失败: {}", e))).into_response();
            }

            let file_path_actual = upload_path_actual.join(&filename);
            let file_path_logical = upload_path_logical.join(&filename);

            // 读取文件内容
            match field.bytes().await {
                Ok(data) => {
                    // 写入文件
                    match fs::File::create(&file_path_actual).await {
                        Ok(mut file) => {
                            if let Err(e) = file.write_all(&data).await {
                                return Json(ApiResponse::<()>::error(format!("写入文件失败: {}", e))).into_response();
                            }

                            uploaded_files.push(UploadedFile {
                                name: filename,
                                size: data.len() as u64,
                                path: relative_path(&state.root_dir, &file_path_logical),
                            });
                        }
                        Err(e) => {
                            return Json(ApiResponse::<()>::error(format!("创建文件失败: {}", e))).into_response();
                        }
                    }
                }
                Err(e) => {
                    return Json(ApiResponse::<()>::error(format!("读取上传数据失败: {}", e))).into_response();
                }
            }
        }
    }

    Json(ApiResponse::success(UploadResponse {
        files: uploaded_files,
    })).into_response()
}
/// 下载文件
pub async fn download_file(
    State(state): State<AppState>,
    Query(query): Query<PathQuery>,
) -> Response {
    let user_path = query.path.unwrap_or_default();

    let paths = match safe_path(&state.root_dir, &user_path) {
        Ok(p) => p,
        Err(e) => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(e))
                .unwrap();
        }
    };

    if !paths.actual.exists() {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("文件不存在"))
            .unwrap();
    }

    if paths.actual.is_dir() {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from("不能下载文件夹"))
            .unwrap();
    }

    let filename = paths.actual
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "download".to_string());

    // 读取文件
    match fs::read(&paths.actual).await {
        Ok(data) => {
            let mime = mime_guess::from_path(&paths.actual)
                .first_or_octet_stream()
                .to_string();

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                .header(
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{}\"", filename),
                )
                .body(Body::from(data))
                .unwrap()
        }
        Err(e) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from(format!("读取文件失败: {}", e)))
            .unwrap(),
    }
}
/// 重命名
pub async fn rename(
    State(state): State<AppState>,
    Json(req): Json<RenameRequest>,
) -> impl IntoResponse {
    let old_paths = match safe_path(&state.root_dir, &req.path) {
        Ok(p) => p,
        Err(e) => return Json(ApiResponse::<()>::error(e)).into_response(),
    };

    if !old_paths.actual.exists() {
        return Json(ApiResponse::<()>::error("文件不存在")).into_response();
    }

    let new_path_actual = old_paths.actual.parent().unwrap().join(&req.new_name);
    let new_path_logical = old_paths.logical.parent().unwrap().join(&req.new_name);

    if new_path_actual.exists() {
        return Json(ApiResponse::<()>::error("目标名称已存在")).into_response();
    }

    match fs::rename(&old_paths.actual, &new_path_actual).await {
        Ok(_) => Json(ApiResponse::success(OperationResponse {
            message: "重命名成功".to_string(),
            new_path: Some(relative_path(&state.root_dir, &new_path_logical)),
        })).into_response(),
        Err(e) => Json(ApiResponse::<()>::error(format!("重命名失败: {}", e))).into_response(),
    }
}
/// 移动文件
pub async fn move_file(
    State(state): State<AppState>,
    Json(req): Json<MoveRequest>,
) -> impl IntoResponse {
    let source = match safe_path(&state.root_dir, &req.source) {
        Ok(p) => p,
        Err(e) => return Json(ApiResponse::<()>::error(e)).into_response(),
    };

    let dest_dir = match safe_path(&state.root_dir, &req.destination) {
        Ok(p) => p,
        Err(e) => return Json(ApiResponse::<()>::error(e)).into_response(),
    };

    if !source.actual.exists() {
        return Json(ApiResponse::<()>::error("源文件不存在")).into_response();
    }

    let filename = source.actual.file_name().unwrap();
    let dest_actual = dest_dir.actual.join(filename);
    let dest_logical = dest_dir.logical.join(filename);

    if dest_actual.exists() {
        return Json(ApiResponse::<()>::error("目标位置已存在同名文件")).into_response();
    }

    // 检查是否移动到自身子目录
    if source.actual.is_dir() && dest_actual.starts_with(&source.actual) {
        return Json(ApiResponse::<()>::error("不能移动到自身子目录")).into_response();
    }

    match fs::rename(&source.actual, &dest_actual).await {
        Ok(_) => Json(ApiResponse::success(OperationResponse {
            message: "移动成功".to_string(),
            new_path: Some(relative_path(&state.root_dir, &dest_logical)),
        })).into_response(),
        Err(e) => Json(ApiResponse::<()>::error(format!("移动失败: {}", e))).into_response(),
    }
}
/// 复制文件
pub async fn copy_file(
    State(state): State<AppState>,
    Json(req): Json<CopyRequest>,
) -> impl IntoResponse {
    let source = match safe_path(&state.root_dir, &req.source) {
        Ok(p) => p,
        Err(e) => return Json(ApiResponse::<()>::error(e)).into_response(),
    };

    let dest_dir = match safe_path(&state.root_dir, &req.destination) {
        Ok(p) => p,
        Err(e) => return Json(ApiResponse::<()>::error(e)).into_response(),
    };

    if !source.actual.exists() {
        return Json(ApiResponse::<()>::error("源文件不存在")).into_response();
    }

    let filename = source.actual.file_name().unwrap().to_string_lossy().to_string();
    let ext = source.actual.extension().map(|e| e.to_string_lossy().to_string());
    let stem = source.actual.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();

    // 处理同名文件
    let mut dest_actual = dest_dir.actual.join(&filename);
    let mut dest_logical = dest_dir.logical.join(&filename);
    let mut counter = 1;
    while dest_actual.exists() {
        let new_name = match &ext {
            Some(e) => format!("{} ({}). {}", stem, counter, e),
            None => format!("{} ({})", stem, counter),
        };
        dest_actual = dest_dir.actual.join(&new_name);
        dest_logical = dest_dir.logical.join(&new_name);
        counter += 1;
    }

    let result = if source.actual.is_dir() {
        copy_dir(&source.actual, &dest_actual).await
    } else {
        fs::copy(&source.actual, &dest_actual)
            .await
            .map(|_| ())
            .map_err(|e| format!("复制失败: {}", e))
    };

    match result {
        Ok(_) => Json(ApiResponse::success(OperationResponse {
            message: "复制成功".to_string(),
            new_path: Some(relative_path(&state.root_dir, &dest_logical)),
        })).into_response(),
        Err(e) => Json(ApiResponse::<()>::error(e)).into_response(),
    }
}
/// 删除文件/文件夹
pub async fn delete_file(
    State(state): State<AppState>,
    Json(req): Json<DeleteRequest>,
) -> impl IntoResponse {
    let paths = match safe_path(&state.root_dir, &req.path) {
        Ok(p) => p,
        Err(e) => return Json(ApiResponse::<()>::error(e)).into_response(),
    };

    if !paths.actual.exists() {
        return Json(ApiResponse::<()>::error("文件不存在")).into_response();
    }

    let result = if paths.actual.is_dir() {
        fs::remove_dir_all(&paths.actual).await
    } else {
        fs::remove_file(&paths.actual).await
    };

    match result {
        Ok(_) => Json(ApiResponse::success(OperationResponse {
            message: "删除成功".to_string(),
            new_path: None,
        })).into_response(),
        Err(e) => Json(ApiResponse::<()>::error(format!("删除失败: {}", e))).into_response(),
    }
}
/// 获取文件/文件夹信息
pub async fn get_info(
    State(state): State<AppState>,
    Query(query): Query<PathQuery>,
) -> impl IntoResponse {
    let user_path = query.path.unwrap_or_default();

    let paths = match safe_path(&state.root_dir, &user_path) {
        Ok(p) => p,
        Err(e) => return Json(ApiResponse::<()>::error(e)).into_response(),
    };

    if !paths.actual.exists() {
        return Json(ApiResponse::<()>::error("文件不存在")).into_response();
    }

    let info = match get_file_info(&state.root_dir, &paths.logical).await {
        Ok(i) => i,
        Err(e) => return Json(ApiResponse::<()>::error(e)).into_response(),
    };

    let (children, size, size_formatted) = if paths.actual.is_dir() {
        let mut count = 0;
        if let Ok(mut entries) = fs::read_dir(&paths.actual).await {
            while entries.next_entry().await.ok().flatten().is_some() {
                count += 1;
            }
        }
        let dir_size = get_dir_size(&paths.actual).await;
        (Some(count), dir_size, format_size(dir_size))
    } else {
        (None, info.size, info.size_formatted.clone())
    };

    Json(ApiResponse::success(InfoResponse {
        info: FileInfoDetail {
            name: info.name,
            path: info.path,
            file_type: info.file_type,
            size,
            size_formatted,
            modified: info.modified,
            created: info.created,
            children,
        },
    })).into_response()
}
/// 获取所有文件夹
pub async fn get_folders(State(state): State<AppState>) -> impl IntoResponse {
    let mut folders = Vec::new();

    async fn scan_dir(
        root: &Path,
        dir: &Path,
        prefix: &str,
        folders: &mut Vec<FolderItem>,
    ) {
        let rel_path = relative_path(root, dir);
        let display_name = if rel_path == "/" {
            "根目录".to_string()
        } else {
            dir.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default()
        };

        folders.push(FolderItem {
            path: rel_path,
            display: format!("{}{}", prefix, display_name),
        });

        if let Ok(mut entries) = fs::read_dir(dir).await {
            let mut subdirs = Vec::new();
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_dir() {
                    subdirs.push(path);
                }
            }
            subdirs.sort();

            for subdir in subdirs {
                Box::pin(scan_dir(root, &subdir, &format!("{}　", prefix), folders)).await;
            }
        }
    }

    scan_dir(&state.root_dir, &state.root_dir, "", &mut folders).await;

    Json(ApiResponse::success(FoldersResponse { folders }))
}
/// 获取磁盘信息
pub async fn get_disk_info(State(state): State<AppState>) -> impl IntoResponse {
    use sysinfo::Disks;

    let disks = Disks::new_with_refreshed_list();

    // 查找根目录所在的磁盘
    let mut total = 500 * 1024 * 1024 * 1024u64; // 默认 500GB
    let mut free = 400 * 1024 * 1024 * 1024u64;

    for disk in disks.list() {
        if state.root_dir.starts_with(disk.mount_point()) {
            total = disk.total_space();
            free = disk.available_space();
            break;
        }
    }

    let used = total.saturating_sub(free);

    Json(ApiResponse::success(DiskResponse {
        total,
        used,
        free,
        used_formatted: format_size(used),
    }))
}
/// 搜索文件
pub async fn search_files(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    let paths = match safe_path(&state.root_dir, &query.path.unwrap_or_else(|| "/".to_string())) {
        Ok(p) => p,
        Err(e) => return Json(ApiResponse::<()>::error(e)).into_response(),
    };

    let query_lower = query.query.to_lowercase();
    let mut results = Vec::new();

    async fn search_in_dir(
        root: &Path,
        dir: &Path,
        query: &str,
        results: &mut Vec<FileInfo>,
        limit: usize,
    ) {
        if results.len() >= limit {
            return;
        }

        if let Ok(mut entries) = fs::read_dir(dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if results.len() >= limit {
                    break;
                }

                let path = entry.path();
                let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();

                if name.contains(query) {
                    if let Ok(info) = get_file_info(root, &path).await {
                        results.push(info);
                    }
                }

                if path.is_dir() && results.len() < limit {
                    Box::pin(search_in_dir(root, &path, query, results, limit)).await;
                }
            }
        }
    }

    search_in_dir(&state.root_dir, &paths.actual, &query_lower, &mut results, 100).await;

    Json(ApiResponse::success(SearchResponse { results })).into_response()
}