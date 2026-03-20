# miniserve-gui

跨平台的 [miniserve](https://github.com/svenstaro/miniserve) 提供跨平台的图形化界面客户端。

## 功能特性

- ✅ **引擎自动管理** - 自动检测/下载最新版本的 miniserve 二进制文件
- ✅ **可视化配置** - 支持所有 miniserve CLI 参数的图形化配置
- ✅ **服务控制** - 一键启动/停止服务，实时显示服务状态
- ✅ **二维码分享** - 生成二维码，移动端扫码即访问
- ✅ **配置持久化** - 保存配置到本地，重启后自动加载

## 支持的参数

| 分类 | 参数 | 说明 |
|------|------|------|
| 基础运行 | `PATH` | 要分享的文件夹路径 |
| | `-p, --port` | 服务端口（默认 8080）|
| | `-i, --interfaces` | 绑定网卡（0.0.0.0 或 127.0.0.1）|
| 安全控制 | `-a, --auth` | 用户名:密码 认证 |
| | `-u, --upload` | 允许访客上传文件 |
| | `-M, --mkdir` | 允许创建目录 |
| 界面展示 | `--color-scheme` | 配色主题（squirrel", "archlinux", "zenburn", "monokai）|
| | `--title` | 网页标题 |
| 高级进阶  
| | `-H, --hidden` | 显示隐藏文件 |

## 技术栈

- **前端**: Vue 3 + TypeScript + Vite + Element Plus
- **后端**: Tauri 2 (Rust)
- **引擎**: [miniserve](https://github.com/svenstaro/miniserve)

## 开发

### 环境要求

- Node.js 18+
- pnpm 8+
- Rust 1.70+
- Windows / macOS / Linux

### 安装依赖

```bash
pnpm install
```

### 开发模式

```bash
pnpm run tauri dev
```

### 构建发布

```bash
pnpm run tauri build
```

## 配置文件位置

- **Windows**: `%LOCALAPPDATA%/miniserve-gui/bin/miniserve.exe`
- **Linux/macOS**: `~/.local/share/miniserve-gui/bin/miniserve`

配置 JSON:
- **Windows**: `%APPDATA%/miniserve-gui/config.json`
- **Linux/macOS**: `~/.config/miniserve-gui/config.json`

## 许可证

MIT
