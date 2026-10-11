use super::cost::Tokens;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub event_id: String,
    pub session_id: String,
    pub occurred_at: i64,
    #[serde(default)]
    pub branch: String,
    #[serde(default)]
    pub generation: String,
    pub scope: String,
    #[serde(default)]
    pub source: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub tokens: Tokens,
    pub input_semantics: String,
    pub official_credits: Option<f64>,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub files: usize,
    pub imported: usize,
    pub duplicates: usize,
    pub unsupported: usize,
    pub malformed: usize,
    pub unreadable: usize,
    pub skipped_streams: usize,
    pub updated_at: u64,
}
pub fn parse(line: &str) -> Result<Option<Record>, String> {
    let v: Value = serde_json::from_str(line).map_err(|_| "invalid JSON")?;
    let record = if v.get("schema").and_then(Value::as_str) == Some("droid-switch-usage-v1") {
        serde_json::from_value::<Record>(v).map_err(|_| "invalid usage metadata")?
    } else if v.get("type").and_then(Value::as_str) == Some("result")
        && v.get("tokenUsage").is_some()
    {
        // SDK exports are per turn, not lifetime totals. Identity/time must be supplied by the export.
        let id = v.get("id").or(v.get("turnId")).and_then(Value::as_str);
        let session = v.get("sessionId").and_then(Value::as_str);
        let time = v.get("timestamp").and_then(|t| {
            t.as_i64().or_else(|| {
                t.as_str().and_then(|s| {
                    chrono::DateTime::parse_from_rfc3339(s)
                        .ok()
                        .map(|d| d.timestamp())
                })
            })
        });
        let (Some(id), Some(session), Some(time)) = (id, session, time) else {
            return Ok(None);
        };
        let u = &v["tokenUsage"];
        Record {
            event_id: id.into(),
            session_id: session.into(),
            occurred_at: time,
            branch: String::new(),
            generation: String::new(),
            scope: "request".into(),
            source: None,
            provider: None,
            model: v.get("modelId").and_then(Value::as_str).map(str::to_owned),
            tokens: Tokens {
                input: u.get("inputTokens").and_then(Value::as_u64),
                cache_read: u.get("cacheReadTokens").and_then(Value::as_u64),
                cache_write: u.get("cacheCreationTokens").and_then(Value::as_u64),
                output: u.get("outputTokens").and_then(Value::as_u64),
            },
            input_semantics: "unknown".into(),
            official_credits: u.get("factoryCredits").and_then(Value::as_f64),
        }
    } else {
        return Ok(None);
    };
    if record.event_id.is_empty()
        || record.session_id.is_empty()
        || !matches!(record.scope.as_str(), "request" | "cumulative")
        || !matches!(
            record.input_semantics.as_str(),
            "exclusive" | "inclusive" | "unknown"
        )
        || record.occurred_at < 0
        || record.occurred_at > 32_503_680_000
        || record
            .official_credits
            .is_some_and(|v| !v.is_finite() || v < 0.)
    {
        return Err("invalid metadata".into());
    }
    if [
        &record.tokens.input,
        &record.tokens.cache_read,
        &record.tokens.cache_write,
        &record.tokens.output,
    ]
    .into_iter()
    .flatten()
    .any(|n| *n > i64::MAX as u64)
    {
        return Err("token overflow".into());
    }
    Ok(Some(record))
}
fn normalize(mut tokens: Tokens, semantics: &str) -> Tokens {
    tokens.input = match semantics {
        "exclusive" => tokens.input,
        "inclusive" => tokens
            .input
            .zip(tokens.cache_read)
            .zip(tokens.cache_write)
            .and_then(|((i, r), w)| i.checked_sub(r)?.checked_sub(w)),
        _ => None,
    };
    tokens
}
fn delta(now: Option<u64>, old: Option<u64>) -> Option<u64> {
    now.zip(old).and_then(|(n, o)| n.checked_sub(o))
}
fn walk(path: &Path, depth: usize, out: &mut Vec<PathBuf>) -> Result<(), String> {
    if depth > 12 || out.len() > 10000 {
        return Err("扫描目录过大，请指定更小的会话目录".into());
    }
    let meta = std::fs::symlink_metadata(path).map_err(|_| "无法访问采集路径")?;
    if meta.is_symlink() {
        return Ok(());
    }
    if meta.is_file() {
        if path
            .extension()
            .is_some_and(|e| e == "jsonl" || e == "ndjson")
        {
            out.push(path.into());
        }
        return Ok(());
    }
    for entry in std::fs::read_dir(path).map_err(|_| "无法读取采集目录")? {
        walk(
            &entry.map_err(|_| "无法读取采集条目")?.path(),
            depth + 1,
            out,
        )?;
    }
    Ok(())
}
pub fn scan(db: &mut Connection, path: &Path, cutoff: i64) -> Result<ScanReport, String> {
    let mut paths = vec![];
    walk(path, 0, &mut paths)?;
    paths.sort();
    let mut report = ScanReport::default();
    let mut records = vec![];
    let mut total = 0usize;
    for path in paths {
        report.files += 1;
        let snapshot = match crate::fs::read_snapshot(&path) {
            Ok(s) => s,
            Err(_) => {
                report.unreadable += 1;
                continue;
            }
        };
        total += snapshot.text.len();
        if total > 128 * 1024 * 1024 {
            return Err("扫描内容超过 128 MiB，请缩小目录范围".into());
        }
        for line in snapshot.text.lines().filter(|l| !l.trim().is_empty()) {
            match parse(line) {
                Ok(Some(r)) => records.push((r, path.to_string_lossy().to_string())),
                Ok(None) => report.unsupported += 1,
                Err(_) => report.malformed += 1,
            }
        }
    }
    records.sort_by_key(|(r, _)| r.occurred_at);
    let tx = db.transaction().map_err(|_| "无法创建采集事务")?;
    for (r, file) in records {
        if r.occurred_at < cutoff {
            continue;
        }
        let stream = crate::config::pipeline::hash(
            format!("{}\0{}\0{}", r.session_id, r.branch, r.generation).as_bytes(),
        );
        let id = crate::config::pipeline::hash(format!("{stream}\0{}", r.event_id).as_bytes());
        let exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM usage_events WHERE id=?1)",
                [&id],
                |row| row.get(0),
            )
            .map_err(|_| "无法检查重复事件")?;
        if exists {
            report.duplicates += 1;
            continue;
        }
        let cursor = format!("usage-stream:{stream}");
        let state: Option<String> = tx
            .query_row(
                "SELECT state FROM sync_cursors WHERE source_file=?1",
                [&cursor],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| "无法读取累计状态")?;
        let state: Value = state
            .map(|v| serde_json::from_str(&v))
            .transpose()
            .map_err(|_| "累计状态损坏")?
            .unwrap_or(json!({}));
        if state
            .get("scope")
            .and_then(Value::as_str)
            .is_some_and(|s| s != r.scope)
        {
            report.skipped_streams += 1;
            continue;
        }
        let mut tokens = normalize(r.tokens.clone(), &r.input_semantics);
        let mut credits = r.official_credits;
        if r.scope == "cumulative" {
            if let Some(previous) = state.get("tokens") {
                if state["time"].as_i64().is_some_and(|t| t >= r.occurred_at) {
                    report.skipped_streams += 1;
                    continue;
                }
                let old: Tokens =
                    serde_json::from_value(previous.clone()).map_err(|_| "累计状态结构无效")?;
                let decreased = [
                    (tokens.input, old.input),
                    (tokens.cache_read, old.cache_read),
                    (tokens.cache_write, old.cache_write),
                    (tokens.output, old.output),
                ]
                .iter()
                .any(|(n, o)| n.zip(*o).is_some_and(|(n, o)| n < o));
                if decreased {
                    report.skipped_streams += 1;
                    continue;
                }
                tokens = Tokens {
                    input: delta(tokens.input, old.input),
                    cache_read: delta(tokens.cache_read, old.cache_read),
                    cache_write: delta(tokens.cache_write, old.cache_write),
                    output: delta(tokens.output, old.output),
                };
                credits = credits
                    .zip(state.get("credits").and_then(Value::as_f64))
                    .and_then(|(n, o)| (n >= o).then_some(n - o));
            }
        }
        let source = match r.source.as_deref() {
            Some("official") => "official",
            Some("api") => "api",
            _ => "unknown",
        };
        let price: Option<(String, String, super::cost::Price)> = if source == "api" {
            super::commands::price_at(&tx, r.model.as_deref(), r.occurred_at)?
        } else {
            None
        };
        let (estimated, currency, price_id) = match price {
            Some((id, currency, p)) => (
                super::cost::estimate(&tokens, &p)?,
                Some(currency),
                Some(id),
            ),
            None => (None, None, None),
        };
        tx.execute("INSERT INTO usage_events(id,source_file,event_key,session_id,occurred_at,source,provider_id_at_event,model_id_at_event,input_tokens,cache_read_tokens,cache_write_tokens,output_tokens,official_credits,estimated_cost,currency,price_id_at_event) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",rusqlite::params![id,file,format!("{}:{id}",r.scope),r.session_id,r.occurred_at,source,r.provider,r.model,tokens.input.map(|n|n as i64),tokens.cache_read.map(|n|n as i64),tokens.cache_write.map(|n|n as i64),tokens.output.map(|n|n as i64),credits,estimated,currency,price_id]).map_err(|_|"保存用量事件失败")?;
        let raw = normalize(r.tokens, &r.input_semantics);
        let state =
            json!({"scope":r.scope,"tokens":raw,"time":r.occurred_at,"credits":r.official_credits});
        tx.execute("INSERT INTO sync_cursors(source_file,file_identity,byte_offset,state,updated_at) VALUES (?1,'usage-stream',0,?2,unixepoch()) ON CONFLICT(source_file) DO UPDATE SET state=excluded.state,updated_at=excluded.updated_at",rusqlite::params![cursor,state.to_string()]).map_err(|_|"保存累计状态失败")?;
        report.imported += 1;
    }
    report.updated_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    tx.commit().map_err(|_| "提交用量事件失败")?;
    Ok(report)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn record(id: &str, time: i64, input: u64) -> Value {
        json!({"schema":"droid-switch-usage-v1","eventId":id,"sessionId":"session","occurredAt":time,"scope":"cumulative","source":"api","model":"m","provider":"p","tokens":{"input":input,"cacheRead":0,"cacheWrite":0,"output":input},"inputSemantics":"exclusive"})
    }
    #[test]
    fn usage_scan_replay_rotation_cumulative_and_unknown_are_safe() {
        let d = tempfile::tempdir().unwrap();
        let log = d.path().join("usage.jsonl");
        let dbpath = d.path().join("stats.db");
        let mut db = crate::db::open(&dbpath).unwrap();
        let text = format!("{}\n{}\n", record("a", 1, 10), record("b", 2, 15));
        std::fs::write(&log, &text).unwrap();
        assert_eq!(scan(&mut db, d.path(), 0).unwrap().imported, 2);
        assert_eq!(scan(&mut db, d.path(), 0).unwrap().imported, 0);
        std::fs::write(d.path().join("rotated.jsonl"), &text).unwrap();
        assert_eq!(scan(&mut db, d.path(), 0).unwrap().imported, 0);
        assert_eq!(
            db.query_row("SELECT sum(input_tokens) FROM usage_events", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            15
        );
        let reset = record("c", 3, 1);
        std::fs::write(&log, format!("{text}{reset}\n")).unwrap();
        assert_eq!(scan(&mut db, d.path(), 0).unwrap().skipped_streams, 1);
        assert!(normalize(
            Tokens {
                input: Some(10),
                ..Default::default()
            },
            "unknown"
        )
        .input
        .is_none());
        assert!(parse(r#"{"type":"assistant","content":"private code"}"#)
            .unwrap()
            .is_none());
    }
    #[test]
    fn usage_scan_invalid_tokens_and_unknown_timestamps_not_fabricated() {
        let mut v = record("a", 1, 10);
        v["tokens"]["input"] = json!(-1);
        assert!(parse(&v.to_string()).is_err());
        assert!(
            parse(r#"{"type":"result","sessionId":"s","tokenUsage":{"inputTokens":12}}"#)
                .unwrap()
                .is_none()
        );
    }
}
