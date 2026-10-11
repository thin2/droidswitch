# 用量开发契约与真实日志边界

当前本机 Droid 0.238.0 只有文本诊断日志，没有会话用量 JSONL 样本。不能声称已兼容所有真实会话格式。Windows 实测时应提供脱敏的结构/计数夹具补充适配；未识别行在 UI 单列，不当作零。

解析器支持明确标识的 droid-switch-usage-v1 元数据 JSONL（开发与导出适配契约），以及带稳定 id/turnId、sessionId、timestamp 的 SDK result 导出。SDK 未提供稳定事件 ID/时间时不臆造请求记录。普通输入与缓存包含关系未知时普通输入为未知，不给总计或完整费用。来源不明保持 unknown，不按当前模型设置归因。

示例（合成数据，不是真实 Droid 日志）：

```json
{"schema":"droid-switch-usage-v1","eventId":"request-1","sessionId":"session-1","occurredAt":1791691200,"scope":"request","source":"api","provider":"provider-snapshot","model":"api-model-id","tokens":{"input":100,"cacheRead":0,"cacheWrite":0,"output":10},"inputSemantics":"exclusive"}
```

scope 为 request 或 cumulative。branch 与 generation 可选，分支/计数器重置必须显式区分 generation；不明确的计数下降跳过并报告。同一 stream 只采用一种计数口径，避免回合汇总和累计更新相加。eventId 在 session/branch/generation 内稳定，跨文件去重。累计记录只展示累计差额，不冒充逐请求明细。时间为 Unix 秒或 SDK RFC3339；不依赖文件修改时间估计事件时间。

当前扫描读取文件后只增量插入未见事件（最多 128 MiB/批、单文件16 MiB），未做字节偏移优化；轮转和同事件多文件不会重复相加。只保存统计元数据。清理只删除本应用统计并记录截止边界，不删除原始日志。人工单价使用6位小数精度定点计算，费用12位小数；缺计数/单价不填零；改价不重算已保存历史，按事件时间选有效价格。

依据：[Factory SDK token usage](https://docs.factory.com/sdk/typescript#token-and-context-usage)：result 是单回合汇总（可能含委派工作），部分 token 更新是会话累计；不能与子任务/累计记录无区别相加。现有 SDK 导出入口并不自动采集用户运行中的 Droid；真实磁盘日志适配仍是 Windows 终验项。
