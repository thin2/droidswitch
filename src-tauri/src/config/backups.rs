use super::{
    pipeline::{self, hash, read, Backup},
    redact::pair,
};
use crate::{
    fs,
    workspace::{Store, Workspace},
};
use serde::Serialize;
use serde_json::{json, Value};
use std::path::Path;
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub id: String,
    pub target_path: String,
    pub created_at: u64,
    pub file_existed: bool,
}
fn load(store: &Store, id: &str) -> Result<Backup, String> {
    let s = fs::read_snapshot(&pipeline::backup_path(store, id)?)
        .map_err(|_| "备份不存在或无法读取")?;
    serde_json::from_str(&s.text).map_err(|_| "备份结构无效".into())
}
#[tauri::command]
pub fn list_backups(store: tauri::State<'_, Store>) -> Result<Vec<Summary>, String> {
    let dir = store.data_dir.join("backups");
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut items = vec![];
    for entry in std::fs::read_dir(dir).map_err(|_| "无法读取备份目录")? {
        let entry = entry.map_err(|_| "无法读取备份条目")?;
        if entry.path().extension().is_some_and(|s| s == "json") {
            let id = entry
                .path()
                .file_stem()
                .and_then(|v| v.to_str())
                .ok_or("备份名称无效")?
                .to_owned();
            let b = load(&store, &id)?;
            items.push(Summary {
                id: b.id,
                target_path: b.target_path,
                created_at: b.created_at,
                file_existed: b.before.is_some(),
            });
        }
    }
    items.sort_by_key(|a| std::cmp::Reverse(a.created_at));
    Ok(items)
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePreview {
    pub token: String,
    pub before: Value,
    pub after: Value,
    pub removes_file: bool,
}
fn preview(store: &Store, w: &Workspace, id: &str) -> Result<RestorePreview, String> {
    let b = load(store, id)?;
    if b.target_path != w.target_path {
        return Err("此备份属于其他配置路径，请先切换目标路径".into());
    }
    let current = read(Path::new(&w.target_path))?;
    let after = match &b.before {
        Some(s) => serde_json::from_str(s).map_err(|_| "备份不是有效 JSON")?,
        None => Value::Null,
    };
    let token = hash(
        &serde_json::to_vec(&json!([w.revision, id, current.fingerprint, after]))
            .map_err(|_| "恢复预览失败")?,
    );
    let (before, after, _) = pair(&current.document, &after);
    Ok(RestorePreview {
        token,
        before,
        after,
        removes_file: b.before.is_none(),
    })
}
#[tauri::command]
pub fn preview_restore(
    store: tauri::State<'_, Store>,
    id: String,
) -> Result<RestorePreview, String> {
    preview(&store, &store.load()?, &id)
}
pub fn restore(store: &Store, id: &str, revision: u64, token: &str) -> Result<Workspace, String> {
    let result = store.edit(revision, |w| {
        let p = preview(store, w, id)?;
        if p.token != token {
            return Err("恢复预览已过期，请重新确认".into());
        }
        let b = load(store, id)?;
        w.baseline = Some(read(Path::new(&w.target_path))?);
        if preview(store, w, id)?.token != token {
            return Err("恢复预览已过期，请重新确认".into());
        }
        let mut next = b.previous;
        next.usage_settings = w.usage_settings.clone();
        next.catalog = next.applied_catalog.clone();
        next.scheme = next.applied_scheme.clone();
        pipeline::write_with_backup(store, w, &mut next, b.before.as_deref().map(str::as_bytes))?;
        *w = next;
        w.revision = revision;
        Ok(())
    });
    if result.is_ok() {
        let _ = std::fs::remove_file(store.data_dir.join("pending-apply.json"));
    }
    result
}
#[tauri::command]
pub fn restore_backup(
    store: tauri::State<'_, Store>,
    id: String,
    revision: u64,
    token: String,
) -> Result<Workspace, String> {
    restore(&store, &id, revision, &token)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restore_backup_exact_bytes_and_preserves_current_as_new_backup() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(d.path()).unwrap();
        let target = d.path().join("settings.json");
        let original = "{ \"customModels\": [], \"future\": true }\n";
        std::fs::write(&target, original).unwrap();
        s.edit(0, |w| {
            w.target_path = target.to_string_lossy().into();
            w.baseline = Some(read(&target)?);
            Ok(())
        })
        .unwrap();
        s.edit(1, |w| {
            let mut next = w.clone();
            pipeline::write_with_backup(&s, w, &mut next, Some(b"{\"future\":false}"))?;
            *w = next;
            w.revision = 1;
            Ok(())
        })
        .unwrap();
        std::fs::remove_file(s.data_dir.join("pending-apply.json")).unwrap();
        let entry = std::fs::read_dir(s.data_dir.join("backups"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        let id = entry
            .path()
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let w = s.load().unwrap();
        let p = preview(&s, &w, &id).unwrap();
        restore(&s, &id, w.revision, &p.token).unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), original);
        assert_eq!(
            std::fs::read_dir(s.data_dir.join("backups"))
                .unwrap()
                .count(),
            2
        );
    }
}
