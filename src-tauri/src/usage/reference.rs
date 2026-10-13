//! https://developers.openai.com/api/docs/pricing.md
//! Reviewed 2026-10-13. Current public reference rates, not historical invoices.
//! https://platform.claude.com/docs/en/about-claude/pricing
//! https://api-docs.deepseek.com/zh-cn/quick_start/pricing/
use super::{
    cost::{self, Price, Tokens},
    pricing::{self, StoredPrice},
};
use rusqlite::Connection;

// Exact API identifiers only. Unknown aliases never inherit another model's price.
const RATES: &[(&str, &str, &str, &str, &str, &str)] = &[
    // Standard short-context rates. Empty fields mean unpublished, never free.
    ("gpt-6-astra", "USD", "10", "1", "12.5", "50"),
    ("gpt-6.1-sol", "USD", "2", "0.1", "2.5", "10"),
    ("gpt-6-luna", "USD", "0.1", "0.01", "0.125", "0.5"),
    ("gpt-6-sol", "USD", "2", "0.2", "2.5", "10"),
    ("gpt-5.6-sol", "USD", "4", "0.4", "5", "20"),
    ("gpt-5.6-terra", "USD", "2", "0.2", "2.5", "12"),
    ("gpt-5.6-luna", "USD", "0.2", "0.02", "0.25", "1.2"),
    ("gpt-5.5", "USD", "5", "0.5", "", "30"),
    ("gpt-5.5-pro", "USD", "30", "", "", "180"),
    ("gpt-5.4", "USD", "2.5", "0.25", "", "15"),
    ("gpt-5.4-mini", "USD", "0.75", "0.075", "", "4.5"),
    ("gpt-5.4-nano", "USD", "0.2", "0.02", "", "1.25"),
    ("gpt-5.4-pro", "USD", "30", "", "", "180"),
    ("gpt-5.3-codex", "USD", "1.75", "0.175", "", "14"),
    ("gpt-5.2", "USD", "1.75", "0.175", "", "14"),
    ("gpt-5.2-pro", "USD", "21", "", "", "168"),
    ("gpt-5.1", "USD", "1.25", "0.125", "", "10"),
    ("gpt-5", "USD", "1.25", "0.125", "", "10"),
    ("gpt-5-mini", "USD", "0.25", "0.025", "", "2"),
    ("gpt-5-nano", "USD", "0.05", "0.005", "", "0.4"),
    ("gpt-4.1", "USD", "2", "0.5", "", "8"),
    ("gpt-4.1-mini", "USD", "0.4", "0.1", "", "1.6"),
    ("gpt-4.1-nano", "USD", "0.1", "0.025", "", "0.4"),
    ("gpt-4o", "USD", "2.5", "1.25", "", "10"),
    ("gpt-4o-mini", "USD", "0.15", "0.075", "", "0.6"),
    ("o3", "USD", "2", "0.5", "", "8"),
    ("o4-mini", "USD", "1.1", "0.275", "", "4.4"),
    ("claude-fable-5-1", "USD", "10", "0.25", "12.5", "50"),
    ("claude-fable-5", "USD", "10", "1", "12.5", "50"),
    ("claude-mythos-5-1", "USD", "10", "0.25", "12.5", "50"),
    ("claude-mythos-5", "USD", "10", "1", "12.5", "50"),
    // Standard <=100k prompt tier; the UI labels reference-price assumptions.
    ("claude-haiku-5-5", "USD", "0.1", "0.01", "0.125", "0.5"),
    ("claude-opus-5-5", "USD", "4", "0.2", "5", "20"),
    ("claude-opus-5", "USD", "5", "0.5", "6.25", "25"),
    ("claude-opus-4-8", "USD", "5", "0.5", "6.25", "25"),
    ("claude-opus-4-7", "USD", "5", "0.5", "6.25", "25"),
    ("claude-opus-4-6", "USD", "5", "0.5", "6.25", "25"),
    ("claude-sonnet-5-5", "USD", "2", "0.1", "2.5", "10"),
    ("claude-sonnet-5", "USD", "2", "0.2", "2.5", "10"),
    ("claude-sonnet-4-6", "USD", "3", "0.3", "3.75", "15"),
    ("claude-sonnet-4-5", "USD", "3", "0.3", "3.75", "15"),
    (
        "claude-sonnet-4-5-20250929",
        "USD",
        "3",
        "0.3",
        "3.75",
        "15",
    ),
    ("claude-haiku-4-5", "USD", "1", "0.1", "1.25", "5"),
    ("claude-haiku-4-5-20251001", "USD", "1", "0.1", "1.25", "5"),
    // Peak reference rates. Off-peak is half; no holiday/calendar inference.
    // Cache creation uses uncached input pricing, not Anthropic's multiplier.
    ("deepseek-flash", "CNY", "2", "0.04", "2", "8"),
    ("deepseek-v4-pro", "CNY", "9", "0.3", "9", "27"),
    ("deepseek-v4-flash", "CNY", "2", "0.04", "2", "8"),
    ("deepseek-v4-flash-vision-exp", "CNY", "2", "0.04", "2", "8"),
];

pub fn prices() -> Vec<StoredPrice> {
    RATES
        .iter()
        .map(
            |&(model, currency, input, read, write, output)| StoredPrice {
                id: format!("reference:2026-10-13:{model}"),
                provider_id: String::new(),
                api_model_id: model.into(),
                currency: currency.into(),
                input: Some(input.into()),
                cache_read: (!read.is_empty()).then(|| read.into()),
                cache_write: (!write.is_empty()).then(|| write.into()),
                output: Some(output.into()),
                thinking: Some(output.into()),
                source: "reference".into(),
                effective_at: 0,
                created_at: 0,
            },
        )
        .collect()
}

pub fn price(model: &str) -> Option<(String, String, Price)> {
    let stored = prices().into_iter().find(|p| p.api_model_id == model)?;
    Some((
        stored.id,
        stored.currency.clone(),
        Price {
            provider_id: String::new(),
            model: model.into(),
            currency: stored.currency,
            input: stored.input,
            cache_read: stored.cache_read,
            cache_write: stored.cache_write,
            output: stored.output,
            thinking: stored.thinking,
            effective_at: 0,
        },
    ))
}

/// Fill only records which have never had a price snapshot. Keep existing
/// custom prices and previously calculated historical amounts unchanged.
pub fn fill_unpriced(db: &mut Connection) -> Result<usize, String> {
    let tx = db.transaction().map_err(|_| "无法开始参考价估算")?;
    let events = {
        let mut q = tx.prepare("SELECT id,provider_id_at_event,api_model_id,occurred_at,input_tokens,cache_read_tokens,cache_write_tokens,output_tokens,thinking_tokens FROM usage_events WHERE source='api' AND price_id_at_event IS NULL AND estimated_cost IS NULL AND api_model_id IS NOT NULL").map_err(|_| "无法读取未定价记录")?;
        let rows = q
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    Tokens {
                        input: r
                            .get::<_, Option<i64>>(4)?
                            .and_then(|v| u64::try_from(v).ok()),
                        cache_read: r
                            .get::<_, Option<i64>>(5)?
                            .and_then(|v| u64::try_from(v).ok()),
                        cache_write: r
                            .get::<_, Option<i64>>(6)?
                            .and_then(|v| u64::try_from(v).ok()),
                        output: r
                            .get::<_, Option<i64>>(7)?
                            .and_then(|v| u64::try_from(v).ok()),
                        thinking: r
                            .get::<_, Option<i64>>(8)?
                            .and_then(|v| u64::try_from(v).ok()),
                    },
                ))
            })
            .map_err(|_| "无法读取未定价记录")?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "未定价记录格式错误")?;
        rows
    };
    let mut updated = 0;
    for (id, provider, model, at, tokens) in events {
        let Some((price_id, currency, price)) =
            pricing::price_at(&tx, provider.as_deref(), &model, at)?
        else {
            continue;
        };
        let estimate = cost::estimate(&tokens, &price)?;
        let units = estimate
            .cost
            .as_deref()
            .and_then(cost::parse_cost)
            .map(i64::try_from)
            .transpose()
            .map_err(|_| "费用超出存储范围")?;
        let snapshot = serde_json::to_string(&price).map_err(|_| "无法保存参考价快照")?;
        updated += tx.execute("UPDATE usage_events SET estimated_cost=?2,cost_units=?3,cost_complete=?4,currency=?5,price_id_at_event=?6,price_snapshot=?7 WHERE id=?1", rusqlite::params![id,estimate.cost,units,estimate.complete,currency,price_id,snapshot]).map_err(|_| "无法保存参考估算")?;
    }
    tx.commit().map_err(|_| "无法提交参考估算")?;
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn latest_reference_catalog_is_unique_and_keeps_unpublished_prices_unknown() {
        let catalog = prices();
        let ids: std::collections::HashSet<_> =
            catalog.iter().map(|p| p.api_model_id.as_str()).collect();
        assert_eq!(ids.len(), catalog.len());
        for (model, input, cached, output) in [
            ("gpt-6-astra", "10", "1", "50"),
            ("gpt-6.1-sol", "2", "0.1", "10"),
            ("gpt-6-luna", "0.1", "0.01", "0.5"),
            ("gpt-5.3-codex", "1.75", "0.175", "14"),
            ("claude-fable-5-1", "10", "0.25", "50"),
            ("claude-haiku-5-5", "0.1", "0.01", "0.5"),
        ] {
            let p = price(model).unwrap().2;
            assert_eq!(p.input.as_deref(), Some(input));
            assert_eq!(p.cache_read.as_deref(), Some(cached));
            assert_eq!(p.output.as_deref(), Some(output));
        }
        let p = price("gpt-5.5-pro").unwrap().2;
        assert!(p.cache_read.is_none());
        assert!(p.cache_write.is_none());
        let estimate = cost::estimate(
            &Tokens {
                input: Some(1_000_000),
                cache_read: Some(1),
                cache_write: Some(0),
                output: Some(0),
                thinking: Some(0),
            },
            &p,
        )
        .unwrap();
        assert!(!estimate.complete);
        assert_eq!(estimate.cost.as_deref(), Some("30"));
        assert!(price("custom:gpt-6-astra-0").is_none());
        assert!(price("gpt-6-astra-fast").is_none());
    }

    #[test]
    fn astra_backfill_only_prices_unpriced_api_records() {
        let mut db = Connection::open_in_memory().unwrap();
        crate::db::schema::migrate_to(&mut db, crate::db::schema::SCHEMA_VERSION).unwrap();
        for (id, source) in [("new", "api"), ("old", "api"), ("subscription", "official")] {
            db.execute("INSERT INTO usage_events(id,source_file,event_key,session_id,occurred_at,source,api_model_id,input_tokens,cache_read_tokens,cache_write_tokens,output_tokens,thinking_tokens) VALUES (?1,'log',?1,'s',1,?2,'gpt-6-astra',1000000,0,0,0,0)", rusqlite::params![id,source]).unwrap();
        }
        db.execute(
            "UPDATE usage_events SET estimated_cost='3',price_id_at_event='manual' WHERE id='old'",
            [],
        )
        .unwrap();
        assert_eq!(fill_unpriced(&mut db).unwrap(), 1);
        assert_eq!(fill_unpriced(&mut db).unwrap(), 0);
        let cost_of = |id: &str| {
            db.query_row(
                "SELECT estimated_cost FROM usage_events WHERE id=?1",
                [id],
                |r| r.get::<_, Option<String>>(0),
            )
            .unwrap()
        };
        assert_eq!(cost_of("new").as_deref(), Some("10"));
        assert_eq!(cost_of("old").as_deref(), Some("3"));
        assert_eq!(cost_of("subscription"), None);
    }
    #[test]
    fn references_are_exact_and_split_currency_and_thinking() {
        assert!(price("claude-opus-5-5-unverified").is_none());
        for p in prices() {
            price(&p.api_model_id).unwrap().2.validate().unwrap();
        }
        let tokens = Tokens {
            input: Some(1_000_000),
            cache_read: Some(1_000_000),
            cache_write: Some(1_000_000),
            output: Some(1_000_000),
            thinking: Some(1_000_000),
        };
        assert_eq!(
            cost::estimate(&tokens, &price("claude-opus-5-5").unwrap().2)
                .unwrap()
                .cost
                .as_deref(),
            Some("49.2")
        );
        let (_, currency, p) = price("deepseek-flash").unwrap();
        assert_eq!(currency, "CNY");
        assert_eq!(
            cost::estimate(&tokens, &p).unwrap().cost.as_deref(),
            Some("20.04")
        );
    }
    #[test]
    fn backfill_preserves_invoices_unknowns_official_and_is_idempotent() {
        let mut db = Connection::open_in_memory().unwrap();
        crate::db::schema::migrate_to(&mut db, crate::db::schema::SCHEMA_VERSION).unwrap();
        for (id, source, model) in [
            ("ref", "api", "claude-opus-5-5"),
            ("manual", "api", "claude-opus-5-5"),
            ("official", "official", "claude-opus-5-5"),
            ("unknown", "api", "unverified"),
        ] {
            db.execute("INSERT INTO usage_events(id,source_file,event_key,session_id,occurred_at,source,api_model_id,input_tokens,cache_read_tokens,cache_write_tokens,output_tokens,thinking_tokens) VALUES (?1,'log',?1,'s',1,?2,?3,1000000,0,0,0,0)",rusqlite::params![id,source,model]).unwrap();
        }
        db.execute("UPDATE usage_events SET estimated_cost='99',cost_units=99000000000000,price_id_at_event='custom' WHERE id='manual'",[]).unwrap();
        assert_eq!(fill_unpriced(&mut db).unwrap(), 1);
        assert_eq!(fill_unpriced(&mut db).unwrap(), 0);
        assert_eq!(
            db.query_row(
                "SELECT estimated_cost FROM usage_events WHERE id='ref'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "4"
        );
        assert_eq!(
            db.query_row(
                "SELECT estimated_cost FROM usage_events WHERE id='manual'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "99"
        );
        assert_eq!(
            db.query_row(
                "SELECT COUNT(*) FROM usage_events WHERE estimated_cost IS NULL",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            2
        );
        db.execute("INSERT INTO usage_prices(id,provider_id,api_model_id,currency,input_per_million,source,effective_at,created_at) VALUES ('custom','qa','claude-opus-5-5','USD','7','manual',0,0)",[]).unwrap();
        assert_eq!(
            pricing::price_at(&db, Some("qa"), "claude-opus-5-5", 1)
                .unwrap()
                .unwrap()
                .2
                .input
                .as_deref(),
            Some("7")
        );
        assert_eq!(
            pricing::price_at(&db, Some("other"), "claude-opus-5-5", 1)
                .unwrap()
                .unwrap()
                .2
                .input
                .as_deref(),
            Some("4")
        );
    }
}
