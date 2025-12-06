# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Filest is a Rust-based web file manager with a Windows-style UI, built using Axum framework. It features HTTP Basic authentication and provides a web interface for remote file management operations.

## Architecture

### Core Components

- **src/main.rs**: Application entry point with Axum server setup, CLI argument parsing, and route configuration
- **src/auth.rs**: HTTP Basic authentication middleware for API endpoints
- **src/handlers.rs**: HTTP request handlers for all file operations (CRUD, upload, download, search)
- **src/models.rs**: Data structures for file info, API responses, and upload session management
- **static/index.html**: Embedded web UI (compiled into binary)

### Key Architecture Patterns

1. **Embedded Frontend**: HTML is embedded directly into the binary using `include_str!()` macro
2. **Authentication**: Only API routes require auth middleware; the main UI route (`/`) is public
3. **Chunked Upload**: Supports large file uploads via chunked streaming (5MB chunks, 10GB total limit)
4. **Safe Path Handling**: All file operations use `safe_path()` to prevent directory traversal attacks
5. **Async Operations**: All file I/O operations are asynchronous using tokio

## Common Development Commands

```bash
# Development build and run
cargo run -- --root ./test_files

# Production build
cargo build --release

# Run tests
cargo test

# Code formatting
cargo fmt

# Linting
cargo clippy

# Run with custom configuration
cargo run -- --root /path/to/files --port 8080 --user admin --password secret
```

## CLI Configuration Options

- `--root` (`-r`): File root directory (default: `./files`)
- `--port` (`-p`): Server port (default: `3000`)
- `--user` (`-u`): Login username (default: `admin`)
- `--password` (`-P`): Login password (default: `admin123`)
- `--bind` (`-b`): Bind address (default: `0.0.0.0`)

## API Structure

All API endpoints are prefixed with `/api` and require HTTP Basic authentication:

- `GET /api/files?path=`: List directory contents
- `POST /api/folder`: Create new folder
- `POST /api/upload`: Upload files (multipart/form-data)
- `GET /api/download?path=`: Download file
- `PUT /api/rename`: Rename file/folder
- `PUT /api/move`: Move file/folder
- `POST /api/copy`: Copy file/folder
- `DELETE /api/delete`: Delete file/folder
- `GET /api/info?path=`: Get file metadata
- `GET /api/folders`: Get folder tree
- `GET /api/disk`: Get disk usage information
- `GET /api/search?query=`: Search files

### Chunked Upload Endpoints

- `POST /api/upload/init`: Initialize chunked upload session
- `POST /api/upload/chunk`: Upload file chunk
- `POST /api/upload/complete`: Finalize chunked upload
- `POST /api/upload/abort`: Abort chunked upload

## Key Dependencies

- **axum**: Web framework with async support and multipart file handling
- **tokio**: Async runtime for file operations
- **tower-http**: CORS and static file serving
- **serde**: JSON serialization/deserialization
- **sysinfo**: Cross-platform disk information
- **uuid**: Session ID generation for chunked uploads
- **tracing**: Structured logging

## Security Features

1. **Path Traversal Protection**: `safe_path()` function validates all user paths
2. **HTTP Basic Authentication**: All API endpoints require authentication
3. **Upload Size Limits**: 10GB limit with memory-efficient streaming
4. **CORS Configuration**: Configurable cross-origin request support

## Development Notes

- The application is single-binary with embedded frontend assets
- All file paths are resolved relative to the configured root directory
- Async error handling uses `Result<T, String>` pattern in handlers
- Logging is configured via `tracing` with configurable verbosity
- Chunked upload support enables handling large files with constant memory usage
- 前端修改或完善时同时兼容pc端和移动端