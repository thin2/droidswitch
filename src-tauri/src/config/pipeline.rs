use super::{
    merge::{self, Binding, Counts},
    redact::pair,
};
use crate::{
    fs::{self, Fingerprint},
    workspace::{Store, Workspace},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::Path;
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Baseline {
    pub document: Value,
    pub fingerprint: Option<Fingerprint>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub token: String,
    pub before: Value,
    pub after: Value,
    pub counts: Counts,
    pub credential_changed: bool,
    pub version: String,
    pub warnings: Vec<String>,
}
pub fn read(path: &Path) -> Result<Baseline, String> {
    match fs::read_snapshot(path) {
        Ok(s) => {
            let document: Value =
                serde_json::from_str(&s.text).map_err(|_| "配置 JSON 无效，未更改任何内容")?;
            if !document.is_object() {
                return Err("配置顶层必须为 JSON 对象".into());
            }
            Ok(Baseline {
                document,
                fingerprint: Some(s.fingerprint),
            })
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Baseline {
            document: json!({}),
            fingerprint: None,
        }),
        Err(_) => Err("无法安全读取配置，请检查权限、文件类型和大小".into()),
    }
}
pub(super) fn read_for_restore(path: &Path) -> Result<Baseline, String> {
    match fs::read_snapshot(path){
 Ok(s)=>Ok(Baseline{document:serde_json::from_str(&s.text).unwrap_or(json!({"invalidJson":true,"notice":"Current bytes will be backed up before replacement"})),fingerprint:Some(s.fingerprint)}),
 Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(Baseline{document:json!({}),fingerprint:None}),
 Err(_)=>Err("Cannot safely read the restore target".into()),
 }
}
pub(crate) fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn prepare(w: &Workspace, version: &str) -> Result<(Preview, Value, Vec<Binding>), String> {
    let baseline = w.baseline.as_ref().ok_or("请先载入目标配置，再预览应用")?;
    let current = read(Path::new(&w.target_path))?;
    if current.fingerprint != baseline.fingerprint {
        return Err("磁盘配置已被外部修改，请重新载入；你的草稿仍保留".into());
    }
    let mut merged = merge::merge(&current.document, &w.bindings, &w.catalog, version)?;
    crate::model::naming::validate_names(&merged.document)?;
    let warnings = if w.scheme.source.is_some() {
        let observed = crate::switch::catalog::discover(&merged.document)?;
        crate::switch::scheme::apply_scheme(&mut merged.document, &w.scheme, &w.catalog, &observed)?
    } else {
        vec![]
    };
    let token = hash(
        &serde_json::to_vec(&json!([
            w.revision,
            w.target_path,
            current.fingerprint,
            merged.document,
            version
        ]))
        .map_err(|_| "预览序列化失败")?,
    );
    let (before, after, credential_changed) = pair(&current.document, &merged.document);
    Ok((
        Preview {
            token,
            before,
            after,
            counts: merged.counts,
            credential_changed,
            version: version.into(),
            warnings,
        },
        merged.document,
        merged.bindings,
    ))
}
#[tauri::command]
pub fn reload_configuration(
    store: tauri::State<'_, Store>,
    revision: u64,
) -> Result<Workspace, String> {
    store.edit(revision, |w| {
        let baseline = read(Path::new(&w.target_path))?;
        // Adopt only uniquely matched, newly added Droid metadata; preserve all drafts.
        super::merge::refresh_metadata_bindings(&baseline.document, &mut w.bindings);
        w.baseline = Some(baseline);
        Ok(())
    })
}
#[tauri::command]
pub async fn preview_configuration(store: tauri::State<'_, Store>) -> Result<Preview, String> {
    let w = store.load()?;
    tauri::async_runtime::spawn_blocking(move || {
        let version = super::cli::version()?;
        prepare(&w, &version).map(|r| r.0)
    })
    .await
    .map_err(|_| "预览任务失败")?
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub id: String,
    pub target_path: String,
    pub created_at: u64,
    pub before: Option<String>,
    pub after_hash: String,
    pub previous: Workspace,
    pub next: Workspace,
}
pub(super) fn backup_path(store: &Store, id: &str) -> Result<std::path::PathBuf, String> {
    let id = uuid::Uuid::parse_str(id).map_err(|_| "备份 ID 无效")?;
    Ok(store.data_dir.join("backups").join(format!("{id}.json")))
}
// Journal remains until the database commit succeeds. On restart recovery updates only
// application metadata when the target still matches the staged content hash.
pub fn recover(store: &Store) -> Result<(), String> {
    let path = store.data_dir.join("pending-apply.json");
    let text = match fs::read_snapshot(&path) {
        Ok(v) => v.text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err("无法读取应用恢复记录".into()),
    };
    let b: Backup = serde_json::from_str(&text).map_err(|_| "应用恢复记录损坏，请保留数据目录")?;
    let w = store.load()?;
    if w.revision == b.next.revision {
        std::fs::remove_file(&path).map_err(|_| "无法清理恢复记录")?;
        return Ok(());
    }
    let current = match fs::read_snapshot(Path::new(&b.target_path)) {
        Ok(v) => Some(v.text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => return Err("无法检查中断的应用操作".into()),
    };
    if current
        .as_ref()
        .map(|s| hash(s.as_bytes()))
        .unwrap_or("missing".into())
        == b.after_hash
    {
        store.edit_recovery(b.previous.revision, |w| {
            *w = b.next.clone();
            w.baseline = Some(read(Path::new(&b.target_path))?);
            w.revision = b.previous.revision;
            Ok(())
        })?;
    } else if current != b.before {
        return Err(
            "应用中断后配置又被修改。请保留 pending-apply.json 和备份，先处理恢复冲突".into(),
        );
    }
    std::fs::remove_file(&path).map_err(|_| "无法清理恢复记录")?;
    Ok(())
}
pub(super) fn write_with_backup(
    store: &Store,
    w: &Workspace,
    next: &mut Workspace,
    bytes: Option<&[u8]>,
) -> Result<(), String> {
    if store.data_dir.join("pending-apply.json").exists() {
        return Err("有未完成的应用恢复记录，请先重启恢复".into());
    }
    let path = std::path::PathBuf::from(&w.target_path);
    let current = read_for_restore(&path)?;
    if Some(&current.fingerprint) != w.baseline.as_ref().map(|b| &b.fingerprint) {
        return Err("磁盘配置已变化，请重新载入".into());
    }
    let before = match current.fingerprint {
        Some(_) => Some(fs::read_snapshot(&path).map_err(|_| "无法备份原配置")?.text),
        None => None,
    };
    if before.as_ref().map(|s| hash(s.as_bytes()))
        != current.fingerprint.as_ref().map(|f| f.sha256.clone())
    {
        return Err("配置在备份期间变化，请重新载入".into());
    }
    next.revision = w.revision + 1;
    let id = uuid::Uuid::new_v4().to_string();
    std::fs::create_dir_all(store.data_dir.join("backups")).map_err(|_| "无法创建备份目录")?;
    let mut backup = Backup {
        id: id.clone(),
        target_path: w.target_path.clone(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        before,
        after_hash: bytes.map(hash).unwrap_or("missing".into()),
        previous: w.clone(),
        next: next.clone(),
    };
    let metadata = serde_json::to_vec(&backup).map_err(|_| "备份序列化失败")?;
    fs::atomic_replace(&backup_path(store, &id)?, &metadata).map_err(|_| "备份失败，未写入目标")?;
    let journal = store.data_dir.join("pending-apply.json");
    fs::atomic_replace(&journal, &metadata).map_err(|_| "创建恢复记录失败，未写入目标")?;
    if read_for_restore(&path)?.fingerprint != current.fingerprint {
        return Err("写入前配置发生变化，已保留备份与恢复记录".into());
    }
    match bytes {
        Some(bytes) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|_| "无法创建配置目录")?;
            }
            fs::atomic_replace(&path, bytes).map_err(|_| "写入失败，原配置或备份已保留")?;
        }
        None => {
            if current.fingerprint.is_some() {
                std::fs::remove_file(&path).map_err(|_| "无法恢复不存在文件状态")?;
            }
        }
    }
    next.baseline = Some(read(&path)?);
    backup.next = next.clone();
    fs::atomic_replace(
        &journal,
        &serde_json::to_vec(&backup).map_err(|_| "恢复记录序列化失败")?,
    )
    .map_err(|_| "配置已写入，更新恢复记录失败，请重启恢复")?;
    Ok(())
}
pub fn apply(
    store: &Store,
    revision: u64,
    token: &str,
    version: &str,
) -> Result<Workspace, String> {
    if store.data_dir.join("pending-apply.json").exists() {
        return Err("有未完成的应用恢复记录，请重启应用处理".into());
    }
    let result = store.edit(revision, |w| {
        let (preview, document, bindings) = prepare(w, version)?;
        if preview.token != token {
            return Err("预览已过期，请重新预览后确认".into());
        }
        let bytes = serde_json::to_vec_pretty(&document).map_err(|_| "无法序列化配置")?;
        let mut next = w.clone();
        next.bindings = bindings;
        next.applied_catalog = w.catalog.clone();
        next.applied_scheme = w.scheme.clone();
        write_with_backup(store, w, &mut next, Some(&bytes))?;
        *w = next;
        w.revision = revision;
        Ok(())
    });
    if result.is_ok() {
        let _ = std::fs::remove_file(store.data_dir.join("pending-apply.json"));
        // Record the attribution history only after the apply actually succeeded,
        // so a failed apply can never produce an effective mapping.
        let target = store.load().map(|w| w.target_path).unwrap_or_default();
        if !target.is_empty() {
            let _ = crate::usage::identity::record_after_apply(store, &target);
        }
    }
    result
}
/// Explicitly authorized, isolated integration: never apply unrelated pending drafts.
pub(crate) fn apply_direct(
    store: &Store,
    update: impl FnOnce(&mut Workspace) -> Result<(), String>,
) -> Result<Workspace, String> {
    let version = super::cli::version()?;
    apply_direct_version(store, &version, update)
}
fn apply_direct_version(
    store: &Store,
    version: &str,
    update: impl FnOnce(&mut Workspace) -> Result<(), String>,
) -> Result<Workspace, String> {
    let revision = store.load()?.revision;
    let result = store.edit(revision, |w| {
        if w.catalog != w.applied_catalog || w.scheme != w.applied_scheme {
            return Err(
                "已登录，但存在其他未应用的草稿。请先应用或放弃这些草稿，再点击接入 Droid".into(),
            );
        }
        let mut before = w.clone();
        if before.baseline.is_none() {
            before.baseline = Some(read(Path::new(&before.target_path))?);
        }
        let mut next = before.clone();
        update(&mut next)?;
        next.catalog.validate()?;
        let (_, document, bindings) = prepare(&next, version)?;
        next.bindings = bindings;
        next.applied_catalog = next.catalog.clone();
        next.applied_scheme = next.scheme.clone();
        let bytes = serde_json::to_vec_pretty(&document).map_err(|_| "配置编码失败")?;
        write_with_backup(store, &before, &mut next, Some(&bytes))?;
        *w = next;
        w.revision = revision;
        Ok(())
    });
    if result.is_ok() {
        let _ = std::fs::remove_file(store.data_dir.join("pending-apply.json"));
        let _ = crate::usage::identity::record_after_apply(store, &store.load()?.target_path);
    }
    result
}
#[tauri::command]
pub async fn apply_configuration(
    store: tauri::State<'_, Store>,
    registry: tauri::State<'_, crate::proxy::ProxyRegistry>,
    revision: u64,
    token: String,
) -> Result<Workspace, String> {
    let version = tauri::async_runtime::spawn_blocking(super::cli::version)
        .await
        .map_err(|_| "CLI 检测失败")??;
    let pending = store.load()?;
    if pending.catalog.models.iter().any(|m| {
        m.enabled
            && pending.catalog.provider(m.provider_id).is_ok_and(|p| {
                m.effective_protocol(p.protocol) == crate::model::entry::Protocol::GoogleGemini
            })
    }) && !registry.is_running()
    {
        return Err(
            "Gemini 本地代理未启动：请检查 127.0.0.1:18318 端口占用后重启应用；未写入配置".into(),
        );
    }
    for model in pending.catalog.models.iter().filter(|m| m.enabled) {
        let provider = pending.catalog.provider(model.provider_id)?;
        if model.effective_protocol(provider.protocol)
            == crate::model::entry::Protocol::GoogleGemini
        {
            provider
                .credential
                .resolve()
                .map_err(|_| "Gemini 凭据不可用：应用进程必须能读取环境变量；未写入配置")?;
            crate::proxy::handler::endpoint(&provider.base_url, model.api_model_id.as_str())
                .map_err(|_| "Gemini 原生端点或模型 ID 无效；未写入配置")?;
        }
    }
    let result = apply(&store, revision, &token, &version)?;

    // Refresh proxy registry after successful apply
    if let Err(e) = registry.refresh(&result) {
        eprintln!("代理注册表刷新失败: {}", e);
    }

    Ok(result)
}
#[tauri::command]
pub fn discard_draft(store: tauri::State<'_, Store>, revision: u64) -> Result<Workspace, String> {
    store.edit(revision, |w| {
        w.catalog = w.applied_catalog.clone();
        w.scheme = w.applied_scheme.clone();
        Ok(())
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::Catalog;
    fn setup() -> (tempfile::TempDir, Store) {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(d.path()).unwrap();
        s.edit(0, |w| {
            w.target_path = d.path().join("settings.json").to_string_lossy().into();
            std::fs::write(
                &w.target_path,
                r#"{"customModels":[{"external":true}],"future":1}"#,
            )
            .unwrap();
            w.baseline = Some(read(Path::new(&w.target_path))?);
            let p = crate::provider::tests::sample();
            let m = crate::model::entry::ModelEntry::new(
                p.id,
                crate::model::ids::ApiModelId::try_from("model".to_owned()).unwrap(),
                "Test".into(),
            );
            w.catalog = Catalog {
                providers: vec![p],
                models: vec![m],
            };
            Ok(())
        })
        .unwrap();
        (d, s)
    }
    #[test]
    fn direct_apply_preserves_unrelated_drafts_and_atomically_backs_up() {
        let (_dir, store) = setup();
        let original = store.load().unwrap();
        let disk = std::fs::read(&original.target_path).unwrap();
        assert!(apply_direct_version(&store, "0.238.0", |_| Ok(()))
            .err()
            .unwrap()
            .contains("草稿"));
        assert_eq!(std::fs::read(&original.target_path).unwrap(), disk);
        assert_eq!(store.load().unwrap().revision, original.revision);
        let preview = prepare(&original, "0.238.0").unwrap().0;
        apply(&store, original.revision, &preview.token, "0.238.0").unwrap();
        let updated = apply_direct_version(&store, "0.238.0", |w| {
            w.catalog.models[0].display_name = "Direct update".into();
            Ok(())
        })
        .unwrap();
        assert!(updated.catalog == updated.applied_catalog);
        assert_eq!(
            read(Path::new(&updated.target_path)).unwrap().document["customModels"][1]
                ["displayName"],
            "Direct update"
        );
        assert_eq!(
            std::fs::read_dir(store.data_dir.join("backups"))
                .unwrap()
                .count(),
            2
        );
        assert!(!store.data_dir.join("pending-apply.json").exists());
    }
    #[test]
    fn preview_rejects_duplicate_names_without_default_and_accepts_suggestions() {
        let (_dir, store) = setup();
        let mut w = store.load().unwrap();
        let mut provider = w.catalog.providers[0].clone();
        provider.id = crate::model::ids::InternalId::new();
        provider.name = "Other".into();
        let mut model = w.catalog.models[0].clone();
        model.id = crate::model::ids::InternalId::new();
        model.provider_id = provider.id;
        w.catalog.providers.push(provider);
        w.catalog.models.push(model);
        assert!(w.scheme.source.is_none());
        let before = std::fs::read(&w.target_path).unwrap();
        let error = prepare(&w, "0.238.0").err().unwrap();
        assert!(error.contains("模型显示名重复"));
        assert_eq!(std::fs::read(&w.target_path).unwrap(), before);
        crate::model::naming::apply_suggestions(&mut w).unwrap();
        assert!(prepare(&w, "0.238.0").is_ok());
    }
    #[test]
    fn fingerprint_merge_apply_rechecks_external_changes_and_preserves_draft() {
        let (_d, s) = setup();
        let w = s.load().unwrap();
        let (preview, _, _) = prepare(&w, "0.238.0").unwrap();
        std::fs::write(&w.target_path, "{}").unwrap();
        assert!(apply(&s, w.revision, &preview.token, "0.238.0").is_err());
        assert_eq!(s.load().unwrap().catalog.models.len(), 1);
        assert_eq!(std::fs::read_to_string(&w.target_path).unwrap(), "{}");
    }
    #[test]
    fn atomic_write_backup_apply_preserves_unknown_and_is_idempotent() {
        let (_d, s) = setup();
        let w = s.load().unwrap();
        let (p, _, _) = prepare(&w, "0.238.0").unwrap();
        let applied = apply(&s, w.revision, &p.token, "0.238.0").unwrap();
        let disk = read(Path::new(&w.target_path)).unwrap();
        assert_eq!(disk.document["future"], 1);
        assert_eq!(disk.document["customModels"].as_array().unwrap().len(), 2);
        assert_eq!(prepare(&applied, "0.238.0").unwrap().0.counts.added, 0);
        assert_eq!(
            std::fs::read_dir(s.data_dir.join("backups"))
                .unwrap()
                .count(),
            1
        );
        assert!(!s.data_dir.join("pending-apply.json").exists());
        assert!(apply(&s, w.revision, &p.token, "0.238.0").is_err());
    }
}
#[tauri::command]
pub fn export_draft(store: tauri::State<'_, Store>) -> Result<String, String> {
    let w = store.load()?;
    let value = json!({"format":"droid-switch-redacted-draft-v1","targetPath":w.target_path,"revision":w.revision,"catalog":w.catalog});
    serde_json::to_string_pretty(&super::redact::redact(&value)).map_err(|_| "无法导出草稿".into())
}
#[cfg(test)]
mod recovery_tests {
    use super::*;
    #[test]
    fn atomic_write_recovers_database_after_file_commit_and_keeps_draft() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::open(dir.path()).unwrap();
        let path = dir.path().join("settings.json");
        s.edit(0, |w| {
            w.target_path = path.to_string_lossy().into();
            w.baseline = Some(read(&path)?);
            Ok(())
        })
        .unwrap();
        let w = s.load().unwrap();
        let mut next = w.clone();
        write_with_backup(&s, &w, &mut next, Some(b"{\"recovered\":true}")).unwrap();
        assert!(s.edit(w.revision, |_| Ok(())).is_err());
        recover(&s).unwrap();
        let restored = s.load().unwrap();
        assert_eq!(restored.revision, next.revision);
        assert_eq!(
            restored.baseline.unwrap().document,
            json!({"recovered":true})
        );
        assert!(!s.data_dir.join("pending-apply.json").exists());
    }
    #[test]
    fn atomic_write_recovery_never_overwrites_external_edit() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::open(dir.path()).unwrap();
        let path = dir.path().join("settings.json");
        s.edit(0, |w| {
            w.target_path = path.to_string_lossy().into();
            w.baseline = Some(read(&path)?);
            Ok(())
        })
        .unwrap();
        let w = s.load().unwrap();
        write_with_backup(&s, &w, &mut w.clone(), Some(b"{}")).unwrap();
        std::fs::write(&path, b"{\"external\":true}").unwrap();
        assert!(recover(&s).is_err());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{\"external\":true}"
        );
        assert!(s.data_dir.join("pending-apply.json").exists());
    }
}
#[tauri::command]
pub fn resolve_recovery(
    store: tauri::State<'_, Store>,
    revision: u64,
    preserve_current: bool,
) -> Result<Workspace, String> {
    if !preserve_current {
        return Err("需明确确认保留当前磁盘配置".into());
    }
    let journal = store.data_dir.join("pending-apply.json");
    let record = fs::read_snapshot(&journal).map_err(|_| "恢复记录不存在或无法读取")?;
    let archive = store
        .data_dir
        .join(format!("recovery-{}.json", uuid::Uuid::new_v4()));
    fs::atomic_replace(&archive, record.text.as_bytes()).map_err(|_| "无法归档恢复记录")?;
    let mut w = store.edit_recovery(revision, |w| {
        w.baseline = Some(read(Path::new(&w.target_path))?);
        w.recovery_pending = false;
        Ok(())
    })?;
    std::fs::remove_file(journal).map_err(|_| "已保留当前状态，但无法解除恢复锁")?;
    w.recovery_pending = false;
    Ok(w)
}
