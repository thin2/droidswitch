use super::{
    cost::Price,
    scanner::{self, ScanReport},
};
use crate::workspace::{Store, Workspace};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSettings {
    pub path: String,
    pub retention_days: u32,
}
impl Default for UsageSettings {
    fn default() -> Self {
        Self {
            path: "~/.factory/projects".into(),
            retention_days: 90,
        }
    }
}
pub fn price_at(
    db: &Connection,
    model: Option<&str>,
    time: i64,
) -> Result<Option<(String, String, Price)>, String> {
    let Some(model) = model else { return Ok(None) };
    db.query_row("SELECT id,currency,input_per_million,cache_read_per_million,cache_write_per_million,output_per_million,effective_at FROM model_prices WHERE model_id=?1 AND effective_at<=?2 ORDER BY effective_at DESC,id DESC LIMIT 1",rusqlite::params![model,time],|r|{let id:String=r.get(0)?;let currency:String=r.get(1)?;Ok((id,currency.clone(),Price{model:model.into(),currency,input:r.get(2)?,cache_read:r.get(3)?,cache_write:r.get(4)?,output:r.get(5)?,effective_at:r.get(6)?}))}).optional().map_err(|_|"无法读取历史单价".into())
}
#[tauri::command]
pub fn save_price(store: tauri::State<'_, Store>, price: Price) -> Result<(), String> {
    price.validate()?;
    let db = store.db.lock().map_err(|_| "数据库繁忙")?;
    db.execute("INSERT INTO model_prices(id,model_id,currency,input_per_million,cache_read_per_million,cache_write_per_million,output_per_million,source,effective_at) VALUES (?1,?2,?3,?4,?5,?6,?7,'manual',?8)",rusqlite::params![uuid::Uuid::new_v4().to_string(),price.model,price.currency,price.input,price.cache_read,price.cache_write,price.output,price.effective_at]).map_err(|_|"单价保存失败，同一模型/币种/生效时间不能重复")?;
    Ok(())
}
#[tauri::command]
pub fn save_usage_settings(
    store: tauri::State<'_, Store>,
    settings: UsageSettings,
    revision: u64,
) -> Result<Workspace, String> {
    if !(1..=3650).contains(&settings.retention_days) {
        return Err("保留期需为 1 至 3650 天".into());
    }
    crate::config::path::resolve(&settings.path)?;
    store.edit(revision, |w| {
        w.usage_settings = settings;
        Ok(())
    })
}
#[tauri::command]
pub async fn scan_usage(store: tauri::State<'_, Store>) -> Result<ScanReport, String> {
    let settings = store.load()?.usage_settings;
    let path = crate::config::path::resolve(&settings.path)?;
    let mut db = store.db.lock().map_err(|_| "数据库繁忙")?;
    let removed: Option<i64> = db
        .query_row(
            "SELECT byte_offset FROM sync_cursors WHERE source_file='usage-cleanup-cutoff'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| "无法读取清理边界")?;
    let cutoff = (now() - i64::from(settings.retention_days) * 86400).max(removed.unwrap_or(0));
    let report = scanner::scan(&mut db, &path, cutoff)?;
    db.execute("INSERT INTO sync_cursors(source_file,file_identity,byte_offset,state,updated_at) VALUES ('usage-last-scan','usage-report',0,?1,unixepoch()) ON CONFLICT(source_file) DO UPDATE SET state=excluded.state,updated_at=excluded.updated_at",[serde_json::to_string(&report).map_err(|_|"报告序列化失败")?]).map_err(|_|"保存采集报告失败")?;
    db.execute("DELETE FROM usage_events WHERE occurred_at<?1", [cutoff])
        .map_err(|_| "清理过期统计失败")?;
    Ok(report)
}
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageEvent {
    pub id: String,
    pub session: String,
    pub occurred_at: i64,
    pub source: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub input: Option<i64>,
    pub cache_read: Option<i64>,
    pub cache_write: Option<i64>,
    pub output: Option<i64>,
    pub credits: Option<f64>,
    pub cost: Option<String>,
    pub currency: Option<String>,
    pub kind: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageData {
    pub events: Vec<UsageEvent>,
    pub report: Option<ScanReport>,
    pub truncated: bool,
}
#[tauri::command]
pub async fn load_usage(
    store: tauri::State<'_, Store>,
    from: i64,
    to: i64,
) -> Result<UsageData, String> {
    if to < from {
        return Err("时间范围无效".into());
    }
    let db = store.db.lock().map_err(|_| "数据库繁忙")?;
    let mut statement=db.prepare("SELECT id,session_id,occurred_at,source,provider_id_at_event,model_id_at_event,input_tokens,cache_read_tokens,cache_write_tokens,output_tokens,official_credits,estimated_cost,currency,event_key FROM usage_events WHERE occurred_at>=?1 AND occurred_at<?2 ORDER BY occurred_at DESC LIMIT 50001").map_err(|_|"无法读取统计")?;
    let rows = statement
        .query_map(rusqlite::params![from, to], |r| {
            let key: String = r.get(13)?;
            Ok(UsageEvent {
                id: r.get(0)?,
                session: r.get(1)?,
                occurred_at: r.get(2)?,
                source: r.get(3)?,
                provider: r.get(4)?,
                model: r.get(5)?,
                input: r.get(6)?,
                cache_read: r.get(7)?,
                cache_write: r.get(8)?,
                output: r.get(9)?,
                credits: r.get(10)?,
                cost: r.get(11)?,
                currency: r.get(12)?,
                kind: if key.starts_with("cumulative:") {
                    "累计差额"
                } else {
                    "回合汇总"
                }
                .into(),
            })
        })
        .map_err(|_| "读取统计失败")?;
    let mut events: Vec<_> = rows
        .collect::<Result<_, _>>()
        .map_err(|_| "用量记录结构无效")?;
    let truncated = events.len() > 50000;
    events.truncate(50000);
    let report: Option<String> = db
        .query_row(
            "SELECT state FROM sync_cursors WHERE source_file='usage-last-scan'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| "读取采集报告失败")?;
    Ok(UsageData {
        events,
        report: report
            .map(|s| serde_json::from_str(&s))
            .transpose()
            .map_err(|_| "采集报告结构无效")?,
        truncated,
    })
}
#[tauri::command]
pub fn clear_usage(
    store: tauri::State<'_, Store>,
    before: i64,
    confirmed: bool,
) -> Result<usize, String> {
    if !confirmed {
        return Err("清理统计需要确认".into());
    }
    if before > now() {
        return Err("清理截止时间不能晚于当前时间".into());
    }
    let mut db = store.db.lock().map_err(|_| "数据库繁忙")?;
    let tx = db.transaction().map_err(|_| "无法开始清理事务")?;
    let count = tx
        .execute("DELETE FROM usage_events WHERE occurred_at<?1", [before])
        .map_err(|_| "清理统计失败")?;
    tx.execute("INSERT INTO sync_cursors(source_file,file_identity,byte_offset,state,updated_at) VALUES ('usage-cleanup-cutoff','retention',?1,'{}',unixepoch()) ON CONFLICT(source_file) DO UPDATE SET byte_offset=max(byte_offset,excluded.byte_offset)",[before]).map_err(|_|"保存清理边界失败")?;
    tx.commit().map_err(|_| "提交清理失败")?;
    Ok(count)
}
