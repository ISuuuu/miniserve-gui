# AGENTS.md - 开发指南

## 项目概述

Tauri 2 桌面应用，为 [miniserve](https://github.com/svenstaro/miniserve) 提供图形化界面。
- **前端**: Vue 3 + TypeScript + Element Plus + vue-i18n
- **后端**: Tauri 2 (Rust，多模块架构)
- **包管理**: pnpm 9+，Node.js 20+

## 核心开发命令

```bash
pnpm install          # 安装依赖
pnpm run tauri dev    # 开发模式（前端热重载 + Rust 后端）
pnpm run tauri build  # 生产构建（含前端类型检查 + Rust 编译）
pnpm run build        # 仅前端构建（vue-tsc --noEmit && vite build）
```

**注意**: 没有独立的 lint/test/typecheck 脚本。类型检查集成在 `pnpm run build` 中。

## 项目结构

```
├── src/                        # 前端源码
│   ├── App.vue                # 主组件（业务逻辑）
│   ├── main.ts                # Vue 入口
│   ├── components/
│   │   ├── ConfigPanel.vue    # 配置面板组件
│   │   ├── StatusCard.vue     # 服务状态卡片
│   │   └── LogPanel.vue       # 运行日志面板
│   └── i18n/
│       ├── index.ts           # i18n 配置（自动检测系统语言）
│       ├── zh-CN.ts           # 中文语言包
│       └── en.ts              # 英文语言包
├── src-tauri/                  # Rust 后端
│   ├── src/
│   │   ├── lib.rs             # 模块声明、Job Object、托盘、入口
│   │   ├── main.rs            # 程序入口
│   │   ├── commands.rs        # Tauri commands（所有前端 API）
│   │   ├── state.rs           # AppState、类型定义（ServerConfig 等）
│   │   └── utils.rs           # 辅助函数、参数构建、配置验证
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── capabilities/          # Tauri 权限配置
├── scripts/
│   └── sync-version.js        # Git tag 同步版本号到配置文件
└── .github/workflows/
    └── release.yml            # CI/CD 发布流程
```

## 关键架构点

### 前端
- 组件化架构：`ConfigPanel`、`StatusCard`、`LogPanel` 独立组件
- `App.vue` 负责业务逻辑和组件编排
- 使用 Vue 3 Composition API (`<script setup>`)
- 通过 `invoke()` 调用 Rust 后端命令
- 使用 `listen()` 监听后端事件（下载进度、服务日志）
- vue-i18n 国际化支持（中文/英文自动切换）

### 后端
- `commands.rs` - 所有 Tauri commands（前端 API）
- `state.rs` - `AppState`、`ServerConfig`、`ServerStatus` 等类型定义
- `utils.rs` - 路径获取、IP 获取、miniserve 参数构建、配置验证
- `lib.rs` - Windows Job Object、系统托盘、应用入口

### Windows 特殊处理
- 使用 Windows Job Object (`job_object` 模块) 确保子进程随父进程退出
- `CREATE_NO_WINDOW` 全局常量避免弹出控制台窗口
- 需要 `unsafe impl Send/Sync` for `AppState`（因为 Job Object 句柄）

## 配置文件位置

- **引擎二进制**: `%LOCALAPPDATA%/miniserve-gui/bin/miniserve.exe` (Windows)
- **配置 JSON**: `%APPDATA%/miniserve-gui/config.json` (Windows)
- Linux/macOS: `~/.local/share/miniserve-gui/bin/` 和 `~/.config/miniserve-gui/`

## 构建和发布

### 版本同步
版本号需同时更新三处：
1. `package.json` - `version` 字段
2. `src-tauri/tauri.conf.json` - `version` 字段
3. `src-tauri/Cargo.toml` - `version` 字段

CI 会自动通过 `sed` 命令同步，本地可运行 `node scripts/sync-version.js`。

### Release 流程
- 推送 `v*` tag 触发 GitHub Actions
- 构建 Windows (NSIS) 和 Linux (deb/AppImage) 安装包
- 自动生成 `latest.json` 用于 Tauri updater
- 代理配置：`tauri.conf.json` 中 `plugins.updater.proxy`

## 常见陷阱

1. **Rust 编译慢**: 首次构建需下载依赖，后续增量编译较快
2. **前端端口固定**: Vite 开发服务器固定使用 `1420` 端口
3. **子进程管理**: 修改 `start_server`/`stop_server` 时注意 Windows Job Object 的清理
4. **配置兼容性**: 前端 `loadConfig()` 会自动升级旧配置（如 `0.0.0.0` → `::`）

## 代码风格

- **TypeScript**: strict mode, noUnusedLocals, noUnusedParameters
- **Rust**: 标准 rustfmt 风格，使用 `log` crate 记录日志
- **UI**: Element Plus 组件库，中文界面
