# 依赖排障 · 2026-10-11

用户要求优先解决依赖，当前任务仍为 #010。

## 已确认

- Node/npm 可用；xcode-select 指向 /Library/Developer/CommandLineTools，xcrun 可解析 MacOSX.sdk。
- 常用 Rust 安装位置及缓存中未找到 cargo/rustc/rustup。
- npm 配置的 registry 是官方 registry.npmjs.org；会话继承代理 127.0.0.1:7897。
- 使用可写 /private/tmp npm 缓存重试 npm install，仍因 connect EPERM 被拒，排除了仅缓存目录不可写的问题。
- 申请 npm install 沙箱提权被审批策略直接拒绝（sandbox_approval=false）；没有可用的放行提示。
- 上轮离线安装确认缺少 Tauri CLI；不能靠现有缓存补齐依赖。

## 已落地

scripts/setup-deps-macos.sh：检查 Node/macOS SDK，安装 npm 依赖（存在锁文件用 npm ci），检测或安装 Rust stable，再执行 npm run build 和 cargo build。使用官方 npm 配置和 Rust rustup，保留用户代理，不修改 shell 启动文件。README 已补入口。

验证：bash -n scripts/setup-deps-macos.sh 通过。安装脚本未完整执行，依赖和构建仍未通过；不将 #010 标为已验收。

## 必需的外部操作

需要允许网络下载及 Rust 用户目录写入的执行环境，或由用户在正常 macOS 终端运行：

    bash scripts/setup-deps-macos.sh

运行完成后当前会话可重新读取依赖、执行验收并更新状态。Git 元数据权限是另一项尚未解除的限制，本次没有改动 Git 或绕过集成门槛。

## 用户授权后的复核

用户已明确授权依赖安装提权。再次请求 npm install 提权时，执行层仍返回 sandbox_approval=false，直接拒绝请求。授权有效，但当前环境限制未解除；没有重复要求用户批准同一动作。依赖安装仍待可联网执行环境。

## 安装脚本实际执行结果

按用户指令执行 bash scripts/setup-deps-macos.sh，退出码 1。npm 获取 @tauri-apps/cli 时连接代理 127.0.0.1:7897 被 EPERM 拒绝；脚本按失败即停策略退出，未执行 Rust 安装与构建。#010 仍为 IN_PROGRESS，环境阻塞未解除。
