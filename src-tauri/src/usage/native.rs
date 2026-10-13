//! Native Droid session source: `<uuid>.settings.json` cumulative snapshots and
//! `<uuid>.jsonl` activity windows. Read-only; only counters and timestamps are
//! kept, never message content.

use super::cost::Tokens;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Counters {
    pub tokens: Tokens,
    pub credits: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub session_uuid: String,
    pub project_key: String,
    pub observed_at: i64,
    pub selector: Option<String>,
    pub own: Counters,
    pub inclusive: Counters,
    pub child_sessions: u64,
}

fn counter(value: Option<&Value>, key: &str) -> Option<u64> {
    value?.get(key)?.as_u64()
}

fn credits(value: Option<&Value>) -> Option<f64> {
    value?.get("factoryCredits")?.as_f64()
}

fn counters(value: Option<&Value>) -> Counters {
    Counters {
        tokens: Tokens {
            input: counter(value, "inputTokens"),
            cache_read: counter(value, "cacheReadTokens"),
            cache_write: counter(value, "cacheCreationTokens"),
            output: counter(value, "outputTokens"),
            thinking: counter(value, "thinkingTokens"),
        },
        credits: credits(value),
    }
}

/// The snapshot is only usable when it carries the own cumulative counters; a
/// session with no usage has none of them, which is data rather than corruption.
pub fn parse_snapshot(
    text: &str,
    session_uuid: &str,
    project_key: &str,
    observed_at: i64,
) -> Result<Option<Snapshot>, String> {
    let document: Value = serde_json::from_str(text).map_err(|_| "会话快照 JSON 无效")?;
    let usage = document.get("tokenUsage");
    if !usage.map(Value::is_object).unwrap_or(false) {
        return Ok(None);
    }
    let own = counters(usage);
    if own.tokens.total().is_none() && !own.tokens.known_any() && own.credits.is_none() {
        return Ok(None);
    }
    let inclusive_raw = document.get("inclusiveTokenUsage");
    let mut inclusive = match inclusive_raw.map(Value::is_object).unwrap_or(false) {
        true => counters(inclusive_raw),
        false => own.clone(),
    };
    if inclusive.tokens.total().is_none() && inclusive.tokens.known_any() {
        inclusive.tokens = own.tokens.clone();
    }
    let child_sessions = document
        .get("childInclusiveTokenUsageBySessionId")
        .and_then(Value::as_object)
        .map(|m| m.len() as u64)
        .unwrap_or(0);
    Ok(Some(Snapshot {
        session_uuid: session_uuid.to_owned(),
        project_key: project_key.to_owned(),
        observed_at,
        selector: document
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_owned),
        own,
        inclusive,
        child_sessions,
    }))
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityState {
    #[serde(default)]
    pub offset: u64,
    #[serde(default)]
    pub first: Option<i64>,
    #[serde(default)]
    pub last: Option<i64>,
    #[serde(default)]
    pub messages: u64,
    #[serde(default)]
    pub malformed: u64,
}

#[derive(Default)]
pub struct Activity {
    pub first: Option<i64>,
    pub last: Option<i64>,
    pub messages: u64,
    pub malformed: u64,
    pub consumed: usize,
}

fn timestamp(value: &Value) -> Option<i64> {
    let raw = value.get("timestamp")?;
    if let Some(number) = raw.as_i64() {
        return Some(number);
    }
    let text = raw.as_str()?;
    chrono::DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|d| d.timestamp())
}

/// Read only complete lines; a partially written trailing line is left for the
/// next pass so a half-written record is never counted.
pub fn parse_activity(text: &str) -> Activity {
    let mut activity = Activity::default();
    let mut consumed = 0usize;
    for line in text.split_inclusive('\n') {
        if !line.ends_with('\n') {
            break;
        }
        consumed += line.len();
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
            activity.malformed += 1;
            continue;
        };
        if value.get("type").and_then(Value::as_str) == Some("message") {
            activity.messages += 1;
        }
        if let Some(at) = timestamp(&value) {
            activity.first = Some(activity.first.map_or(at, |f: i64| f.min(at)));
            activity.last = Some(activity.last.map_or(at, |l: i64| l.max(at)));
        }
    }
    activity.consumed = consumed;
    activity
}

pub fn merge_activity(state: &mut ActivityState, activity: &Activity) {
    if let Some(first) = activity.first {
        state.first = Some(state.first.map_or(first, |f: i64| f.min(first)));
    }
    if let Some(last) = activity.last {
        state.last = Some(state.last.map_or(last, |l: i64| l.max(last)));
    }
    state.messages += activity.messages;
    state.malformed += activity.malformed;
    state.offset += activity.consumed as u64;
}

pub struct SessionFiles {
    pub session_uuid: String,
    pub project_key: String,
    pub settings: Option<PathBuf>,
    pub transcript: Option<PathBuf>,
}

fn project_key(path: &Path, root: &Path) -> String {
    path.parent()
        .filter(|parent| *parent != root)
        .and_then(|parent| parent.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Session files under a root, using the project directory layout Droid writes.
pub fn discover(root: &Path) -> Result<Vec<SessionFiles>, String> {
    let mut files: Vec<SessionFiles> = vec![];
    scan_dir(root, root, 0, &mut files)?;
    files.sort_by(|a, b| a.session_uuid.cmp(&b.session_uuid));
    Ok(files)
}

fn scan_dir(
    dir: &Path,
    root: &Path,
    depth: usize,
    out: &mut Vec<SessionFiles>,
) -> Result<(), String> {
    if depth > 2 {
        return Ok(());
    }
    let entries = std::fs::read_dir(dir).map_err(|_| "无法读取会话目录")?;
    for entry in entries {
        let entry = entry.map_err(|_| "无法读取会话条目")?;
        let path = entry.path();
        let meta = std::fs::symlink_metadata(&path).map_err(|_| "无法访问会话条目")?;
        if meta.is_symlink() {
            continue;
        }
        if meta.is_dir() {
            scan_dir(&path, root, depth + 1, out)?;
            continue;
        }
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        let (uuid, kind) = if let Some(uuid) = name.strip_suffix(".settings.json") {
            (uuid.to_owned(), 1)
        } else if let Some(uuid) = name.strip_suffix(".jsonl") {
            (uuid.to_owned(), 2)
        } else {
            continue;
        };
        if uuid.is_empty() {
            continue;
        }
        let key = project_key(&path, root);
        if let Some(existing) = out.iter_mut().find(|f| f.session_uuid == uuid) {
            match kind {
                1 => existing.settings = Some(path),
                _ => existing.transcript = Some(path),
            }
            continue;
        }
        out.push(SessionFiles {
            session_uuid: uuid,
            project_key: key,
            settings: (kind == 1).then(|| path.clone()),
            transcript: (kind == 2).then(|| path.clone()),
        });
    }
    Ok(())
}

pub fn session_id(session_uuid: &str, _project_key: &str) -> String {
    crate::config::pipeline::hash(format!("session\0{session_uuid}").as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str =
        include_str!("../../fixtures/native/native-session.synthetic.settings.json");

    #[test]
    fn session_snapshot_reads_own_and_inclusive_without_content() {
        let snapshot = parse_snapshot(FIXTURE, "u", "p", 1000).unwrap().unwrap();
        assert_eq!(snapshot.own.tokens.input, Some(1200));
        assert_eq!(snapshot.own.tokens.cache_read, Some(4096));
        assert_eq!(snapshot.own.tokens.thinking, Some(300));
        assert_eq!(snapshot.inclusive, snapshot.own);
        assert_eq!(snapshot.child_sessions, 0);
        assert_eq!(snapshot.selector.as_deref(), Some("custom:vendor-model-0"));
        assert_eq!(snapshot.own.tokens.total(), Some(5996));
    }

    #[test]
    fn missing_or_empty_usage_is_not_fabricated_as_zero() {
        assert!(parse_snapshot("{}", "u", "p", 1).unwrap().is_none());
        assert!(parse_snapshot("not json", "u", "p", 1).is_err());
        let empty = parse_snapshot(r#"{"tokenUsage":{}}"#, "u", "p", 1).unwrap();
        assert!(empty.is_none());
        let partial = parse_snapshot(r#"{"tokenUsage":{"inputTokens":5}}"#, "u", "p", 1)
            .unwrap()
            .unwrap();
        assert_eq!(partial.own.tokens.output, None);
        assert_eq!(partial.own.tokens.total(), None);
        assert!(partial.own.tokens.known_any());
    }

    #[test]
    fn activity_window_ignores_partial_tail_and_counts_messages() {
        let first =
            parse_activity("{\"type\":\"message\",\"timestamp\":\"2026-01-01T00:00:02Z\"}\n");
        assert_eq!(first.first, Some(1767225602));
        assert_eq!(first.messages, 1);
        let tail = parse_activity(
            "{\"type\":\"message\",\"timestamp\":\"2026-01-01T00:00:09Z\"}\n{\"type\":\"mess",
        );
        assert_eq!(tail.messages, 1);
        assert_eq!(tail.first, Some(1767225609));
        assert_eq!(
            tail.consumed,
            "{\"type\":\"message\",\"timestamp\":\"2026-01-01T00:00:09Z\"}\n".len()
        );
        let broken = parse_activity("{oops}\n");
        assert_eq!(broken.malformed, 1);
        let mut state = ActivityState::default();
        merge_activity(&mut state, &first);
        merge_activity(&mut state, &tail);
        assert_eq!(state.first, Some(1767225602));
        assert_eq!(state.last, Some(1767225609));
        assert_eq!(state.messages, 2);
        assert_eq!(state.offset, (first.consumed + tail.consumed) as u64);
    }

    #[test]
    fn discovery_groups_settings_and_transcript_per_session() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("-Users-me-repo");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join("a.settings.json"), "{}").unwrap();
        std::fs::write(project.join("a.settings.json.bak"), "{}").unwrap();
        std::fs::write(project.join("a.jsonl"), "").unwrap();
        std::fs::write(project.join("b.jsonl"), "").unwrap();
        std::fs::write(dir.path().join("c.settings.json"), "{}").unwrap();
        let files = discover(dir.path()).unwrap();
        assert_eq!(files.len(), 3);
        let a = files.iter().find(|f| f.session_uuid == "a").unwrap();
        assert_eq!(a.project_key, "-Users-me-repo");
        assert!(a.settings.is_some() && a.transcript.is_some());
        let b = files.iter().find(|f| f.session_uuid == "b").unwrap();
        assert!(b.settings.is_none() && b.transcript.is_some());
        let c = files.iter().find(|f| f.session_uuid == "c").unwrap();
        assert_eq!(c.project_key, "");
    }
}
