use crate::{
    model::{
        entry::ModelEntry,
        ids::{ApiModelId, InternalId},
    },
    workspace::{Store, Workspace},
};

#[tauri::command]
pub fn save_model(
    store: tauri::State<'_, Store>,
    mut model: ModelEntry,
    revision: u64,
) -> Result<Workspace, String> {
    // Selector IDs are discovered by Droid, never accepted from an editor.
    model.droid_selector_id = None;
    store.edit(revision, |w| {
        if let Some(old) = w.catalog.models.iter_mut().find(|m| m.id == model.id) {
            *old = model;
        } else {
            w.catalog.models.push(model);
        }
        Ok(())
    })
}
#[tauri::command]
pub fn delete_model(
    store: tauri::State<'_, Store>,
    id: InternalId,
    revision: u64,
) -> Result<Workspace, String> {
    store.edit(revision, |w| {
        if !w.catalog.models.iter().any(|m| m.id == id) {
            return Err("模型不存在".into());
        }
        w.catalog.models.retain(|m| m.id != id);
        Ok(())
    })
}
pub fn add_models(
    catalog: &mut crate::provider::Catalog,
    provider_id: InternalId,
    ids: Vec<ApiModelId>,
) -> Result<(), String> {
    catalog.provider(provider_id)?;
    if ids.len() > 10000 {
        return Err("一次最多导入 10000 个模型".into());
    }
    for api in ids {
        if !catalog
            .models
            .iter()
            .any(|m| m.provider_id == provider_id && m.api_model_id == api)
        {
            let name = api.as_str().to_owned();
            catalog.models.push(ModelEntry::new(provider_id, api, name));
        }
    }
    Ok(())
}
#[tauri::command]
pub fn import_models(
    store: tauri::State<'_, Store>,
    provider_id: InternalId,
    ids: Vec<ApiModelId>,
    revision: u64,
) -> Result<Workspace, String> {
    store.edit(revision, |w| add_models(&mut w.catalog, provider_id, ids))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_batch_keeps_existing_parameters_and_other_providers() {
        let p = crate::provider::tests::sample();
        let mut q = p.clone();
        q.id = InternalId::new();
        let mut c = crate::provider::Catalog {
            providers: vec![p.clone(), q.clone()],
            models: vec![],
        };
        let id = ApiModelId::try_from("exact/model:1".to_owned()).unwrap();
        add_models(&mut c, p.id, vec![id.clone(), id.clone()]).unwrap();
        c.models[0].display_name = "Edited".into();
        c.models[0].enabled = false;
        add_models(&mut c, p.id, vec![id.clone()]).unwrap();
        add_models(&mut c, q.id, vec![id]).unwrap();
        assert_eq!(c.models.len(), 2);
        assert_eq!(c.models[0].display_name, "Edited");
        assert!(!c.models[0].enabled);
        assert!(c.validate().is_ok());
    }
}
