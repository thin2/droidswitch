# Droid Switch

Factory Droid CLI 本地模型配置管理器，Tauri 2 + React + TypeScript + SQLite。

已接通供应商/模型草稿、模型批量获取、连接测试、外部条目接管、脱敏预览、指纹冲突检查、私有备份与恢复、官方/API 双方案、CC Switch 只读导入及用量统计界面。浏览器预览无法操作真实本地配置，需运行桌面版。

当前写入适配开发契约限定 Droid 0.238.0；真实 Windows 行为尚待验收。用量采集目前支持明确的元数据契约和带稳定 ID/时间的 SDK 导出，**不能宣称已完整支持 Droid 原生会话日志**，详见 [用量范围](planning/droid-usage-contract.md)。

## 开发与测试

需要 Node.js >=22.12、Rust >=1.90（推荐 stable）及 [Tauri 平台依赖](https://v2.tauri.app/start/prerequisites/)。macOS 可运行 bash scripts/setup-deps-macos.sh 处理依赖；脚本不改 shell 启动文件。新终端可用 source "$HOME/.cargo/env" 加载 Rust。

```sh
npm ci
npm run tauri dev
npm run build
npm test
cargo test --locked --manifest-path src-tauri/Cargo.toml
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run test:ui  # 默认使用已安装 Chrome；CI 使用 Playwright Chromium
```

UI 自动测试使用隔离的合成 IPC 数据，不会写入真实 Droid 配置。截图见 docs/screenshots。

## 安装包

```sh
npm run package:mac
```

macOS 产物位于 src-tauri/target/release/bundle/macos 和 dmg。打包脚本使用 hdiutil，避开当前环境中失败的 Finder DMG 美化流程。应用采用 ad-hoc 签名，未公证。

Windows PowerShell 构建：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-windows.ps1
```

详细步骤与终验清单：[Windows 构建与实测](docs/windows-testing.md)。CI 已配置 Windows NSIS/macOS DMG，但尚未推送或运行远端流水线；配置存在不代表已通过 Windows 验收。

## 状态管理

planning/tasks.json 是任务真相源，planning/codex-tasks.md 为 dual-ai-planning 派生视图。planning/progress-reports 保存验证、自审与未完成项，.dual-ai-collab/checkpoints 保存恢复游标。

尚待真实环境完成：#039 原生日志格式适配与跨来源汇总边界、#043 Windows 实际安装产物、原阶段 0 与 #044 实机验收。P3 深色主题、增强诊断、Analytics/代理采集属于后续迭代。
