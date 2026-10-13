//! Ingestion for the native usage sources.
//!
//! Both sources are read outside the database lock; only the write phase holds
//! it, and that phase is a single transaction in three ordered stages: session
//! identities, request detail, then ledger reconciliation. Requests must be
//! stored before reconciliation so a session baseline only fills the part that
//! detail does not already account for.
//!
//! The ledger rule: a session's cumulative snapshot is authoritative and request
//! detail is a subset of it, so stored detail can never exceed the confirmed
//! total.

use super::{cost, identity, logs, native, pricing};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const SESSION_SCOPE: &str = "native-session";
const LOG_SCOPE: &str = "native-log";
const MAX_SCAN_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub log_files: usize,
    pub log_requests: usize,
    pub log_unpaired: usize,
    pub log_inconsistent: usize,
    pub session_files: usize,
    pub sessions: usize,
    pub snapshots: usize,
    pub baselines: usize,
    pub deltas: usize,
    pub counter_resets: usize,
    pub unsupported: usize,
    pub malformed: usize,
    pub unreadable: usize,
    pub duplicates: usize,
    pub imported: usize,
    pub updated_at: i64,
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Stamp {
    pub size: u64,
    pub modified_ns: i64,
}

impl Stamp {
    pub fn identity(&self) -> String {
        format!("{}:{}", self.size, self.modified_ns)
    }
    pub fn parse(text: &str) -> Option<Self> {
        let (size, modified_ns) = text.split_once(':')?;
        Some(Self {
            size: size.parse().ok()?,
            modified_ns: modified_ns.parse().ok()?,
        })
    }
}

fn stamp(path: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let modified_ns = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos() as i64;
    Some(Stamp {
        size: meta.len(),
        modified_ns,
    })
}

/// Read complete lines from `offset`, leaving a half-written trailing line for
/// the next pass. Returns the text and the number of bytes it covers.
fn read_tail(path: &Path, offset: u64, budget: &mut u64) -> Result<(String, u64), String> {
    let mut file = std::fs::File::open(path).map_err(|_| "无法读取用量文件")?;
    let length = file.metadata().map_err(|_| "无法读取文件长度")?.len();
    let offset = offset.min(length);
    file.seek(SeekFrom::Start(offset))
        .map_err(|_| "无法定位文件")?;
    let allowed = (length - offset).min(*budget).min(32 * 1024 * 1024);
    let mut bytes = Vec::with_capacity(allowed as usize);
    file.take(allowed)
        .read_to_end(&mut bytes)
        .map_err(|_| "无法读取文件")?;
    *budget = budget.saturating_sub(bytes.len() as u64);
    let complete = bytes
        .iter()
        .rposition(|b| *b == b'\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let text = String::from_utf8_lossy(&bytes[..complete]).into_owned();
    Ok((text, complete as u64))
}

fn prefix_hash(path: &Path, offset: u64) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?;
    if f.metadata().ok()?.len() < offset {
        return None;
    }
    let mut bytes = vec![0; offset.min(4096) as usize];
    f.read_exact(&mut bytes).ok()?;
    if offset > 4096 {
        f.seek(SeekFrom::Start(offset.saturating_sub(4096))).ok()?;
        let mut tail = vec![0; 4096];
        f.read_exact(&mut tail).ok()?;
        bytes.extend(tail);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let m = f.metadata().ok()?;
        bytes.extend(m.ino().to_le_bytes());
        bytes.extend(m.dev().to_le_bytes());
    }
    Some(crate::config::pipeline::hash(&bytes))
}

/// Diagnostic logs oldest first, with the live file last so pairing stays ordered.
pub fn log_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut live = vec![];
    let mut rotated = vec![];
    for entry in std::fs::read_dir(root).map_err(|_| "无法读取日志目录")? {
        let path = entry.map_err(|_| "无法读取日志条目")?.path();
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        if !name.starts_with("droid-log-single.log") || path.is_dir() {
            continue;
        }
        if name == "droid-log-single.log" {
            live.push(path);
        } else {
            rotated.push(path);
        }
    }
    rotated.sort();
    rotated.extend(live);
    Ok(rotated)
}

#[derive(Clone, Debug, PartialEq)]
pub struct SessionUpdate {
    pub uuid: String,
    pub project_key: String,
    pub first_activity: Option<i64>,
    pub last_activity: Option<i64>,
    pub transcript_messages: Option<u64>,
    pub observed_at: Option<i64>,
    pub changed: bool,
}

pub struct Inputs {
    pub session_updates: Vec<SessionUpdate>,
    pub snapshots: Vec<native::Snapshot>,
    pub requests: Vec<logs::Request>,
    pub report: ScanReport,
}

#[derive(Default)]
pub struct Offsets {
    pub settings: BTreeMap<String, Stamp>,
    pub transcripts: BTreeMap<String, native::ActivityState>,
    pub logs: BTreeMap<String, u64>,
    pub log_prefixes: BTreeMap<String, String>,
    pub pairing: logs::Pairing,
}

/// Read every source without touching the database.
pub fn collect(
    sessions_root: Option<&Path>,
    logs_root: Option<&Path>,
    offsets: &mut Offsets,
) -> Inputs {
    let mut budget = MAX_SCAN_BYTES;
    let mut inputs = Inputs {
        session_updates: vec![],
        snapshots: vec![],
        requests: vec![],
        report: ScanReport::default(),
    };
    if let Some(root) = sessions_root {
        collect_sessions(root, offsets, &mut inputs, &mut budget);
    }
    if let Some(root) = logs_root {
        collect_logs(root, offsets, &mut inputs, &mut budget);
    }
    inputs
}

fn collect_sessions(root: &Path, offsets: &mut Offsets, inputs: &mut Inputs, budget: &mut u64) {
    let files = match native::discover(root) {
        Ok(files) => files,
        Err(_) => {
            inputs.report.unreadable += 1;
            return;
        }
    };
    for entry in files {
        inputs.report.session_files +=
            usize::from(entry.settings.is_some()) + usize::from(entry.transcript.is_some());
        let mut update = SessionUpdate {
            uuid: entry.session_uuid.clone(),
            project_key: entry.project_key.clone(),
            first_activity: None,
            last_activity: None,
            transcript_messages: None,
            observed_at: None,
            changed: false,
        };
        if let Some(path) = entry.settings.as_ref() {
            let key = path.to_string_lossy().into_owned();
            let current = stamp(path);
            let previous = offsets.settings.get(&key).cloned();
            if current != previous {
                update.changed = true;
                match std::fs::read(path) {
                    Ok(bytes) => match String::from_utf8(bytes) {
                        Ok(text) => {
                            let observed = stamp(path)
                                .map(|s| s.modified_ns / 1_000_000_000)
                                .unwrap_or(0);
                            update.observed_at = Some(observed);
                            match native::parse_snapshot(
                                &text,
                                &entry.session_uuid,
                                &entry.project_key,
                                observed,
                            ) {
                                Ok(Some(snapshot)) => {
                                    inputs.report.snapshots += 1;
                                    inputs.snapshots.push(snapshot);
                                    if let Some(stamp) = current.clone() {
                                        offsets.settings.insert(key.clone(), stamp);
                                    }
                                }
                                Ok(None) => inputs.report.unsupported += 1,
                                Err(_) => inputs.report.malformed += 1,
                            }
                        }
                        Err(_) => inputs.report.malformed += 1,
                    },
                    Err(_) => inputs.report.unreadable += 1,
                }
            }
        }
        if let Some(path) = entry.transcript.as_ref() {
            let key = path.to_string_lossy().into_owned();
            let mut state = offsets.transcripts.get(&key).cloned().unwrap_or_default();
            let size = stamp(path).map(|s| s.size).unwrap_or(0);
            if size < state.offset {
                // Truncated or replaced: recount from the beginning.
                state.offset = 0;
                state.messages = 0;
                update.changed = true;
                inputs.report.counter_resets += 1;
            }
            if size > state.offset {
                match read_tail(path, state.offset, budget) {
                    Ok((text, _)) => {
                        let activity = native::parse_activity(&text);
                        inputs.report.malformed += activity.malformed as usize;
                        native::merge_activity(&mut state, &activity);
                        update.changed = true;
                    }
                    Err(_) => inputs.report.unreadable += 1,
                }
            }
            offsets.transcripts.insert(key, state.clone());
            update.first_activity = state.first;
            update.last_activity = state.last;
            update.transcript_messages = Some(state.messages);
        }
        inputs.session_updates.push(update);
    }
    inputs.report.sessions = inputs.session_updates.len();
}

fn collect_logs(root: &Path, offsets: &mut Offsets, inputs: &mut Inputs, budget: &mut u64) {
    let files = match log_files(root) {
        Ok(files) => files,
        Err(_) => {
            inputs.report.unreadable += 1;
            return;
        }
    };
    for path in files {
        let key = path.to_string_lossy().into_owned();
        let old_offset = offsets.logs.get(&key).copied().unwrap_or(0);
        let unchanged = offsets
            .log_prefixes
            .get(&key)
            .is_some_and(|h| prefix_hash(&path, old_offset).as_ref() == Some(h));
        let offset = if unchanged { old_offset } else { 0 };
        inputs.report.log_files += 1;
        match read_tail(&path, offset, budget) {
            Ok((text, consumed)) => {
                let next = offset + consumed;
                if let Some(hash) = prefix_hash(&path, next) {
                    offsets.log_prefixes.insert(key.clone(), hash);
                }
                offsets.logs.insert(key, next);
                for line in text.lines() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    match logs::parse_line(line) {
                        Some(parsed) => {
                            if let Some(request) = offsets.pairing.feed(parsed) {
                                inputs.report.log_requests += 1;
                                if request.endpoint.is_none() {
                                    inputs.report.log_unpaired += 1;
                                }
                                let components = [
                                    request.record.tokens.input,
                                    request.record.tokens.cache_read,
                                    request.record.tokens.cache_write,
                                ];
                                if let (Some(total), true) = (
                                    request.record.total_input,
                                    components.iter().all(Option::is_some),
                                ) {
                                    if components.into_iter().flatten().sum::<u64>() != total {
                                        inputs.report.log_inconsistent += 1;
                                    }
                                }
                                inputs.requests.push(request);
                            }
                        }
                        None => {
                            // Only records this parser claims to understand count
                            // as unrecognised; the log holds many other kinds.
                            if line.contains("Streaming result")
                                || line.contains("chat_client_time_to_first_token")
                            {
                                inputs.report.unsupported += 1;
                            }
                        }
                    }
                }
            }
            Err(_) => inputs.report.unreadable += 1,
        }
    }
}

struct Existing {
    snapshot_at: Option<i64>,
    own: cost::Tokens,
    credits: Option<f64>,
    child_sessions: u64,
}

fn read_existing(db: &Connection, id: &str) -> Result<Option<Existing>, String> {
    db.query_row(
        "SELECT snapshot_at,own_input_tokens,own_cache_read_tokens,own_cache_write_tokens,own_output_tokens,own_thinking_tokens,own_credits,child_sessions FROM usage_sessions WHERE id=?1",
        [id],
        |r| {
            Ok(Existing {
                snapshot_at: r.get(0)?,
                own: cost::Tokens {
                    input: r.get::<_, Option<i64>>(1)?.map(|v| v as u64),
                    cache_read: r.get::<_, Option<i64>>(2)?.map(|v| v as u64),
                    cache_write: r.get::<_, Option<i64>>(3)?.map(|v| v as u64),
                    output: r.get::<_, Option<i64>>(4)?.map(|v| v as u64),
                    thinking: r.get::<_, Option<i64>>(5)?.map(|v| v as u64),
                },
                credits: r.get(6)?,
                child_sessions: r.get::<_, i64>(7)? as u64,
            })
        },
    )
    .optional()
    .map_err(|_| "无法读取会话账本".into())
}

struct Covered {
    sums: cost::Tokens,
    unknown: [bool; 5],
}

/// Detail already stored for a session inside an interval. A counter with any
/// unknown value is unknown, never zero, so it cannot justify a fill-in record.
fn covered_since(
    db: &Connection,
    session_ref: &str,
    after: Option<i64>,
    through: Option<i64>,
) -> Result<Covered, String> {
    let mut sql = String::from(
        "SELECT COALESCE(SUM(input_tokens),0),COALESCE(SUM(cache_read_tokens),0),COALESCE(SUM(cache_write_tokens),0),COALESCE(SUM(output_tokens),0),COALESCE(SUM(thinking_tokens),0),COUNT(CASE WHEN input_tokens IS NULL THEN 1 END),COUNT(CASE WHEN cache_read_tokens IS NULL THEN 1 END),COUNT(CASE WHEN cache_write_tokens IS NULL THEN 1 END),COUNT(CASE WHEN output_tokens IS NULL THEN 1 END),COUNT(CASE WHEN thinking_tokens IS NULL THEN 1 END) FROM usage_events WHERE session_ref=?1",
    );
    if after.is_some() {
        sql.push_str(" AND occurred_at>?2");
    }
    if through.is_some() {
        sql.push_str(match after.is_some() {
            true => " AND occurred_at<=?3",
            false => " AND occurred_at<=?2",
        });
    }
    db.prepare(&sql)
        .map_err(|_| "无法汇总已存明细")?
        .query_row(
            rusqlite::params_from_iter(
                std::iter::once(rusqlite::types::Value::Text(session_ref.to_owned())).chain(
                    after
                        .into_iter()
                        .chain(through)
                        .map(rusqlite::types::Value::Integer),
                ),
            ),
            |r| {
                Ok(Covered {
                    sums: cost::Tokens {
                        input: Some(r.get::<_, i64>(0)? as u64),
                        cache_read: Some(r.get::<_, i64>(1)? as u64),
                        cache_write: Some(r.get::<_, i64>(2)? as u64),
                        output: Some(r.get::<_, i64>(3)? as u64),
                        thinking: Some(r.get::<_, i64>(4)? as u64),
                    },
                    unknown: [
                        r.get::<_, i64>(5)? > 0,
                        r.get::<_, i64>(6)? > 0,
                        r.get::<_, i64>(7)? > 0,
                        r.get::<_, i64>(8)? > 0,
                        r.get::<_, i64>(9)? > 0,
                    ],
                })
            },
        )
        .map_err(|_| "无法汇总已存明细".into())
}

/// The part of a total that no stored detail accounts for yet.
fn fill(total: &cost::Tokens, covered: &Covered) -> cost::Tokens {
    let diff = |total: Option<u64>, covered: Option<u64>, unknown: bool| -> Option<u64> {
        let total = total?;
        if unknown {
            return None;
        }
        Some(total.saturating_sub(covered?))
    };
    cost::Tokens {
        input: diff(total.input, covered.sums.input, covered.unknown[0]),
        cache_read: diff(
            total.cache_read,
            covered.sums.cache_read,
            covered.unknown[1],
        ),
        cache_write: diff(
            total.cache_write,
            covered.sums.cache_write,
            covered.unknown[2],
        ),
        output: diff(total.output, covered.sums.output, covered.unknown[3]),
        thinking: diff(total.thinking, covered.sums.thinking, covered.unknown[4]),
    }
}

struct Event {
    scope: &'static str,
    event_key: String,
    session_uuid: String,
    session_ref: Option<String>,
    occurred_at: i64,
    time_confidence: &'static str,
    granularity: &'static str,
    source: &'static str,
    provider_key: Option<String>,
    endpoint_id: Option<String>,
    selector: Option<String>,
    api_model: Option<String>,
    tokens: cost::Tokens,
    credits: Option<f64>,
    request_id: Option<String>,
    attempt: Option<u32>,
    interval: Option<(i64, i64)>,
    evidence: &'static str,
    file: String,
}

fn source_of(byok: Option<bool>) -> &'static str {
    match byok {
        Some(true) => "api",
        Some(false) => "official",
        None => "unknown",
    }
}

fn write_event(tx: &Connection, event: &Event) -> Result<bool, String> {
    let id = crate::config::pipeline::hash(
        format!(
            "{}\0{}\0{}",
            event.scope, event.session_uuid, event.event_key
        )
        .as_bytes(),
    );
    let exists: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM usage_events WHERE id=?1)",
            [&id],
            |r| r.get(0),
        )
        .map_err(|_| "无法检查重复记录")?;
    if exists {
        return Ok(false);
    }
    let priced = match (event.source, event.api_model.as_deref()) {
        ("api", Some(model)) => {
            pricing::price_at(tx, event.provider_key.as_deref(), model, event.occurred_at)?
        }
        _ => None,
    };
    let (estimated, currency, price_id, complete, price_snapshot) = match priced {
        Some((id, currency, price)) => {
            let estimate = cost::estimate(&event.tokens, &price)?;
            (
                estimate.cost,
                Some(currency),
                Some(id),
                estimate.complete,
                serde_json::to_string(&price).ok(),
            )
        }
        None => (None, None, None, false, None),
    };
    let cost_units = estimated.as_deref().and_then(cost::parse_cost);
    tx.execute(
        "INSERT INTO usage_events(id,source_file,event_key,session_id,occurred_at,source,provider_id_at_event,model_id_at_event,input_tokens,cache_read_tokens,cache_write_tokens,output_tokens,official_credits,estimated_cost,currency,price_id_at_event,granularity,time_confidence,session_ref,endpoint_id,model_selector,api_model_id,thinking_tokens,request_id,attempt,interval_start,interval_end,evidence,cost_units,cost_complete,price_snapshot) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31)",
        rusqlite::params![
            id,
            event.file,
            event.event_key,
            event.session_uuid,
            event.occurred_at,
            event.source,
            event.provider_key,
            event.selector.clone().or_else(|| event.api_model.clone()),
            event.tokens.input.map(|v| v as i64),
            event.tokens.cache_read.map(|v| v as i64),
            event.tokens.cache_write.map(|v| v as i64),
            event.tokens.output.map(|v| v as i64),
            event.credits,
            estimated,
            currency,
            price_id,
            event.granularity,
            event.time_confidence,
            event.session_ref,
            event.endpoint_id,
            event.selector,
            event.api_model,
            event.tokens.thinking.map(|v| v as i64),
            event.request_id,
            event.attempt,
            event.interval.map(|(start, _)| start),
            event.interval.map(|(_, end)| end),
            event.evidence,
            cost_units.map(i64::try_from).transpose().map_err(|_| "费用超出可存储范围")?,
            complete, price_snapshot,
        ],
    )
    .map_err(|_| "保存用量记录失败")?;
    Ok(true)
}

/// When late request detail arrives, replace the matching residual counters,
/// rather than adding it on top of the already imported cumulative baseline.
fn consume_residual(
    tx: &Connection,
    session: &str,
    at: i64,
    tokens: &cost::Tokens,
) -> Result<(), String> {
    let mut statement = tx.prepare("SELECT id,input_tokens,cache_read_tokens,cache_write_tokens,output_tokens,thinking_tokens FROM usage_events WHERE session_ref=?1 AND ((granularity='historical_baseline' AND occurred_at>=?2) OR (granularity='observed_delta' AND interval_start<?2 AND interval_end>=?2)) ORDER BY occurred_at,id").map_err(|_|"无法读取累计余量")?;
    let rows = statement
        .query_map(rusqlite::params![session, at], |r| {
            Ok((
                r.get::<_, String>(0)?,
                [
                    r.get::<_, Option<i64>>(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ],
            ))
        })
        .map_err(|_| "无法读取累计余量")?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "累计余量无效")?;
    let mut left = [
        tokens.input,
        tokens.cache_read,
        tokens.cache_write,
        tokens.output,
        tokens.thinking,
    ];
    for (id, mut values) in rows {
        for (value, remaining) in values.iter_mut().zip(left.iter_mut()) {
            if let (Some(v), Some(n)) = (value.as_mut(), remaining.as_mut()) {
                let used = (*v as u64).min(*n);
                *v -= used as i64;
                *n -= used;
            }
        }
        tx.execute("UPDATE usage_events SET input_tokens=?2,cache_read_tokens=?3,cache_write_tokens=?4,output_tokens=?5,thinking_tokens=?6 WHERE id=?1",rusqlite::params![id,values[0],values[1],values[2],values[3],values[4]]).map_err(|_|"无法调整累计余量")?;
        tx.execute("DELETE FROM usage_events WHERE id=?1 AND input_tokens=0 AND cache_read_tokens=0 AND cache_write_tokens=0 AND output_tokens=0 AND thinking_tokens=0 AND COALESCE(official_credits,0)=0",[id]).map_err(|_|"无法移除已覆盖累计")?;
    }
    Ok(())
}

pub struct Applied {
    pub report: ScanReport,
}

pub struct Cursor {
    pub key: String,
    pub identity: String,
    pub offset: u64,
    pub state: String,
}

/// Write everything collected in one transaction.
pub fn apply(
    db: &mut Connection,
    inputs: Inputs,
    offsets: &Offsets,
    cutoff: i64,
) -> Result<Applied, String> {
    let tx = db.transaction().map_err(|_| "无法创建采集事务")?;
    let mut report = inputs.report.clone();
    let configured = identity::endpoint_providers(&tx)?;
    let mut endpoint_keys: BTreeMap<String, String> = BTreeMap::new();
    let mut wanted: BTreeMap<String, String> = BTreeMap::new();
    for request in &inputs.requests {
        if let Some(endpoint) = request.endpoint.as_ref() {
            wanted.insert(endpoint.endpoint.clone(), endpoint.api_model.clone());
        }
    }
    for endpoint in wanted.keys() {
        let key = identity::touch_endpoint(&tx, endpoint, now())?;
        if let Some((_, _, name)) = configured.iter().find(|(e, _, _)| e == endpoint) {
            tx.execute(
                "UPDATE usage_endpoints SET label=?2 WHERE id=?1",
                rusqlite::params![key, name],
            )
            .map_err(|_| "无法更新端点标签")?;
        }
        endpoint_keys.insert(endpoint.clone(), key);
    }

    // Stage 1: session identities and their cumulative snapshots. A snapshot
    // whose timestamp has not advanced is not promoted, so the growth it carries
    // is still reconciled by a later scan instead of being lost.
    type PlanEntry = (
        String,
        SessionUpdate,
        Option<native::Snapshot>,
        Option<Existing>,
        bool,
    );
    let mut planned: Vec<PlanEntry> = vec![];
    for update in &inputs.session_updates {
        let id = native::session_id(&update.uuid, &update.project_key);
        let existing = read_existing(&tx, &id)?;
        let snapshot = inputs
            .snapshots
            .iter()
            .find(|s| s.session_uuid == update.uuid && s.project_key == update.project_key)
            .cloned();
        let previous_at = existing.as_ref().and_then(|e| e.snapshot_at);
        let mut advancing = match (snapshot.as_ref(), previous_at) {
            (Some(snapshot), Some(previous)) => snapshot.observed_at > previous,
            (Some(_), None) => true,
            (None, _) => false,
        };
        if let (Some(snapshot), Some(previous)) = (snapshot.as_ref(), previous_at) {
            if snapshot.observed_at < previous {
                report.counter_resets += 1;
            }
        }
        if let (Some(new), Some(old)) = (snapshot.as_ref(), existing.as_ref()) {
            if [
                (new.own.tokens.input, old.own.input),
                (new.own.tokens.cache_read, old.own.cache_read),
                (new.own.tokens.cache_write, old.own.cache_write),
                (new.own.tokens.output, old.own.output),
                (new.own.tokens.thinking, old.own.thinking),
            ]
            .iter()
            .any(|(n, o)| n.zip(*o).is_some_and(|(n, o)| n < o))
            {
                report.counter_resets += 1;
                advancing = false;
            }
        }
        let promoted = match advancing {
            true => snapshot.as_ref(),
            false => None,
        };
        let (own, credits, inclusive, children, snapshot_at) = match promoted {
            Some(snapshot) => (
                snapshot.own.tokens.clone(),
                snapshot.own.credits,
                snapshot.inclusive.clone(),
                snapshot.child_sessions,
                Some(snapshot.observed_at),
            ),
            None => match existing.as_ref() {
                Some(existing) => (
                    existing.own.clone(),
                    existing.credits,
                    native::Counters {
                        tokens: existing.own.clone(),
                        credits: existing.credits,
                    },
                    existing.child_sessions,
                    existing.snapshot_at,
                ),
                None => (
                    cost::Tokens::default(),
                    None,
                    native::Counters::default(),
                    0,
                    None,
                ),
            },
        };
        tx.execute("INSERT INTO usage_sessions(id,session_uuid,project_key,first_activity,last_activity,transcript_messages,snapshot_at,own_input_tokens,own_cache_read_tokens,own_cache_write_tokens,own_output_tokens,own_thinking_tokens,own_credits,inclusive_input_tokens,inclusive_cache_read_tokens,inclusive_cache_write_tokens,inclusive_output_tokens,inclusive_thinking_tokens,child_sessions,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20) ON CONFLICT(id) DO UPDATE SET first_activity=COALESCE(excluded.first_activity,first_activity),last_activity=COALESCE(excluded.last_activity,last_activity),transcript_messages=COALESCE(excluded.transcript_messages,transcript_messages),snapshot_at=COALESCE(excluded.snapshot_at,snapshot_at),own_input_tokens=COALESCE(excluded.own_input_tokens,own_input_tokens),own_cache_read_tokens=COALESCE(excluded.own_cache_read_tokens,own_cache_read_tokens),own_cache_write_tokens=COALESCE(excluded.own_cache_write_tokens,own_cache_write_tokens),own_output_tokens=COALESCE(excluded.own_output_tokens,own_output_tokens),own_thinking_tokens=COALESCE(excluded.own_thinking_tokens,own_thinking_tokens),own_credits=COALESCE(excluded.own_credits,own_credits),inclusive_input_tokens=COALESCE(excluded.inclusive_input_tokens,inclusive_input_tokens),inclusive_cache_read_tokens=COALESCE(excluded.inclusive_cache_read_tokens,inclusive_cache_read_tokens),inclusive_cache_write_tokens=COALESCE(excluded.inclusive_cache_write_tokens,inclusive_cache_write_tokens),inclusive_output_tokens=COALESCE(excluded.inclusive_output_tokens,inclusive_output_tokens),inclusive_thinking_tokens=COALESCE(excluded.inclusive_thinking_tokens,inclusive_thinking_tokens),child_sessions=MAX(child_sessions,excluded.child_sessions),updated_at=excluded.updated_at",
        rusqlite::params![
            id,
            update.uuid,
            update.project_key,
            update.first_activity,
            update.last_activity,
            update.transcript_messages.unwrap_or(0) as i64,
            snapshot_at,
            own.input.map(|v| v as i64),
            own.cache_read.map(|v| v as i64),
            own.cache_write.map(|v| v as i64),
            own.output.map(|v| v as i64),
            own.thinking.map(|v| v as i64),
            credits,
            inclusive.tokens.input.map(|v| v as i64),
            inclusive.tokens.cache_read.map(|v| v as i64),
            inclusive.tokens.cache_write.map(|v| v as i64),
            inclusive.tokens.output.map(|v| v as i64),
            inclusive.tokens.thinking.map(|v| v as i64),
            children as i64,
            now(),
        ],
    )
    .map_err(|_| "保存会话账本失败")?;
        planned.push((id, update.clone(), snapshot, existing, advancing));
    }

    // Stage 2: request detail, so reconciliation sees it.
    for request in &inputs.requests {
        if request.record.at < cutoff {
            continue;
        }
        let session_ref = inputs
            .session_updates
            .iter()
            .find(|u| u.uuid == request.record.session_uuid)
            .map(|u| native::session_id(&u.uuid, &u.project_key));
        let endpoint = request.endpoint.as_ref();
        let endpoint_key = endpoint.and_then(|e| endpoint_keys.get(&e.endpoint).cloned());
        let written = write_event(
            &tx,
            &Event {
                scope: LOG_SCOPE,
                event_key: format!(
                    "req:{}:{}:{}",
                    request.record.request_id, request.record.selector, request.record.attempt
                ),
                session_uuid: request.record.session_uuid.clone(),
                session_ref,
                occurred_at: request.record.at,
                time_confidence: "event",
                granularity: "request",
                source: source_of(request.record.byok),
                provider_key: endpoint_key.clone(),
                endpoint_id: endpoint_key,
                selector: Some(request.record.selector.clone()),
                api_model: endpoint.map(|e| e.api_model.clone()),
                tokens: request.record.tokens.clone(),
                credits: None,
                request_id: Some(request.record.request_id.clone()),
                attempt: Some(request.record.attempt),
                interval: None,
                evidence: match endpoint.is_some() {
                    true => "log_endpoint",
                    false => "none",
                },
                file: format!("log:{}", request.record.mode),
            },
        )?;
        if written {
            let session = native::session_id(&request.record.session_uuid, "");
            consume_residual(&tx, &session, request.record.at, &request.record.tokens)?;
        }
        report.imported += usize::from(written);
        report.duplicates += usize::from(!written);
    }

    // Stage 3: ledger reconciliation.
    for (id, update, snapshot, existing, advancing) in &planned {
        if !advancing {
            continue;
        }
        let Some(snapshot) = snapshot else { continue };
        if snapshot.observed_at < cutoff {
            continue;
        }
        let previous_at = existing.as_ref().and_then(|e| e.snapshot_at);
        let (event_key, granularity, confidence, interval, total, credits) = match previous_at {
            None => (
                format!("baseline:{}", snapshot.observed_at),
                "historical_baseline",
                "unknown",
                None,
                snapshot.own.tokens.clone(),
                snapshot.own.credits,
            ),
            Some(previous_at) => {
                if snapshot.observed_at <= previous_at {
                    continue;
                }
                let old = existing.as_ref().map(|e| e.own.clone()).unwrap_or_default();
                let pairs = [
                    (old.input, snapshot.own.tokens.input),
                    (old.cache_read, snapshot.own.tokens.cache_read),
                    (old.cache_write, snapshot.own.tokens.cache_write),
                    (old.output, snapshot.own.tokens.output),
                    (old.thinking, snapshot.own.tokens.thinking),
                ];
                if pairs
                    .iter()
                    .any(|(old, new)| new.zip(*old).is_some_and(|(n, o)| n < o))
                {
                    // A counter going backwards is a reset or branch, never
                    // negative consumption, so nothing is fabricated here.
                    report.counter_resets += 1;
                    continue;
                }
                let growth = cost::Tokens {
                    input: pairs[0].1.zip(pairs[0].0).map(|(n, o)| n - o),
                    cache_read: pairs[1].1.zip(pairs[1].0).map(|(n, o)| n - o),
                    cache_write: pairs[2].1.zip(pairs[2].0).map(|(n, o)| n - o),
                    output: pairs[3].1.zip(pairs[3].0).map(|(n, o)| n - o),
                    thinking: pairs[4].1.zip(pairs[4].0).map(|(n, o)| n - o),
                };
                let credits = snapshot
                    .own
                    .credits
                    .zip(existing.as_ref().and_then(|e| e.credits))
                    .map(|(new, old)| new - old)
                    .filter(|value| *value > 0.0);
                (
                    format!("delta:{}:{}", previous_at, snapshot.observed_at),
                    "observed_delta",
                    "interval",
                    Some((previous_at, snapshot.observed_at)),
                    growth,
                    credits,
                )
            }
        };
        let covered = covered_since(
            &tx,
            id,
            interval.map(|(start, _)| start),
            interval.map(|(_, end)| end),
        )?;
        let fill = fill(&total, &covered);
        if !fill.known_any() && credits.is_none() {
            continue;
        }
        if fill.total() == Some(0) && credits.unwrap_or(0.0) == 0.0 {
            continue;
        }
        let written = write_event(
            &tx,
            &Event {
                scope: SESSION_SCOPE,
                event_key,
                session_uuid: update.uuid.clone(),
                session_ref: Some(id.clone()),
                occurred_at: snapshot.observed_at,
                time_confidence: confidence,
                granularity,
                source: "unknown",
                provider_key: None,
                endpoint_id: None,
                selector: snapshot.selector.clone(),
                api_model: None,
                tokens: fill,
                credits,
                request_id: None,
                attempt: None,
                interval,
                evidence: "none",
                file: format!("session:{}", update.uuid),
            },
        )?;
        if written {
            report.imported += 1;
            match granularity {
                "historical_baseline" => report.baselines += 1,
                _ => report.deltas += 1,
            }
        }
    }

    report.updated_at = now();
    let mut cursors: Vec<Cursor> = offsets
        .settings
        .iter()
        .map(|(path, stamp)| Cursor {
            key: format!("usage-settings:{path}"),
            identity: stamp.identity(),
            offset: stamp.size,
            state: "{}".into(),
        })
        .collect();
    cursors.extend(offsets.transcripts.iter().map(|(path, state)| Cursor {
        key: format!("usage-transcript:{path}"),
        identity: "append".into(),
        offset: state.offset,
        state: serde_json::to_string(state).unwrap_or_else(|_| "{}".into()),
    }));
    cursors.extend(offsets.logs.iter().map(|(path, offset)| Cursor {
        key: format!("usage-log:{path}"),
        identity: "append".into(),
        offset: *offset,
        state: serde_json::json!({"prefix":offsets.log_prefixes.get(path)}).to_string(),
    }));
    cursors.push(Cursor {
        key: "usage-log-pairing".into(),
        identity: "pairing".into(),
        offset: 0,
        state: serde_json::to_string(&offsets.pairing.sessions)
            .map_err(|_| "配对状态序列化失败")?,
    });
    cursors.push(Cursor {
        key: "usage-last-scan".into(),
        identity: "usage-report".into(),
        offset: 0,
        state: serde_json::to_string(&report).map_err(|_| "报告序列化失败")?,
    });
    for cursor in cursors {
        tx.execute("INSERT INTO sync_cursors(source_file,file_identity,byte_offset,state,updated_at) VALUES (?1,?2,?3,?4,unixepoch()) ON CONFLICT(source_file) DO UPDATE SET file_identity=excluded.file_identity,byte_offset=excluded.byte_offset,state=excluded.state,updated_at=excluded.updated_at",rusqlite::params![cursor.key,cursor.identity,cursor.offset as i64,cursor.state])
            .map_err(|_| "保存采集游标失败")?;
    }
    tx.commit().map_err(|_| "提交采集失败")?;
    Ok(Applied { report })
}

pub fn load_offsets(db: &Connection) -> Result<Offsets, String> {
    let mut offsets = Offsets::default();
    let mut statement = db
        .prepare("SELECT source_file,file_identity,byte_offset,state FROM sync_cursors WHERE source_file LIKE 'usage-%'")
        .map_err(|_| "无法读取采集游标")?;
    let rows = statement
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)? as u64,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|_| "无法读取采集游标")?;
    for row in rows {
        let (key, identity, offset, state) = row.map_err(|_| "采集游标结构无效")?;
        if let Some(path) = key.strip_prefix("usage-settings:") {
            if let Some(stamp) = Stamp::parse(&identity) {
                offsets.settings.insert(path.to_owned(), stamp);
            }
        } else if let Some(path) = key.strip_prefix("usage-transcript:") {
            offsets.transcripts.insert(
                path.to_owned(),
                serde_json::from_str(&state).unwrap_or_default(),
            );
        } else if let Some(path) = key.strip_prefix("usage-log:") {
            offsets.logs.insert(path.to_owned(), offset);
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&state) {
                if let Some(prefix) = v["prefix"].as_str() {
                    offsets
                        .log_prefixes
                        .insert(path.to_owned(), prefix.to_owned());
                }
            }
        } else if key == "usage-log-pairing" {
            if let Ok(sessions) = serde_json::from_str(&state) {
                offsets.pairing.sessions = sessions;
            }
        }
    }
    Ok(offsets)
}

pub fn last_report(db: &Connection) -> Result<Option<ScanReport>, String> {
    let state: Option<String> = db
        .query_row(
            "SELECT state FROM sync_cursors WHERE source_file='usage-last-scan'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| "读取采集报告失败")?;
    match state {
        Some(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|_| "采集报告结构无效".into()),
        None => Ok(None),
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const LOG_FIXTURE: &str = include_str!("../../fixtures/native/droid-log.synthetic.log");
    const SETTINGS_FIXTURE: &str =
        include_str!("../../fixtures/native/native-session.synthetic.settings.json");
    const TRANSCRIPT_FIXTURE: &str =
        include_str!("../../fixtures/native/native-session.synthetic.jsonl");
    const SESSION_A: &str = "11111111-1111-4111-8111-111111111111";

    fn open(path: &Path) -> Connection {
        crate::db::open(path).unwrap()
    }

    fn scan(sessions: &Path, logs: Option<&Path>, db: &mut Connection, cutoff: i64) -> ScanReport {
        let mut offsets = load_offsets(db).unwrap();
        let inputs = collect(Some(sessions), logs, &mut offsets);
        apply(db, inputs, &offsets, cutoff).unwrap().report
    }

    fn tokens(db: &Connection) -> (i64, i64, i64) {
        db.query_row(
            "SELECT COALESCE(SUM(input_tokens),0),COALESCE(SUM(output_tokens),0),COUNT(*) FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap()
    }

    fn count(db: &Connection, sql: &str) -> i64 {
        db.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    fn bump(path: &Path) {
        // Snapshot times are whole seconds, so a follow-up write needs a new one.
        std::fs::write(path, std::fs::read(path).unwrap()).unwrap();
        std::thread::sleep(Duration::from_millis(1100));
    }

    #[test]
    #[ignore = "requires native fixture files"]
    fn native_scan_is_idempotent_and_unmodified_files_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let sessions = dir.path().join("sessions");
        let project = sessions.join("-Users-me-repo");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join("session.settings.json"), SETTINGS_FIXTURE).unwrap();
        std::fs::write(project.join("session.jsonl"), TRANSCRIPT_FIXTURE).unwrap();
        let mut db = open(&dir.path().join("db.sqlite"));
        let first = scan(&sessions, None, &mut db, 0);
        assert_eq!(first.snapshots, 1);
        assert_eq!(first.baselines, 1);
        assert_eq!(first.sessions, 1);
        assert_eq!(first.session_files, 2);
        assert_eq!(tokens(&db), (1200, 400, 1));
        assert_eq!(
            count(
                &db,
                "SELECT count(*) FROM usage_events WHERE time_confidence='unknown'"
            ),
            1
        );
        let second = scan(&sessions, None, &mut db, 0);
        assert_eq!(second.snapshots, 0);
        assert_eq!(second.imported, 0);
        assert_eq!(tokens(&db), (1200, 400, 1));
        assert_eq!(
            db.query_row("SELECT transcript_messages FROM usage_sessions", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap(),
            2
        );
        assert_eq!(
            db.query_row("SELECT first_activity FROM usage_sessions", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1767225601
        );
    }

    #[test]
    #[ignore = "requires native fixture files"]
    fn growth_becomes_one_observed_delta_and_a_shrinking_counter_is_flagged() {
        let dir = tempfile::tempdir().unwrap();
        let sessions = dir.path().join("sessions");
        let project = sessions.join("-Users-me-repo");
        std::fs::create_dir_all(&project).unwrap();
        let path = project.join("session.settings.json");
        std::fs::write(&path, SETTINGS_FIXTURE).unwrap();
        let mut db = open(&dir.path().join("db.sqlite"));
        assert_eq!(scan(&sessions, None, &mut db, 0).baselines, 1);
        bump(&path);
        let grown = SETTINGS_FIXTURE
            .replace("\"inputTokens\": 1200", "\"inputTokens\": 2200")
            .replace("\"outputTokens\": 400", "\"outputTokens\": 900");
        std::fs::write(&path, grown).unwrap();
        let report = scan(&sessions, None, &mut db, 0);
        assert_eq!(report.deltas, 1);
        assert_eq!(tokens(&db), (2200, 900, 2));
        assert_eq!(
            count(
                &db,
                "SELECT count(*) FROM usage_events WHERE time_confidence='interval'"
            ),
            1
        );
        bump(&path);
        std::fs::write(&path, SETTINGS_FIXTURE).unwrap();
        let report = scan(&sessions, None, &mut db, 0);
        assert_eq!(report.counter_resets, 1);
        assert_eq!(report.deltas, 0);
        assert_eq!(tokens(&db), (2200, 900, 2));
        assert_eq!(
            db.query_row("SELECT own_thinking_tokens FROM usage_sessions", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap(),
            300
        );
    }

    #[test]
    #[ignore = "requires native fixture files"]
    fn requests_are_stored_before_reconciliation_so_a_baseline_never_double_counts() {
        let dir = tempfile::tempdir().unwrap();
        let sessions = dir.path().join("sessions");
        let logs = dir.path().join("logs");
        let project = sessions.join("-Users-me-repo");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(&logs).unwrap();
        std::fs::write(project.join("session.settings.json"), SETTINGS_FIXTURE).unwrap();
        std::fs::write(logs.join("droid-log-single.log"), LOG_FIXTURE).unwrap();
        let mut db = open(&dir.path().join("db.sqlite"));
        let mut offsets = load_offsets(&db).unwrap();
        let inputs = collect(Some(&sessions), Some(&logs), &mut offsets);
        assert_eq!(inputs.report.log_requests, 5);
        assert_eq!(inputs.report.log_unpaired, 1);
        assert_eq!(inputs.report.unsupported, 0);
        let applied = apply(&mut db, inputs, &offsets, 0).unwrap();
        assert_eq!(applied.report.baselines, 1);
        // The file session has no requests of its own, so its whole total is a
        // baseline; the log sessions only add their own detail.
        assert_eq!(tokens(&db), (3015, 1670, 6));
        assert_eq!(
            count(
                &db,
                "SELECT count(*) FROM usage_events WHERE evidence='log_endpoint'"
            ),
            4
        );
        assert_eq!(
            count(
                &db,
                "SELECT count(*) FROM usage_events WHERE granularity='historical_baseline'"
            ),
            1
        );
        assert_eq!(
            count(&db, "SELECT count(*) FROM usage_events WHERE source='api'"),
            4
        );
        assert_eq!(
            count(
                &db,
                "SELECT count(*) FROM usage_events WHERE source='official'"
            ),
            1
        );
        assert_eq!(count(&db, "SELECT count(*) FROM usage_sessions"), 1); // One discovered session; log-only sessions remain in events.
                                                                          // Replaying the log after rotation must not add a second copy.
        let mut offsets = load_offsets(&db).unwrap();
        offsets.logs.clear();
        let inputs = collect(Some(&sessions), Some(&logs), &mut offsets);
        let applied = apply(&mut db, inputs, &offsets, 0).unwrap();
        assert_eq!(applied.report.duplicates, 5);
        assert_eq!(tokens(&db), (3015, 1670, 6));
    }

    #[test]
    #[ignore = "requires native fixture files"]
    fn detail_that_covers_a_session_total_produces_no_baseline_and_keeps_attribution() {
        let dir = tempfile::tempdir().unwrap();
        let sessions = dir.path().join("sessions");
        let logs = dir.path().join("logs");
        let project = sessions.join("-Users-me-repo");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(&logs).unwrap();
        std::fs::write(
            project.join(format!("{SESSION_A}.settings.json")),
            SETTINGS_FIXTURE,
        )
        .unwrap();
        std::fs::write(logs.join("droid-log-single.log"), LOG_FIXTURE).unwrap();
        let mut db = open(&dir.path().join("db.sqlite"));
        let report = scan(&sessions, Some(&logs), &mut db, 0);
        assert_eq!(report.log_requests, 5);
        // Input/output are covered; cache-read still has 3796 unallocated tokens.
        assert_eq!(report.baselines, 1);
        assert_eq!(tokens(&db), (1815, 1270, 6));
        assert_eq!(db.query_row("SELECT cache_read_tokens FROM usage_events WHERE granularity='historical_baseline'",[],|r|r.get::<_,i64>(0)).unwrap(),3796);
        assert_eq!(
            count(
                &db,
                "SELECT count(*) FROM usage_events WHERE session_ref IS NOT NULL"
            ),
            4
        );
        assert_eq!(
            db.query_row("SELECT own_input_tokens FROM usage_sessions", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1200
        );
        let providers: Vec<(Option<String>, i64)> = db
            .prepare("SELECT provider_id_at_event,count(*) FROM usage_events GROUP BY provider_id_at_event ORDER BY provider_id_at_event")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(providers.len(), 3);
        assert_eq!(providers[0], (None, 2));
        assert!(providers[1].0.is_some() && providers[2].0.is_some());
        assert_ne!(providers[1].0, providers[2].0);
    }

    #[test]
    #[ignore = "requires native fixture files"]
    fn cleanup_boundary_keeps_the_ledger_and_stops_reviving_old_history() {
        let dir = tempfile::tempdir().unwrap();
        let sessions = dir.path().join("sessions");
        let project = sessions.join("-Users-me-repo");
        std::fs::create_dir_all(&project).unwrap();
        let path = project.join("session.settings.json");
        std::fs::write(&path, SETTINGS_FIXTURE).unwrap();
        let mut db = open(&dir.path().join("db.sqlite"));
        assert_eq!(scan(&sessions, None, &mut db, 0).baselines, 1);
        let boundary = db
            .query_row("SELECT snapshot_at FROM usage_sessions", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();
        // Simulate retention cleanup: the derived records go, the ledger stays.
        db.execute(
            "DELETE FROM usage_events WHERE occurred_at<?1",
            [boundary + 1],
        )
        .unwrap();
        assert_eq!(tokens(&db), (0, 0, 0));
        let report = scan(&sessions, None, &mut db, boundary + 1);
        assert_eq!(report.baselines, 0);
        assert_eq!(
            tokens(&db),
            (0, 0, 0),
            "cleared history must not be imported again"
        );
        assert_eq!(
            db.query_row("SELECT own_input_tokens FROM usage_sessions", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1200
        );
        bump(&path);
        let grown = SETTINGS_FIXTURE.replace("\"inputTokens\": 1200", "\"inputTokens\": 1300");
        std::fs::write(&path, grown).unwrap();
        let report = scan(&sessions, None, &mut db, boundary + 1);
        assert_eq!(report.deltas, 1);
        assert_eq!(
            tokens(&db),
            (100, 0, 1),
            "only consumption after the boundary is counted"
        );
    }

    #[test]
    fn missing_roots_are_reported_instead_of_panicking() {
        let dir = tempfile::tempdir().unwrap();
        let mut db = open(&dir.path().join("db.sqlite"));
        let report = scan(&dir.path().join("missing-sessions"), None, &mut db, 0);
        assert_eq!(report.unreadable, 1);
        assert_eq!(tokens(&db), (0, 0, 0));
    }
    #[test]
    #[ignore = "requires native fixture files"]
    fn late_details_replace_baseline_and_replay_does_not_change_totals() {
        let dir = tempfile::tempdir().unwrap();
        let sessions = dir.path().join("sessions");
        let project = sessions.join("project");
        let logs = dir.path().join("logs");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(&logs).unwrap();
        std::fs::write(
            project.join(format!("{SESSION_A}.settings.json")),
            SETTINGS_FIXTURE,
        )
        .unwrap();
        let mut db = open(&dir.path().join("db"));
        scan(&sessions, None, &mut db, 0);
        let first: String = LOG_FIXTURE
            .lines()
            .take(2)
            .map(|s| format!("{s}\n"))
            .collect();
        std::fs::write(logs.join("droid-log-single.log"), first).unwrap();
        let report = scan(&sessions, Some(&logs), &mut db, 0);
        assert_eq!(report.log_requests, 1);
        assert_eq!(tokens(&db), (1200, 400, 2));
        assert_eq!(
            db.query_row(
                "SELECT input_tokens FROM usage_events WHERE granularity='historical_baseline'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        let before = tokens(&db);
        scan(&sessions, Some(&logs), &mut db, 0);
        assert_eq!(tokens(&db), before);
    }
    #[test]
    #[ignore = "requires native fixture files"]
    fn rotation_and_same_length_replacement_are_read_without_duplicate_charges() {
        let dir = tempfile::tempdir().unwrap();
        let logs = dir.path().join("logs");
        std::fs::create_dir_all(&logs).unwrap();
        let path = logs.join("droid-log-single.log");
        let sessions = dir.path().join("sessions");
        std::fs::create_dir_all(&sessions).unwrap();
        let original: String = LOG_FIXTURE
            .lines()
            .take(2)
            .map(|s| format!("{s}\n"))
            .collect();
        std::fs::write(&path, &original).unwrap();
        let mut db = open(&dir.path().join("db"));
        scan(&sessions, Some(&logs), &mut db, 0);
        assert_eq!(tokens(&db), (1200, 400, 1));
        let replaced = original
            .replace("resp-synthetic-1", "resp-synthetic-X")
            .replace("trace-1", "trace-X");
        assert_eq!(original.len(), replaced.len());
        std::fs::write(&path, replaced).unwrap();
        scan(&sessions, Some(&logs), &mut db, 0);
        assert_eq!(tokens(&db), (2400, 800, 2));
        std::fs::write(&path, "").unwrap();
        scan(&sessions, Some(&logs), &mut db, 0);
        std::fs::write(&path, original).unwrap();
        scan(&sessions, Some(&logs), &mut db, 0);
        assert_eq!(tokens(&db), (2400, 800, 2));
    }
    #[test]
    #[ignore = "requires native fixture files"]
    fn no_change_scan_is_incremental_and_under_one_second() {
        let dir = tempfile::tempdir().unwrap();
        let sessions = dir.path().join("sessions");
        let project = sessions.join("project");
        std::fs::create_dir_all(&project).unwrap();
        for i in 0..100 {
            std::fs::write(
                project.join(format!("s{i}.settings.json")),
                SETTINGS_FIXTURE,
            )
            .unwrap();
        }
        let mut db = open(&dir.path().join("db"));
        scan(&sessions, None, &mut db, 0);
        let start = std::time::Instant::now();
        let report = scan(&sessions, None, &mut db, 0);
        let elapsed = start.elapsed();
        eprintln!("100 unchanged sessions: {}ms", elapsed.as_millis());
        assert_eq!(report.snapshots, 0);
        assert_eq!(report.imported, 0);
        assert!(elapsed.as_secs_f64() < 1.0);
    }
    #[test]
    #[ignore = "explicit read-only local smoke; statistics database is temporary"]
    fn native_local_readonly_smoke() {
        let home = std::env::var("HOME").unwrap();
        let sessions = PathBuf::from(&home).join(".factory/sessions");
        let logs = PathBuf::from(&home).join(".factory/logs");
        let dir = tempfile::tempdir().unwrap();
        let mut db = open(&dir.path().join("smoke.db"));
        let first = scan(&sessions, Some(&logs), &mut db, 0);
        let before = tokens(&db);
        let second = scan(&sessions, Some(&logs), &mut db, 0);
        assert!(first.sessions > 0);
        assert!(first.snapshots > 0);
        assert!(first.log_requests > 0);
        assert_eq!(second.imported, 0);
        assert_eq!(tokens(&db), before);
        eprintln!("read-only local smoke: sessions={} snapshots={} requests={} unpaired={} unsupported={} malformed={} replay_imported={}",first.sessions,first.snapshots,first.log_requests,first.log_unpaired,first.unsupported,first.malformed,second.imported);
    }
}
