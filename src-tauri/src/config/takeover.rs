use super::merge::Binding;
use crate::{
    model::{
        entry::ModelEntry,
        ids::{ApiModelId, InternalId},
    },
    provider::{Credential, CredentialMode, ProviderConnection},
    workspace::{Store, Workspace},
};
use serde_json::Value;
fn required(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("外部条目缺少支持的 {key} 字段，无法接管"))
}
pub(super) fn parse_item(
    item: &Value,
    allow_plaintext: bool,
) -> Result<(ProviderConnection, ModelEntry), String> {
    let key = required(item, "apiKey")?;
    let credential = if let Some(name) = key.strip_prefix("${").and_then(|v| v.strip_suffix('}')) {
        Credential {
            mode: CredentialMode::Environment,
            value: name.into(),
        }
    } else {
        if !allow_plaintext {
            return Err("此条目含明文凭据，请确认保存到私有管理数据库及备份".into());
        }
        Credential {
            mode: CredentialMode::Direct,
            value: key,
        }
    };
    let api = required(item, "model")?;
    let name = item
        .get("displayName")
        .and_then(Value::as_str)
        .unwrap_or(&api)
        .to_owned();
    let p = ProviderConnection {
        id: InternalId::new(),
        name: format!("导入 · {name}"),
        base_url: required(item, "baseUrl")?,
        protocol: serde_json::from_value(
            item.get("provider").cloned().ok_or("缺少 provider 协议")?,
        )
        .map_err(|_| "不支持此外部协议，保持只读")?,
        credential,
        model_list_url: None,
        extra_headers: match item.get("extraHeaders") {
            None => Default::default(),
            Some(v) => serde_json::from_value(v.clone()).map_err(|_| "extraHeaders 格式不支持")?,
        },
    };
    p.validate()?;
    let mut m = ModelEntry::new(
        p.id,
        ApiModelId::try_from(api).map_err(|_| "API 模型 ID 无效")?,
        name,
    );
    m.max_output_tokens = match item.get("maxOutputTokens") {
        None => None,
        Some(v) => {
            Some(serde_json::from_value(v.clone()).map_err(|_| "maxOutputTokens 必须为正整数")?)
        }
    };
    m.image_input = match item.get("noImageSupport") {
        None => true,
        Some(v) => !v.as_bool().ok_or("noImageSupport 必须为布尔值")?,
    };
    m.extra_args = match item.get("extraArgs") {
        None => Default::default(),
        Some(v) => v.as_object().cloned().ok_or("extraArgs 必须为对象")?,
    };
    Ok((p, m))
}

pub fn adopt(w: &mut Workspace, index: usize, allow_plaintext: bool) -> Result<(), String> {
    adopt_linked(w, index, allow_plaintext, None, false)
}

pub fn adopt_linked(
    w: &mut Workspace,
    index: usize,
    allow_plaintext: bool,
    existing_model_id: Option<InternalId>,
    create_separate: bool,
) -> Result<(), String> {
    let item = super::reconcile::available_item(w, index, None)?;
    let candidates = super::reconcile::association_candidates(w, &item);
    if let Some(id) = existing_model_id {
        if !candidates.contains(&id) || w.bindings.iter().any(|b| b.model_id == id) {
            return Err("只能关联同接口、同协议、同 API 模型且尚未接管的记录".into());
        }
    } else if !candidates.is_empty() && !create_separate {
        return Err("已有相同接口的模型，请明确关联已有记录；已接管的记录请先处理冲突".into());
    }
    let (provider, mut model) = parse_item(&item, allow_plaintext)?;
    if let Some(id) = existing_model_id {
        model.id = id;
        model.manufacturer = w
            .catalog
            .models
            .iter()
            .find(|m| m.id == id)
            .and_then(|m| m.manufacturer.clone());
    }
    super::reconcile::accept_file_model(w, provider, model.clone(), true)?;
    w.bindings.push(Binding {
        model_id: model.id,
        original: item,
    });
    Ok(())
}

#[tauri::command]
pub fn takeover_model(
    store: tauri::State<'_, Store>,
    index: usize,
    allow_plaintext: bool,
    existing_model_id: Option<InternalId>,
    create_separate: Option<bool>,
    revision: u64,
) -> Result<Workspace, String> {
    store.edit(revision, |w| {
        adopt_linked(
            w,
            index,
            allow_plaintext,
            existing_model_id,
            create_separate.unwrap_or(false),
        )
    })
}
#[tauri::command]
pub fn release_binding(
    store: tauri::State<'_, Store>,
    id: InternalId,
    revision: u64,
) -> Result<Workspace, String> {
    store.edit(revision, |w| {
        w.bindings.retain(|b| b.model_id != id);
        w.catalog.models.retain(|m| m.id != id);
        w.applied_catalog.models.retain(|m| m.id != id);
        Ok(())
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::pipeline::read;
    use serde_json::json;
    #[test]
    fn takeover_requires_plaintext_consent_and_preserves_unknown() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(d.path()).unwrap();
        let item = json!({"model":"external","displayName":"External","provider":"openai","baseUrl":"https://example.com/v1","apiKey":"private-key","futureField":12});
        let path = d.path().join("settings.json");
        std::fs::write(&path, json!({"customModels":[item]}).to_string()).unwrap();
        s.edit(0, |w| {
            w.target_path = path.to_string_lossy().into();
            w.baseline = Some(read(&path)?);
            assert!(adopt(w, 0, false).is_err());
            adopt(w, 0, true)?;
            assert!(adopt(w, 0, true).is_err());
            let merged = crate::config::merge::merge(
                &w.baseline.as_ref().unwrap().document,
                &w.bindings,
                &w.catalog,
                "0.238.0",
            )?;
            assert_eq!(merged.document["customModels"][0]["futureField"], 12);
            Ok(())
        })
        .unwrap();
        assert_eq!(read(&path).unwrap().document["customModels"][0], item);
    }
}
