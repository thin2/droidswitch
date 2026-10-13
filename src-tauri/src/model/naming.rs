use crate::{
    config::merge::matches_original,
    model::ids::InternalId,
    workspace::{Store, Workspace},
};
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub model_id: InternalId,
    pub provider_name: String,
    pub before: String,
    pub after: String,
}
fn display_name(item: &Value) -> Option<&str> {
    item.get("displayName")
        .and_then(Value::as_str)
        .or_else(|| item.get("model").and_then(Value::as_str))
        .map(str::trim)
}
pub fn suggestions(w: &Workspace) -> Result<Vec<Suggestion>, String> {
    let mut counts = HashMap::<String, usize>::new();
    let mut sources = HashMap::<String, HashSet<InternalId>>::new();
    for model in w.catalog.models.iter().filter(|m| m.enabled) {
        *counts.entry(model.display_name.trim().into()).or_default() += 1;
        sources
            .entry(model.api_model_id.as_str().into())
            .or_default()
            .insert(model.provider_id);
    }
    if let Some(items) = w
        .baseline
        .as_ref()
        .and_then(|b| b.document.get("customModels"))
        .and_then(Value::as_array)
    {
        for item in items.iter().filter(|item| {
            !w.bindings
                .iter()
                .any(|b| matches_original(item, &b.original))
        }) {
            if let Some(name) = display_name(item) {
                *counts.entry(name.into()).or_default() += 1;
            }
        }
    }
    let mut occupied: HashSet<String> = counts.keys().cloned().collect();
    let mut result = vec![];
    for model in w.catalog.models.iter().filter(|m| {
        m.enabled
            && (counts.get(m.display_name.trim()).is_some_and(|n| *n > 1)
                || (m.display_name.trim() == m.api_model_id.as_str()
                    && sources
                        .get(m.api_model_id.as_str())
                        .is_some_and(|p| p.len() > 1)))
    }) {
        let provider = w.catalog.provider(model.provider_id)?;
        let base = format!("{} ({})", model.display_name.trim(), provider.name.trim());
        let mut candidate = base.clone();
        let mut suffix = 2;
        while occupied.contains(&candidate) {
            candidate = format!(
                "{} ({} · {})",
                model.display_name.trim(),
                provider.name.trim(),
                suffix
            );
            suffix += 1;
        }
        occupied.insert(candidate.clone());
        result.push(Suggestion {
            model_id: model.id,
            provider_name: provider.name.clone(),
            before: model.display_name.clone(),
            after: candidate,
        });
    }
    Ok(result)
}
// Check the actual merged output, including external entries, even without a default scheme.
pub fn validate_names(document: &Value) -> Result<(), String> {
    let mut seen = HashSet::new();
    if let Some(items) = document.get("customModels").and_then(Value::as_array) {
        for item in items {
            if let Some(name) = display_name(item) {
                if !seen.insert(name) {
                    return Err("模型显示名重复，无法在 Droid CLI 中明确区分。请在模型库查看命名建议，或编辑显示名称；未接管条目请先接管或在文件中整理。API 模型 ID 无需修改。".into());
                }
            }
        }
    }
    Ok(())
}
pub fn apply_suggestions(w: &mut Workspace) -> Result<(), String> {
    for suggestion in suggestions(w)? {
        let model = w
            .catalog
            .models
            .iter_mut()
            .find(|m| m.id == suggestion.model_id)
            .ok_or("模型已变化，请刷新后重试")?;
        model.display_name = suggestion.after;
        model.droid_selector_id = None;
    }
    Ok(())
}
#[tauri::command]
pub fn suggest_model_names(
    store: tauri::State<'_, Store>,
    revision: u64,
) -> Result<Vec<Suggestion>, String> {
    let w = store.load()?;
    if w.revision != revision {
        return Err("草稿已变化，请刷新后重试".into());
    }
    suggestions(&w)
}
#[tauri::command]
pub fn apply_model_names(
    store: tauri::State<'_, Store>,
    revision: u64,
) -> Result<Workspace, String> {
    store.edit(revision, apply_suggestions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{
            merge::{merge, Binding},
            pipeline::Baseline,
        },
        model::{entry::ModelEntry, ids::ApiModelId},
    };
    use serde_json::json;
    fn setup() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        store
            .edit(0, |w| {
                for name in ["QA", "zzshu"] {
                    let mut p = crate::provider::tests::sample();
                    p.name = name.into();
                    w.catalog.models.push(ModelEntry::new(
                        p.id,
                        ApiModelId::try_from("shared".to_string()).unwrap(),
                        "shared".into(),
                    ));
                    w.catalog.providers.push(p);
                }
                w.scheme.api.model_id = Some(w.catalog.models[0].id);
                w.applied_catalog = w.catalog.clone();
                Ok(())
            })
            .unwrap();
        (dir, store)
    }
    #[test]
    fn suggestions_preserve_identity_api_ids_defaults_and_are_idempotent() {
        let (_dir, store) = setup();
        let before = store.load().unwrap();
        let changes = suggestions(&before).unwrap();
        assert_eq!(changes[0].after, "shared (QA)");
        assert_eq!(changes[1].after, "shared (zzshu)");
        let w = store.edit(before.revision, apply_suggestions).unwrap();
        for (a, b) in before.catalog.models.iter().zip(&w.catalog.models) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.api_model_id, b.api_model_id);
            assert_eq!(a.provider_id, b.provider_id);
        }
        assert_eq!(w.scheme.api.model_id, before.scheme.api.model_id);
        assert!(w.applied_catalog == before.applied_catalog);
        assert!(suggestions(&w).unwrap().is_empty());
        assert!(store.edit(before.revision, apply_suggestions).is_err());
        let output = merge(&json!({}), &[], &w.catalog, "0.238.0").unwrap();
        validate_names(&output.document).unwrap();
        assert_eq!(output.document["customModels"][0]["model"], "shared");
    }
    #[test]
    fn collisions_with_same_provider_names_and_external_entries_get_unique_suffixes() {
        let (_dir, store) = setup();
        let mut w = store.load().unwrap();
        w.catalog.providers[1].name = "QA".into();
        w.baseline = Some(Baseline {
            fingerprint: None,
            document: json!({"customModels":[{"model":"other","displayName":"shared (QA)"},{"model":"third","displayName":"shared (QA · 2)"}]}),
        });
        let changes = suggestions(&w).unwrap();
        assert_eq!(changes[0].after, "shared (QA · 3)");
        assert_eq!(changes[1].after, "shared (QA · 4)");
        apply_suggestions(&mut w).unwrap();
        assert!(suggestions(&w).unwrap().is_empty());
    }
    #[test]
    fn disabled_models_and_unique_custom_names_are_untouched() {
        let (_dir, store) = setup();
        let mut w = store.load().unwrap();
        w.catalog.models[1].enabled = false;
        assert!(suggestions(&w).unwrap().is_empty());
        w.catalog.models[1].enabled = true;
        w.catalog.models[0].display_name = "My favorite".into();
        let proposed = suggestions(&w).unwrap();
        assert_eq!(proposed.len(), 1);
        assert_eq!(proposed[0].after, "shared (zzshu)");
        w.catalog.models[1].display_name = "Other favorite".into();
        assert!(suggestions(&w).unwrap().is_empty());
    }
    #[test]
    fn managed_snapshots_are_not_double_counted_but_external_collisions_are() {
        let (_dir, store) = setup();
        let mut w = store.load().unwrap();
        w.catalog.models[1].display_name = "Unique".into();
        w.catalog.models[1].api_model_id = ApiModelId::try_from("other".to_owned()).unwrap();
        let item = json!({"model":"shared","displayName":"shared"});
        w.bindings.push(Binding {
            model_id: w.catalog.models[0].id,
            original: item.clone(),
        });
        w.baseline = Some(Baseline {
            fingerprint: None,
            document: json!({"customModels":[item]}),
        });
        assert!(suggestions(&w).unwrap().is_empty());
        w.bindings.clear();
        assert_eq!(suggestions(&w).unwrap().len(), 1);
        assert!(validate_names(
            &json!({"customModels":[{"model":"shared"},{"model":"other","displayName":" shared "}]})
        )
        .is_err());
        assert!(validate_names(&json!({"customModels":[{"model":"same","displayName":"A"},{"model":"same","displayName":"B"}]})).is_ok());
    }
}
