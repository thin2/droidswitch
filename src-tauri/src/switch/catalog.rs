use serde::Serialize;
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservedModel {
    pub id: String,
    pub name: String,
    pub source: String,
    pub reasoning: Vec<String>,
    pub default_reasoning: Option<String>,
}
pub fn parse(text: &str) -> Result<Vec<ObservedModel>, String> {
    let mut section = "";
    let mut models: Vec<ObservedModel> = vec![];
    for line in text.lines() {
        let line = line.trim();
        match line {
            "Available Models:" => {
                section = "official";
                continue;
            }
            "Custom Models:" => {
                section = "api";
                continue;
            }
            "Model details:" => {
                section = "details";
                continue;
            }
            _ => {}
        }
        if line.is_empty() {
            continue;
        }
        if line.ends_with(':') && !line.starts_with('-') {
            section = "";
            continue;
        }
        if section == "official" || section == "api" {
            if let Some((id, name)) = line.split_once(char::is_whitespace) {
                if id.starts_with('-') {
                    continue;
                }
                let name = name
                    .trim()
                    .strip_suffix(" (default)")
                    .unwrap_or(name.trim());
                models.push(ObservedModel {
                    id: id.into(),
                    name: name.into(),
                    source: section.into(),
                    reasoning: vec![],
                    default_reasoning: None,
                });
            }
        } else if section == "details" {
            if let Some(detail) = line.strip_prefix("- ") {
                if let Some((name, rest)) = detail.split_once(": supports reasoning:") {
                    let supported = rest
                        .split("supported: [")
                        .nth(1)
                        .and_then(|s| s.split(']').next());
                    let default = rest.split("default: ").nth(1).map(str::trim);
                    for m in models.iter_mut().filter(|m| m.name == name) {
                        m.reasoning = supported
                            .map(|s| {
                                s.split(',')
                                    .map(|v| v.trim().to_owned())
                                    .filter(|s| !s.is_empty())
                                    .collect()
                            })
                            .unwrap_or_default();
                        m.default_reasoning = default.map(str::to_owned);
                    }
                }
            }
        }
    }
    if models.iter().all(|m| m.source != "official") {
        return Err("无法识别 Droid 模型目录，保持草稿且不写入默认模型".into());
    }
    Ok(models)
}
pub fn discover(document: &serde_json::Value) -> Result<Vec<ObservedModel>, String> {
    crate::config::cli::version()?;
    let file = tempfile::NamedTempFile::new().map_err(|_| "无法创建模型检测配置")?;
    crate::permissions::restrict_file(file.path()).map_err(|_| "无法保护模型检测配置")?;
    std::fs::write(
        file.path(),
        serde_json::to_vec(document).map_err(|_| "检测配置无效")?,
    )
    .map_err(|_| "无法写入临时检测配置")?;
    let path = file.path().to_str().ok_or("检测路径编码不支持")?;
    parse(&crate::config::cli::run(&[
        "exec",
        "--settings",
        path,
        "--help",
    ])?)
}
#[tauri::command]
pub async fn discover_models(
    store: tauri::State<'_, crate::workspace::Store>,
) -> Result<Vec<ObservedModel>, String> {
    let w = store.load()?;
    tauri::async_runtime::spawn_blocking(move || {
        let version = crate::config::cli::version()?;
        let disk = crate::config::pipeline::read(std::path::Path::new(&w.target_path))?;
        let merged =
            crate::config::merge::merge(&disk.document, &w.bindings, &w.catalog, &version)?;
        discover(&merged.document)
    })
    .await
    .map_err(|_| "目录检测失败")?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires planning/verification/fixtures/probe/help.txt"]
    fn switch_catalog_uses_observed_selectors_and_capabilities() {
        let text = include_str!("../../../planning/verification/fixtures/probe/help.txt");
        let m = parse(text).unwrap();
        let custom = m.iter().find(|m| m.name == "Probe B").unwrap();
        assert_eq!(custom.id, "custom:Probe-B-0");
        assert_eq!(custom.source, "api");
        let official = m.iter().find(|m| m.id == "gpt-6-sol").unwrap();
        assert!(official.reasoning.contains(&"high".into()));
        assert!(parse("unknown output").is_err());
    }
}
#[tauri::command]
pub fn open_factory_login() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let command = "open";
    #[cfg(target_os = "windows")]
    let command = "explorer";
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let command = "xdg-open";
    std::process::Command::new(command)
        .arg("https://app.factory.ai")
        .spawn()
        .map(|_| ())
        .map_err(|_| "无法打开浏览器，请访问 https://app.factory.ai 并在 Droid 中完成登录".into())
}
