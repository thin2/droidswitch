# 模型管理与路径（2026-10-11）

当前会话实现并自审：
- #015：Windows/macOS 默认路径及主目录展开测试通过；设置页可指定绝对路径，后端校验现有 JSON/普通文件，保存路径不创建或改写目标。
- #025：供应商模型列表 GET，保留自定义基础路径与查询参数，不强行追加 v1；鉴权环境变量解析；严格 data/ID 校验和去重。界面搜索、多选、确认导入；已有参数与远程消失项保留；后续分页明确提示。models-fetch 前端测试和 fetch 后端测试通过。
- #026：模型编辑/手动新增、独立内部 ID、原始 API ID、显示名/厂商/显式协议、可空正数输出上限、图片能力和 JSON 参数；API ID 同供应商唯一、不同供应商可并存。model-editor 前端测试通过（无效 JSON 不保存、清除上限、保留 ID）。模型批量幂等/保留编辑参数后端测试通过。
- #029 实现中：协议化短调用、强制单次额度确认、无项目正文、30 秒超时、不重试、错误分类、耗时及测试时间持久化。尚需补齐读取历史测试结果与配置校验入口，故不提交完成审查。
- #017 实现中：纯合并四项测试通过；应用时重读磁盘将在配置事务管道接入，暂不标为完成。

总体验证：Rust 34 项单元测试及 3 项 compile-fail 文档测试通过（加入 connection 后另行验证）；前端 3 项测试通过；TypeScript/Vite 构建与 clippy -D warnings 通过。

API 请求字段依据 [Anthropic Messages](https://platform.claude.com/docs/en/api/messages/create) 与 [OpenAI Responses](https://developers.openai.com/api/reference/resources/responses/methods/create)。本轮未使用真实凭据调用服务，未执行 Windows 真机验证。Windows 终验仍保留。
