# Droid Switch

Factory Droid CLI 本地模型配置管理器，Tauri 2 + React + TypeScript。

当前为工程骨架：五个页面路由和状态说明已建立，业务数据和配置写入尚未接入。

## 开发

环境需要 Node.js >=22.12、Rust >=1.90（推荐 stable），以及 [Tauri 平台前置依赖](https://v2.tauri.app/start/prerequisites/)。

macOS 可先运行 `bash scripts/setup-deps-macos.sh`：安装 npm 依赖、缺失的 Rust stable 并执行两端构建。脚本沿用现有代理配置，通过 [官方 rustup](https://rust-lang.org/tools/install/) 安装，不修改 shell 启动文件。需要允许联网及写入 Rust 用户目录。

```sh
npm ci
npm run dev          # 浏览器预览
npm run build        # TypeScript 检查和前端构建
npm run tauri dev    # 桌面开发
cargo build --manifest-path src-tauri/Cargo.toml
npm run tauri build # 在对应 Windows/macOS 主机打包
```

前端入口位于 src/，Rust 入口位于 src-tauri/src/。导航使用 hash 路由，兼容桌面资源路径。
Windows/macOS 构建矩阵位于 .github/workflows/build.yml；配置存在不代表已经通过双平台验收。

## 任务状态

planning/tasks.json 是真相源，planning/codex-tasks.md 为工具生成视图。原始任务板.md 保留为需求来源。
使用 dual-ai-planning 管理状态、验收、自审和恢复游标；实际执行证据保存在 planning/progress-reports/。

Rust 安装脚本不会修改 shell 启动文件；新终端可执行 `source "$HOME/.cargo/env"` 加载工具链。依赖由 package-lock.json 与 src-tauri/Cargo.lock 固定，CI 使用锁文件构建。
