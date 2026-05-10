# CLAUDE.md

本文件为 Claude Code (claude.ai/code) 在本仓库中工作时提供指引。

## 项目概述

基于 Tauri 2 的桌面应用，为 [miniserve](https://github.com/svenstaro/miniserve)（命令行文件共享工具）提供图形化界面。自动管理 miniserve 二进制文件，将 CLI 参数暴露为 GUI，并提供二维码生成功能方便移动端访问。

- **前端**: Vue 3 (Composition API / `<script setup>`) + TypeScript + Element Plus + vue-i18n
- **后端**: Tauri 2 (Rust，多模块架构)
- **包管理**: pnpm 9+ / Node.js 20+ / Rust stable 1.77+

## 核心开发命令

```bash
pnpm install          # 安装依赖
pnpm run tauri dev    # 开发模式（前端热重载 + Rust 后端）
pnpm run tauri build  # 生产构建（类型检查 + Vite 构建 + Rust 编译）
pnpm run build        # 仅前端构建（vue-tsc --noEmit && vite build）
```

没有独立的 lint / test / format 脚本，类型检查集成在 `pnpm run build` 中通过 `vue-tsc --noEmit` 执行。

## 架构

### 前端 ↔ 后端通信

- 前端通过 `invoke("command_name", { args })` 调用 Rust 后端 — 两端通过 Tauri 命令系统强类型绑定。
- Rust 通过 `app_handle.emit("event-name", payload)` 推送事件到前端 — 用于下载进度和服务日志。
- 共 13 个 Tauri 命令，在 `src-tauri/src/lib.rs` 注册，在 `src-tauri/src/commands.rs` 实现。

### 前端 (`src/`)

- **`App.vue`** — 全部业务逻辑：引擎管理、服务启停、配置加载/保存（500ms 防抖自动保存）、二维码生成、URL 复制、更新流程、事件监听。
- **`components/ConfigPanel.vue`** — miniserve 参数的表单控件。纯展示组件，通过 props 接收配置，emit `selectPath` 事件。
- **`components/StatusCard.vue`** — 显示运行中的服务 URL，带复制按钮和二维码预览（悬停显示）。
- **`components/LogPanel.vue`** — 终端风格日志查看器，自动滚动到底部。
- **`i18n/`** — vue-i18n 国际化，包含 `zh-CN.ts` 和 `en.ts`；通过 `navigator.language` 自动检测系统语言。

### 后端 (`src-tauri/src/`)

- **`commands.rs`** — 所有 `#[tauri::command]` 函数（引擎下载、配置读写、服务生命周期、二维码生成、更新器）。
- **`state.rs`** — `AppState`（持有 miniserve 子进程的 `Mutex<Option<Child>>`）、`ServerConfig`、`EngineStatus`、`ServerStatus`、`QrCodeResponse`。
- **`utils.rs`** — `get_engine_path()`、`get_config_path()`、`validate_config()`、`get_local_ips()`、`build_miniserve_args()`。
- **`lib.rs`** — 模块声明、Windows Job Object、系统托盘（显示/退出）、窗口关闭拦截（隐藏到托盘）、插件注册。调用 `main.rs` → `lib::run()`。
- **`main.rs`** — 程序入口；在 Linux 上设置 `WEBKIT_DISABLE_COMPOSITING_MODE=1` 以兼容 VMware，然后调用 `lib::run()`。

## Windows 特殊处理

- **Job Object** (`lib.rs`)：通过原生 Win32 FFI 创建带 `KILL_ON_JOB_CLOSE` 标志的 Job Object — 确保应用崩溃时 miniserve 子进程被一并终止。`AppState` 需要 `unsafe impl Send/Sync`。
- **CREATE_NO_WINDOW**：所有子进程创建时使用此标志，避免弹出控制台窗口。
- 窗口关闭时隐藏到系统托盘而非退出；托盘"退出"菜单会杀死子进程并关闭 Job Object。

## 版本管理

版本号需在三个文件中保持同步（CI 通过 `sed` 自动处理，本地可运行 `node scripts/sync-version.js`）：

1. `package.json` — `version`
2. `src-tauri/tauri.conf.json` — `version`
3. `src-tauri/Cargo.toml` — `version`

发布：推送 `v*` 标签触发 `.github/workflows/release.yml`（构建 Windows NSIS + Linux deb/AppImage，生成 `latest.json` 供更新器使用）。

## 运行时数据路径

| 数据 | Windows | Linux/macOS |
|------|---------|-------------|
| 引擎二进制 | `%LOCALAPPDATA%/miniserve-gui/bin/miniserve.exe` | `~/.local/share/miniserve-gui/bin/miniserve` |
| 配置文件 | `%APPDATA%/miniserve-gui/config.json` | `~/.config/miniserve-gui/config.json` |

## 注意事项

- **Vite 开发端口硬编码为 `1420`**，同时存在于 `vite.config.ts` 和 `src-tauri/tauri.conf.json` — 修改时必须同步更新。
- **配置自动升级**：`App.vue` 中的 `loadConfig()` 会迁移旧配置（如 `0.0.0.0` → `::` 实现双栈）。修改配置相关逻辑时需保留此行为。
- **子进程清理**：修改 `start_server`/`stop_server` 时必须遵循 `lib.rs` 中 Windows Job Object 的生命周期管理。
- **Rust 日志**：`env_logger` 仅在 debug 构建中初始化，避免 release 版弹出控制台窗口。
- **随机路由**：`random_route` 功能通过解析 miniserve stdout 中的 HTTP URL 来捕获路由后缀。
