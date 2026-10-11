# 供应商模型管理验收

## 实现
- #059：供应商卡片获取模型（锁定当前连接）、批量手动添加、查看模型筛选入口。
- 每行一个精确 API ID，修剪首尾空白、跳过空行、同供应商去重并跳过已有模型，预览计数，复用 import_models 草稿命令。保留已有模型参数，跨供应商允许同 ID。
- URL 记录供应商筛选，刷新仍生效；供应商小标签不修改用户显示名或底层 API ID。
- 获取弹窗提供切换手动添加入口；保存失败保留输入。单次最多 10000 个不同 ID，预览前 100 项。
- 回归中发现已有未提交用量 UI 把未采集误判为解析失败，最小修正判定和采集设置入口文案，保留原有组件布局。

## 真实验收
- verify run 059：npm run build 通过；npm test 8/8；npm run test:ui 5/5。
- verify run 060：Rust model_batch 1/1；config::merge::tests 6/6。
- 浏览器新增测试覆盖当前供应商锁定、手动添加空输入禁用、输入去重和已有跳过、提交参数、模型筛选和刷新恢复。
- 实际检查截图 /tmp/droid-switch-batch-models.png，弹窗、预览和操作按钮显示正常。
- git diff --check 通过。
- 首轮失败记录：旧用量空状态回归失败，修正后通过；浏览器 label 查询未匹配 select，改用其语义 combobox 查询后通过。

## 状态与限制
当前会话自审，无独立审查人。任务 #059/#060 保持待集成；工作区存在本轮开始前的未提交 UI 改动，未将这些改动混入提交，未将任务标记 DONE。未更新安装包，现有 releases 文件仍为此前版本；启动开发版本才会看到本轮代码。没有写真实 Droid 配置，没有调用付费模型。Windows 真机验证保持待办。同名模型折叠、Codex 代理均不在本轮实现范围。


## 后续集成与发布（2026-10-11）
以上“待集成、未打包”为前一轮状态，已在本轮完成：
- 功能提交 33c224d；测试端口隔离提交 c2fad5a。功能已进入 main，其他预存 UI 改动仍留在工作区，未混入提交或安装包。
- 在同一提交的干净 worktree /tmp/droidswitch-provider-release 独立端口 1421 验收：构建、8 项单元测试、5 项浏览器测试通过；后端批量导入 1 项、配置合并 6 项通过。
- dual-ai-planning board integrate #059/#060 通过，任务均为 DONE。具体记录见 .dual-ai-collab/integration-reports/。
- npm run package:mac 成功，codesign --verify --deep --strict 通过。macOS arm64 DMG 已保存 releases/Droid_Switch_0.1.0_arm64-provider-models.dmg；正式包未做 Apple 公证。
- Windows 供应商增量检查步骤补充至 docs/windows-testing.md；未生成或验证 Windows 安装包。
- 没有替换已安装的应用、没有重启用户会话或写入真实 Droid 配置。
