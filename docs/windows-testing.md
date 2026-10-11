# Windows 构建与实测

当前开发机是 macOS，Windows 安装/真实会话测试尚未通过。CI 已配置 Windows NSIS 与 macOS DMG，但未推送/触发远端构建。

## 构建

安装 Node.js 24 LTS、Rust stable（MSVC toolchain）、Microsoft C++ Build Tools 的“使用 C++ 的桌面开发”工作负载与 Windows SDK，确保系统有 WebView2。依赖依据 [Tauri 官方 Windows 前置条件](https://v2.tauri.app/start/prerequisites/#windows)。重新打开 PowerShell 后，在源码目录执行：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-windows.ps1
```

脚本先运行自动测试，再生成 src-tauri\target\release\bundle\nsis 下的安装程序。当前构建没有发行者签名。

## 实测顺序

1. 安装并打开；设置页检测 Droid，当前写入适配仅支持 0.238.0。先用临时 settings.json 路径测试。
2. 创建环境变量连接和两个同 API ID、不同供应商模型；重启应用检查草稿恢复。
3. 载入带外部模型/未知字段的配置；不接管时保持原样，接管后编辑并检查预览。
4. 预览后手工编辑文件，确认应用拒绝覆盖；重新载入应保留草稿。
5. 确认应用，检查配置无内部管理字段、凭据模式正确、备份文件访问权限；恢复备份应先备份当前文件。
6. 官方 → API → 官方切换，开新 Droid 会话用 /model 核对。已有会话、项目/CLI/Spec/Skills/子任务显式设置的优先级需单独确认。
7. 只有你确认额度后执行调用测试，检查 401/403、404、429、超时等分类。
8. CC Switch 仍开启且有 WAL 时只读导入；重复导入不增殖，取消不改草稿，无模型连接可后续手动添加。
9. 用实际 Droid 会话日志验证用量格式。当前支持范围与合成夹具契约见 planning/droid-usage-contract.md；不识别的格式应显示覆盖缺失，不可记为 0。回传样本仅需字段结构和计数，移除正文、密钥及个人路径。
10. 1024×720 与大窗口检查详情抽屉、键盘焦点、错误/空状态、CSV 导出和备份恢复。

最终验收来源仍是 planning/tasks.json 的原阶段 0 与 #044；自动测试通过不能代替上述真实会话验证。
