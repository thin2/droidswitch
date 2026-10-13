use super::{
    merge::{matches_original, Binding},
    pipeline::read,
    takeover::parse_item,
};
use crate::{
    model::{entry::ModelEntry, ids::InternalId},
    provider::{Catalog, ProviderConnection},
    workspace::{Store, Workspace},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeSet, path::Path};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingStatus {
    model_id: InternalId,
    status: &'static str,
    matches: Vec<usize>,
    candidates: Vec<Candidate>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    index: usize,
    fields: Vec<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalEntry {
    index: usize,
    existing_model_ids: Vec<InternalId>,
    selectable: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnosis {
    revision: u64,
    bindings: Vec<BindingStatus>,
    external: Vec<ExternalEntry>,
}
fn items(w: &Workspace) -> Result<&Vec<Value>, String> {
    w.baseline
        .as_ref()
        .ok_or("请先载入配置")?
        .document
        .get("customModels")
        .and_then(Value::as_array)
        .ok_or("配置中没有 customModels 数组".into())
}
fn matching(items: &[Value], binding: &Binding) -> Vec<usize> {
    items
        .iter()
        .enumerate()
        .filter(|(_, item)| matches_original(item, &binding.original))
        .map(|(i, _)| i)
        .collect()
}
fn related(item: &Value, original: &Value) -> bool {
    ["id", "model"].iter().any(|key| {
        original
            .get(key)
            .and_then(Value::as_str)
            .is_some_and(|v| item.get(key).and_then(Value::as_str) == Some(v))
    })
}
// Never return values, header names, or arbitrary unknown property names.
fn changed_fields(item: &Value, original: &Value) -> Vec<String> {
    let known = [
        "id",
        "index",
        "model",
        "displayName",
        "baseUrl",
        "provider",
        "apiKey",
        "noImageSupport",
        "maxOutputTokens",
        "extraArgs",
        "extraHeaders",
    ];
    let mut fields = BTreeSet::new();
    for key in item
        .as_object()
        .into_iter()
        .flat_map(|o| o.keys())
        .chain(original.as_object().into_iter().flat_map(|o| o.keys()))
    {
        if item.get(key) != original.get(key) {
            fields.insert(if known.contains(&key.as_str()) {
                key.clone()
            } else {
                "其他字段".into()
            });
        }
    }
    fields.into_iter().collect()
}
pub(super) fn association_candidates(w: &Workspace, item: &Value) -> Vec<InternalId> {
    w.catalog
        .models
        .iter()
        .filter(|m| {
            w.catalog.provider(m.provider_id).is_ok_and(|p| {
                item.get("model").and_then(Value::as_str) == Some(m.api_model_id.as_str())
                    && item.get("baseUrl").and_then(Value::as_str) == Some(p.base_url.as_str())
                    && item.get("provider")
                        == serde_json::to_value(m.effective_protocol(p.protocol))
                            .ok()
                            .as_ref()
            })
        })
        .map(|m| m.id)
        .collect()
}
pub fn diagnose(w: &Workspace) -> Result<Diagnosis, String> {
    // Empty config is a valid state, including first launch.
    let empty = vec![];
    let document = &w.baseline.as_ref().ok_or("请先载入配置")?.document;
    let items = match document.get("customModels") {
        None => &empty,
        Some(v) => v.as_array().ok_or("customModels 必须为数组")?,
    };
    let bindings = w
        .bindings
        .iter()
        .map(|b| {
            let matches = matching(items, b);
            let shared = matches.iter().any(|i| {
                w.bindings.iter().any(|other| {
                    other.model_id != b.model_id && matches_original(&items[*i], &other.original)
                })
            });
            let status = if matches.len() == 1 && !shared {
                "matched"
            } else if matches.len() > 1 || shared {
                "ambiguous"
            } else {
                "changed_or_missing"
            };
            let candidates = if status == "matched" {
                vec![]
            } else {
                items
                    .iter()
                    .enumerate()
                    .filter(|(_, item)| related(item, &b.original))
                    .map(|(index, item)| Candidate {
                        index,
                        fields: changed_fields(item, &b.original),
                    })
                    .collect()
            };
            BindingStatus {
                model_id: b.model_id,
                status,
                matches,
                candidates,
            }
        })
        .collect();
    let external = items
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            !w.bindings
                .iter()
                .any(|b| matches_original(item, &b.original))
        })
        .map(|(index, item)| ExternalEntry {
            index,
            existing_model_ids: association_candidates(w, item),
            selectable: item.is_object()
                && items.iter().filter(|other| *other == item).count() == 1,
        })
        .collect();
    Ok(Diagnosis {
        revision: w.revision,
        bindings,
        external,
    })
}
#[tauri::command]
pub fn inspect_configuration(
    store: tauri::State<'_, Store>,
    revision: u64,
) -> Result<Diagnosis, String> {
    let w = store.load()?;
    if w.revision != revision {
        return Err("草稿已变化，请刷新后重试".into());
    }
    diagnose(&w)
}
pub(super) fn available_item(
    w: &Workspace,
    index: usize,
    owner: Option<InternalId>,
) -> Result<Value, String> {
    let baseline = w.baseline.as_ref().ok_or("请先载入配置")?;
    if read(Path::new(&w.target_path))?.fingerprint != baseline.fingerprint {
        return Err("外部配置已变化，请重新载入".into());
    }
    let items = items(w)?;
    let item = items
        .get(index)
        .filter(|v| v.is_object())
        .ok_or("请选择有效的磁盘模型条目")?;
    if items.iter().filter(|v| *v == item).count() != 1 {
        return Err("磁盘存在完全相同的重复条目，请先消除歧义并重新载入".into());
    }
    if w.bindings
        .iter()
        .any(|b| Some(b.model_id) != owner && matches_original(item, &b.original))
    {
        return Err("此磁盘条目已由其他模型接管".into());
    }
    Ok(item.clone())
}
fn same_connection(a: &ProviderConnection, b: &ProviderConnection) -> bool {
    a.base_url == b.base_url
        && a.protocol == b.protocol
        && a.credential == b.credential
        && a.extra_headers == b.extra_headers
}
fn put_model(catalog: &mut Catalog, provider: &ProviderConnection, model: &ModelEntry) {
    if !catalog.providers.iter().any(|p| p.id == provider.id) {
        catalog.providers.push(provider.clone());
    }
    if let Some(old) = catalog.models.iter_mut().find(|m| m.id == model.id) {
        *old = model.clone();
    } else {
        catalog.models.push(model.clone());
    }
}
pub(super) fn accept_file_model(
    w: &mut Workspace,
    mut provider: ProviderConnection,
    mut model: ModelEntry,
    use_file: bool,
) -> Result<(), String> {
    // Reuse only an identical connection. A changed key must never update siblings.
    // Also require compatibility with the applied catalog so discard remains safe.
    if let Some(existing) = w
        .catalog
        .providers
        .iter()
        .chain(w.applied_catalog.providers.iter())
        .find(|p| {
            same_connection(p, &provider)
                && [&w.catalog, &w.applied_catalog].iter().all(|catalog| {
                    !catalog.models.iter().any(|m| {
                        m.id != model.id
                            && m.provider_id == p.id
                            && m.api_model_id == model.api_model_id
                    })
                })
                && w.catalog
                    .providers
                    .iter()
                    .chain(w.applied_catalog.providers.iter())
                    .filter(|other| other.id == p.id)
                    .all(|other| same_connection(other, &provider))
        })
    {
        provider = existing.clone();
    }
    model.provider_id = provider.id;
    for catalog in [&w.applied_catalog, &w.catalog] {
        if catalog.models.iter().any(|m| {
            m.id != model.id && m.provider_id == provider.id && m.api_model_id == model.api_model_id
        }) {
            return Err("关联后将产生重复模型，请先整理已有模型记录".into());
        }
    }
    put_model(&mut w.applied_catalog, &provider, &model);
    if use_file {
        put_model(&mut w.catalog, &provider, &model);
    }
    Ok(())
}
#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    File,
    App,
}
pub fn resolve(
    w: &mut Workspace,
    id: InternalId,
    index: usize,
    resolution: Resolution,
    allow_plaintext: bool,
) -> Result<(), String> {
    let report = diagnose(w)?;
    if !report
        .bindings
        .iter()
        .any(|b| b.model_id == id && b.status != "matched")
    {
        return Err("该模型没有需要处理的接管冲突".into());
    }
    let item = available_item(w, index, Some(id))?;
    let old = w
        .catalog
        .models
        .iter()
        .find(|m| m.id == id)
        .ok_or("模型已从草稿删除，请先丢弃删除草稿或解除旧接管")?;
    let (provider, mut model) = parse_item(&item, allow_plaintext)?;
    model.id = id;
    model.manufacturer = old.manufacturer.clone();
    accept_file_model(w, provider, model, matches!(resolution, Resolution::File))?;
    w.bindings
        .iter_mut()
        .find(|b| b.model_id == id)
        .ok_or("接管关系不存在")?
        .original = item;
    Ok(())
}
#[tauri::command]
pub fn resolve_binding(
    store: tauri::State<'_, Store>,
    id: InternalId,
    index: usize,
    resolution: Resolution,
    allow_plaintext: bool,
    revision: u64,
) -> Result<Workspace, String> {
    store.edit(revision, |w| {
        resolve(w, id, index, resolution, allow_plaintext)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        adapter,
        config::{merge, takeover},
        model::ids::ApiModelId,
        provider::tests::sample,
        switch::scheme::Source,
    };
    use serde_json::json;

    fn setup() -> (tempfile::TempDir, Store, InternalId) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        let provider = sample();
        let first = ModelEntry::new(
            provider.id,
            ApiModelId::try_from("first".to_owned()).unwrap(),
            "First".into(),
        );
        let second = ModelEntry::new(
            provider.id,
            ApiModelId::try_from("second".to_owned()).unwrap(),
            "Second".into(),
        );
        let id = first.id;
        let originals: Vec<_> = [&first, &second]
            .into_iter()
            .map(|m| adapter::custom_model(m, &provider, "0.238.0").unwrap())
            .collect();
        let path = dir.path().join("settings.json");
        let mut disk = originals.clone();
        for (i, item) in disk.iter_mut().enumerate() {
            item["index"] = json!(i);
            item["id"] = json!(format!("custom:{i}"));
        }
        std::fs::write(
            &path,
            json!({"customModels":disk, "unrelated":true}).to_string(),
        )
        .unwrap();
        store
            .edit(0, |w| {
                w.target_path = path.to_string_lossy().into();
                w.catalog = Catalog {
                    providers: vec![provider],
                    models: vec![first, second],
                };
                w.applied_catalog = w.catalog.clone();
                w.bindings = w
                    .catalog
                    .models
                    .iter()
                    .zip(originals)
                    .map(|(m, original)| Binding {
                        model_id: m.id,
                        original,
                    })
                    .collect();
                w.scheme.source = Some(Source::Api);
                w.scheme.api.model_id = Some(id);
                w.applied_scheme = w.scheme.clone();
                w.baseline = Some(read(&path)?);
                Ok(())
            })
            .unwrap();
        (dir, store, id)
    }
    fn edit_disk(store: &Store, edit: impl FnOnce(&mut Value)) {
        let w = store.load().unwrap();
        let mut doc = read(Path::new(&w.target_path)).unwrap().document;
        edit(&mut doc);
        std::fs::write(&w.target_path, doc.to_string()).unwrap();
        store
            .edit(w.revision, |w| {
                w.baseline = Some(read(Path::new(&w.target_path))?);
                Ok(())
            })
            .unwrap();
    }
    #[test]
    fn diagnosis_accepts_metadata_and_hides_all_values_and_unknown_keys() {
        let (_dir, store, _) = setup();
        let report = diagnose(&store.load().unwrap()).unwrap();
        assert!(report.bindings.iter().all(|b| b.status == "matched"));
        assert!(report.external.is_empty());
        edit_disk(&store, |d| {
            d["customModels"][0]["apiKey"] = json!("secret-new");
            d["customModels"][0]["secret-property"] = json!("secret-value");
        });
        let report = diagnose(&store.load().unwrap()).unwrap();
        assert_eq!(report.bindings[0].status, "changed_or_missing");
        let serialized = serde_json::to_string(&report).unwrap();
        assert!(serialized.contains("apiKey"));
        assert!(serialized.contains("其他字段"));
        assert!(!serialized.contains("secret"));
    }
    #[test]
    fn file_resolution_preserves_identity_default_and_isolates_shared_credentials() {
        let (_dir, store, id) = setup();
        let before = store.load().unwrap();
        edit_disk(&store, |d| {
            d["customModels"][0]["apiKey"] = json!("changed-key");
            d["customModels"][0]["noImageSupport"] = json!(false);
        });
        let w = store.load().unwrap();
        let disk = std::fs::read(&w.target_path).unwrap();
        assert!(store
            .edit(w.revision, |w| resolve(w, id, 0, Resolution::File, false))
            .is_err());
        assert_eq!(store.load().unwrap().revision, w.revision);
        let w = store
            .edit(w.revision, |w| resolve(w, id, 0, Resolution::File, true))
            .unwrap();
        assert_eq!(w.scheme.api.model_id, Some(id));
        assert_eq!(w.catalog.models[0].id, id);
        assert!(w.catalog.models[0].image_input);
        assert_ne!(
            w.catalog.models[0].provider_id,
            w.catalog.models[1].provider_id
        );
        assert!(w.catalog.providers[0] == before.catalog.providers[0]);
        assert_eq!(w.catalog.models.len(), 2);
        assert_eq!(w.bindings.len(), 2);
        assert!(diagnose(&w)
            .unwrap()
            .bindings
            .iter()
            .all(|b| b.status == "matched"));
        assert_eq!(std::fs::read(&w.target_path).unwrap(), disk);
        let merged = merge::merge(
            &w.baseline.as_ref().unwrap().document,
            &w.bindings,
            &w.catalog,
            "0.238.0",
        )
        .unwrap();
        assert_eq!(merged.counts.modified, 0);
    }
    #[test]
    fn app_resolution_preserves_draft_and_unknown_fields_and_discard_uses_file() {
        let (_dir, store, id) = setup();
        edit_disk(&store, |d| {
            d["customModels"][0]["noImageSupport"] = json!(false);
            d["customModels"][0]["index"] = json!(20);
            d["customModels"][0]["future"] = json!(7);
        });
        let w = store.load().unwrap();
        let w = store
            .edit(w.revision, |w| resolve(w, id, 0, Resolution::App, true))
            .unwrap();
        assert!(!w.catalog.models[0].image_input);
        assert!(w.applied_catalog.models[0].image_input);
        let merged = merge::merge(
            &w.baseline.as_ref().unwrap().document,
            &w.bindings,
            &w.catalog,
            "0.238.0",
        )
        .unwrap();
        assert_eq!(merged.counts.modified, 1);
        assert_eq!(merged.document["customModels"][0]["future"], 7);
        assert_eq!(merged.document["customModels"][0]["index"], 20);
        assert_eq!(merged.document["customModels"][0]["noImageSupport"], true);
        let discarded = store
            .edit(w.revision, |w| {
                w.catalog = w.applied_catalog.clone();
                w.scheme = w.applied_scheme.clone();
                Ok(())
            })
            .unwrap();
        let merged = merge::merge(
            &discarded.baseline.as_ref().unwrap().document,
            &discarded.bindings,
            &discarded.catalog,
            "0.238.0",
        )
        .unwrap();
        assert_eq!(merged.counts.modified, 0);
    }
    #[test]
    fn rejects_duplicates_occupied_targets_stale_revision_and_changed_file_without_writes() {
        let (_dir, store, id) = setup();
        edit_disk(&store, |d| {
            d["customModels"][0]["apiKey"] = json!("changed");
        });
        let w = store.load().unwrap();
        assert!(store
            .edit(w.revision - 1, |w| resolve(w, id, 0, Resolution::App, true))
            .is_err());
        assert!(store
            .edit(w.revision, |w| resolve(w, id, 1, Resolution::App, true))
            .is_err());
        let mut doc = w.baseline.as_ref().unwrap().document.clone();
        doc["new"] = json!(1);
        std::fs::write(&w.target_path, doc.to_string()).unwrap();
        assert!(store
            .edit(w.revision, |w| resolve(w, id, 0, Resolution::App, true))
            .is_err());
        assert_eq!(store.load().unwrap().revision, w.revision);
        edit_disk(&store, |d| {
            let item = d["customModels"][0].clone();
            d["customModels"].as_array_mut().unwrap().push(item);
        });
        let w = store.load().unwrap();
        assert!(store
            .edit(w.revision, |w| resolve(w, id, 0, Resolution::App, true))
            .is_err());
        assert_eq!(store.load().unwrap().revision, w.revision);
    }
    #[test]
    fn linked_takeover_reuses_model_and_provider_instead_of_duplicating() {
        let (_dir, store, id) = setup();
        let w = store.load().unwrap();
        let w = store
            .edit(w.revision, |w| {
                w.bindings.clear();
                Ok(())
            })
            .unwrap();
        assert!(store
            .edit(w.revision, |w| takeover::adopt(w, 0, true))
            .is_err());
        let w = store
            .edit(w.revision, |w| {
                takeover::adopt_linked(w, 0, true, Some(id), false)
            })
            .unwrap();
        assert_eq!(w.catalog.models.len(), 2);
        assert_eq!(w.catalog.providers.len(), 1);
        assert_eq!(w.bindings.len(), 1);
        assert_eq!(w.bindings[0].model_id, id);
        assert_eq!(w.scheme.api.model_id, Some(id));
        assert!(store
            .edit(w.revision, |w| takeover::adopt_linked(
                w,
                1,
                true,
                Some(id),
                false
            ))
            .is_err());
    }
    #[test]
    fn explicitly_separate_same_endpoint_connection_is_supported() {
        let (_dir, store, _) = setup();
        edit_disk(&store, |d| {
            let mut item = d["customModels"][0].clone();
            item["apiKey"] = json!("second-account");
            item["displayName"] = json!("Second account");
            item["id"] = json!("custom:another");
            item["index"] = json!(2);
            d["customModels"].as_array_mut().unwrap().push(item);
        });
        let w = store.load().unwrap();
        assert!(store
            .edit(w.revision, |w| takeover::adopt(w, 2, true))
            .is_err());
        let w = store
            .edit(w.revision, |w| {
                takeover::adopt_linked(w, 2, true, None, true)
            })
            .unwrap();
        assert_eq!(w.catalog.models.len(), 3);
        assert_eq!(w.catalog.providers.len(), 2);
        assert_ne!(
            w.catalog.models[0].provider_id,
            w.catalog.models[2].provider_id
        );
        assert!(diagnose(&w)
            .unwrap()
            .bindings
            .iter()
            .all(|b| b.status == "matched"));
    }
    #[test]
    fn recovery_does_not_overwrite_shared_provider_draft_or_break_discard() {
        let (_dir, store, id) = setup();
        let w = store.load().unwrap();
        store
            .edit(w.revision, |w| {
                w.catalog.providers[0].credential.value = "DRAFT_KEY".into();
                Ok(())
            })
            .unwrap();
        edit_disk(&store, |d| {
            d["customModels"][0]["noImageSupport"] = json!(false);
        });
        let w = store.load().unwrap();
        let w = store
            .edit(w.revision, |w| resolve(w, id, 0, Resolution::File, true))
            .unwrap();
        assert_eq!(w.catalog.providers[0].credential.value, "DRAFT_KEY");
        assert_eq!(
            w.applied_catalog.providers[0].credential.value,
            "PROVIDER_KEY"
        );
        let merged = merge::merge(
            &w.baseline.as_ref().unwrap().document,
            &w.bindings,
            &w.catalog,
            "0.238.0",
        )
        .unwrap();
        assert_eq!(
            merged.document["customModels"][0]["apiKey"],
            "${PROVIDER_KEY}"
        );
        assert_eq!(merged.document["customModels"][1]["apiKey"], "${DRAFT_KEY}");
        let discarded = merge::merge(
            &w.baseline.as_ref().unwrap().document,
            &w.bindings,
            &w.applied_catalog,
            "0.238.0",
        )
        .unwrap();
        assert_eq!(discarded.counts.modified, 0);
    }
    #[test]
    fn missing_and_ambiguous_entries_are_not_silently_rebound() {
        let (_dir, store, _) = setup();
        edit_disk(&store, |d| {
            d["customModels"].as_array_mut().unwrap().remove(0);
        });
        let report = diagnose(&store.load().unwrap()).unwrap();
        assert_eq!(report.bindings[0].status, "changed_or_missing");
        assert!(report.bindings[0].candidates.is_empty());
        edit_disk(&store, |d| {
            let item = d["customModels"][0].clone();
            d["customModels"].as_array_mut().unwrap().push(item);
        });
        let report = diagnose(&store.load().unwrap()).unwrap();
        assert_eq!(report.bindings[1].status, "ambiguous");
    }
}
