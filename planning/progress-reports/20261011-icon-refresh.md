# 图标调整（2026-10-11）

根据用户反馈，替换占满底板的大写 D 为双向切换符号，深灰底搭配米白/橙色。底板边长由 928 缩为 800（减少约 14%），透明边距由 48 增为 112；不改变操作系统图标尺寸设置。

从 assets/app-icon.svg 重新生成 Windows ICO、macOS ICNS 及 PNG。已目视检查 512 和 32 像素图标，核验 PNG 尺寸与 ICO/ICNS 格式。npm run package:mac 通过，包含 TypeScript/Vite、Rust release 构建及签名完整性验证。源码包和 macOS DMG 同步更新；Windows 原生显示仍待用户实测。

本次为小范围视觉修订，按 dual-ai-planning 规则记录进展，不新增任务，不更改未完成任务状态。
