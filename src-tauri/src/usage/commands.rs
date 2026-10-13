use super::{aggregate, ingest, ingest::ScanReport, pricing, scanner};
use crate::workspace::{Store, Workspace};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

/// The path early builds shipped. It is not a real Droid directory, so a stored
/// value equal to it is migrated once the real default exists.
pub const LEGACY_DEFAULT_PATH: &str = "~/.factory/projects";
pub const DEFAULT_SESSIONS: &str = "~/.factory/sessions";
pub const DEFAULT_LOGS: &str = "~/.factory/logs";

fn default_sessions() -> String {
    DEFAULT_SESSIONS.into()
}
fn default_logs() -> String {
    DEFAULT_LOGS.into()
}
fn default_retention() -> u32 {
    90
}
fn yes() -> bool {
    true
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageSettings {
    #[serde(default = "default_sessions")]
    pub path: String,
    #[serde(default = "default_logs")]
    pub logs_path: String,
    /// Optional directory of explicit exports (`droid-switch-usage-v1` or SDK).
    #[serde(default)]
    pub exports_path: String,
    #[serde(default = "default_retention")]
    pub retention_days: u32,
    #[serde(default = "yes")]
    pub auto_scan: bool,
}

impl Default for UsageSettings {
    fn default() -> Self {
        Self {
            path: default_sessions(),
            logs_path: default_logs(),
            exports_path: String::new(),
            retention_days: default_retention(),
            auto_scan: true,
        }
    }
}

static SCANNING: AtomicBool = AtomicBool::new(false);

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn optional_path(value: &str) -> Result<Option<PathBuf>, String> {
    match value.trim().is_empty() {
        true => Ok(None),
        false => crate::config::path::resolve(value).map(Some),
    }
}

/// Stored settings plus a one-time migration notice for the legacy default.
fn effective_settings(store: &Store) -> Result<(UsageSettings, Option<String>), String> {
    let mut settings = store.load()?.usage_settings;
    let mut notice = None;
    if settings.path == LEGACY_DEFAULT_PATH {
        let exists = crate::config::path::resolve(LEGACY_DEFAULT_PATH)
            .map(|path| path.exists())
            .unwrap_or(false);
        if !exists {
            settings.path = default_sessions();
            notice = Some(format!(
                "原默认采集路径 {LEGACY_DEFAULT_PATH} 不存在，本次使用 {DEFAULT_SESSIONS}"
            ));
        }
    }
    if settings.logs_path.trim().is_empty() {
        settings.logs_path = default_logs();
    }
    Ok((settings, notice))
}

fn cutoff_for(db: &rusqlite::Connection, retention_days: u32) -> Result<i64, String> {
    let removed: Option<i64> = db
        .query_row(
            "SELECT byte_offset FROM sync_cursors WHERE source_file='usage-cleanup-cutoff'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| "无法读取清理边界")?;
    Ok((now() - i64::from(retention_days) * 86400).max(removed.unwrap_or(0)))
}

async fn scan_once(store: &Store) -> Result<ScanReport, String> {
    let (settings, _) = effective_settings(store)?;
    let sessions = optional_path(&settings.path)?;
    let logs = optional_path(&settings.logs_path)?;
    let exports = optional_path(&settings.exports_path)?;
    let mut offsets = {
        let db = store.db.lock().map_err(|_| "数据库繁忙")?;
        ingest::load_offsets(&db)?
    };
    // File discovery, reading and parsing never hold the database lock and never
    // run on the async runtime thread.
    let (inputs, offsets) = tauri::async_runtime::spawn_blocking(move || {
        let inputs = ingest::collect(sessions.as_deref(), logs.as_deref(), &mut offsets);
        (inputs, offsets)
    })
    .await
    .map_err(|_| "采集任务失败")?;
    let mut db = store.db.lock().map_err(|_| "数据库繁忙")?;
    let cutoff = cutoff_for(&db, settings.retention_days)?;
    let applied = ingest::apply(&mut db, inputs, &offsets, cutoff)?;
    let mut report = applied.report;
    if let Some(exports) = exports.as_deref() {
        match scanner::scan(&mut db, exports, cutoff) {
            Ok(legacy) => {
                report.imported += legacy.imported;
                report.duplicates += legacy.duplicates;
                report.unsupported += legacy.unsupported;
                report.malformed += legacy.malformed;
                report.unreadable += legacy.unreadable;
                report.counter_resets += legacy.skipped_streams;
            }
            Err(message) => {
                report.unreadable += 1;
                let _ = message;
            }
        }
    }
    db.execute("DELETE FROM usage_events WHERE occurred_at<?1", [cutoff])
        .map_err(|_| "清理过期统计失败")?;
    db.execute(
        "INSERT INTO sync_cursors(source_file,file_identity,byte_offset,state,updated_at) VALUES ('usage-last-scan','usage-report',0,?1,unixepoch()) ON CONFLICT(source_file) DO UPDATE SET state=excluded.state,updated_at=excluded.updated_at",
        [serde_json::to_string(&report).map_err(|_| "报告序列化失败")?],
    )
    .map_err(|_| "保存采集报告失败")?;
    Ok(report)
}

#[tauri::command]
pub async fn scan_usage(store: tauri::State<'_, Store>) -> Result<ScanReport, String> {
    run_scan(&store).await
}

async fn run_scan(store: &Store) -> Result<ScanReport, String> {
    if SCANNING.swap(true, Ordering::SeqCst) {
        return Err("已有采集任务在进行中".into());
    }
    let _guard = ScanGuard;
    scan_once(store).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsagePayload {
    pub settings: UsageSettings,
    pub notice: Option<String>,
    pub report: Option<ScanReport>,
    pub page: aggregate::Page,
    pub scanning: bool,
}

#[tauri::command]
pub async fn load_usage(
    store: tauri::State<'_, Store>,
    filter: aggregate::Filter,
    limit: i64,
    offset: i64,
) -> Result<UsagePayload, String> {
    let (settings, notice) = effective_settings(&store)?;
    let mut db = store.db.lock().map_err(|_| "数据库繁忙")?;
    super::reference::fill_unpriced(&mut db)?;
    let page = aggregate::page(&db, &filter, limit, offset)?;
    let report = ingest::last_report(&db)?;
    Ok(UsagePayload {
        settings,
        notice,
        report,
        page,
        scanning: SCANNING.load(Ordering::SeqCst),
    })
}

fn validate_settings(settings: &UsageSettings) -> Result<(), String> {
    if !(1..=3650).contains(&settings.retention_days) {
        return Err("保留期需为 1 至 3650 天".into());
    }
    if settings.path.trim().is_empty() {
        return Err("会话目录不能为空".into());
    }
    crate::config::path::resolve(&settings.path)?;
    if !settings.logs_path.trim().is_empty() {
        crate::config::path::resolve(&settings.logs_path)?;
    }
    if !settings.exports_path.trim().is_empty() {
        crate::config::path::resolve(&settings.exports_path)?;
    }
    Ok(())
}

#[tauri::command]
pub fn save_usage_settings(
    store: tauri::State<'_, Store>,
    settings: UsageSettings,
    revision: u64,
) -> Result<Workspace, String> {
    validate_settings(&settings)?;
    store.edit(revision, |w| {
        w.usage_settings = settings;
        Ok(())
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearResult {
    pub removed: usize,
    pub boundary: i64,
}

/// Deletes derived statistics before a boundary. The ledger keeps its cumulative
/// baseline so a later scan only counts new consumption, and the recorded
/// boundary stops cleared history from being imported again.
fn clear_before(store: &Store, before: i64) -> Result<usize, String> {
    let mut db = store.db.lock().map_err(|_| "数据库繁忙")?;
    let tx = db.transaction().map_err(|_| "无法开始清理事务")?;
    let removed = tx
        .execute("DELETE FROM usage_events WHERE occurred_at<?1", [before])
        .map_err(|_| "清理统计失败")?;
    tx.execute("INSERT INTO sync_cursors(source_file,file_identity,byte_offset,state,updated_at) VALUES ('usage-cleanup-cutoff','retention',?1,'{}',unixepoch()) ON CONFLICT(source_file) DO UPDATE SET byte_offset=max(byte_offset,excluded.byte_offset)",[before]).map_err(|_|"保存清理边界失败")?;
    tx.commit().map_err(|_| "提交清理失败")?;
    Ok(removed)
}

#[tauri::command]
pub fn clear_usage(
    store: tauri::State<'_, Store>,
    before: i64,
    confirmed: bool,
) -> Result<ClearResult, String> {
    if !confirmed {
        return Err("清理统计需要确认".into());
    }
    if before > now() {
        return Err("清理截止时间不能晚于当前时间".into());
    }
    Ok(ClearResult {
        removed: clear_before(&store, before)?,
        boundary: before,
    })
}

#[tauri::command]
pub fn export_usage(
    store: tauri::State<'_, Store>,
    filter: aggregate::Filter,
) -> Result<aggregate::Export, String> {
    let db = store.db.lock().map_err(|_| "数据库繁忙")?;
    aggregate::export_csv(&db, &filter)
}

#[tauri::command]
pub fn delete_price(store: tauri::State<'_, Store>, id: String) -> Result<(), String> {
    let db = store.db.lock().map_err(|_| "数据库繁忙")?;
    let removed = db
        .execute("DELETE FROM usage_prices WHERE id=?1", [id])
        .map_err(|_| "删除单价失败")?;
    if removed == 0 {
        return Err("单价不存在".into());
    }
    Ok(())
}

#[tauri::command]
pub fn usage_prices(store: tauri::State<'_, Store>) -> Result<Vec<pricing::StoredPrice>, String> {
    let db = store.db.lock().map_err(|_| "数据库繁忙")?;
    pricing::list_prices(&db)
}

struct ScanGuard;
impl Drop for ScanGuard {
    fn drop(&mut self) {
        SCANNING.store(false, Ordering::SeqCst);
    }
}

/// The worker is tied to the desktop process, not to the usage page being open.
pub fn start_auto_scan(app: tauri::AppHandle) {
    use tauri::{Emitter, Manager};
    std::thread::spawn(move || loop {
        let store = app.state::<Store>();
        if store.load().is_ok_and(|w| w.usage_settings.auto_scan) {
            let result = tauri::async_runtime::block_on(run_scan(&store));
            let _ = app.emit("usage-scan-finished", result.as_ref().err().cloned());
        }
        std::thread::sleep(std::time::Duration::from_secs(60));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::Store;

    fn store(dir: &std::path::Path) -> Store {
        Store::open(dir).unwrap()
    }

    #[test]
    fn legacy_default_path_is_replaced_only_when_it_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        store
            .edit(0, |w| {
                w.usage_settings.path = LEGACY_DEFAULT_PATH.into();
                Ok(())
            })
            .unwrap();
        let (settings, notice) = effective_settings(&store).unwrap();
        assert_eq!(settings.path, DEFAULT_SESSIONS);
        assert!(notice.is_some());
        // A stored custom path is respected.
        store
            .edit(1, |w| {
                w.usage_settings.path = dir.path().to_string_lossy().into();
                Ok(())
            })
            .unwrap();
        let (settings, notice) = effective_settings(&store).unwrap();
        assert_eq!(settings.path, dir.path().to_string_lossy());
        assert!(notice.is_none());
    }

    #[test]
    fn settings_validation_rejects_impossible_values() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        let settings = UsageSettings {
            retention_days: 0,
            ..Default::default()
        };
        assert!(validate_settings(&settings).is_err());
        let settings = UsageSettings {
            path: "  ".into(),
            ..Default::default()
        };
        assert!(validate_settings(&settings).is_err());
        let settings = UsageSettings::default();
        assert!(validate_settings(&settings).is_ok());
        save_usage_settings_inner(&store, settings, 0).unwrap();
        assert_eq!(store.load().unwrap().usage_settings.path, DEFAULT_SESSIONS);
    }

    #[test]
    fn clearing_records_a_boundary_and_removes_only_older_records() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        {
            let db = store.db.lock().unwrap();
            db.execute("INSERT INTO usage_events(id,source_file,event_key,session_id,occurred_at,source,input_tokens) VALUES ('a','log','k','s',10,'api',5)", []).unwrap();
            db.execute("INSERT INTO usage_events(id,source_file,event_key,session_id,occurred_at,source,input_tokens) VALUES ('b','log','k2','s',100,'api',7)", []).unwrap();
        }
        assert_eq!(clear_before(&store, 50).unwrap(), 1);
        let db = store.db.lock().unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM usage_events", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            db.query_row(
                "SELECT byte_offset FROM sync_cursors WHERE source_file='usage-cleanup-cutoff'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            50
        );
        drop(db);
        assert_eq!(clear_before(&store, 10).unwrap(), 0);
        let db = store.db.lock().unwrap();
        assert_eq!(
            db.query_row(
                "SELECT byte_offset FROM sync_cursors WHERE source_file='usage-cleanup-cutoff'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            50,
            "an earlier boundary must not move the cutoff backwards"
        );
    }

    fn save_usage_settings_inner(
        store: &Store,
        settings: UsageSettings,
        revision: u64,
    ) -> Result<Workspace, String> {
        validate_settings(&settings)?;
        store.edit(revision, |w| {
            w.usage_settings = settings;
            Ok(())
        })
    }
}
