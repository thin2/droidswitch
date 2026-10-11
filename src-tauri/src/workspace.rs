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
    pub target_path: String,
    pub catalog: Catalog,
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
        load(&db)
    }
    pub fn edit(
        &self,
        expected: u64,
        edit: impl FnOnce(&mut Workspace) -> Result<(), String>,
    ) -> Result<Workspace, String> {
        let mut db = self.db.lock().map_err(|_| "数据库繁忙")?;
        let tx = db.transaction().map_err(|_| "数据库事务失败")?;
        let mut workspace = load(&tx)?;
        if workspace.revision != expected {
            return Err("草稿已在其他窗口变化，请刷新后重试".into());
        }
        edit(&mut workspace)?;
        workspace.catalog.validate()?;
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
            target_path: default_target()?.to_string_lossy().into(),
            catalog: Catalog::default(),
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
