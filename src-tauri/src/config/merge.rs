use crate::{adapter, model::ids::InternalId, provider::Catalog};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Binding {
    pub model_id: InternalId,
    pub original: Value,
}
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub added: usize,
    pub modified: usize,
    pub disabled: usize,
    pub deleted: usize,
}
pub struct Merged {
    pub document: Value,
    pub bindings: Vec<Binding>,
    pub counts: Counts,
}
const FIELDS: &[&str] = &[
    "model",
    "displayName",
    "baseUrl",
    "apiKey",
    "provider",
    "maxOutputTokens",
    "noImageSupport",
    "extraArgs",
    "extraHeaders",
];
// Droid can add bookkeeping fields when it saves a custom model. Only accept
// newly added, well-typed metadata; changes to existing fields remain conflicts.
fn matches_original(current: &Value, original: &Value) -> bool {
    if current == original {
        return true;
    }
    let (Some(now), Some(before)) = (current.as_object(), original.as_object()) else {
        return false;
    };
    let mut normalized = now.clone();
    for key in ["id", "index"] {
        if !before.contains_key(key) {
            if let Some(value) = normalized.get(key) {
                let valid = if key == "id" {
                    value.as_str().is_some_and(|s| !s.is_empty())
                } else {
                    value.as_u64().is_some()
                };
                if !valid {
                    return false;
                }
            }
            normalized.remove(key);
        }
    }
    &Value::Object(normalized) == original
}
pub fn refresh_metadata_bindings(disk: &Value, bindings: &mut [Binding]) {
    let Some(items) = disk.get("customModels").and_then(Value::as_array) else {
        return;
    };
    for binding in bindings {
        let matches: Vec<_> = items
            .iter()
            .filter(|v| matches_original(v, &binding.original))
            .collect();
        if matches.len() == 1 {
            binding.original = matches[0].clone();
        }
    }
}
pub fn merge(
    disk: &Value,
    owned: &[Binding],
    catalog: &Catalog,
    version: &str,
) -> Result<Merged, String> {
    adapter::versions::require_supported(version)?;
    catalog.validate()?;
    let mut document = disk.clone();
    if !document.is_object() {
        return Err("Droid 配置顶层必须为 JSON 对象".into());
    }
    let current = match document.get("customModels") {
        None => vec![],
        Some(Value::Array(a)) => a.clone(),
        _ => return Err("customModels 必须为数组".into()),
    };
    let mut output = current.clone();
    let mut bindings = Vec::new();
    let mut counts = Counts::default();
    let mut consumed = std::collections::HashSet::new();
    for binding in owned {
        let matches: Vec<usize> = current
            .iter()
            .enumerate()
            .filter(|(_, v)| matches_original(v, &binding.original))
            .map(|(i, _)| i)
            .collect();
        if matches.len() != 1 || !consumed.insert(matches[0]) {
            return Err("已管理条目发生变化或出现歧义，请重新载入并确认接管关系".into());
        }
        let index = matches[0];
        let model = catalog.models.iter().find(|m| m.id == binding.model_id);
        if let Some(model) = model.filter(|m| m.enabled) {
            let mapped =
                adapter::custom_model(model, catalog.provider(model.provider_id)?, version)?;
            let mut merged = current[index].clone();
            let object = merged.as_object_mut().ok_or("管理条目不是对象")?;
            for key in FIELDS {
                object.remove(*key);
            }
            object.extend(mapped.as_object().unwrap().clone());
            if merged != current[index] {
                counts.modified += 1;
            }
            output[index] = merged.clone();
            bindings.push(Binding {
                model_id: model.id,
                original: merged,
            });
        } else {
            output[index] = Value::Null;
            if model.is_some() {
                counts.disabled += 1;
            } else {
                counts.deleted += 1;
            }
        }
    }
    // Remove only our tombstones, never an unmanaged null or unknown record.
    let mut output: Vec<Value> = output
        .into_iter()
        .enumerate()
        .filter(|(i, v)| !(consumed.contains(i) && v.is_null()))
        .map(|(_, v)| v)
        .collect();
    for model in catalog
        .models
        .iter()
        .filter(|m| m.enabled && !owned.iter().any(|b| b.model_id == m.id))
    {
        let mapped = adapter::custom_model(model, catalog.provider(model.provider_id)?, version)?;
        if current.iter().any(|v| v == &mapped) {
            return Err("存在相同的外部模型，请先明确接管，避免重复添加".into());
        }
        output.push(mapped.clone());
        bindings.push(Binding {
            model_id: model.id,
            original: mapped,
        });
        counts.added += 1;
    }
    if !output.is_empty() || document.get("customModels").is_some() {
        document["customModels"] = json!(output);
    }
    Ok(Merged {
        document,
        bindings,
        counts,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::{entry::ModelEntry, ids::ApiModelId},
        provider::tests::sample,
    };
    fn catalog() -> Catalog {
        let p = sample();
        let m = ModelEntry::new(
            p.id,
            ApiModelId::try_from("id".to_owned()).unwrap(),
            "Name".into(),
        );
        Catalog {
            providers: vec![p],
            models: vec![m],
        }
    }
    #[test]
    fn merge_preserves_unmanaged_unknown_fields_and_idempotence() {
        let c = catalog();
        let disk = json!({"customModels":[null,{"vendor":"unknown","apiKey":"private"}],"theme":"dark","other":{"x":1}});
        let first = merge(&disk, &[], &c, "0.238.0").unwrap();
        let second = merge(&first.document, &first.bindings, &c, "0.238.0").unwrap();
        assert_eq!(first.document, second.document);
        assert_eq!(second.counts.added, 0);
        assert_eq!(second.document["customModels"][0], Value::Null);
        assert_eq!(second.document["other"], disk["other"]);
    }
    #[test]
    fn merge_disabled_removes_only_managed_and_keeps_draft() {
        let mut c = catalog();
        let first = merge(&json!({"x":1}), &[], &c, "0.238.0").unwrap();
        c.models[0].enabled = false;
        let next = merge(&first.document, &first.bindings, &c, "0.238.0").unwrap();
        assert_eq!(next.counts.disabled, 1);
        assert_eq!(next.document["customModels"], json!([]));
        assert_eq!(c.models.len(), 1);
    }
    #[test]
    fn merge_rejects_changed_owned_record() {
        let c = catalog();
        let first = merge(&json!({}), &[], &c, "0.238.0").unwrap();
        let mut changed = first.document;
        changed["customModels"][0]["model"] = json!("external-change");
        assert!(merge(&changed, &first.bindings, &c, "0.238.0").is_err());
    }
    #[test]
    fn merge_managed_unknown_fields_survive() {
        let c = catalog();
        let mut first = merge(&json!({}), &[], &c, "0.238.0").unwrap();
        first.document["customModels"][0]["futureField"] = json!(true);
        first.bindings[0].original = first.document["customModels"][0].clone();
        let next = merge(&first.document, &first.bindings, &c, "0.238.0").unwrap();
        assert_eq!(next.document["customModels"][0]["futureField"], true);
    }
    #[test]
    fn merge_accepts_added_droid_metadata_and_preserves_it() {
        let c = catalog();
        let first = merge(&json!({}), &[], &c, "0.238.0").unwrap();
        let mut disk = first.document.clone();
        disk["customModels"][0]["id"] = json!("custom:model-0");
        disk["customModels"][0]["index"] = json!(0);
        let result = merge(&disk, &first.bindings, &c, "0.238.0").unwrap();
        assert_eq!(result.document, disk);
        assert_eq!(result.counts.modified, 0);
        let mut bindings = first.bindings.clone();
        refresh_metadata_bindings(&disk, &mut bindings);
        assert_eq!(bindings[0].original, disk["customModels"][0]);
    }
    #[test]
    fn metadata_reconciliation_rejects_real_changes_and_duplicates() {
        let c = catalog();
        let first = merge(&json!({}), &[], &c, "0.238.0").unwrap();
        for (field, value) in [
            ("apiKey", json!("different")),
            ("baseUrl", json!("https://different.example")),
            ("extra", json!(true)),
            ("index", json!("invalid")),
        ] {
            let mut disk = first.document.clone();
            disk["customModels"][0]["id"] = json!("custom:model-0");
            disk["customModels"][0][field] = value;
            assert!(merge(&disk, &first.bindings, &c, "0.238.0").is_err());
            let mut bindings = first.bindings.clone();
            refresh_metadata_bindings(&disk, &mut bindings);
            assert!(bindings[0].original == first.bindings[0].original);
        }
        let original = first.document["customModels"][0].clone();
        let disk = json!({"customModels":[original.clone(), original]});
        assert!(merge(&disk, &first.bindings, &c, "0.238.0").is_err());
        let mut with_id = first.document["customModels"][0].clone();
        with_id["id"] = json!("old-id");
        let mut changed = with_id.clone();
        changed["id"] = json!("new-id");
        assert!(!matches_original(&changed, &with_id));
    }
}
