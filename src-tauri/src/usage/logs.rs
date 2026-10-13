//! Request-level source: Droid's diagnostic log.
//!
//! Two record kinds are used. `[Agent] Streaming result` carries per-request
//! counters; `metrics_log_chat_client_time_to_first_token` carries the endpoint
//! that request went to. They are paired one-to-one inside a session, which is
//! what makes provider attribution possible when a selector id is shared by two
//! providers.
//!
//! Only numeric counters, identifiers and endpoints are read. Message content
//! never appears in these records, and no raw line is stored.

use super::cost::Tokens;
use super::identity::normalize_endpoint;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct UsageRecord {
    pub at: i64,
    pub at_ms: i64,
    pub session_uuid: String,
    pub selector: String,
    pub tokens: Tokens,
    pub total_input: Option<u64>,
    pub request_id: String,
    pub attempt: u32,
    pub byok: Option<bool>,
    pub mode: String,
    pub protocol: String,
    pub trace: Option<String>,
    pub span: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EndpointRecord {
    pub at: i64,
    pub at_ms: i64,
    pub session_uuid: String,
    pub selector: String,
    pub endpoint: String,
    pub api_model: String,
    pub protocol: String,
    pub trace: Option<String>,
    pub span: Option<String>,
    pub byok: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Parsed {
    Usage(Box<UsageRecord>),
    Endpoint(EndpointRecord),
}

pub fn parse_at(line: &str) -> Option<i64> {
    let end = line.find(']')?;
    if !line.starts_with('[') {
        return None;
    }
    chrono::DateTime::parse_from_rfc3339(&line[1..end])
        .ok()
        .map(|d| d.timestamp())
}

fn boolean(value: Option<&Value>) -> Option<bool> {
    match value? {
        Value::Bool(flag) => Some(*flag),
        Value::String(text) => match text.as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn text(tags: &Value, key: &str) -> Option<String> {
    tags.get(key)?.as_str().map(str::to_owned)
}

fn counters(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

/// A single diagnostic log line, or `None` for every other record in the file.
pub fn parse_line(line: &str) -> Option<Parsed> {
    let context = line.find("Context: ")?;
    let head = &line[..context];
    let name = head;
    let at = parse_at(line)?;
    let at_ms = chrono::DateTime::parse_from_rfc3339(&line[1..line.find(']')?])
        .ok()?
        .timestamp_millis();
    let value: Value = serde_json::from_str(line[context + 9..].trim()).ok()?;
    let tags = value.get("tags").cloned().unwrap_or(Value::Null);
    let session_uuid = text(&tags, "sessionId")?;
    let selector = text(&tags, "modelId")?;
    let byok = boolean(tags.get("isByok"));
    let protocol = value
        .get("modelProvider")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_default();
    if name.contains("Streaming result") {
        let request_id = value.get("upstreamResponseId")?.as_str()?.to_owned();
        let tokens = Tokens {
            input: counters(&value, "inputTokens"),
            cache_read: counters(&value, "cacheReadInputTokens"),
            cache_write: counters(&value, "cachedTokensWritten"),
            output: counters(&value, "outputTokens"),
            thinking: counters(&value, "reasoningTokens"),
        };
        if !tokens.known_any() {
            return None;
        }
        return Some(Parsed::Usage(Box::new(UsageRecord {
            at,
            at_ms,
            session_uuid,
            selector,
            tokens,
            total_input: counters(&value, "totalInputTokens"),
            request_id,
            attempt: counters(&value, "attempt").unwrap_or(1).max(1) as u32,
            byok,
            mode: text(&tags, "droidMode").unwrap_or_default(),
            trace: text(&tags, "traceId"),
            span: text(&tags, "spanId"),
            protocol,
        })));
    }
    if name.contains("chat_client_time_to_first_token") {
        let endpoint = normalize_endpoint(value.get("baseUrl")?.as_str()?)?;
        let api_model = value.get("modelId")?.as_str()?.to_owned();
        return Some(Parsed::Endpoint(EndpointRecord {
            at,
            at_ms,
            session_uuid,
            selector,
            endpoint,
            api_model,
            trace: text(&tags, "traceId"),
            span: text(&tags, "spanId"),
            protocol: value
                .get("modelProvider")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or(protocol),
            byok,
        }));
    }
    None
}

const MAX_PENDING: usize = 64;

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairState {
    /// Everything up to this instant was already paired at least once, so a
    /// re-read of a rotated file cannot re-pair the same endpoint evidence.
    #[serde(default)]
    pub paired_through: i64,
    #[serde(default)]
    pub pending: Vec<EndpointRecord>,
    #[serde(default)]
    pub used: Vec<String>,
}

impl PairState {
    fn push(&mut self, record: EndpointRecord) {
        let (Some(trace), Some(span)) = (&record.trace, &record.span) else {
            return;
        };
        let key = format!("{trace}\0{span}\0{}\0{}", record.at_ms, record.endpoint);
        if self.used.contains(&key) {
            return;
        }
        self.pending.retain(|p| {
            !(p.trace == record.trace
                && p.span == record.span
                && p.at_ms == record.at_ms
                && p.endpoint == record.endpoint)
        });
        self.pending.push(record);
        if self.pending.len() > MAX_PENDING {
            self.pending.remove(0);
        }
    }

    fn take(&mut self, usage: &UsageRecord) -> Option<EndpointRecord> {
        let (Some(trace), Some(span)) = (&usage.trace, &usage.span) else {
            return None;
        };
        let index = self.pending.iter().position(|p| {
            p.at_ms <= usage.at_ms
                && p.selector == usage.selector
                && p.trace.as_ref() == Some(trace)
                && p.span.as_ref() == Some(span)
        })?;
        let record = self.pending.remove(index);
        self.paired_through = self.paired_through.max(record.at);
        self.used.push(format!(
            "{trace}\0{span}\0{}\0{}",
            record.at_ms, record.endpoint
        ));
        if self.used.len() > 512 {
            self.used.remove(0);
        }
        Some(record)
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }
}

#[derive(Default)]
pub struct Pairing {
    pub sessions: BTreeMap<String, PairState>,
    pub unpaired: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub record: UsageRecord,
    pub endpoint: Option<EndpointRecord>,
}

impl Pairing {
    /// The per-session endpoint evidence kept between scans. Bounded so the
    /// cursor state cannot grow without limit on a long-lived installation.
    pub fn states(&mut self, max_sessions: usize) -> BTreeMap<String, PairState> {
        if self.sessions.len() > max_sessions {
            let mut ordered: Vec<_> = self
                .sessions
                .iter()
                .map(|(k, v)| (v.paired_through, k.clone()))
                .collect();
            ordered.sort();
            for (_, key) in ordered.into_iter().take(self.sessions.len() - max_sessions) {
                self.sessions.remove(&key);
            }
        }
        std::mem::take(&mut self.sessions)
    }

    pub fn feed(&mut self, parsed: Parsed) -> Option<Request> {
        match parsed {
            Parsed::Endpoint(record) => {
                self.sessions
                    .entry(record.session_uuid.clone())
                    .or_default()
                    .push(record);
                None
            }
            Parsed::Usage(record) => {
                let endpoint = self
                    .sessions
                    .get_mut(&record.session_uuid)
                    .and_then(|state| state.take(&record));
                if endpoint.is_none() {
                    self.unpaired += 1;
                }
                Some(Request {
                    record: *record,
                    endpoint,
                })
            }
        }
    }
}

/// Read a byte range as lines, keeping a half-written trailing line for later.
pub fn split_complete_lines(text: &str) -> (&str, usize) {
    match text.rfind('\n') {
        Some(index) => (&text[..index], index + 1),
        None => ("", 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../fixtures/native/droid-log.synthetic.log");

    fn parsed() -> Vec<Parsed> {
        FIXTURE.lines().filter_map(parse_line).collect()
    }

    #[test]
    fn request_counters_and_endpoints_are_read_from_the_fixture() {
        let records = parsed();
        assert_eq!(records.len(), 9);
        let usage: Vec<_> = records
            .iter()
            .filter_map(|r| match r {
                Parsed::Usage(u) => Some(u),
                _ => None,
            })
            .collect();
        assert_eq!(usage.len(), 5);
        let first = usage[0];
        assert_eq!(first.session_uuid, "11111111-1111-4111-8111-111111111111");
        assert_eq!(first.selector, "custom:vendor-model-0");
        assert_eq!(first.tokens.input, Some(1200));
        assert_eq!(first.tokens.thinking, Some(300));
        assert_eq!(first.byok, Some(true));
        assert_eq!(first.attempt, 1);
        let second = usage[1];
        assert_eq!(second.tokens.cache_write, Some(4096));
        assert_eq!(
            second.tokens.input.unwrap()
                + second.tokens.cache_read.unwrap()
                + second.tokens.cache_write.unwrap(),
            second.total_input.unwrap()
        );
        assert!(second.tokens.thinking.unwrap() > second.tokens.output.unwrap());
        assert_eq!(usage[2].attempt, 2);
        assert_eq!(usage[4].byok, Some(false));
        assert!(parse_line("[2026-01-01T00:00:00Z] INFO: unrelated | Context: {}").is_none());
        assert!(parse_line("not a log line").is_none());
    }

    #[test]
    fn shared_selector_is_paird_to_the_endpoint_that_won_the_request() {
        let mut pairing = Pairing::default();
        let mut requests = vec![];
        for record in parsed() {
            if let Some(request) = pairing.feed(record) {
                requests.push(request);
            }
        }
        assert_eq!(requests.len(), 5);
        let endpoints: Vec<_> = requests
            .iter()
            .map(|r| {
                r.endpoint
                    .as_ref()
                    .map(|e| (e.endpoint.clone(), e.api_model.clone()))
            })
            .collect();
        assert_eq!(
            endpoints[0],
            Some(("https://qa.example.invalid".into(), "vendor-model".into()))
        );
        assert_eq!(
            endpoints[1],
            Some(("https://qa.example.invalid".into(), "vendor-model".into()))
        );
        assert_eq!(
            endpoints[2],
            Some(("https://qa.example.invalid".into(), "vendor-model".into()))
        );
        // Same selector id, different provider: the endpoint keeps them apart.
        assert_eq!(
            endpoints[3],
            Some((
                "https://zzshu.example.invalid".into(),
                "vendor-model".into()
            ))
        );
        assert_eq!(endpoints[4], None);
        assert_eq!(pairing.unpaired, 1);
        assert_ne!(endpoints[0], endpoints[3]);
    }

    #[test]
    fn replaying_an_already_read_log_never_hands_stale_evidence_to_a_request() {
        let all = parsed();
        let mut pairing = Pairing::default();
        let mut first = vec![];
        for record in all.iter().take(4).cloned() {
            if let Some(request) = pairing.feed(record) {
                first.push(request);
            }
        }
        assert_eq!(first.len(), 2);
        assert!(first[1].endpoint.is_some());
        // The file is read again from the start, as happens after rotation.
        let mut replayed = vec![];
        for record in all {
            if let Some(request) = pairing.feed(record) {
                replayed.push(request);
            }
        }
        assert_eq!(replayed.len(), 5);
        assert_eq!(replayed[0].record.selector, "custom:vendor-model-0");
        // The replayed request must not be handed the evidence of a later one.
        assert_eq!(replayed[0].endpoint, None);
        assert_eq!(replayed[1].endpoint, None);
        assert_eq!(
            replayed[2].endpoint.as_ref().map(|e| e.endpoint.clone()),
            Some("https://qa.example.invalid".into())
        );
        assert_eq!(
            replayed[3].endpoint.as_ref().map(|e| e.endpoint.clone()),
            Some("https://zzshu.example.invalid".into())
        );
        assert_eq!(replayed[4].endpoint, None);
    }

    #[test]
    fn line_splitting_keeps_a_half_written_tail() {
        let (complete, consumed) = split_complete_lines("a\nb\nc");
        assert_eq!(complete, "a\nb");
        assert_eq!(consumed, 4);
        let (none, empty) = split_complete_lines("partial");
        assert_eq!(none, "");
        assert_eq!(empty, 0);
    }

    #[test]
    fn reused_trace_span_and_same_second_requests_keep_separate_evidence() {
        let first: String = FIXTURE.lines().take(2).map(|l| format!("{l}\n")).collect();
        let second = first
            .replace("01.000Z", "01.100Z")
            .replace("01.060Z", "01.160Z")
            .replace("resp-synthetic-1", "resp-synthetic-X")
            .replace("qa.example.invalid", "zzshu.example.invalid");
        let mut pairs = Pairing::default();
        let requests: Vec<_> = format!("{first}{second}")
            .lines()
            .filter_map(parse_line)
            .filter_map(|p| pairs.feed(p))
            .collect();
        assert_eq!(requests.len(), 2);
        assert!(requests.iter().all(|r| r.endpoint.is_some()));
        assert_ne!(
            requests[0].endpoint.as_ref().unwrap().endpoint,
            requests[1].endpoint.as_ref().unwrap().endpoint
        );
    }
}
