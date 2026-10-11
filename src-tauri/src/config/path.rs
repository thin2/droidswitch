use std::path::{Path, PathBuf};
pub fn default_for(home: &str, windows: bool) -> String {
    if windows {
        format!(
            "{}\\.factory\\settings.json",
            home.trim_end_matches(['\\', '/'])
        )
    } else {
        format!("{}/.factory/settings.json", home.trim_end_matches('/'))
    }
}
pub fn resolve(input: &str) -> Result<PathBuf, String> {
    let value = input.trim();
    if value.is_empty() {
        return Err("配置路径不能为空".into());
    }
    let path = if value == "~" || value.starts_with("~/") || value.starts_with("~\\") {
        let default = crate::workspace::default_target()?;
        let home = default
            .parent()
            .and_then(Path::parent)
            .ok_or("主目录无效")?;
        home.join(value.get(2..).unwrap_or(""))
    } else {
        PathBuf::from(value)
    };
    if !path.is_absolute() {
        return Err("请填写绝对路径".into());
    }
    Ok(path)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn path_platform_defaults() {
        assert_eq!(
            default_for("C:\\Users\\a", true),
            "C:\\Users\\a\\.factory\\settings.json"
        );
        assert_eq!(
            default_for("/Users/a/", false),
            "/Users/a/.factory/settings.json"
        );
    }
    #[test]
    fn path_requires_absolute_and_expands_home() {
        assert!(resolve("relative.json").is_err());
        assert!(resolve("~/.factory/settings.json").unwrap().is_absolute());
    }
}

#[tauri::command]
pub fn set_target_path(
    store: tauri::State<'_, crate::workspace::Store>,
    path: String,
    revision: u64,
) -> Result<crate::workspace::Workspace, String> {
    let resolved = resolve(&path)?;
    // Validate existing files now; a missing target is created only by Apply.
    match std::fs::symlink_metadata(&resolved) {
        Ok(m) if !m.is_file() => return Err("目标必须是普通文件，不能是目录或符号链接".into()),
        Ok(_) => {
            let snapshot = crate::fs::read_snapshot(&resolved).map_err(|_| "无法读取目标配置")?;
            let value: serde_json::Value =
                serde_json::from_str(&snapshot.text).map_err(|_| "目标配置不是有效 JSON")?;
            if !value.is_object() {
                return Err("配置顶层必须为对象".into());
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("无法访问目标配置".into()),
    }
    store.edit(revision, |w| {
        w.target_path = resolved.to_string_lossy().into();
        Ok(())
    })
}
