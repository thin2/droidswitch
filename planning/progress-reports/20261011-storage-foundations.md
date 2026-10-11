# Rust 基础设施与 SQLite · 2026-10-11

## #011 实现

- fs.rs：有界 UTF-8 读取、SHA-256/长度/纳秒时间戳指纹，同目录临时文件、收紧权限、写入、sync_all、原子替换；失败清理临时文件。
- permissions.rs：Unix 0600；Windows 受保护的仅所有者 ACL，权限设置失败即中止写入。
- commands.rs：注册文件读取/检查预期指纹后写入/HTTP 请求的 Tauri commands。应用内文件访问串行化，IPC 错误仅返回固定错误码。
- net.rs：HTTP(S)、自定义请求头、GET/POST、单次请求、禁用重试/重定向、最大 60 秒、最大 1 MiB 请求/响应。HTTP 非 2xx 原样返回状态，传输错误不带 URL/密钥。
- 文件原语不等于产品的“应用到 Droid”：备份、JSON 合并/校验和完整冲突流程仍归 #020/#021。检查与替换之间对外部进程仍有 TOCTOU 窗口，不宣称已经解决全部冲突保护。

## #012 实现

七张 STRICT 表：provider_connections、model_entries、apply_snapshots、drafts、usage_events、sync_cursors、model_prices。

迁移在 IMMEDIATE 事务中执行，维护 user_version；可重复、可从 1 回滚到 0，拒绝未知未来版本。连接启用外键、busy_timeout、FULL 同步；数据库文件使用私有权限。历史来源归属不依赖可删除的供应商行；未知 Token 保留 NULL；价格用十进制字符串存储，计算校验留给计价模块。

Schema 是数据层骨架，尚未连接 UI CRUD、统计采集或正式配置应用。

## 测试证据

cargo test --manifest-path src-tauri/Cargo.toml：16/16 单元测试通过。

- 文件与命令 7 项：替换、同长度编辑检测、大小限制/目录失败、部分写入注入失败、无效 UTF-8、0600/符号链接保护、过期指纹中止。
- HTTP 4 项：请求头与 401、302 不跟随、响应大小/URL 校验、超时分类。全部使用回环测试端点，没有调用真实模型。
- DB 5 项：全表迁移/幂等/回滚、DDL 中途失败回滚、未来版本拒绝、外键/同模型多供应商/Token 上限约束、重启草稿恢复/事件去重/未知 Token。
- Windows 权限源码通过 x86_64-pc-windows-msvc 的独立 cargo check。没有 Windows 实机，因此不等于 ACL 运行时或整个 Windows 应用已通过验收。
- CI Windows/macOS 矩阵增加 cargo test --locked。

参考：tempfile persist 官方 API 保证同文件系统内替换；目录重命名的掉电持久性不在本原语保证内。
https://docs.rs/tempfile/latest/tempfile/struct.NamedTempFile.html#method.persist

正式验收：verify run 011 的 fs/net 两条命令通过（7+4 项），verify run 012 的 db 命令通过（5 项）。cargo clippy --all-targets -- -D warnings 通过。当前会话自审通过。
