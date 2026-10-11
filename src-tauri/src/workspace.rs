use crate::{
    db,
    model::ids::InternalId,
    provider::{Catalog, ProviderConnection},
};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub revision: u64,
    #[serde(default)]
    pub recovery_pending: bool,
    pub target_path: String,
    pub catalog: Catalog,
    #[serde(default)]
    pub usage_settings: crate::usage::commands::UsageSettings,
    #[serde(default)]
    pub imports: std::collections::BTreeMap<String, crate::import::mapping::Imported>,
    #[serde(default)]
    pub scheme: crate::switch::scheme::Scheme,
    #[serde(default)]
    pub applied_scheme: crate::switch::scheme::Scheme,
    #[serde(default)]
    pub applied_catalog: Catalog,
    #[serde(default)]
    pub bindings: Vec<crate::config::merge::Binding>,
    #[serde(default)]
    pub baseline: Option<crate::config::pipeline::Baseline>,
}
pub struct Store {
    pub db: Mutex<Connection>,
    pub data_dir: PathBuf,
}
impl Store {
    pub fn open(data_dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(data_dir).map_err(|_| "无法创建应用数据目录")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(data_dir, std::fs::Permissions::from_mode(0o700))
                .map_err(|_| "无法保护应用数据目录")?;
        }
        let db = db::open(&data_dir.join("droid-switch.db"))
            .map_err(|_| "无法打开管理数据库，请检查权限或数据库版本")?;
        Ok(Self {
            db: Mutex::new(db),
            data_dir: data_dir.into(),
        })
    }
    pub fn load(&self) -> Result<Workspace, String> {
        let db = self.db.lock().map_err(|_| "数据库繁忙")?;
        let mut w = load(&db)?;
        w.recovery_pending = self.data_dir.join("pending-apply.json").exists();
        Ok(w)
    }
    pub fn edit(
        &self,
        expected: u64,
        edit: impl FnOnce(&mut Workspace) -> Result<(), String>,
    ) -> Result<Workspace, String> {
        self.edit_inner(expected, edit, false)
    }
    pub(crate) fn edit_recovery(
        &self,
        expected: u64,
        edit: impl FnOnce(&mut Workspace) -> Result<(), String>,
    ) -> Result<Workspace, String> {
        self.edit_inner(expected, edit, true)
    }
    fn edit_inner(
        &self,
        expected: u64,
        edit: impl FnOnce(&mut Workspace) -> Result<(), String>,
        recovering: bool,
    ) -> Result<Workspace, String> {
        let mut db = self.db.lock().map_err(|_| "数据库繁忙")?;
        let tx = db.transaction().map_err(|_| "数据库事务失败")?;
        if !recovering && self.data_dir.join("pending-apply.json").exists() {
            return Err("存在未完成的配置事务，请重启应用恢复后再编辑".into());
        }
        let mut workspace = load(&tx)?;
        if workspace.revision != expected {
            return Err("草稿已在其他窗口变化，请刷新后重试".into());
        }
        edit(&mut workspace)?;
        workspace.catalog.validate()?;
        workspace.scheme.validate(&workspace.catalog)?;
        workspace.revision += 1;
        let payload = serde_json::to_string(&workspace).map_err(|_| "草稿序列化失败")?;
        tx.execute("INSERT INTO drafts(id,target_path,payload,revision,updated_at) VALUES ('workspace',?1,?2,?3,unixepoch()) ON CONFLICT(id) DO UPDATE SET target_path=excluded.target_path,payload=excluded.payload,revision=excluded.revision,updated_at=excluded.updated_at",rusqlite::params![workspace.target_path,payload,workspace.revision as i64]).map_err(|_|"保存草稿失败")?;
        tx.commit().map_err(|_| "保存草稿失败")?;
        Ok(workspace)
    }
}
fn load(db: &Connection) -> Result<Workspace, String> {
    let payload: Option<String> = db
        .query_row("SELECT payload FROM drafts WHERE id='workspace'", [], |r| {
            r.get(0)
        })
        .optional()
        .map_err(|_| "读取草稿失败")?;
    match payload {
        Some(p) => {
            serde_json::from_str(&p).map_err(|_| "草稿结构未知，请保留数据库并检查版本".into())
        }
        None => Ok(Workspace {
            revision: 0,
            recovery_pending: false,
            target_path: default_target()?.to_string_lossy().into(),
            catalog: Catalog::default(),
            usage_settings: Default::default(),
            imports: Default::default(),
            scheme: Default::default(),
            applied_scheme: Default::default(),
            applied_catalog: Catalog::default(),
            bindings: vec![],
            baseline: None,
        }),
    }
}
pub fn default_target() -> Result<PathBuf, String> {
    #[cfg(windows)]
    let home = std::env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let home = std::env::var_os("HOME");
    home.filter(|v| !v.is_empty())
        .map(|h| PathBuf::from(h).join(".factory").join("settings.json"))
        .ok_or("无法确定用户主目录，请指定配置路径".into())
}
#[tauri::command]
pub fn load_workspace(store: tauri::State<'_, Store>) -> Result<Workspace, String> {
    store.load()
}
#[tauri::command]
pub fn save_provider(
    store: tauri::State<'_, Store>,
    provider: ProviderConnection,
    revision: u64,
    allow_plaintext: bool,
) -> Result<Workspace, String> {
    provider.validate()?;
    if provider.credential.mode == crate::provider::CredentialMode::Direct && !allow_plaintext {
        return Err("使用兼容模式前需确认配置及备份含明文密钥".into());
    }
    store.edit(revision, |w| {
        if let Some(p) = w.catalog.providers.iter_mut().find(|p| p.id == provider.id) {
            *p = provider;
        } else {
            w.catalog.providers.push(provider);
        }
        Ok(())
    })
}
#[tauri::command]
pub fn copy_provider(
    store: tauri::State<'_, Store>,
    id: InternalId,
    revision: u64,
) -> Result<Workspace, String> {
    store.edit(revision, |w| {
        let mut copy = w.catalog.provider(id)?.clone();
        copy.id = InternalId::new();
        copy.name = format!("{} · 副本", copy.name);
        w.catalog.providers.push(copy);
        Ok(())
    })
}
#[tauri::command]
pub fn delete_provider(
    store: tauri::State<'_, Store>,
    id: InternalId,
    revision: u64,
    confirm_models: usize,
) -> Result<Workspace, String> {
    store.edit(revision, |w| {
        w.catalog.provider(id)?;
        let count = w
            .catalog
            .models
            .iter()
            .filter(|m| m.provider_id == id)
            .count();
        if count != confirm_models {
            return Err("受影响模型数量已变化，请重新确认".into());
        }
        w.catalog.models.retain(|m| m.provider_id != id);
        w.catalog.providers.retain(|p| p.id != id);
        Ok(())
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::tests::sample;
    #[test]
    fn provider_draft_survives_restart_and_stale_save_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        let p = sample();
        let first = store
            .edit(0, |w| {
                w.catalog.providers.push(p);
                Ok(())
            })
            .unwrap();
        assert_eq!(first.revision, 1);
        assert!(store.edit(0, |_| Ok(())).is_err());
        drop(store);
        assert_eq!(
            Store::open(dir.path())
                .unwrap()
                .load()
                .unwrap()
                .catalog
                .providers
                .len(),
            1
        );
    }
}
#[cfg(test)]
mod draft_tests {
    use super::*;
    #[test]
    fn draft_discard_restores_applied_catalog_without_touching_target() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(d.path()).unwrap();
        let p = crate::provider::tests::sample();
        s.edit(0, |w| {
            w.catalog.providers.push(p.clone());
            w.applied_catalog = w.catalog.clone();
            Ok(())
        })
        .unwrap();
        s.edit(1, |w| {
            w.catalog.providers[0].name = "Draft name".into();
            Ok(())
        })
        .unwrap();
        drop(s);
        let s = Store::open(d.path()).unwrap();
        assert_eq!(s.load().unwrap().catalog.providers[0].name, "Draft name");
        let w = s
            .edit(2, |w| {
                w.catalog = w.applied_catalog.clone();
                Ok(())
            })
            .unwrap();
        assert_eq!(w.catalog.providers[0].name, p.name);
    }
}
