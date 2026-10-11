pub const DEVELOPMENT_VERSION: &str = "0.238.0";
pub fn require_supported(version: &str) -> Result<(), String> {
    if version.trim() == DEVELOPMENT_VERSION {
        Ok(())
    } else {
        Err("当前 Droid 版本未建立兼容契约，已保留草稿并阻止配置写入".into())
    }
}
