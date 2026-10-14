pub const MIN_SUPPORTED_VERSION: &str = "0.238.0";

pub fn require_supported(version: &str) -> Result<(), String> {
    let version = version.trim();

    // Parse version numbers
    let parse_version = |v: &str| -> Option<(u32, u32, u32)> {
        let parts: Vec<&str> = v.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        Some((
            parts[0].parse().ok()?,
            parts[1].parse().ok()?,
            parts[2].parse().ok()?,
        ))
    };

    let current = parse_version(version).ok_or("无法解析 Droid 版本号".to_string())?;
    let minimum = parse_version(MIN_SUPPORTED_VERSION).ok_or("内部版本号格式错误".to_string())?;

    // Compare versions (major.minor.patch)
    if current >= minimum {
        Ok(())
    } else {
        Err(format!(
            "当前 Droid 版本 {} 低于最低支持版本 {}，已保留草稿并阻止配置写入",
            version, MIN_SUPPORTED_VERSION
        ))
    }
}
