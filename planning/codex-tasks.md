# Codex 任务板 - Droid Switch

**创建时间**: 2026-10-11 11:02:24
**规范文档**: planning/specs/20261011-110224-droid-switch.md
**总任务数**: 58
**预计总工时**: 首版(P1+P2)约 139 小时;含 P3 约 155 小时(理想专注工时,不含等待实测环境与真机验收往返)

> 优先级映射:P1=首版内核/前置,P2=首版紧随增量,P3=后续迭代。与文档 P0/P1/P2 的对应见规格文档 §0.3。
> 近期(M0–M2)已拆到 1–3h;标注"epic"的任务为较粗粒度,待阶段 0 实测结果出来后再细化为 1–3h 子任务。

---

> 新增待评审方案：planning/specs/20261011-chatgpt-local-router.md；#050–#058 仅规划，增量估算 13–23 个有效开发日，不计入上述旧版工时。原 #039/#043 和 Windows 实测未完成项继续保留。

## 任务 #001: 搭建阶段 0 实测环境与版本记录基线

**优先级**: P1
**状态**: DONE
**分配给**: Claude
**预计工时**: 2 小时
**依赖任务**: 无
**执行级别**: quick

### 任务描述
确认目标 Droid CLI 版本并记录版本号;准备一个最小 `~/.factory/settings.json` 样例;建立 `planning/verification/` 目录用于沉淀每项实测的输入配置、`/model` 观察结果与日志样本(作为后续适配器/解析器的夹具)。本机已有 Droid、暂缺 BYOK key,本任务不需要 key。

### 技术要求
- 记录 `droid --version`(或等价)精确版本号到 `planning/verification/version.md`
- 建立 `planning/verification/fixtures/` 存放真实 settings.json 片段与日志样本
- 不提交任何真实密钥;样本一律脱敏

### 验收标准
- [ ] `test -f planning/verification/version.md`
- [ ] `test -d planning/verification/fixtures`
- [ ] 版本号与采集方法已记录

### 相关文件
- `planning/verification/version.md`
- `planning/verification/fixtures/`

### 开发进展（2026-10-11）
当前会话已采集 Droid 0.238.0，验收 2/2 通过；详见 planning/progress-reports/20261011-bootstrap.md。等待 Git 集成确认。

---

## 任务 #002: 实测 /model 选择器 ID 生成规则 (V-01)

**优先级**: P1
**状态**: OPEN
**分配给**: Claude
**预计工时**: 3 小时
**依赖任务**: #001
**执行级别**: normal

### 任务描述
向 settings.json 的 customModels 写入已知模型,启动 Droid 进入 `/model`,抄下其实际显示/生成的选择器 ID,反推生成规则。严禁凭空假设 `custom:模型名` 前缀。

### 技术要求
- 覆盖至少 3 种协议/命名的模型
- 记录"写入字段 → 实际选择器 ID"的映射与推断规则

### 验收标准
- [ ] `test -f planning/verification/v01-selector-id.md`
- [ ] 记录了真实选择器 ID 样本,未使用凭空前缀

### 相关文件
- `planning/verification/v01-selector-id.md`

---

## 任务 #003: 实测默认模型字段(顶层 model vs sessionDefaultSettings)(V-03)

**优先级**: P1
**状态**: OPEN
**分配给**: Claude
**预计工时**: 3 小时
**依赖任务**: #001
**执行级别**: normal

### 任务描述
分别只写顶层 `model`、只写 `sessionDefaultSettings`,各开新会话验证哪个生效;确认目标版本应写哪处。严禁同时盲写两处。

### 技术要求
- 记录两种写法各自的生效结果与新会话验证方法
- 给出适配器应采用的字段结论(按版本)

### 验收标准
- [ ] `test -f planning/verification/v03-default-model.md`
- [ ] 明确结论:该版本默认模型写入字段 + 不双写

### 相关文件
- `planning/verification/v03-default-model.md`

---

## 任务 #004: 实测配置重新加载时机 (V-04)

**优先级**: P1
**状态**: OPEN
**分配给**: Claude
**预计工时**: 2 小时
**依赖任务**: #001
**执行级别**: quick

### 任务描述
确认 Droid 何时读取 settings.json(启动时/每会话/热加载),用于指导"应用后提示在 Droid 验证,不宣称会话已切换"的文案。

### 验收标准
- [ ] `test -f planning/verification/v04-reload-timing.md`
- [ ] 记录了重载时机结论

### 相关文件
- `planning/verification/v04-reload-timing.md`

---

## 任务 #005: 实测项目覆盖/旧版配置与目标文件覆盖关系 (V-05)

**优先级**: P1
**状态**: OPEN
**分配给**: Claude
**预计工时**: 2 小时
**依赖任务**: #001
**执行级别**: quick

### 任务描述
测项目级覆盖、旧版配置与目标 `settings.json` 的优先级/覆盖关系,确认初版"只写一个选定目标文件 + 来源提示,不静默迁移"的边界。

### 验收标准
- [ ] `test -f planning/verification/v05-override.md`
- [ ] 记录了覆盖关系与来源提示策略

### 相关文件
- `planning/verification/v05-override.md`

---

## 任务 #006: 实测会话日志:位置/字段/缓存口径/Token 口径/切换记录 (V-06 + Token 口径)

**优先级**: P1
**状态**: OPEN
**分配给**: Claude
**预计工时**: 3 小时
**依赖任务**: #001
**执行级别**: normal

### 任务描述
定位 Droid 本地会话日志位置、字段结构、缓存读/写口径、普通输入/输出/思考 Token 的包含关系与模型切换记录;普通终端启动的 Droid 也要纳入。产出夹具样本供统计解析器单测。若日志不落 token 明细,如实记录"缺失"。

### 技术要求
- 覆盖:普通终端启动、软件应用后启动两种场景
- 明确互斥口径与 null 语义;不猜测补齐

### 验收标准
- [ ] `test -f planning/verification/v06-usage-logs.md`
- [ ] `test -d planning/verification/fixtures/logs`
- [ ] 记录字段结构、缓存口径、Token 包含关系结论

### 相关文件
- `planning/verification/v06-usage-logs.md`
- `planning/verification/fixtures/logs/`

---

## 任务 #007: 核对协议映射(以官方 BYOK 文档为准)(V-07)

**优先级**: P1
**状态**: DONE
**分配给**: Claude
**预计工时**: 2 小时
**依赖任务**: 无
**执行级别**: quick

### 任务描述
以 https://docs.factory.com/model-independence/byok 为准,核对 Anthropic Messages / OpenAI Responses / OpenAI Chat Completions 的字段与凭据(含 `${VAR}` 引用)映射,形成适配器映射表。

### 验收标准
- [ ] `test -f planning/verification/v07-protocol-map.md`
- [ ] 三种协议字段映射表完成,标注文档出处与核查日期

### 相关文件
- `planning/verification/v07-protocol-map.md`

### 开发进展（2026-10-11）
官方三协议映射核查及验收 1/1 通过；详见 planning/progress-reports/20261011-bootstrap.md。仅文档核对，等待 Git 集成确认。

---

## 任务 #008: 实测重复模型(同 ID 多供应商)识别方式 (V-02)

**优先级**: P1
**状态**: OPEN
**分配给**: Claude
**预计工时**: 2 小时
**依赖任务**: #001
**执行级别**: quick

### 任务描述
写入两家供应商的相同模型 ID,观察目标版本如何处理与区分(选择器层面)。部分验证需真实调用,等 key 到位补做;先完成配置层识别。

### 技术要求
- **接口依赖**: 真实调用部分依赖 BYOK key(基于配置层先行,调用层后补)

### 验收标准
- [ ] `test -f planning/verification/v02-duplicate-models.md`
- [ ] 记录配置层识别结论,标注待 key 补验项

### 相关文件
- `planning/verification/v02-duplicate-models.md`

---

## 任务 #009: 产出 Droid 版本契约文档 (V-08)

**优先级**: P1
**状态**: OPEN
**分配给**: Claude
**预计工时**: 2 小时
**依赖任务**: #002, #003, #004, #005, #006, #007
**执行级别**: quick

### 任务描述
汇总 V-01~V-07 实测结果,形成"版本契约文档":记录 Droid 版本号 + 每项字段/规则/日志结论,作为适配器的权威配置源。标注哪些结论待 key 补验。

### 验收标准
- [ ] `test -f planning/droid-version-contract.md`
- [ ] 契约覆盖选择器 ID、默认字段、重载时机、覆盖关系、日志口径、协议映射

### 相关文件
- `planning/droid-version-contract.md`

---

## 任务 #010: 初始化 Tauri 2 + React + TS 骨架 (S-01)

**优先级**: P1
**状态**: DONE
**分配给**: Codex
**预计工时**: 3 小时
**依赖任务**: 无
**执行级别**: normal

### 任务描述
初始化 Tauri 2 + React + TypeScript 项目骨架,配置 Win/mac 构建,建立前端路由与基础目录结构;初始化 git 仓库。

### 技术要求
- Tauri 2 稳定版;Vite + React + TS
- 前端目录与 `src-tauri/` Rust 目录就绪

### 验收标准
- [ ] `cd /Users/hanning/Desktop/droidswitch && npm install`
- [ ] `cd /Users/hanning/Desktop/droidswitch && npm run build`
- [ ] `cargo build --manifest-path src-tauri/Cargo.toml`

### 相关文件
- `package.json`
- `src-tauri/Cargo.toml`
- `src/`

### 开发进展（2026-10-11）
骨架代码已落地；环境阻塞：npm 下载不可达且离线缺依赖、Rust 工具链缺失、.git 不可写。构建未通过，保持 IN_PROGRESS。详见 planning/progress-reports/20261011-bootstrap.md。恢复后先安装依赖并执行完整验收。

### 依赖排障（2026-10-11）
macOS SDK 已确认可用。npm 联网被权限策略拒绝，提权申请亦被拒，Rust 未安装。新增 scripts/setup-deps-macos.sh，bash 语法检查通过，安装及构建尚未完成。恢复步骤及证据：planning/progress-reports/20261011-dependencies.md。保持 IN_PROGRESS。

### 构建验收通过（2026-10-11）
权限阻塞已解除。修复 React 类型依赖冲突，安装 Rust 1.99.0，生成 npm/Cargo 锁文件并初始化 Git；verify run 010 的三条命令全部通过。详见 planning/progress-reports/20261011-bootstrap-verified.md。Windows 实机及 UI 交互尚未验证；下一步待授权本地提交及集成核验。

---

## 任务 #011: Rust 基础能力:文件读写/原子替换/文件指纹/网络 (S-02)

**优先级**: P1
**状态**: DONE
**分配给**: Codex
**预计工时**: 3 小时
**依赖任务**: #010
**执行级别**: normal

### 任务描述
在 Rust 层实现:读写文件、原子替换(临时文件 + os::rename)、文件指纹(hash + mtime/size)、HTTP 请求基础封装、文件权限收紧工具函数。暴露为 Tauri command。

### 技术要求
- 原子替换跨平台(Win/mac)
- 指纹用于冲突检测;网络封装支持自定义 header

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml fs`
- [ ] 原子替换与指纹有单测覆盖
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml net`

### 相关文件
- `src-tauri/src/fs.rs`
- `src-tauri/src/net.rs`

### 实现记录（2026-10-11）
实现及测试见 planning/progress-reports/20261011-storage-foundations.md。当前会话直接实现和自审；本机 macOS 测试及 Windows 权限代码交叉编译通过，Windows 实机未验证。

---

## 任务 #012: SQLite schema + 迁移 (S-03)

**优先级**: P1
**状态**: DONE
**分配给**: Codex
**预计工时**: 3 小时
**依赖任务**: #010
**执行级别**: normal

### 任务描述
建立 SQLite schema 与迁移:ProviderConnection、ModelEntry、ApplySnapshot、草稿、统计事件、同步游标、单价/币种设置。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml db`
- [ ] 迁移可从空库建全表并回滚

### 相关文件
- `src-tauri/src/db/schema.rs`
- `src-tauri/src/db/migrations/`

### 实现记录（2026-10-11）
实现及测试见 planning/progress-reports/20261011-storage-foundations.md。当前会话直接实现和自审；本机 macOS 测试及 Windows 权限代码交叉编译通过，Windows 实机未验证。

---

## 任务 #013: 三类 ID 分离的核心数据模型 (S-04)

**优先级**: P1
**状态**: DONE
**分配给**: Codex
**预计工时**: 2 小时
**依赖任务**: #012
**执行级别**: quick

### 任务描述
定义内部数据类型,物理分离应用内部 UUID、API 模型 ID、Droid 选择器 ID 三者;ModelEntry 通过 providerId 引用连接。严禁用显示名或拼接前缀作为标识。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml model`
- [ ] 类型层面三类 ID 不可互相误用(编译期区分)
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --doc model`

### 相关文件
- `src-tauri/src/model/ids.rs`
- `src-tauri/src/model/entry.rs`

### 实现与验收（2026-10-11）
三类 ID newtype 与 ModelEntry 已实现；20 项全量单元测试及 3 项 compile_fail 文档测试通过，clippy 无警告。详见 planning/progress-reports/20261011-model-identities.md。选择器真实值仍待阶段 0 实测。

---

## 任务 #014: 配置适配器层骨架(版本化、字段映射)(S-05)

**优先级**: P1
**状态**: DONE
**分配给**: Codex
**预计工时**: 3 小时
**依赖任务**: #049, #011, #013
**执行级别**: normal

### 任务描述
实现适配器层:以版本契约文档为配置源,集中处理字段映射与版本差异;只输出合并后的 Droid 支持字段;不认识的版本优雅降级并提示。用阶段 0 夹具做单测。

### 技术要求
- 适配器按目标 Droid 版本选择映射
- 协议映射来自 V-07 表

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml adapter`
- [ ] 夹具驱动的字段映射单测通过

### 相关文件
- `src-tauri/src/adapter/mod.rs`
- `src-tauri/src/adapter/versions.rs`

### Windows 验收后置
用户于 2026-10-11 要求先完成首版开发，再由用户在 Windows 实测。开发依据 #049；原阶段 0 和 #044 会话级验收保留，不能据开发测试替代。

---

## 任务 #015: 默认目标路径解析 + 可配置路径 (C-01)

**优先级**: P1
**状态**: DONE
**分配给**: Codex
**预计工时**: 2 小时
**依赖任务**: #014
**执行级别**: quick

### 任务描述
默认目标 `~/.factory/settings.json`,设置页可指定其他路径;Win/mac 路径解析各自实现。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml path`
- [ ] Win/mac 默认路径解析有测试

### 相关文件
- `src-tauri/src/config/path.rs`

---

## 任务 #016: 首次导入标"外部配置" + 接管机制 (C-02)

**优先级**: P1
**状态**: DONE
**分配给**: Codex
**预计工时**: 2 小时
**依赖任务**: #015
**执行级别**: quick

### 任务描述
首次导入已有条目标记为"外部配置";用户明确接管后才允许修改或移除。检测项目覆盖/旧配置时展示来源提示,不静默迁移。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml takeover`
- [ ] 未接管项不可被改/删(有测试)

### 相关文件
- `src-tauri/src/config/import.rs`

---

## 任务 #017: 非破坏式读取合并(保留未知字段/未管理模型)(C-03)

**优先级**: P1
**状态**: DONE
**分配给**: Codex
**预计工时**: 3 小时
**依赖任务**: #014
**执行级别**: normal

### 任务描述
应用时重读磁盘,保留未管理模型、未知字段及其他设置,仅更新本软件管理的字段。改一个模型不得影响其余语义;无变化不重复添加。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml merge`
- [ ] 未知字段/未管理模型在合并后语义不变(有测试,对应 A-02)

### 相关文件
- `src-tauri/src/config/merge.rs`

---

## 任务 #018: 不写管理字段进 customModels (C-12)

**优先级**: P1
**状态**: DONE
**分配给**: Codex
**预计工时**: 2 小时
**依赖任务**: #017
**执行级别**: quick

### 任务描述
确保 enabled、内部 UUID、测试耗时等管理字段不写入 customModels,只写 Droid 支持字段。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml customModels`
- [ ] 输出 JSON 不含任何管理字段(有测试)

### 相关文件
- `src-tauri/src/config/merge.rs`

---

## 任务 #019: 变更预览(新增/修改/停用/删除 + 脱敏差异)(C-04)

**优先级**: P1
**状态**: DONE
**分配给**: Codex
**预计工时**: 3 小时
**依赖任务**: #017
**执行级别**: normal

### 任务描述
生成变更预览:分组统计新增/修改/停用/删除数量 + 脱敏差异;密钥只显示"密钥已更改",不显示新旧值;展示写入路径。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml preview`
- [ ] 差异中无完整密钥(有测试)

### 相关文件
- `src-tauri/src/config/preview.rs`

---

## 任务 #020: 原子写入(备份→临时→校验 JSON→替换)(C-05)

**优先级**: P1
**状态**: DONE
**分配给**: Codex
**预计工时**: 3 小时
**依赖任务**: #011, #017
**执行级别**: normal

### 任务描述
实现应用写入:生成完整备份 → 写临时文件 → 校验 JSON → 原子替换;写入失败保留原文件与草稿。含故障注入测试(写一半中断、无效 JSON、磁盘满)。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml atomic_write`
- [ ] 故障注入下原配置不损坏(有测试,对应 A-05)

### 相关文件
- `src-tauri/src/config/apply.rs`

---

## 任务 #021: 文件指纹 + 写入前核对 + 冲突中止 (C-06)

**优先级**: P1
**状态**: DONE
**分配给**: Codex
**预计工时**: 3 小时
**依赖任务**: #020
**执行级别**: normal

### 任务描述
读取时记录文件指纹,写入前再次核对;外部文件变化时停止应用,提示"配置已被其他程序修改",提供重新载入/重新合并。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml fingerprint`
- [ ] 外部修改后应用被中止(有测试,对应 A-04)

### 相关文件
- `src-tauri/src/config/apply.rs`

---

## 任务 #022: 重新载入不丢草稿(导出/取消)(C-07)

**优先级**: P1
**状态**: DONE
**分配给**: Codex
**预计工时**: 2 小时
**依赖任务**: #021
**执行级别**: quick

### 任务描述
重新载入不直接丢弃草稿,允许导出或取消;重启软件可恢复草稿;放弃更改回到已应用状态。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml draft`
- [ ] 重启恢复草稿、放弃回到已应用态(有测试,对应 A-03)

### 相关文件
- `src-tauri/src/config/draft.rs`

---

## 任务 #023: 备份恢复(先备份当前 + 展示影响)(C-08)

**优先级**: P1
**状态**: DONE
**分配给**: Codex
**预计工时**: 2 小时
**依赖任务**: #020
**执行级别**: quick

### 任务描述
恢复备份前先备份当前文件;展示完整恢复将影响的内容;恢复后重新导入内容。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml restore`
- [ ] 恢复后文件与选定快照一致(有测试,对应 A-05)

### 相关文件
- `src-tauri/src/config/backup.rs`

---

## 任务 #024: 供应商连接 CRUD + 同站多连接 + 协议/厂商分存 (P-01/P-02)

**优先级**: P1
**状态**: DONE
**分配给**: Codex
**预计工时**: 3 小时
**依赖任务**: #013
**执行级别**: normal

### 任务描述
供应商连接新增/编辑/复制/删除;同一中转站可多连接;供应商名称、API 协议、模型厂商分别保存,不按模型名自动判协议;API 基础地址不盲目追加/删除 /v1。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml provider`
- [ ] 同站多连接可共存(有测试)

### 相关文件
- `src-tauri/src/provider/mod.rs`
- `src/views/providers/`

---

## 任务 #025: 获取模型多选窗口(搜索/全选/批量/已添加/不覆盖/消失只提示)(P-03/04/05)

**优先级**: P1
**状态**: DONE
**分配给**: Codex
**预计工时**: 3 小时
**依赖任务**: #024
**执行级别**: normal

### 任务描述
"获取模型"多选窗口:搜索、全选当前结果、批量添加;接口返回 ID 原样保留;已有模型标"已添加",重复获取不覆盖本地名称/参数;远端消失的模型只提示不自动删除。

### 验收标准
- [ ] `npm run test -- models-fetch`
- [ ] 重复获取不覆盖本地编辑(有测试,对应 A-02)

### 相关文件
- `src/views/models/FetchDialog.tsx`
- `src-tauri/src/provider/fetch.rs`

---

## 任务 #026: 模型字段编辑 + 显示名建议 + 手动输入 ID (P-06/07/11)

**优先级**: P1
**状态**: DONE
**分配给**: Codex
**预计工时**: 3 小时
**依赖任务**: #024
**执行级别**: normal

### 任务描述
模型字段:显示名、真实 ID、协议覆盖、启用、最大输出 Token(可空正整数)、图片输入、高级参数;显示名建议"模型名 · 连接名";内部用独立 UUID;提供手动输入模型 ID 入口。

### 验收标准
- [ ] `npm run test -- model-editor`
- [ ] 最大输出 Token 可留空、协议可覆盖(有测试)

### 相关文件
- `src/views/models/ModelEditor.tsx`

---

## 任务 #027: 停用/删除供应商 + 悬空引用检测 (P-08/09/10)

**优先级**: P1
**状态**: DONE
**分配给**: Codex
**预计工时**: 2 小时
**依赖任务**: #026
**执行级别**: quick

### 任务描述
停用保留本地记录,仅下次应用时移出同步集合;删除供应商前列出受影响模型数,先入草稿,正式移除在预览展示;检测默认模型引用,阻止悬空引用。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml dangling`
- [ ] 删除会产生悬空默认引用时被阻止(有测试)

### 相关文件
- `src-tauri/src/provider/delete.rs`

---

## 任务 #028: 凭据:${VAR} 默认 + 直写兼容 + 检测说明 + 脱敏 (C-09/10/11)

**优先级**: P1
**状态**: DONE
**分配给**: Codex
**预计工时**: 3 小时
**依赖任务**: #020
**执行级别**: normal

### 任务描述
默认 `${VAR_NAME}` 引用模式;检测自身进程能否读取变量,并明确说明"这不证明启动 Droid 的其他终端具备该环境";直写为显式兼容模式,首次清晰告知文件及备份将含明文密钥;文件权限收紧,界面默认遮盖,日志/导出一律脱敏。

### 技术要求
- 两种模式分别可用;UI 文案含"其他终端不保证"说明
- 全链路脱敏

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml credential`
- [ ] 日志/导出无完整密钥(有测试,对应 A-06)

### 相关文件
- `src-tauri/src/config/credential.rs`
- `src/views/providers/CredentialForm.tsx`

---

## 任务 #029: 连接测试:校验/列表/调用/延迟 (T-01~T-04)

**优先级**: P1
**状态**: DONE
**分配给**: Codex
**预计工时**: 3 小时
**依赖任务**: #024
**执行级别**: normal

### 任务描述
三种验证区分:配置校验(字段/JSON/引用)、模型列表获取测试、模型调用测试(用户点击触发、提示耗额度、不带项目代码、标鉴权/响应/延迟);延迟展示完整请求耗时 + 测试时间;分别处理 401/403、404、429、超时、不支持列表的反馈。调用测试需 key,等 key 到位真机验。

### 技术要求
- **接口依赖**: 调用测试依赖 BYOK key(列表/校验可先行)

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml conn_test`
- [ ] 各错误码有对应文案,不自动重试(有测试)

### 相关文件
- `src-tauri/src/test/connection.rs`

---

## 任务 #030: 三栏主界面 + 左导航 + 供应商筛选 + API 视图顶部 (UI-01/02/04/05) [epic]

**优先级**: P1
**状态**: DONE
**分配给**: Codex
**预计工时**: 6 小时
**依赖任务**: #013
**执行级别**: heavy

### 任务描述
三栏布局(左导航/中列表/右详情,小窗详情转抽屉);左导航模型库/供应商/用量统计/配置备份 + 底部设置 + 供应商筛选;API 视图顶部页面名/模型数量/导入配置/获取模型/添加模型;列表行显示模型名+ID/供应商/协议/最近测试/启用开关,选中右侧就地编辑。epic,待细化。

### 验收标准
- [ ] `npm run build`
- [ ] 三栏布局与小窗抽屉切换符合效果图

### 相关文件
- `src/App.tsx`
- `src/views/models/ModelLibrary.tsx`
- `src/components/Layout/`

---

## 任务 #031: 底部变更条 + 草稿语义("保存修改"=次要=草稿)(UI-06/07)

**优先级**: P1
**状态**: DONE
**分配给**: Codex
**预计工时**: 2 小时
**依赖任务**: #019, #030
**执行级别**: quick

### 任务描述
底部固定变更条:左待应用数量,右"放弃更改"/"预览变更"/"应用到 Droid";编辑面板"保存修改"为次要按钮,明确仅存草稿。

### 验收标准
- [ ] `npm run build`
- [ ] 保存修改只改草稿、不写盘(有测试,对应 A-03)

### 相关文件
- `src/components/ChangeBar.tsx`

---

## 任务 #032: 视觉基线 + 字体 + 响应式 + 可访问性 (UI-08~UI-11) [epic]

**优先级**: P1
**状态**: DONE
**分配给**: Codex
**预计工时**: 5 小时
**依赖任务**: #030
**执行级别**: heavy

### 任务描述
实现视觉基线(暖白/面板/侧栏/陶土橙 token)、字体(中文无衬线/Inter/等宽)、尺寸步进、圆角;响应式(<1200px 折叠详情,最小 1024×720);可访问性(控件≥36px、键盘焦点、状态文字说明)。Win/mac webview 各回归一次。epic,待细化。

### 验收标准
- [ ] `npm run build`
- [ ] 关键路径键盘可达、状态有文字说明

### 相关文件
- `src/styles/tokens.css`
- `src/styles/theme.ts`

### 开发与实机验收边界
按用户先完成开发、后在 Windows 实测的安排，本任务开发验收采用 1440/1024 浏览器渲染与键盘回归；Windows/macOS 原生 WebView 回归统一保留在 #044，不将浏览器测试计为原生验证。

---

## 任务 #033: 顶部分段控件 + 两套方案保存 (SW-01/02, UI-03)

**优先级**: P2
**状态**: DONE
**分配给**: Codex
**预计工时**: 3 小时
**依赖任务**: #014, #031
**执行级别**: normal

### 任务描述
顶部分段控件区分浏览方案/待应用方案/已应用来源;官方订阅与 API 两套方案分别保存 modelSource、最近有效模型 ID、相应思考参数;切换只产生待应用方案,底部应用后才生效。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml source_scheme`
- [ ] 两套方案独立保存、互不覆盖(有测试)

### 相关文件
- `src-tauri/src/switch/scheme.rs`
- `src/components/SourceSegment.tsx`

---

## 任务 #034: 官方页:模型选择/刷新/前往登录/状态待确认 (SW-03)

**优先级**: P2
**状态**: DONE
**分配给**: Codex
**预计工时**: 3 小时
**依赖任务**: #033
**执行级别**: normal

### 任务描述
官方订阅页:展示可用官方模型、当前选择、刷新、"前往 Droid 登录";读不到登录信息显示"状态待确认",不推测套餐或额度。

### 验收标准
- [ ] `npm run build`
- [ ] 无登录信息时显示"状态待确认"(有测试)

### 相关文件
- `src/views/official/OfficialView.tsx`

---

## 任务 #035: 切换生成目标支持默认设置 + 预览 + 不覆盖/不回退 (SW-04~08) [epic]

**优先级**: P2
**状态**: DONE
**分配给**: Codex
**预计工时**: 5 小时
**依赖任务**: #049, #033
**执行级别**: heavy

### 任务描述
切换仅生成目标版本支持的默认设置;参数不兼容时用目标模型默认值并在预览列出;不清空 customModels、不覆盖整份 settings.json、不改登录凭据、不自动开会员;子 Agent/Skills/Spec/Missions 显式模型不被覆盖;优先级提示(已开会话/CLI 显式/项目设置/任务覆盖);不自动付费回退。依赖 V-03 默认字段结论。epic,待细化。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml switch`
- [ ] "API→官方→API"往返不丢两套配置、无自动回退(有测试,对应 A-08)

### 相关文件
- `src-tauri/src/switch/apply.rs`

### Windows 验收后置
用户于 2026-10-11 要求先完成首版开发，再由用户在 Windows 实测。开发依据 #049；原阶段 0 和 #044 会话级验收保留，不能据开发测试替代。

---

## 任务 #036: 定位数据目录 + 只读 WAL 读 cc-switch.db (I-02/I-03)

**优先级**: P2
**状态**: DONE
**分配给**: Codex
**预计工时**: 3 小时
**依赖任务**: #011
**执行级别**: normal

### 任务描述
定位 CC Switch 数据目录(默认 `~/.cc-switch`,支持自定义/手选);以只读连接 + 一致性读取(兼容 WAL)读取 `cc-switch.db` 供应商数据库;读取失败/未知结构明确提示,不当空库处理。用本机真实 db 验证。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml ccswitch_read`
- [ ] 只读打开、源文件不被修改(有测试,对应 A-09)

### 相关文件
- `src-tauri/src/import/ccswitch.rs`

---

## 任务 #037: 导入向导 + 来源选择 + 字段映射 + 重复项 + 脱敏 (I-01/04/05/06/08) [epic]

**优先级**: P2
**状态**: DONE
**分配给**: Codex
**预计工时**: 6 小时
**依赖任务**: #036, #026
**执行级别**: heavy

### 任务描述
入口模型库顶部"导入配置→从 CC Switch 导入";向导(定位→选来源应用/供应商→检查连接及模型→处理重复→导入草稿);导入 Claude Code/Codex 的 API 供应商与模型;原生接口依赖协议转换的标"不支持直接导入",允许补 Droid 兼容接口;重复项识别(来源应用+供应商 ID+源位置+目标记录),默认跳过相同,修改项提供更新/另存不静默覆盖;凭据按模式保存并脱敏预览。epic,待细化。

### 验收标准
- [ ] `npm run build`
- [ ] 重复导入不增殖(有测试,对应 A-09)

### 相关文件
- `src/views/import/ImportWizard.tsx`
- `src-tauri/src/import/mapping.rs`

---

## 任务 #038: 导入边界:无模型/读取失败/结果汇总/不导历史 (I-07/09/10/11)

**优先级**: P2
**状态**: DONE
**分配给**: Codex
**预计工时**: 3 小时
**依赖任务**: #037
**执行级别**: normal

### 任务描述
无模型记录时先导入供应商,提供获取/手动添加入口;读取失败提示退出 CC Switch 重试或选备份;结果汇总(已导入/已跳过/待补充),仅"应用到 Droid"才改配置;不导入 CC Switch 历史用量;取消不改草稿。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml import_edge`
- [ ] 取消导入不改草稿、不导历史用量(有测试,对应 A-09)

### 相关文件
- `src-tauri/src/import/summary.rs`

---

## 任务 #039: 只读扫描 Droid 日志 + 稳定事件键去重 + 累计转增量 (U-03/U-04) [epic]

**优先级**: P2
**状态**: IN_PROGRESS
**分配给**: Codex
**预计工时**: 6 小时
**依赖任务**: #049, #012
**执行级别**: heavy

### 任务描述
只读扫描 Droid 本地会话记录,增量导入用量;用稳定事件键去重;累计计数按会话有效增量;处理重放、文件轮转、计数重置、分支、重试;相同事件的多来源不重复相加;只有累计值时不伪造逐请求明细。基于 V-06 夹具开发。epic,待细化。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml usage_scan`
- [ ] 重复扫描不重复计数、累计转增量(有测试,对应 A-10)

### 相关文件
- `src-tauri/src/usage/scanner.rs`
- `src-tauri/src/usage/dedup.rs`

### Windows 验收后置
用户于 2026-10-11 要求先完成首版开发，再由用户在 Windows 实测。开发依据 #049；原阶段 0 和 #044 会话级验收保留，不能据开发测试替代。

### 当前实现与未完成项
元数据契约/SDK 导出解析、事件去重、累计差额已实现；本机缺真实 Droid 会话用量样本，原生日志格式适配与委派汇总跨源去重仍待真实夹具确认，保持 IN_PROGRESS。详见 planning/droid-usage-contract.md。

---

## 任务 #040: Token 口径归一 + 费用估算 + 手动单价/币种 (U-05/06/07/08)

**优先级**: P2
**状态**: DONE
**分配给**: Codex
**预计工时**: 3 小时
**依赖任务**: #049, #012
**执行级别**: normal

### 任务描述
Token 归一为互斥的普通输入/缓存读/缓存写/输出,null=未知不转 0,无法确定包含关系时保留分项隐藏总计;API 估算费用按各类 Token × 各自每百万单价求和;手动单价 + 币种,保存价格来源/生效时间,改价不默认改写历史;来源分别统计,无法确认归"未知来源"。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml usage_cost`
- [ ] 缺单价显示"未定价"、null 不转 0(有测试,对应 A-10)

### 相关文件
- `src-tauri/src/usage/cost.rs`

### 开发依赖与真实数据边界
计费核心可按 #049 开发期元数据契约独立验收，避免用户要求的 Windows 后置测试阻塞本地开发。真实日志解析兼容仍由 #039/#006/#044 跟踪，不以费用单元测试替代。

---

## 任务 #041: 用量页 UI + 筛选 + 趋势/分布 + 明细切换 + CSV + 保留策略 (U-01/02/09/10/11) [epic]

**优先级**: P2
**状态**: DONE
**分配给**: Codex
**预计工时**: 6 小时
**依赖任务**: #040
**执行级别**: heavy

### 任务描述
独立用量统计页:筛选栏(今日/近7天/近30天/自定义、来源、供应商、模型,本机时区)+ 核心指标 + Token 趋势/模型分布 + 明细切换(模型统计/供应商统计/使用记录)+ 刷新/单价设置/导出 CSV;显示数据来源/覆盖范围/最后更新;无数据给采集设置入口,部分缺失说明范围不用 0 代替;默认只存统计元数据,支持保留期/导出/清理,不删 Droid 原始会话。epic,待细化。

### 验收标准
- [ ] `npm run build`
- [ ] 导出 CSV 可用、无数据有采集入口(有测试)

### 相关文件
- `src/views/usage/UsageView.tsx`

---

## 任务 #042: 辅助页面与空/错/载入状态 + 配置备份页 (§6 辅助页面)

**优先级**: P2
**状态**: DONE
**分配给**: Codex
**预计工时**: 3 小时
**依赖任务**: #032
**执行级别**: normal

### 任务描述
辅助页面与弹窗:供应商页、获取模型页、变更预览页、配置备份页(时间/来源/变更摘要、先预览再恢复、可打开所在目录)、设置页;空白与错误状态(无供应商引导添加、读取失败显示修复入口不展示虚假数据)。

### 验收标准
- [ ] `npm run build`
- [ ] 空/错/载入状态齐全,不展示演示数据

### 相关文件
- `src/views/backup/BackupView.tsx`
- `src/components/states/`

---

## 任务 #043: 打包:Windows + macOS 安装包 + 设置页

**优先级**: P2
**状态**: IN_PROGRESS
**分配给**: Codex
**预计工时**: 3 小时
**依赖任务**: #014, #028
**执行级别**: normal

### 任务描述
配置 Tauri 打包产出 Windows 与 macOS 安装包;设置页(配置路径、Droid 检测、外观、凭据说明、备份保留策略);Win/mac 各出一个可安装产物。

### 验收标准
- [ ] `npm run package:mac`
- [ ] Windows NSIS 安装产物在 Windows 构建后确认，不能由 macOS 产物替代。

### 相关文件
- `src-tauri/tauri.conf.json`
- `src/views/settings/SettingsView.tsx`

### 打包环境记录
默认 create-dmg 的 Finder 包装在当前桌面环境失败，已提供不依赖 Finder AppleScript 的 macOS 包装脚本；Windows 构建脚本与 CI NSIS 流程完成，但未在 Windows 运行。保持 IN_PROGRESS 直至 Windows 产物验证。

---

## 任务 #044: 首版验收:A-01~A-10 真机核对 [epic]

**优先级**: P2
**状态**: OPEN
**分配给**: Claude
**预计工时**: 8 小时
**依赖任务**: #023, #027, #028, #029, #031, #035, #038, #041
**执行级别**: heavy

### 任务描述
在受支持的 Droid 版本上,用真实会话样本逐条核对 A-01~A-10 并签字;不得用演示数据代替。需 BYOK key 到位。产出验收报告。epic,逐条可拆。

### 验收标准
- [ ] `test -f planning/acceptance-report.md`
- [ ] A-01~A-10 逐条结论与证据(真机 /model、独立终端凭据、Droid 用量核对)

### 相关文件
- `planning/acceptance-report.md`

### 后置实机验收清单
用户指定首版开发后在 Windows 实测；入口 docs/windows-testing.md。包括原生 WebView、实际默认模型生效、ACL/备份恢复、真实日志适配、Windows 安装产物；这些项目尚未通过。

---

## 任务 #045: 深色主题 (UI-12)

**优先级**: P3
**状态**: OPEN
**分配给**: Codex
**预计工时**: 3 小时
**依赖任务**: #032
**执行级别**: normal

### 任务描述
基于视觉 token 增加深色主题;浅/深色均通过可访问性对比检查。首版不做,列为后续。

### 验收标准
- [ ] `npm run build`
- [ ] 深色主题下关键页面对比度达标

### 相关文件
- `src/styles/theme.ts`

---

## 任务 #046: 流式与工具调用诊断 (T-05)

**优先级**: P3
**状态**: OPEN
**分配给**: Codex
**预计工时**: 3 小时
**依赖任务**: #029
**执行级别**: normal

### 任务描述
扩展连接测试:流式响应诊断与工具调用诊断。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml diag`
- [ ] 流式/工具调用诊断可用

### 相关文件
- `src-tauri/src/test/diagnostics.rs`

---

## 任务 #047: Analytics API 补充源 / 成功率延迟额度 (U-12/U-13)

**优先级**: P3
**状态**: OPEN
**分配给**: Codex
**预计工时**: 3 小时
**依赖任务**: #041
**执行级别**: normal

### 任务描述
Factory Analytics API 作为有权限账号补充来源(UTC、截至昨日);成功率/延迟/官方剩余额度在数据源支持时启用;无权限时保持本地统计并展示限制。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml analytics`
- [ ] 无权限时优雅降级为本地统计

### 相关文件
- `src-tauri/src/usage/analytics.rs`

---

## 任务 #048: 供应商余额查询 / 本地代理采集 (U-14)

**优先级**: P3
**状态**: OPEN
**分配给**: Codex
**预计工时**: 7 小时
**依赖任务**: #041
**执行级别**: heavy

### 任务描述
供应商余额查询;可选本地代理采集(显式开启、支持恢复直连、不转官方登录流量)。后续迭代。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml proxy`
- [ ] 代理采集可显式开关并恢复直连

### 相关文件
- `src-tauri/src/usage/proxy.rs`

---

## 任务 #049: 开发期兼容契约与后置 Windows 实测边界

**状态**: DONE
**优先级**: P1
**分配给**: Codex
**依赖任务**: #001, #007

### 任务描述
按用户最新安排分离开发契约与最终实测，保留原会话验收任务。参见 planning/specs/20261011-windows-validation.md。

### 验收标准
- [ ] `test -f planning/droid-development-contract.md`
- [ ] `test -f planning/verification/fixtures/probe/defaults-observed.json`
- [ ] 契约区分观察、文档依据和未验证项

---

## 任务 #050: 官方订阅与现成代理路线对照、Droid 连通性验证

**优先级**: P2
**状态**: OPEN
**分配给**: Codex
**预计工时**: 1–2 日
**依赖任务**: 无
**执行级别**: heavy

### 任务描述
确认应用分发接入条件，取得官方授权、文本流及一次工具结果往返证据；记录实测 Droid 版本和不兼容字段。失败提供替代范围，不伪造通过。
仅规划，未启动实现。进入开发后按本方案与验收依赖执行。

### 验收标准
- [ ] `test -s planning/progress-reports/router-feasibility.md`
- [ ] 必须人工核验真实账号与合成 Droid 会话证据；文件存在不是通过依据。
上述命令是后续实现验收入口，当前测试目标/报告尚未创建，未执行。

### 相关文件
planning/specs/20261011-chatgpt-local-router.md

### 源码调研与路线修订
2026-10-11 已静态审查 CLIProxyAPI、CC Switch、Sub2API 的固定提交，见 planning/specs/20261011-proxy-implementation-review.md。未运行代理或登录。优先验证外接 CLIProxyAPI，再选择受管进程集成；独立 SIWC 是不同授权路线。验收须包含真实 Droid 多轮工具闭环与中途失败；未通过前不确定复用工期。#051–#058 暂为自建路线参考，选定路线后先重写再实施，均保持 OPEN。

---

## 任务 #051: 官方 OAuth/OIDC 与账号注册

**优先级**: P2
**状态**: OPEN
**分配给**: Codex
**预计工时**: 2–3 日
**依赖任务**: #050
**执行级别**: heavy

### 任务描述
PKCE/state/nonce、JWT 验证、正式 client ID/host ID、账号隔离、权限与登录分离；未验证账号不得覆盖活动账号。
仅规划，未启动实现。进入开发后按本方案与验收依赖执行。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --test router_oauth`
- [ ] 覆盖回调重放、错误身份、无推理权限和用户取消。
上述命令是后续实现验收入口，当前测试目标/报告尚未创建，未执行。

### 相关文件
planning/specs/20261011-chatgpt-local-router.md

---

## 任务 #052: 系统凭据存储、令牌刷新与退出

**优先级**: P2
**状态**: OPEN
**分配给**: Codex
**预计工时**: 1–2 日
**依赖任务**: #051
**执行级别**: heavy

### 任务描述
Keychain/Credential Manager、注册级刷新锁、替换凭据、撤销失败处理；上游令牌不进入普通草稿。
仅规划，未启动实现。进入开发后按本方案与验收依赖执行。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --test router_credentials`
- [ ] 假存储回归后还需 macOS/Windows 真实存储、锁屏与刷新检查。
上述命令是后续实现验收入口，当前测试目标/报告尚未创建，未执行。

### 相关文件
planning/specs/20261011-chatgpt-local-router.md

---

## 任务 #053: 本机路由服务与生命周期

**优先级**: P2
**状态**: OPEN
**分配给**: Codex
**预计工时**: 1–2 日
**依赖任务**: #050
**执行级别**: heavy

### 任务描述
127.0.0.1、独立 Bearer、Host/Origin 校验、固定上游、端口持久化、限并发/背压/取消/关闭。
仅规划，未启动实现。进入开发后按本方案与验收依赖执行。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --test router_server`
- [ ] 错误密钥、恶意 Origin、端口冲突与中断不会留下请求或监听器。
上述命令是后续实现验收入口，当前测试目标/报告尚未创建，未执行。

### 相关文件
planning/specs/20261011-chatgpt-local-router.md

---

## 任务 #054: Responses 流式、多轮与工具协议适配

**优先级**: P2
**状态**: OPEN
**分配给**: Codex
**预计工时**: 3–5 日
**依赖任务**: #051, #052, #053
**执行级别**: heavy

### 任务描述
建立 Droid 字段兼容矩阵；解析 SSE 增量、工具参数及 call ID；历史绑定账号；不支持能力明确失败；流后失败不重放。
仅规划，未启动实现。进入开发后按本方案与验收依赖执行。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --test router_protocol`
- [ ] 先合成帧/多轮用例，再真实 Droid 工具调用闭环；纯文本成功不足以通过。
上述命令是后续实现验收入口，当前测试目标/报告尚未创建，未执行。

### 相关文件
planning/specs/20261011-chatgpt-local-router.md

---

## 任务 #055: 受管模型、供应商与配置接入

**优先级**: P2
**状态**: OPEN
**分配给**: Codex
**预计工时**: 1–2 日
**依赖任务**: #054
**执行级别**: heavy

### 任务描述
专用模型目录解析；本机供应商和来源标签；复用预览、备份与原子写；不把 ChatGPT 订阅标为 Factory 订阅。
仅规划，未启动实现。进入开发后按本方案与验收依赖执行。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --test router_config`
- [ ] 旧工作区迁移、恢复、默认模型、端口稳定、预览导出无上游令牌。
上述命令是后续实现验收入口，当前测试目标/报告尚未创建，未执行。

### 相关文件
planning/specs/20261011-chatgpt-local-router.md

---

## 任务 #056: 账号与本地路由页面

**优先级**: P2
**状态**: OPEN
**分配给**: Codex
**预计工时**: 1–2 日
**依赖任务**: #052, #053, #055
**执行级别**: heavy

### 任务描述
账号注册/选择、授权不足、启停、模型选择、添加到 Droid；说明软件需运行；官方登录样式按文档。
仅规划，未启动实现。进入开发后按本方案与验收依赖执行。

### 验收标准
- [ ] `npm exec vitest run src/views/accounts/router.test.tsx`
- [ ] 覆盖错误和恢复状态、取消授权不替换账号；npm run build 以及两种窗口宽度人工检查。
上述命令是后续实现验收入口，当前测试目标/报告尚未创建，未执行。

### 相关文件
planning/specs/20261011-chatgpt-local-router.md

---

## 任务 #057: 路由用量与脱敏诊断

**优先级**: P2
**状态**: OPEN
**分配给**: Codex
**预计工时**: 1–2 日
**依赖任务**: #054, #056
**执行级别**: heavy

### 任务描述
按请求记录 token/状态并去重；ChatGPT 订阅独立分类、不估算未知余额；不保存正文与工具参数。
仅规划，未启动实现。进入开发后按本方案与验收依赖执行。

### 验收标准
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml --test router_usage`
- [ ] 流中失败、重复完成事件、未知用量及脱敏检查；不依赖原生 Droid 日志适配。
上述命令是后续实现验收入口，当前测试目标/报告尚未创建，未执行。

### 相关文件
planning/specs/20261011-chatgpt-local-router.md

---

## 任务 #058: 路由跨平台验收与交付

**优先级**: P2
**状态**: OPEN
**分配给**: Codex
**预计工时**: 2–3 日
**依赖任务**: #055, #056, #057
**执行级别**: heavy

### 任务描述
macOS/Windows 安装、启动、登录、重启、续期、工具执行、取消、端口占用、撤销；更新用户指南与包。
仅规划，未启动实现。进入开发后按本方案与验收依赖执行。

### 验收标准
- [ ] `test -s planning/progress-reports/router-platform-validation.md`
- [ ] 报告须分别列 macOS/Windows 真实结果；开发时补充平台构建命令，不能用报告存在替代实测。
上述命令是后续实现验收入口，当前测试目标/报告尚未创建，未执行。

### 相关文件
planning/specs/20261011-chatgpt-local-router.md

---
