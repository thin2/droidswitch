//! Provider-scoped pricing.
//!
//! A price belongs to one provider and one API model. A model-only price is an
//! explicit generic fallback the user created; it is never borrowed from another
//! provider. Unknown attribution can only use the generic fallback.

use super::cost::Price;
use crate::workspace::Store;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredPrice {
    pub id: String,
    pub provider_id: String,
    pub api_model_id: String,
    pub currency: String,
    pub input: Option<String>,
    pub cache_read: Option<String>,
    pub cache_write: Option<String>,
    pub output: Option<String>,
    pub thinking: Option<String>,
    pub source: String,
    pub effective_at: i64,
    pub created_at: i64,
}

const COLUMNS: &str = "id,provider_id,api_model_id,currency,input_per_million,cache_read_per_million,cache_write_per_million,output_per_million,thinking_per_million,source,effective_at,created_at";

fn row_to_price(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredPrice> {
    Ok(StoredPrice {
        id: row.get(0)?,
        provider_id: row.get(1)?,
        api_model_id: row.get(2)?,
        currency: row.get(3)?,
        input: row.get(4)?,
        cache_read: row.get(5)?,
        cache_write: row.get(6)?,
        output: row.get(7)?,
        thinking: row.get(8)?,
        source: row.get(9)?,
        effective_at: row.get(10)?,
        created_at: row.get(11)?,
    })
}

fn to_price(stored: &StoredPrice) -> Price {
    Price {
        provider_id: stored.provider_id.clone(),
        model: stored.api_model_id.clone(),
        currency: stored.currency.clone(),
        input: stored.input.clone(),
        cache_read: stored.cache_read.clone(),
        cache_write: stored.cache_write.clone(),
        output: stored.output.clone(),
        thinking: stored.thinking.clone(),
        effective_at: stored.effective_at,
    }
}

fn lookup(
    db: &Connection,
    provider: &str,
    api_model_id: &str,
    at: i64,
) -> Result<Option<StoredPrice>, String> {
    db.query_row(
        &format!("SELECT {COLUMNS} FROM usage_prices WHERE api_model_id=?1 AND provider_id=?2 AND effective_at<=?3 ORDER BY effective_at DESC,id DESC LIMIT 1"),
        rusqlite::params![api_model_id, provider, at],
        row_to_price,
    )
    .optional()
    .map_err(|_| "无法读取历史单价".into())
}

/// The price that applied to one request: the provider's own price when the
/// provider is known, otherwise only the explicit generic fallback.
pub fn price_at(
    db: &Connection,
    provider: Option<&str>,
    api_model_id: &str,
    at: i64,
) -> Result<Option<(String, String, Price)>, String> {
    let provider = provider.unwrap_or("");
    let stored = if !provider.is_empty() {
        // Known provider: try provider-specific, then fallback.
        match lookup(db, provider, api_model_id, at)? {
            Some(found) => Some(found),
            None => lookup(db, "", api_model_id, at)?,
        }
    } else {
        // Unknown provider: only generic fallback.
        lookup(db, "", api_model_id, at)?
    };
    Ok(stored
        .map(|stored| {
            let currency = stored.currency.clone();
            (stored.id.clone(), currency, to_price(&stored))
        })
        .or_else(|| super::reference::price(api_model_id)))
}

#[tauri::command]
pub fn save_price(store: tauri::State<'_, Store>, price: Price) -> Result<(), String> {
    price.validate()?;
    let db = store.db.lock().map_err(|_| "数据库繁忙")?;
    db.execute("INSERT INTO usage_prices(id,provider_id,api_model_id,currency,input_per_million,cache_read_per_million,cache_write_per_million,output_per_million,thinking_per_million,source,effective_at,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'manual',?10,unixepoch())",
        rusqlite::params![uuid::Uuid::new_v4().to_string(),price.provider_id,price.model,price.currency,price.input,price.cache_read,price.cache_write,price.output,price.thinking,price.effective_at])
        .map_err(|_|"单价保存失败，同一供应商/模型/币种/生效时间不能重复")?;
    Ok(())
}

pub fn list_prices(db: &Connection) -> Result<Vec<StoredPrice>, String> {
    let mut prices = db
        .prepare(&format!(
        "SELECT {COLUMNS} FROM usage_prices ORDER BY api_model_id,provider_id,effective_at DESC"
    ))
        .map_err(|_| "无法读取单价")?
        .query_map([], row_to_price)
        .map_err(|_| "无法读取单价")?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "单价结构无效")?;
    prices.extend(super::reference::prices());
    Ok(prices)
}

/// Coverage per provider and API model: how many billable records exist and how
/// many of them have a price. Unknown counts are never treated as zero.
#[derive(Serialize, Deserialize, Default, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Coverage {
    pub provider_key: Option<String>,
    pub api_model_id: Option<String>,
    pub records: u64,
    pub priced_records: u64,
    pub unpriced_records: u64,
    pub incomplete_records: u64,
    pub unknown_records: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::cost;
    use crate::usage::cost::Tokens;

    fn db() -> Connection {
        let mut db = Connection::open_in_memory().unwrap();
        crate::db::schema::migrate_to(&mut db, crate::db::schema::SCHEMA_VERSION).unwrap();
        db
    }

    fn insert(
        db: &Connection,
        provider: &str,
        model: &str,
        currency: &str,
        input: &str,
        at: i64,
    ) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        db.execute("INSERT INTO usage_prices(id,provider_id,api_model_id,currency,input_per_million,cache_read_per_million,cache_write_per_million,output_per_million,thinking_per_million,source,effective_at,created_at) VALUES (?1,?2,?3,?4,?5,'0','0','0','0','manual',?6,?6)",
            rusqlite::params![id, provider, model, currency, input, at]).unwrap();
        id
    }

    fn input_rate(db: &Connection, provider: Option<&str>, model: &str, at: i64) -> Option<String> {
        price_at(db, provider, model, at)
            .unwrap()
            .map(|(_, _, price)| price.input.unwrap())
    }

    #[test]
    fn same_model_on_two_providers_is_priced_independently_and_never_borrowed() {
        let db = db();
        insert(&db, "", "shared", "USD", "1", 0);
        insert(&db, "qa", "shared", "USD", "2", 0);
        insert(&db, "zzshu", "shared", "USD", "3", 0);
        assert_eq!(
            input_rate(&db, Some("qa"), "shared", 10).as_deref(),
            Some("2")
        );
        assert_eq!(
            input_rate(&db, Some("zzshu"), "shared", 10).as_deref(),
            Some("3")
        );
        // Unknown attribution may only use the explicit generic fallback.
        assert_eq!(input_rate(&db, None, "shared", 10).as_deref(), Some("1"));
        assert_eq!(
            input_rate(&db, Some("other-endpoint"), "shared", 10).as_deref(),
            Some("1")
        );
        assert_eq!(input_rate(&db, Some("qa"), "unpriced", 10), None);
    }

    #[test]
    fn the_price_in_force_at_the_event_time_is_the_one_used() {
        let db = db();
        insert(&db, "qa", "shared", "USD", "5", 100);
        insert(&db, "qa", "shared", "USD", "7", 200);
        assert_eq!(
            input_rate(&db, Some("qa"), "shared", 150).as_deref(),
            Some("5")
        );
        assert_eq!(
            input_rate(&db, Some("qa"), "shared", 200).as_deref(),
            Some("7")
        );
        assert_eq!(input_rate(&db, Some("qa"), "shared", 99), None);
        insert(&db, "qa", "shared", "EUR", "9", 300);
        let currencies: Vec<_> = price_at(&db, Some("qa"), "shared", 150)
            .unwrap()
            .map(|(_, currency, _)| currency)
            .into_iter()
            .collect();
        assert_eq!(currencies, vec!["USD".to_string()]);
        let later_currency = price_at(&db, Some("qa"), "shared", 300)
            .unwrap()
            .map(|(_, currency, _)| currency);
        assert_eq!(later_currency.as_deref(), Some("EUR"));
    }

    #[test]
    fn coverage_counts_unknown_records_separately_from_unpriced_ones() {
        let db = db();
        insert(&db, "qa", "shared", "USD", "1.25", 0);
        let price = price_at(&db, Some("qa"), "shared", 10).unwrap().unwrap().2;
        let billable = Tokens {
            input: Some(1_000_000),
            cache_read: Some(0),
            cache_write: Some(0),
            output: Some(0),
            thinking: Some(0),
        };
        let estimate = cost::estimate(&billable, &price).unwrap();
        assert!(estimate.complete);
        assert_eq!(estimate.cost.unwrap(), "1.25");
        let unknown = cost::estimate(&Tokens::default(), &price).unwrap();
        assert!(unknown.cost.is_none() && !unknown.complete);
        assert_eq!(
            list_prices(&db)
                .unwrap()
                .iter()
                .filter(|p| p.source == "manual")
                .count(),
            1
        );
        assert_eq!(list_prices(&db).unwrap()[0].api_model_id, "shared");
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepricePreview {
    pub token: String,
    pub records: usize,
    pub complete_records: usize,
    pub estimate: String,
    pub currency: String,
}
struct RepricePlan {
    preview: RepricePreview,
    rows: Vec<(String, Option<String>, bool)>,
    price_id: String,
    snapshot: String,
}
fn reprice_plan(
    db: &Connection,
    price_id: &str,
    filter: &super::aggregate::Filter,
) -> Result<RepricePlan, String> {
    let stored = db
        .query_row(
            &format!("SELECT {COLUMNS} FROM usage_prices WHERE id=?1"),
            [price_id],
            row_to_price,
        )
        .map_err(|_| "单价不存在")?;
    let price = to_price(&stored);
    let mut rows = vec![];
    let mut total = 0i128;
    let mut complete_records = 0;
    let mut offset = 0;
    let mut evidence = String::new();
    loop {
        let records = super::aggregate::filtered_records(db, filter, 500, offset)?;
        if records.is_empty() {
            break;
        }
        offset += records.len() as i64;
        for r in records {
            if r.source != "api"
                || r.api_model.as_deref() != Some(&price.model)
                || (!price.provider_id.is_empty()
                    && r.provider.as_deref() != Some(&price.provider_id))
            {
                continue;
            }
            let tokens = super::cost::Tokens {
                input: r.input.map(|v| v as u64),
                cache_read: r.cache_read.map(|v| v as u64),
                cache_write: r.cache_write.map(|v| v as u64),
                output: r.output.map(|v| v as u64),
                thinking: r.thinking.map(|v| v as u64),
            };
            let estimate = super::cost::estimate(&tokens, &price)?;
            if let Some(value) = estimate.cost.as_deref().and_then(super::cost::parse_cost) {
                total = total.checked_add(value).ok_or("费用合计溢出")?;
            }
            complete_records += usize::from(estimate.complete);
            evidence.push_str(&serde_json::to_string(&r).map_err(|_| "无法生成预览")?);
            rows.push((r.id, estimate.cost, estimate.complete));
        }
    }
    let snapshot = serde_json::to_string(&price).map_err(|_| "无法保存单价快照")?;
    evidence.push_str(price_id);
    evidence.push_str(&snapshot);
    let preview = RepricePreview {
        token: crate::config::pipeline::hash(evidence.as_bytes()),
        records: rows.len(),
        complete_records,
        estimate: super::cost::format_cost(total),
        currency: price.currency.clone(),
    };
    Ok(RepricePlan {
        preview,
        rows,
        price_id: price_id.into(),
        snapshot,
    })
}
#[tauri::command]
pub fn preview_usage_reprice(
    store: tauri::State<'_, Store>,
    price_id: String,
    filter: super::aggregate::Filter,
) -> Result<RepricePreview, String> {
    let db = store.db.lock().map_err(|_| "数据库繁忙")?;
    Ok(reprice_plan(&db, &price_id, &filter)?.preview)
}
fn commit_reprice(
    db: &mut Connection,
    price_id: &str,
    filter: &super::aggregate::Filter,
    token: &str,
) -> Result<usize, String> {
    let tx = db.transaction().map_err(|_| "无法开始重新估算")?;
    let plan = reprice_plan(&tx, price_id, filter)?;
    if plan.preview.token != token {
        return Err("数据或单价已变化，请重新预览".into());
    }
    for (id, cost, complete) in &plan.rows {
        let units = cost
            .as_deref()
            .and_then(super::cost::parse_cost)
            .map(i64::try_from)
            .transpose()
            .map_err(|_| "费用超出存储范围")?;
        tx.execute("UPDATE usage_events SET estimated_cost=?2,cost_units=?3,cost_complete=?4,currency=?5,price_id_at_event=?6,price_snapshot=?7 WHERE id=?1",rusqlite::params![id,cost,units,complete,plan.preview.currency,plan.price_id,plan.snapshot]).map_err(|_|"无法重新估算")?;
    }
    tx.commit().map_err(|_| "无法提交重新估算")?;
    Ok(plan.rows.len())
}
#[tauri::command]
pub fn apply_usage_reprice(
    store: tauri::State<'_, Store>,
    price_id: String,
    filter: super::aggregate::Filter,
    token: String,
) -> Result<usize, String> {
    let mut db = store.db.lock().map_err(|_| "数据库繁忙")?;
    commit_reprice(&mut db, &price_id, &filter, &token)
}
#[cfg(test)]
mod reprice_tests {
    use super::*;
    #[test]
    fn preview_does_not_mutate_and_commit_rejects_stale_preview() {
        let mut db = Connection::open_in_memory().unwrap();
        crate::db::schema::migrate_to(&mut db, crate::db::schema::SCHEMA_VERSION).unwrap();
        db.execute("INSERT INTO usage_prices(id,provider_id,api_model_id,currency,input_per_million,cache_read_per_million,cache_write_per_million,output_per_million,thinking_per_million,source,effective_at,created_at) VALUES ('price','qa','model','USD','1','0','0','0','0','manual',0,0)",[]).unwrap();
        for (id, provider) in [("a", "qa"), ("b", "zzshu")] {
            db.execute("INSERT INTO usage_events(id,source_file,event_key,session_id,occurred_at,source,provider_id_at_event,api_model_id,input_tokens,cache_read_tokens,cache_write_tokens,output_tokens,thinking_tokens) VALUES (?1,'log',?1,'s',1,'api',?2,'model',1000000,0,0,0,0)",rusqlite::params![id,provider]).unwrap();
        }
        let filter: super::super::aggregate::Filter =
            serde_json::from_value(serde_json::json!({"from":0,"to":100})).unwrap();
        let plan = reprice_plan(&db, "price", &filter).unwrap();
        assert_eq!(plan.preview.records, 1);
        assert_eq!(plan.preview.estimate, "1");
        assert_eq!(
            db.query_row(
                "SELECT COUNT(*) FROM usage_events WHERE cost_units IS NOT NULL",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        db.execute(
            "UPDATE usage_events SET input_tokens=2000000 WHERE id='a'",
            [],
        )
        .unwrap();
        assert!(commit_reprice(&mut db, "price", &filter, &plan.preview.token).is_err());
        let plan = reprice_plan(&db, "price", &filter).unwrap();
        assert_eq!(
            commit_reprice(&mut db, "price", &filter, &plan.preview.token).unwrap(),
            1
        );
        assert_eq!(
            db.query_row(
                "SELECT estimated_cost FROM usage_events WHERE id='a'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "2"
        );
        assert_eq!(
            db.query_row(
                "SELECT COUNT(*) FROM usage_events WHERE id='b' AND estimated_cost IS NULL",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
    }
}
