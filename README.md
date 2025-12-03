# Filest - 远程文件管理器
基于 Rust + Axum 的 Web 文件管理器，支持 HTTP Basic 认证。
## 项目结构
```
filest/
├── Cargo.toml           # 项目配置
├── README.md            # 说明文档
├── src/
│   ├── main.rs          # 主程序入口
│   ├── auth.rs          # HTTP Basic 认证中间件
│   ├── handlers.rs      # API 处理函数
│   └── models.rs        # 数据模型
└── static/
    └── index.html       # 前端界面（嵌入到二进制）
```
## 快速开始
### 1. 编译
```bash
cd filest
cargo build --release
```
编译后的二进制文件在 `target/release/filest`
### 2. 运行
```bash
# 使用默认配置
./target/release/filest
# 或者自定义配置
./target/release/filest \
    --root /path/to/your/files \
    --port 8080 \
    --user admin \
    --password your_secure_password
```
### 3. 访问
打开浏览器访问 `http://localhost:3000`，输入用户名和密码登录。
## 命令行参数
| 参数 | 简写 | 说明 | 默认值 |
|------|------|------|--------|
| `--root` | `-r` | 文件根目录 | `./files` |
| `--port` | `-p` | 服务端口 | `3000` |
| `--user` | `-u` | 登录用户名 | `admin` |
| `--password` | `-P` | 登录密码 | `admin123` |
| `--bind` | `-b` | 绑定地址 | `0.0.0.0` |
## 功能特性
### 文件操作
- ✅ 浏览目录
- ✅ 上传文件（支持多文件、拖拽上传）
- ✅ 下载文件
- ✅ 新建文件夹
- ✅ 重命名
- ✅ 移动文件/文件夹
- ✅ 复制文件/文件夹
- ✅ 删除文件/文件夹
- ✅ 搜索文件
- ✅ 查看文件属性
### 界面功能
- ✅ Windows 风格 UI
- ✅ 网格/列表视图切换
- ✅ 面包屑导航
- ✅ 右键上下文菜单
- ✅ 键盘快捷键支持
- ✅ 磁盘空间显示
### 安全特性
- ✅ HTTP Basic 认证
- ✅ 路径遍历攻击防护
- ✅ 跨域请求支持 (CORS)
## 快捷键
| 快捷键 | 功能 |
|--------|------|
| `Ctrl+C` | 复制 |
| `Ctrl+X` | 剪切 |
| `Ctrl+V` | 粘贴 |
| `Ctrl+A` | 全选 |
| `Delete` | 删除 |
| `F2` | 重命名 |
| `F5` | 刷新 |
| `Backspace` | 返回上级目录 |
| `Enter` | 打开文件/文件夹 |
## API 接口
| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/files?path=` | 获取目录内容 |
| POST | `/api/folder` | 创建文件夹 |
| POST | `/api/upload` | 上传文件 |
| GET | `/api/download?path=` | 下载文件 |
| PUT | `/api/rename` | 重命名 |
| PUT | `/api/move` | 移动文件 |
| POST | `/api/copy` | 复制文件 |
| DELETE | `/api/delete` | 删除文件 |
| GET | `/api/info?path=` | 获取文件信息 |
| GET | `/api/folders` | 获取文件夹列表 |
| GET | `/api/disk` | 获取磁盘信息 |
| GET | `/api/search?query=` | 搜索文件 |
## Docker 部署
```dockerfile
FROM rust:1.75-alpine AS builder
WORKDIR /app
RUN apk add --no-cache musl-dev
COPY . .
RUN cargo build --release
FROM alpine:latest
WORKDIR /app
COPY --from=builder /app/target/release/filest .
ENV ROOT_DIR=/data
EXPOSE 3000
VOLUME ["/data"]
CMD ["./filest", "--root", "/data"]
```
构建并运行：
```bash
docker build -t filest .
docker run -d \
    -p 3000:3000 \
    -v /your/files:/data \
    -e RUST_LOG=info \
    filest \
    --user admin \
    --password your_password
```
## Systemd 服务
创建服务文件 `/etc/systemd/system/filest.service`:
```ini
[Unit]
Description=Filest - Remote File Manager
After=network.target
[Service]
Type=simple
User=www-data
WorkingDirectory=/opt/filest
ExecStart=/opt/filest/filest --root /srv/files --user admin --password secret
Restart=always
RestartSec=5
[Install]
WantedBy=multi-user.target
```
启动服务：
```bash
sudo systemctl daemon-reload
sudo systemctl enable filest
sudo systemctl start filest
```
## 开发
```bash
# 开发模式运行
cargo run -- --root ./test_files
# 运行测试
cargo test
# 格式化代码
cargo fmt
# 代码检查
cargo clippy
```
## 许可证
MIT License