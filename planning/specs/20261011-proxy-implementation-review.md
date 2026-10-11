# 开源反代实现调研与方案修订

日期：2026-10-11。仅静态源码审查；浅克隆位于临时目录，没有运行项目、安装依赖、调用模型、访问本机账号文件或复制第三方实现进入产品。基于指定提交，可复核；不把 README 的兼容承诺视为本项目测试通过。

## 结论

前案 13–23 个开发日是自建授权/协议服务的估算。存在成熟代理可复用，建议改为“先验证外部 CLIProxyAPI，验证后再决定打包成随应用启动的独立进程”。Droid Switch 负责账号入口、服务管理和配置；代理负责上游认证与流式协议。不是先重写整套代理。

必须区分两条认证路线：本次三项目的 Codex OAuth 路径均使用 Codex 客户端身份及 ChatGPT Codex 后端，前案 SIWC 路线使用独立应用注册与公开 Responses API。登录页面在官方域名不等于第三方反代获得了与公开 SIWC 相同的接入保证。二者不能互换 token/client ID/端点；公开 SIWC 文档的参数约束也不能直接当作旧 Codex 路径的契约。

## 源码观察

### CLIProxyAPI：最值得先集成验证

审查提交：3de4e248f6bb12fc7dfafb183404f3b248578922。Go，根 LICENSE 为 MIT。

- 授权模块使用 Codex 客户端 OAuth、PKCE 和浏览器回调；刷新通过 singleflight 合并并发刷新。不是启动 Codex CLI 执行每一次任务。[认证源码](https://github.com/router-for-me/CLIProxyAPI/blob/3de4e248f6bb12fc7dfafb183404f3b248578922/internal/auth/codex/openai_auth.go)
- Codex executor 默认上游是 ChatGPT Codex backend；入口格式经 translator 转换。[执行入口](https://github.com/router-for-me/CLIProxyAPI/blob/3de4e248f6bb12fc7dfafb183404f3b248578922/internal/runtime/executor/codex_executor_execute.go)
- Responses 转换器设置流式/不存储，转换 system 角色，处理不支持的采样/长度字段及空工具参数。这里是实际兼容工作，不是简单改 base URL。[请求适配](https://github.com/router-for-me/CLIProxyAPI/blob/3de4e248f6bb12fc7dfafb183404f3b248578922/internal/translator/codex/openai/responses/codex_openai-responses_request.go)
- 独立管理 API 已有发起/取消 OAuth、查询状态、凭据与模型查询入口。界面可包装这些操作，不需要用户手工粘 token。[管理路由](https://github.com/router-for-me/CLIProxyAPI/blob/3de4e248f6bb12fc7dfafb183404f3b248578922/internal/api/server_management_v8.go)
- 配置提供监听地址、客户端密钥和独立管理密钥。集成时显式限制为本机，不沿用全部默认设置。[配置](https://github.com/router-for-me/CLIProxyAPI/blob/3de4e248f6bb12fc7dfafb183404f3b248578922/config.example.yaml)

可复用的是整个固定版本的代理进程和它的 API；Go 核心直接移植到 Rust 得不偿失。先锁定一个有对应平台产物的版本，确认其管理 API 与所审查代码一致，再做封装。当前阅读的是 HEAD，不承诺已有同提交发布包。

### CC Switch：同技术栈的架构参考

审查提交：df40d53c36b604e044b4d5b9091aeb961aa56990。Rust/Tauri，根 LICENSE 为 MIT。

- Codex OAuth 使用 Device Code：浏览器完成认证后轮询、换取令牌；账号单独管理，供应商通过 authBinding 关联账号；有到期提前刷新和账号级刷新锁。[认证模块](https://github.com/farion1231/cc-switch/blob/df40d53c36b604e044b4d5b9091aeb961aa56990/src-tauri/src/proxy/providers/codex_oauth_auth.rs)
- Codex OAuth 类型走固定上游，避免普通地址编辑把受管凭据发往任意地址。[供应商适配](https://github.com/farion1231/cc-switch/blob/df40d53c36b604e044b4d5b9091aeb961aa56990/src-tauri/src/proxy/providers/claude.rs)
- 请求格式和流式转换拆成独立模块；流状态分别维护文字、推理、工具参数、item/call ID 和完成标记。[流式状态实现示例](https://github.com/farion1231/cc-switch/blob/df40d53c36b604e044b4d5b9091aeb961aa56990/src-tauri/src/proxy/providers/streaming_codex_chat.rs)

借鉴账号与供应商分离、受管认证、刷新锁和 SSE 状态机。其转换模块依赖内部 provider/database/forwarder 等结构，不能把几个文件复制过来就宣称完成集成。现有本地 CC Switch 是否已有此功能，需要另外核实版本；本次未读取它的 OAuth 账号存储。普通供应商导入不等于 OAuth 账号迁移。

### Sub2API：作为边界案例与测试参考

审查提交：3a6fd1c9db07203ca308aaba69e502bc1f35b307。Go/Vue，README 架构含 PostgreSQL、Redis，根 LICENSE 标为 LGPL-3.0。

- OAuth 模块同样维护 Codex 客户端、PKCE、state 会话和过期清理。[OAuth](https://github.com/Wei-Shaw/sub2api/blob/3a6fd1c9db07203ca308aaba69e502bc1f35b307/backend/internal/pkg/openai/oauth.go)
- 网关明确有 ChatGPT Codex Responses 上游，配套大量失败、流式、会话和工具身份回归。[网关](https://github.com/Wei-Shaw/sub2api/blob/3a6fd1c9db07203ca308aaba69e502bc1f35b307/backend/internal/service/openai_gateway_service.go)
- 更偏服务器、多账号、多用户管理。对单人桌面工具不建议引入整套部署依赖。[项目说明](https://github.com/Wei-Shaw/sub2api/blob/3a6fd1c9db07203ca308aaba69e502bc1f35b307/README.md)

许可证仅记录文件标识，未作完整分发审查；真正打包复用时核对具体版本、依赖及通知要求，不能只凭根 LICENSE 下结论。

## 修订后的候选方案

| 方案 | 产品形态 | 粗估工程量 | 判断 |
| --- | --- | --- | --- |
| A 外接现有代理 | 用户已有 CLIProxyAPI，Droid Switch 检查连接并生成 Droid 配置 | 1–2 日连通验证；稳定外接版约 3–5 日 | 最快验证需求；需要另一个服务运行 |
| B 随软件打包 CLIProxyAPI | 用户在本软件登录、启停；代理作为受管子进程 | 首版约 5–10 日 | 推荐在 A 通过后采用；含适度返工，不含等待账号/Windows |
| C 借鉴 CC Switch，自建 Rust 代理 | 单一 Rust 服务，无 Go 子进程 | 约 8–15 日 | 保有较多协议与平台维护工作，不是零成本复制 |
| D 原独立 SIWC 注册方案 | 应用自己的订阅授权及 Responses 服务 | 原估算 13–23 日 | 与旧 Codex OAuth 接入路线不同；保留备选 |

以上是架构判断，不是已证明的工期。A 的验证工作可计入 B，不能把不同路线估算相加。若 Droid 关键工具/历史不兼容，或者必须补独立 SIWC 认证，B 的区间重新估算，不强行保持低估。

## 推荐推进顺序

1. #050 扩充为路线对照验证：固定 CLIProxyAPI 版本，检查授权、模型列表、Droid 单轮、多轮、函数工具/并行结果、取消和半途断流。原 SIWC 接入资格仍记录，作为不同路线的条件，不再将其作为评估所有代理方案的统一前提。
2. A 成功再确定 B：应用负责启动/停止/健康检查/版本检查；配置隔离到 Droid Switch 专用目录；上游账号凭据只由代理管理，不在两处同时刷新。
3. 管理 API 只由 Tauri 后端访问，管理密钥不进入浏览器页面或 Droid 配置；客户端密钥与管理密钥分开；OAuth 跳转目标校验官方域名，授权 URL/回调不进日志。
4. 只在本机监听，停用远程管理、无关供应商、自动账号轮询与付费后备；现有静态供应商继续使用原路径。采用代理的实际配置语义，不把前版配置样例套在新版本上。
5. Windows/macOS 打包需纳入受管进程清理、端口冲突、休眠恢复、进程崩溃、孤儿进程、升级回退、独立配置权限及 macOS 签名。只终止本软件创建的进程，不杀用户已有代理。
6. 用量沿用代理的可验证事件；未知值不补零。关闭应用时告知服务停止；配置中的本机地址保持稳定。

## 尚未验证

本次未验证账号登录是否成功、Droid 请求是否完整兼容、所选版本的平台二进制是否可用、管理 API 稳定性、升级兼容或第三方 Codex 接入的长期可用性。上述明确进入 #050/#058 验收，不以代码存在代替运行结果。

原 #051–#058 是自建路线拆分，暂保留 OPEN 作为参考。#050 决定采用 B 后，必须把它们重写为代理适配/进程管理/凭据边界/UI/验收任务，再开始实施；本轮不按旧表启动开发。
