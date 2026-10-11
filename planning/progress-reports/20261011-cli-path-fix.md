# 已安装 CLI 在桌面应用中无法检测

用户通过安装包启动应用，预览时提示未找到 Droid。检查发现本机实际 CLI 位于 ~/.local/bin/droid，旧实现只执行 Command::new("droid")，依赖当前进程 PATH。

修复：优先查找 PATH 中的绝对目录，再查用户 .local/bin、bin；macOS 增加 Homebrew 常见目录。使用绝对可执行路径启动，不运行用户 shell 初始化脚本、不搜索工作目录。区分无执行权限、启动依赖缺失及其他启动失败。

验证：两项路径/权限单元测试通过；用 PATH=/usr/bin:/bin:/usr/sbin:/sbin 实际执行检测，成功返回 0.238.0；仅运行 --version，未发送生成请求。clippy --all-targets -- -D warnings 通过。Windows 路径分支尚未实机验证。

工作区有其他界面修改和未跟踪文件。本次保留它们，使用独立 detached worktree 的已提交代码加本修复构建安装包，不把未验收界面修改混入该包。临时 smoke 程序不包含密钥，已从构建工作树移除。

独立构建验证完成：TypeScript/Vite、Rust release、macOS .app/DMG 生成及签名完整性检查均通过。修复包另命名为 Droid_Switch_0.1.0_arm64-cli-fix.dmg，避免用户误用先前挂载的旧包。
