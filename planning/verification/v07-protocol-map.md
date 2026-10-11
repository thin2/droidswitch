# V-07 协议映射核对

核查日期：2026-10-11。来源：[Factory BYOK](https://docs.factory.com/model-independence/byok)。性质：官方在线文档核对，不等同于本机 Droid 0.238.0 实测。

| 内部协议 | Droid provider | API |
|---|---|---|
| Anthropic Messages | anthropic | Messages |
| OpenAI Responses | openai | Responses |
| OpenAI Chat Completions | generic-chat-completion-api | Chat Completions |

共同字段：model 为 API ID；displayName 为显示名；baseUrl 为基础地址；apiKey 可引用 ${VAR_NAME}；maxOutputTokens 为输出上限；noImageSupport=true 禁用图片；extraArgs 为请求参数；extraHeaders 为附加头。内部 UUID、enabled、测试耗时不可输出。协议应显式选择，不能根据模型名推断。

文档说明 settings.json / settings.local.json 支持 apiKey 环境变量插值，旧 config.json 不支持。进程环境检测不能证明其他终端也能读取变量。HTTP 无密钥端点可省略 apiKey；设置后不可为空。地址原样保存，不统一补删 /v1。

新增字段与 Bedrock 不纳入本轮支持；默认模型字段、选择器 ID 和本机凭据可用性仍需独立实测，不从在线文档推定。
