//! Unified filtering and aggregation over the usage ledger.
//!
//! The backend owns the filter so the summary, the trends, the groups, the paged
//! detail and the CSV export can never disagree about which records are in scope.
//! Totals are summed in SQL from exact integers; costs are summed from fixed
//! point units so no floating point rounding creeps into a currency total.

use super::{cost, identity};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Sentinel used by the UI for the "unknown" bucket of a filter.
pub const UNKNOWN: &str = "unknown";

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Filter {
    pub from: i64,
    pub to: i64,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub granularity: String,
    #[serde(default = "yes")]
    pub include_unknown_time: bool,
    #[serde(default)]
    pub bucket: String,
    #[serde(default)]
    pub offset_minutes: i32,
}

fn yes() -> bool {
    true
}

impl Filter {
    fn bucket_seconds(&self) -> i64 {
        match self.bucket.as_str() {
            "hour" => 3600,
            _ => 86400,
        }
    }
    fn bucket_of(&self, column: &str) -> String {
        let step = self.bucket_seconds();
        let offset = i64::from(self.offset_minutes) * 60;
        format!("(({column}+{offset})/{step})*{step}-{offset}")
    }
}

struct Scope {
    clause: String,
    params: Vec<Box<dyn rusqlite::ToSql>>,
}

fn scope(filter: &Filter) -> Scope {
    let mut clause = if filter.include_unknown_time {
        String::from("((time_confidence<>'unknown' AND occurred_at>=?1 AND occurred_at<?2) OR time_confidence='unknown')")
    } else {
        String::from("occurred_at>=?1 AND occurred_at<?2")
    };
    let mut params: Vec<Box<dyn rusqlite::ToSql>> =
        vec![Box::new(filter.from), Box::new(filter.to)];
    if !filter.source.is_empty() {
        params.push(Box::new(filter.source.clone()));
        clause.push_str(&format!(" AND source=?{}", params.len()));
    }
    if !filter.provider.is_empty() {
        clause.push_str(&match filter.provider == UNKNOWN {
            true => " AND provider_id_at_event IS NULL".to_string(),
            false => {
                params.push(Box::new(filter.provider.clone()));
                format!(" AND provider_id_at_event=?{}", params.len())
            }
        });
    }
    if !filter.model.is_empty() {
        clause.push_str(&match filter.model == UNKNOWN {
            true => " AND api_model_id IS NULL".to_string(),
            false => {
                params.push(Box::new(filter.model.clone()));
                format!(" AND api_model_id=?{}", params.len())
            }
        });
    }
    if !filter.granularity.is_empty() {
        params.push(Box::new(filter.granularity.clone()));
        clause.push_str(&format!(" AND granularity=?{}", params.len()));
    }
    if !filter.include_unknown_time {
        clause.push_str(" AND time_confidence<>'unknown'");
    }
    Scope { clause, params }
}

fn with(params: &[Box<dyn rusqlite::ToSql>]) -> Vec<&dyn rusqlite::ToSql> {
    params.iter().map(|value| value.as_ref()).collect()
}

#[derive(Default, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TokenTotals {
    pub input: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    pub output: i64,
    pub thinking: i64,
    pub unknown_input: i64,
    pub unknown_cache_read: i64,
    pub unknown_cache_write: i64,
    pub unknown_output: i64,
    pub unknown_thinking: i64,
}

impl TokenTotals {
    fn add(&mut self, other: &TokenTotals) {
        self.input += other.input;
        self.cache_read += other.cache_read;
        self.cache_write += other.cache_write;
        self.output += other.output;
        self.thinking += other.thinking;
        self.unknown_input += other.unknown_input;
        self.unknown_cache_read += other.unknown_cache_read;
        self.unknown_cache_write += other.unknown_cache_write;
        self.unknown_output += other.unknown_output;
        self.unknown_thinking += other.unknown_thinking;
    }
    pub fn total(&self) -> i64 {
        self.input + self.cache_read + self.cache_write + self.output + self.thinking
    }
    pub fn unknown_fields(&self) -> i64 {
        self.unknown_input
            + self.unknown_cache_read
            + self.unknown_cache_write
            + self.unknown_output
            + self.unknown_thinking
    }
}

const SUMS: &str = "COALESCE(SUM(input_tokens),0),COALESCE(SUM(cache_read_tokens),0),COALESCE(SUM(cache_write_tokens),0),COALESCE(SUM(output_tokens),0),COALESCE(SUM(thinking_tokens),0),COALESCE(SUM(input_tokens IS NULL),0),COALESCE(SUM(cache_read_tokens IS NULL),0),COALESCE(SUM(cache_write_tokens IS NULL),0),COALESCE(SUM(output_tokens IS NULL),0),COALESCE(SUM(thinking_tokens IS NULL),0)";

fn read_totals(row: &rusqlite::Row<'_>, at: usize) -> rusqlite::Result<TokenTotals> {
    Ok(TokenTotals {
        input: row.get(at)?,
        cache_read: row.get(at + 1)?,
        cache_write: row.get(at + 2)?,
        output: row.get(at + 3)?,
        thinking: row.get(at + 4)?,
        unknown_input: row.get(at + 5)?,
        unknown_cache_read: row.get(at + 6)?,
        unknown_cache_write: row.get(at + 7)?,
        unknown_output: row.get(at + 8)?,
        unknown_thinking: row.get(at + 9)?,
    })
}

#[derive(Default, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub tokens: TokenTotals,
    pub records: i64,
    pub sessions: i64,
    pub estimates: Vec<CurrencyTotal>,
    pub priced_records: i64,
    pub billable_records: i64,
    pub partial_records: i64,
    pub unknown_records: i64,
    pub credits: f64,
    pub credits_records: i64,
    pub unknown_source_records: i64,
}

#[derive(Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CurrencyTotal {
    pub currency: String,
    pub cost: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TrendBucket {
    pub start: i64,
    pub tokens: TokenTotals,
    pub records: i64,
    pub estimates: Vec<CurrencyTotal>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub key: String,
    pub label: String,
    pub tokens: TokenTotals,
    pub records: i64,
    pub estimates: Vec<CurrencyTotal>,
    pub priced_records: i64,
    pub unpriced_records: i64,
    pub unknown_records: i64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RecordRow {
    pub id: String,
    pub occurred_at: i64,
    pub session: String,
    pub granularity: String,
    pub time_confidence: String,
    pub source: String,
    pub provider: Option<String>,
    pub provider_label: String,
    pub selector: Option<String>,
    pub api_model: Option<String>,
    pub input: Option<i64>,
    pub cache_read: Option<i64>,
    pub cache_write: Option<i64>,
    pub output: Option<i64>,
    pub thinking: Option<i64>,
    pub credits: Option<f64>,
    pub cost: Option<String>,
    pub currency: Option<String>,
    pub evidence: String,
    pub interval_start: Option<i64>,
    pub interval_end: Option<i64>,
    pub complete: bool,
    pub request_id: Option<String>,
    pub attempt: Option<i64>,
}

#[derive(Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Coverage {
    pub ledger_sessions: i64,
    pub ledger_tokens: i64,
    pub ledger_unknown_fields: i64,
    pub detail_tokens: i64,
    pub unallocated_tokens: i64,
    pub detail_records: i64,
    pub request_records: i64,
    pub baseline_tokens: i64,
    pub window_from: Option<i64>,
    pub window_to: Option<i64>,
    pub endpoint_resolved_records: i64,
    pub endpoint_unresolved_records: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub summary: Summary,
    pub trends: Vec<TrendBucket>,
    pub unassigned: Vec<CurrencyTotal>,
    pub unassigned_tokens: TokenTotals,
    pub providers: Vec<Group>,
    pub models: Vec<Group>,
    pub records: Vec<RecordRow>,
    pub total_records: i64,
    pub coverage: Coverage,
    pub prices: Vec<super::pricing::StoredPrice>,
    pub providers_filter: Vec<Option<String>>,
    pub provider_labels: BTreeMap<String, String>,
    pub models_filter: Vec<String>,
}

pub fn page(db: &Connection, filter: &Filter, limit: i64, offset: i64) -> Result<Page, String> {
    if filter.to <= filter.from {
        return Err("时间范围无效".into());
    }
    let scope = scope(filter);
    let params = with(&scope.params);
    let (summary, trends, unassigned_tokens, unassigned, providers, models) =
        read_stats(db, filter, &scope.clause, &params)?;
    let total_records = summary.records;
    let records = read_records(db, &scope.clause, &params, limit, offset)?;
    let coverage = read_coverage(db)?;
    let endpoints = endpoint_labels(db)?;
    let prices = super::pricing::list_prices(db)?;
    let mut providers_filter: Vec<Option<String>> = db
        .prepare("SELECT DISTINCT provider_id_at_event FROM usage_events")
        .map_err(|_| "无法读取供应商筛选")?
        .query_map([], |r| r.get(0))
        .map_err(|_| "无法读取供应商筛选")?
        .collect::<Result<_, _>>()
        .map_err(|_| "供应商筛选无效")?;
    providers_filter.sort();
    let mut models_filter: Vec<String> = db
        .prepare("SELECT DISTINCT api_model_id FROM usage_events WHERE api_model_id IS NOT NULL AND (?1='' OR provider_id_at_event=?1 OR (?1='unknown' AND provider_id_at_event IS NULL))")
        .map_err(|_| "无法读取模型筛选")?
        .query_map([&filter.provider], |r| r.get(0))
        .map_err(|_| "无法读取模型筛选")?
        .collect::<Result<_, _>>()
        .map_err(|_| "模型筛选无效")?;
    models_filter.sort();

    Ok(Page {
        summary,
        trends,
        unassigned,
        unassigned_tokens,
        providers,
        models,
        records,
        total_records,
        coverage,
        prices,
        providers_filter,
        provider_labels: endpoints,
        models_filter,
    })
}

type Stats = (
    Summary,
    Vec<TrendBucket>,
    TokenTotals,
    Vec<CurrencyTotal>,
    Vec<Group>,
    Vec<Group>,
);
fn add_cost(values: &mut Vec<CurrencyTotal>, currency: &str, units: i128, present: bool) {
    if !present || currency.is_empty() {
        return;
    }
    if let Some(v) = values.iter_mut().find(|v| v.currency == currency) {
        v.cost = cost::format_cost(cost::parse_cost(&v.cost).unwrap_or(0) + units);
    } else {
        values.push(CurrencyTotal {
            currency: currency.into(),
            cost: cost::format_cost(units),
        });
    }
}
fn read_stats(
    db: &Connection,
    filter: &Filter,
    clause: &str,
    params: &[&dyn rusqlite::ToSql],
) -> Result<Stats, String> {
    let bucket = filter.bucket_of("occurred_at");
    let start = filter.bucket_of("interval_start");
    let end = filter.bucket_of("interval_end");
    let bucket=format!("CASE WHEN time_confidence='unknown' OR (time_confidence='interval' AND ({start}<>{end})) THEN NULL ELSE {bucket} END");
    // Scan the filtered ledger once. Derive all cards/charts/groups from these
    // disjoint slices so each has exactly the same population and null semantics.
    let sql=format!("SELECT provider_id_at_event,api_model_id,COALESCE(currency,''),{bucket} AS b,{SUMS},COUNT(*),COALESCE(SUM(cost_units),0),COUNT(cost_units),COALESCE(SUM(cost_units IS NOT NULL AND cost_complete=1),0),COALESCE(SUM(cost_units IS NOT NULL AND cost_complete=0),0),COALESCE(SUM(source='api' AND cost_units IS NULL),0),COALESCE(SUM(official_credits),0),COUNT(official_credits),COALESCE(SUM(source='unknown'),0),COALESCE(SUM(input_tokens IS NULL AND output_tokens IS NULL AND cache_read_tokens IS NULL AND cache_write_tokens IS NULL AND thinking_tokens IS NULL),0) FROM usage_events WHERE {clause} GROUP BY provider_id_at_event,api_model_id,currency,b ORDER BY provider_id_at_event,api_model_id,currency,b");
    let mut st = db.prepare(&sql).map_err(|_| "无法准备统计")?;
    let rows = st
        .query_map(params, |r| {
            Ok((
                r.get::<_, Option<String>>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<i64>>(3)?,
                read_totals(r, 4)?,
                r.get::<_, i64>(14)?,
                r.get::<_, i64>(15)?,
                r.get::<_, i64>(16)?,
                r.get::<_, i64>(17)?,
                r.get::<_, i64>(18)?,
                r.get::<_, i64>(19)?,
                r.get::<_, f64>(20)?,
                r.get::<_, i64>(21)?,
                r.get::<_, i64>(22)?,
                r.get::<_, i64>(23)?,
            ))
        })
        .map_err(|_| "无法聚合统计")?;
    let labels = endpoint_labels(db)?;
    let mut summary = Summary::default();
    let mut trends: BTreeMap<i64, TrendBucket> = BTreeMap::new();
    let mut providers: BTreeMap<String, Group> = BTreeMap::new();
    let mut models: BTreeMap<String, Group> = BTreeMap::new();
    let mut unassigned_tokens = TokenTotals::default();
    let mut unassigned = vec![];
    for row in rows {
        let (
            provider,
            model,
            currency,
            bucket,
            tokens,
            records,
            units,
            costs,
            priced,
            partial,
            unpriced,
            credits,
            credit_records,
            unknown_source,
            unknown,
        ) = row.map_err(|_| "统计结构无效或金额合计溢出")?;
        summary.tokens.add(&tokens);
        summary.records += records;
        summary.priced_records += priced;
        summary.partial_records += partial;
        summary.billable_records += unpriced;
        summary.credits += credits;
        summary.credits_records += credit_records;
        summary.unknown_source_records += unknown_source;
        summary.unknown_records += unknown;
        add_cost(&mut summary.estimates, &currency, units as i128, costs > 0);
        if let Some(start) = bucket {
            let v = trends.entry(start).or_insert_with(|| TrendBucket {
                start,
                tokens: TokenTotals::default(),
                records: 0,
                estimates: vec![],
            });
            v.tokens.add(&tokens);
            v.records += records;
            add_cost(&mut v.estimates, &currency, units as i128, costs > 0);
        } else {
            unassigned_tokens.add(&tokens);
            add_cost(&mut unassigned, &currency, units as i128, costs > 0);
        }
        for (map, key, is_provider) in [
            (&mut providers, provider, true),
            (&mut models, model, false),
        ] {
            let unknown = key.is_none();
            let key = key.unwrap_or_else(|| UNKNOWN.into());
            let label = if unknown {
                "未知".into()
            } else if is_provider {
                labels.get(&key).cloned().unwrap_or_else(|| key.clone())
            } else {
                key.clone()
            };
            let v = map.entry(key.clone()).or_insert_with(|| Group {
                key,
                label,
                tokens: TokenTotals::default(),
                records: 0,
                estimates: vec![],
                priced_records: 0,
                unpriced_records: 0,
                unknown_records: 0,
            });
            v.tokens.add(&tokens);
            v.records += records;
            v.priced_records += priced;
            v.unpriced_records += unpriced;
            v.unknown_records += if unknown { records } else { 0 };
            add_cost(&mut v.estimates, &currency, units as i128, costs > 0);
        }
    }
    summary.sessions = db
        .query_row(
            &format!("SELECT COUNT(DISTINCT session_id) FROM usage_events WHERE {clause}"),
            params,
            |r| r.get(0),
        )
        .map_err(|_| "无法统计会话")?;
    Ok((
        summary,
        trends.into_values().collect(),
        unassigned_tokens,
        unassigned,
        providers.into_values().collect(),
        models.into_values().collect(),
    ))
}

/// Endpoint identity to a readable label: the recorded label, else the provider
/// that currently owns that endpoint, else the endpoint itself.
fn endpoint_labels(db: &Connection) -> Result<BTreeMap<String, String>, String> {
    let mut labels: BTreeMap<String, String> = db
        .prepare("SELECT id,COALESCE(label,endpoint) FROM usage_endpoints")
        .map_err(|_| "无法读取端点")?
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|_| "无法读取端点")?
        .collect::<Result<_, _>>()
        .map_err(|_| "端点结构无效")?;
    for (endpoint, _, name) in identity::endpoint_providers(db)? {
        let id = identity::endpoint_id(&endpoint);
        if let Some(label) = labels.get_mut(&id) {
            if label != &endpoint {
                continue;
            }
            *label = name;
        } else {
            labels.insert(id, name);
        }
    }
    Ok(labels)
}

pub fn filtered_records(
    db: &Connection,
    filter: &Filter,
    limit: i64,
    offset: i64,
) -> Result<Vec<RecordRow>, String> {
    if filter.to <= filter.from {
        return Err("时间范围无效".into());
    }
    let scope = scope(filter);
    read_records(db, &scope.clause, &with(&scope.params), limit, offset)
}

fn read_records(
    db: &Connection,
    clause: &str,
    params: &[&dyn rusqlite::ToSql],
    limit: i64,
    offset: i64,
) -> Result<Vec<RecordRow>, String> {
    let labels = endpoint_labels(db)?;
    let limit = limit.clamp(1, 500);
    let offset = offset.max(0);
    let mut statement = db
        .prepare(&format!(
            "SELECT id,occurred_at,session_id,granularity,time_confidence,source,provider_id_at_event,model_selector,api_model_id,input_tokens,cache_read_tokens,cache_write_tokens,output_tokens,thinking_tokens,official_credits,estimated_cost,currency,evidence,interval_start,interval_end,cost_complete,request_id,attempt FROM usage_events WHERE {clause} ORDER BY occurred_at DESC,id DESC LIMIT ?{} OFFSET ?{}",
            params.len() + 1,
            params.len() + 2
        ))
        .map_err(|_| "无法读取明细")?;
    let mut all: Vec<&dyn rusqlite::ToSql> = params.to_vec();
    all.push(&limit);
    all.push(&offset);
    let rows = statement
        .query_map(all.as_slice(), |row| {
            let provider: Option<String> = row.get(6)?;
            Ok(RecordRow {
                id: row.get(0)?,
                occurred_at: row.get(1)?,
                session: row.get(2)?,
                granularity: row.get(3)?,
                time_confidence: row.get(4)?,
                source: row.get(5)?,
                provider_label: match provider.as_deref() {
                    Some(key) => labels.get(key).cloned().unwrap_or_else(|| key.to_owned()),
                    None => "未知".into(),
                },
                provider,
                selector: row.get(7)?,
                api_model: row.get(8)?,
                input: row.get(9)?,
                cache_read: row.get(10)?,
                cache_write: row.get(11)?,
                output: row.get(12)?,
                thinking: row.get(13)?,
                credits: row.get(14)?,
                cost: row.get(15)?,
                currency: row.get(16)?,
                evidence: row.get(17)?,
                interval_start: row.get(18)?,
                interval_end: row.get(19)?,
                complete: row.get(20)?,
                request_id: row.get(21)?,
                attempt: row.get(22)?,
            })
        })
        .map_err(|_| "无法读取明细")?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|_| "明细结构无效".into())
}

/// What the ledger knows versus what detail accounts for. The difference is
/// reported instead of being hidden or silently distributed.
fn read_coverage(db: &Connection) -> Result<Coverage, String> {
    let (ledger_sessions, ledger_tokens): (i64, i64) = db
        .query_row(
            "SELECT COUNT(*),COALESCE(SUM(COALESCE(own_input_tokens,0)+COALESCE(own_cache_read_tokens,0)+COALESCE(own_cache_write_tokens,0)+COALESCE(own_output_tokens,0)+COALESCE(own_thinking_tokens,0)),0) FROM usage_sessions WHERE snapshot_at IS NOT NULL",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| "无法读取会话账本")?;
    let ledger_unknown_fields: i64 = db
        .query_row(
            "SELECT COALESCE(SUM((own_input_tokens IS NULL)+(own_cache_read_tokens IS NULL)+(own_cache_write_tokens IS NULL)+(own_output_tokens IS NULL)+(own_thinking_tokens IS NULL)),0) FROM usage_sessions WHERE snapshot_at IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .map_err(|_| "无法读取会话账本")?;
    let detail_tokens: i64 = db
        .query_row(
            "SELECT COALESCE(SUM(COALESCE(input_tokens,0)+COALESCE(cache_read_tokens,0)+COALESCE(cache_write_tokens,0)+COALESCE(output_tokens,0)+COALESCE(thinking_tokens,0)),0) FROM usage_events",
            [],
            |r| r.get(0),
        )
        .map_err(|_| "无法汇总明细")?;
    let (detail_records, request_records, baseline_tokens): (i64, i64, i64) = db
        .query_row(
            "SELECT COUNT(*),COALESCE(SUM(granularity='request'),0),COALESCE(SUM(CASE WHEN time_confidence='unknown' THEN COALESCE(input_tokens,0)+COALESCE(cache_read_tokens,0)+COALESCE(cache_write_tokens,0)+COALESCE(output_tokens,0)+COALESCE(thinking_tokens,0) ELSE 0 END),0) FROM usage_events",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|_| "无法汇总明细")?;
    let (window_from, window_to): (Option<i64>, Option<i64>) = db
        .query_row(
            "SELECT MIN(occurred_at),MAX(occurred_at) FROM usage_events WHERE granularity='request'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| "无法读取覆盖区间")?;
    let resolved: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE endpoint_id IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .map_err(|_| "无法统计归属")?;
    let unresolved: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM usage_events WHERE endpoint_id IS NULL",
            [],
            |r| r.get(0),
        )
        .map_err(|_| "无法统计归属")?;
    Ok(Coverage {
        ledger_sessions,
        ledger_tokens,
        ledger_unknown_fields,
        detail_tokens,
        unallocated_tokens: ledger_tokens - detail_tokens,
        detail_records,
        request_records,
        baseline_tokens,
        window_from,
        window_to,
        endpoint_resolved_records: resolved,
        endpoint_unresolved_records: unresolved,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Export {
    pub csv: String,
    pub rows: i64,
}

fn cell(value: &str) -> String {
    let mut text = value.to_owned();
    if text.trim_start().starts_with(['=', '+', '-', '@'])
        || text.starts_with('\t')
        || text.starts_with('\r')
    {
        text.insert(0, '\'');
    }
    format!("\"{}\"", text.replace('"', "\"\""))
}

/// One consistent read of every matching record, for the same filter the page
/// shows. Ordering is total so a paged read and the export agree.
pub fn export_csv(db: &Connection, filter: &Filter) -> Result<Export, String> {
    if filter.to <= filter.from {
        return Err("时间范围无效".into());
    }
    let scope = scope(filter);
    let params = with(&scope.params);
    let mut csv = String::from("\u{feff}时间(UTC),会话,记录类型,时间可信度,来源,供应商,Droid选择器,API模型,普通输入,缓存读,缓存写,输出,思考,官方积分,估算费用,币种,定价完整度\r\n");
    let mut rows = 0;
    loop {
        let batch = read_records(db, &scope.clause, &params, 500, rows)?;
        if batch.is_empty() {
            break;
        }
        for r in &batch {
            let number = |v: Option<i64>| v.map(|v| v.to_string()).unwrap_or_else(|| "未知".into());
            let time = if r.time_confidence == "unknown" {
                "时间未知".into()
            } else {
                chrono::DateTime::from_timestamp(r.occurred_at, 0)
                    .map(|t| t.to_rfc3339())
                    .unwrap_or_else(|| "时间未知".into())
            };
            let fields = [
                time,
                r.session.clone(),
                r.granularity.clone(),
                r.time_confidence.clone(),
                r.source.clone(),
                r.provider_label.clone(),
                r.selector.clone().unwrap_or_default(),
                r.api_model.clone().unwrap_or_default(),
                number(r.input),
                number(r.cache_read),
                number(r.cache_write),
                number(r.output),
                number(r.thinking),
                r.credits
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "未知".into()),
                r.cost.clone().unwrap_or_else(|| "未定价".into()),
                r.currency.clone().unwrap_or_default(),
                if r.complete {
                    "完整".into()
                } else {
                    "未定价或部分估算".into()
                },
            ];
            csv.push_str(&fields.map(|v| cell(&v)).join(","));
            csv.push_str("\r\n");
        }
        rows += batch.len() as i64;
    }
    Ok(Export { csv, rows })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn db() -> Connection {
        let mut db = Connection::open_in_memory().unwrap();
        crate::db::schema::migrate_to(&mut db, crate::db::schema::SCHEMA_VERSION).unwrap();
        db
    }
    fn filter() -> Filter {
        serde_json::from_value(serde_json::json!({"from":0,"to":200000,"includeUnknownTime":false}))
            .unwrap()
    }
    fn event(db: &Connection, id: &str, at: i64, kind: &str, time: &str) {
        db.execute("INSERT INTO usage_events(id,source_file,event_key,session_id,occurred_at,source,input_tokens,cache_read_tokens,cache_write_tokens,output_tokens,thinking_tokens,granularity,time_confidence) VALUES (?1,'test',?1,'session',?2,'api',100,0,0,0,0,?3,?4)",rusqlite::params![id,at,kind,time]).unwrap();
    }
    #[test]
    fn empty_and_filtered_out_records_return_zero_not_errors() {
        let db = db();
        let f = filter();
        assert_eq!(page(&db, &f, 100, 0).unwrap().summary.records, 0);
        assert_eq!(export_csv(&db, &f).unwrap().rows, 0);
        event(&db, "outside", 300000, "request", "event");
        assert_eq!(page(&db, &f, 100, 0).unwrap().summary.records, 0);
    }
    #[test]
    fn unknown_history_is_separate_from_dates_and_never_enters_trends() {
        let db = db();
        let mut f = filter();
        event(&db, "history", 999999, "historical_baseline", "unknown");
        event(&db, "dated", 100, "request", "event");
        let p = page(&db, &f, 100, 0).unwrap();
        assert_eq!(p.summary.tokens.total(), 100);
        f.include_unknown_time = true;
        let p = page(&db, &f, 100, 0).unwrap();
        assert_eq!(p.summary.tokens.total(), 200);
        assert_eq!(p.trends.len(), 1);
        assert_eq!(p.trends[0].tokens.total(), 100);
        assert_eq!(p.unassigned_tokens.total(), 100);
        let export = export_csv(&db, &f).unwrap();
        assert_eq!(export.rows, 2);
        assert!(export.csv.contains("时间未知"));
        assert!(export.csv.contains("1970-01-01T00:01:40"));
    }
    #[test]
    fn upgraded_v1_keeps_exact_cost_model_and_cumulative_granularity() {
        let mut db = Connection::open_in_memory().unwrap();
        crate::db::schema::migrate_to(&mut db, 1).unwrap();
        db.execute("INSERT INTO usage_events(id,source_file,event_key,session_id,occurred_at,source,model_id_at_event,input_tokens,estimated_cost,currency) VALUES ('old','export','cumulative:old','s',100,'api','model',100,'1.000000000001','USD')",[]).unwrap();
        crate::db::schema::migrate_to(&mut db, crate::db::schema::SCHEMA_VERSION).unwrap();
        let p = page(&db, &filter(), 100, 0).unwrap();
        assert_eq!(p.summary.estimates[0].cost, "1.000000000001");
        assert_eq!(p.records[0].api_model.as_deref(), Some("model"));
        assert_eq!(p.records[0].granularity, "legacy_cumulative");
    }
    #[test]
    fn large_dataset_totals_pagination_export_and_cost_coverage_agree() {
        let mut db = db();
        let tx = db.transaction().unwrap();
        for i in 0..100_000 {
            event(&tx, &format!("e{i:06}"), 100, "request", "event");
        }
        tx.execute("UPDATE usage_events SET estimated_cost='0.000000000001',cost_units=1,currency='USD',cost_complete=1",[]).unwrap();
        tx.execute(
            "UPDATE usage_events SET cost_complete=0 WHERE id='e000000'",
            [],
        )
        .unwrap();
        tx.commit().unwrap();
        let f = filter();
        let p = page(&db, &f, 100, 0).unwrap();
        assert_eq!(p.summary.tokens.total(), 10_000_000);
        assert_eq!(p.summary.priced_records, 99_999);
        assert_eq!(p.summary.partial_records, 1);
        assert_eq!(p.models[0].tokens.total(), p.summary.tokens.total());
        assert_eq!(p.trends[0].tokens.total(), p.summary.tokens.total());
        assert_eq!(p.summary.estimates[0].cost, "0.0000001");
        assert_eq!(p.records.len(), 100);
        assert_eq!(page(&db, &f, 100, 99_990).unwrap().records.len(), 10);
        let export = export_csv(&db, &f).unwrap();
        assert_eq!(export.rows, 100_000);
        assert_eq!(export.csv.lines().count(), 100_001);
    }
}
