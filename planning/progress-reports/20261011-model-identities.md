# 三类 ID 与模型记录 · 2026-10-11

#013 实现：InternalId 使用随机 UUID v4；ApiModelId 与 DroidSelectorId 分别为私有字段 newtype，API 值逐字保留，空白 ID 拒绝；没有从 API ID 或显示名推导选择器 ID 的转换。

ModelEntry 使用内部 provider_id 关联连接；显示名可变但不改变身份；选择器默认 None。最大输出 Token 为 Option<NonZeroU32>；协议显式继承或覆盖，不由模型名推断。管理模型明确不能直接序列化成 Droid customModels。

验收：完整 cargo test --locked 通过 20 项单元测试、3 项 compile_fail 文档测试；新增 4 项模型测试验证跨供应商同模型、改名身份不变、ID 原值往返、反序列化校验、正数 Token 与协议选择。cargo clippy --all-targets -- -D warnings 通过。

自审范围：类型隔离、序列化校验与默认值。Droid 真实选择器 ID 规则仍需阶段 0 实测；本任务没有猜测规则，也没有写入用户 settings.json。

正式验收 verify run 013 的两条命令通过：model 筛选运行 5 项单测（含一项 DB 模型约束测试），--doc model 运行 3 项编译失败测试。已将文档测试明确加入任务验收，确保集成时也验证类型不可混用。
