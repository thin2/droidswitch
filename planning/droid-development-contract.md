# 开发期兼容契约（不是最终真机验收）

2026-10-11 用户要求继续全部首版开发，完成后在 Windows 电脑实测。阶段 0 会话级实测及 #044 保留待验；开发依赖改为本契约，不能据此把 Windows 验收标为通过。

本机 CLI 版本 0.238.0。只读调用 droid exec --settings <隔离文件> --help 实际观察：
- Anthropic probe/shared / Probe A → custom:Probe-A-0
- OpenAI Responses probe/shared / Probe B → custom:Probe-B-0
- Chat Completions probe.space:1 / Probe C → custom:Probe-C-0

这些是真实 CLI catalog 输出，不是模型调用成功证据，也不是完整 /model 交互验收。应用必须通过 CLI catalog 获取 ID，不能用字符串拼接规则代替。

字段依据：官方 BYOK 的 model/displayName/baseUrl/apiKey/provider/maxOutputTokens/noImageSupport/extraArgs/extraHeaders。默认选择只写 sessionDefaultSettings.model 与 reasoningEffort；依据官方 Settings/SDK 文档，本机 --help 默认值不能证明默认模型生效，故 Windows 首次使用需核对新会话行为。其他 sessionDefaultSettings、spec/mission/subagent 配置均保留。

https://docs.factory.com/model-independence/byok
https://docs.factory.com/droid-cli/settings
https://docs.factory.com/sdk/python

新进程读取 runtime settings 已观察；既有会话是否热加载未知。应用后始终提示重新启动 Droid 并核对 /model，不宣称旧会话已切换。

本机无真实用户会话样本；Token 解析器须标识来源/覆盖，未知字段为 null，不从缺失值伪造 0。支持解析器的样本契约需单独标记 synthetic，Windows 验收后再确认日志格式支持范围。

初始开发仅启用已观察的 0.238.0 版本，其他版本保留草稿并阻止写入。不得改真实 ~/.factory 配置进行自动验收。
