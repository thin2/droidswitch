//! Endpoint identity and the secret-free history of applied model configuration.
//!
//! Attribution evidence order: a request's own endpoint (direct log evidence),
//! then a recorded configuration mapping that was in effect at that time, else
//! unknown. Display names are never used to guess a provider.

use crate::workspace::Store;
use rusqlite::{Connection, OptionalExtension};
use serde_json::Value;

/// Canonical endpoint identity: lowercase scheme and host, default ports and
/// trailing path separators removed, user info, query and fragment dropped.
/// Returns `None` when the value cannot be trusted as an endpoint.
pub fn normalize_endpoint(raw: &str) -> Option<String> {
    let (scheme, rest) = raw.trim().split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    let (authority, path) = match rest.find('/') {
        Some(index) => (&rest[..index], &rest[index..]),
        None => (rest, ""),
    };
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    if authority.is_empty() {
        return None;
    }
    let (host, port) = if let Some(bracketed) = authority.strip_prefix('[') {
        let (host, tail) = bracketed.split_once(']')?;
        let port = tail.strip_prefix(':').filter(|p| is_port(p));
        (format!("[{host}]"), port.map(str::to_owned))
    } else if let Some((host, port)) = authority.rsplit_once(':') {
        match is_port(port) {
            true => (host.to_owned(), Some(port.to_owned())),
            false => (authority.to_owned(), None),
        }
    } else {
        (authority.to_owned(), None)
    };
    let host = host.to_ascii_lowercase();
    if host.is_empty() || host.contains(' ') {
        return None;
    }
    let port = match (scheme.as_str(), port.as_deref()) {
        ("http", Some("80")) | ("https", Some("443")) => String::new(),
        (_, Some(port)) => format!(":{port}"),
        (_, None) => String::new(),
    };
    Some(format!(
        "{scheme}://{host}{port}{}",
        path.trim_end_matches('/')
    ))
}

fn is_port(value: &str) -> bool {
    !value.is_empty() && value.len() <= 5 && value.bytes().all(|b| b.is_ascii_digit())
}

pub fn endpoint_id(endpoint: &str) -> String {
    crate::config::pipeline::hash(format!("endpoint\0{endpoint}").as_bytes())
}

pub fn touch_endpoint(db: &Connection, endpoint: &str, now: i64) -> Result<String, String> {
    let id = endpoint_id(endpoint);
    db.execute(
        "INSERT INTO usage_endpoints(id,endpoint,created_at,updated_at) VALUES (?1,?2,?3,?3) ON CONFLICT(endpoint) DO UPDATE SET updated_at=excluded.updated_at",
        rusqlite::params![id, endpoint, now],
    )
    .map_err(|_| "无法保存端点身份")?;
    Ok(id)
}

/// Every configured provider's normalized endpoint, so a request endpoint can be
/// resolved to a provider without trusting a display name.
pub fn endpoint_providers(db: &Connection) -> Result<Vec<(String, String, String)>, String> {
    let mut statement = db
        .prepare("SELECT id,name,base_url FROM provider_connections")
        .map_err(|_| "无法读取供应商")?;
    let rows = statement
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(|_| "无法读取供应商")?;
    let mut out = vec![];
    for row in rows {
        let (id, name, base_url) = row.map_err(|_| "供应商结构无效")?;
        if let Some(endpoint) = normalize_endpoint(&base_url) {
            out.push((endpoint, id, name));
        }
    }
    Ok(out)
}

pub struct AppliedModel {
    pub selector: String,
    pub api_model_id: String,
    pub display_name: String,
    pub provider_id: Option<String>,
    pub provider_name: String,
    pub endpoint: String,
}

/// Read the selector ids that Droid actually holds. Entries without an `id` are
/// skipped: the runtime id for a duplicated model name is not derivable here, so
/// guessing one would fabricate attribution.
pub fn applied_models(
    document: &Value,
    providers: &[(String, String, String)],
) -> Vec<AppliedModel> {
    let Some(entries) = document.get("customModels").and_then(Value::as_array) else {
        return vec![];
    };
    let mut out = vec![];
    for entry in entries {
        let (Some(selector), Some(api_model_id)) = (
            entry.get("id").and_then(Value::as_str),
            entry.get("model").and_then(Value::as_str),
        ) else {
            continue;
        };
        let Some(endpoint) = entry
            .get("baseUrl")
            .and_then(Value::as_str)
            .and_then(normalize_endpoint)
        else {
            continue;
        };
        let matched = providers.iter().find(|(e, _, _)| e == &endpoint);
        out.push(AppliedModel {
            selector: selector.to_owned(),
            api_model_id: api_model_id.to_owned(),
            display_name: entry
                .get("displayName")
                .and_then(Value::as_str)
                .unwrap_or(api_model_id)
                .to_owned(),
            provider_id: matched.map(|(_, id, _)| id.clone()),
            provider_name: matched
                .map(|(_, _, name)| name.clone())
                .unwrap_or_else(|| endpoint.clone()),
            endpoint,
        });
    }
    out
}

/// Record the mapping introduced by a successful apply. Unchanged mappings keep
/// their open range; changed or removed ones are closed. No secret is stored.
pub fn record_applied_configuration(
    db: &mut Connection,
    document: &Value,
    now: i64,
    config_version: &str,
) -> Result<usize, String> {
    let providers = endpoint_providers(db)?;
    let models = applied_models(document, &providers);
    let tx = db.transaction().map_err(|_| "无法创建归属事务")?;
    let mut written = 0usize;
    let mut selectors = vec![];
    for model in &models {
        let endpoint_id = touch_endpoint(&tx, &model.endpoint, now)?;
        selectors.push(model.selector.clone());
        let open: Option<(String, String, String, String, String)> = tx
            .query_row(
                "SELECT id,endpoint_id,api_model_id,provider_name,display_name FROM usage_model_identities WHERE droid_selector_id=?1 AND effective_to IS NULL ORDER BY effective_from DESC LIMIT 1",
                [&model.selector],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .optional()
            .map_err(|_| "无法读取归属历史")?;
        if let Some((id, endpoint, api_model_id, provider_name, display_name)) = open {
            if endpoint == endpoint_id
                && api_model_id == model.api_model_id
                && display_name == model.display_name
                && provider_name == model.provider_name
            {
                continue;
            }
            tx.execute(
                "UPDATE usage_model_identities SET effective_to=?2 WHERE id=?1",
                rusqlite::params![id, now],
            )
            .map_err(|_| "无法结束历史归属")?;
        }
        tx.execute("INSERT INTO usage_model_identities(id,config_version,effective_from,effective_to,droid_selector_id,api_model_id,internal_model_id,provider_id,provider_name,display_name,endpoint_id) VALUES (?1,?2,?3,NULL,?4,?5,NULL,?6,?7,?8,?9)",
            rusqlite::params![uuid::Uuid::new_v4().to_string(),config_version,now,model.selector,model.api_model_id,model.provider_id,model.provider_name,model.display_name,endpoint_id])
            .map_err(|_| "无法保存历史归属")?;
        written += 1;
    }
    if selectors.is_empty() {
        tx.execute(
            "UPDATE usage_model_identities SET effective_to=?1 WHERE effective_to IS NULL",
            [now],
        )
        .map_err(|_| "无法结束历史归属")?;
    } else {
        let placeholders = (1..=selectors.len())
            .map(|i| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(",");
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now)];
        for selector in &selectors {
            params.push(Box::new(selector.clone()));
        }
        let borrowed: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        tx.execute(
            &format!("UPDATE usage_model_identities SET effective_to=?1 WHERE effective_to IS NULL AND droid_selector_id NOT IN ({placeholders})"),
            borrowed.as_slice(),
        )
        .map_err(|_| "无法结束历史归属")?;
    }
    tx.commit().map_err(|_| "提交归属失败")?;
    Ok(written)
}

type MappingResult = (String, Option<String>, String, String);

/// Mapping in effect for a selector at a point in time, if one was recorded.
pub fn mapping_at(
    db: &Connection,
    selector: &str,
    at: i64,
) -> Result<Option<MappingResult>, String> {
    db.query_row("SELECT endpoint_id,provider_id,provider_name,api_model_id FROM usage_model_identities WHERE droid_selector_id=?1 AND effective_from<=?2 AND (effective_to IS NULL OR effective_to>?2) ORDER BY effective_from DESC LIMIT 1",rusqlite::params![selector,at],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(|_|"无法读取历史归属".into())
}

/// Hook used after a configuration apply succeeded.
pub fn record_after_apply(store: &Store, target_path: &str) -> Result<usize, String> {
    let text = std::fs::read_to_string(target_path).map_err(|_| "无法读取已应用配置")?;
    let document: Value = serde_json::from_str(&text).map_err(|_| "已应用配置无效")?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let mut db = store.db.lock().map_err(|_| "数据库繁忙")?;
    let version: Option<String> = db
        .query_row(
            "SELECT id FROM apply_snapshots ORDER BY created_at DESC, id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| "无法读取应用版本")?;
    record_applied_configuration(
        &mut db,
        &document,
        now,
        version.as_deref().unwrap_or("unversioned"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn endpoint_identity_ignores_noise_and_rejects_unsafe_values() {
        assert_eq!(
            normalize_endpoint("HTTPS://QA.Example.COM:443/v1/").unwrap(),
            "https://qa.example.com/v1"
        );
        assert_eq!(normalize_endpoint("http://host:80").unwrap(), "http://host");
        assert_eq!(
            normalize_endpoint("https://host:8443/a/b").unwrap(),
            "https://host:8443/a/b"
        );
        assert_eq!(
            normalize_endpoint("https://user:pw@HOST/x?a=1#f").unwrap(),
            "https://host/x"
        );
        assert_eq!(
            normalize_endpoint("https://[::1]:8443/v1").unwrap(),
            "https://[::1]:8443/v1"
        );
        assert!(normalize_endpoint("file:///etc/passwd").is_none());
        assert!(normalize_endpoint("qa.example.com").is_none());
        assert!(normalize_endpoint("https://").is_none());
        assert_ne!(
            endpoint_id(&normalize_endpoint("https://qa.example.com").unwrap()),
            endpoint_id(&normalize_endpoint("https://zzshu.example.com").unwrap())
        );
    }

    fn provider(db: &Connection, id: &str, name: &str, url: &str) {
        db.execute("INSERT INTO provider_connections(id,name,base_url,protocol,credential_mode,credential_value,created_at,updated_at) VALUES (?1,?2,?3,'anthropic','environment','KEY',0,0)",
            rusqlite::params![id, name, url]).unwrap();
    }

    #[test]
    fn same_model_on_two_providers_stays_two_rows_and_never_guesses_missing_ids() {
        let mut db = Connection::open_in_memory().unwrap();
        crate::db::schema::migrate_to(&mut db, crate::db::schema::SCHEMA_VERSION).unwrap();
        provider(&db, "qa", "QA", "https://qa.example.com");
        provider(&db, "zzshu", "zzshu", "https://zzshu.example.com");
        let document = json!({"customModels":[
            {"id":"custom:shared-0","model":"shared","displayName":"shared (qa)","baseUrl":"https://qa.example.com"},
            {"model":"shared","displayName":"shared (zzshu)","baseUrl":"https://zzshu.example.com"}
        ]});
        assert_eq!(
            record_applied_configuration(&mut db, &document, 100, "v1").unwrap(),
            1
        );
        assert_eq!(
            db.query_row("SELECT count(*) FROM usage_model_identities", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        let (endpoint, provider_id, _, _) =
            mapping_at(&db, "custom:shared-0", 150).unwrap().unwrap();
        assert_eq!(
            endpoint,
            endpoint_id(&normalize_endpoint("https://qa.example.com").unwrap())
        );
        assert_eq!(provider_id.as_deref(), Some("qa"));
        assert!(mapping_at(&db, "custom:shared-0", 50).unwrap().is_none());
    }

    #[test]
    fn changed_and_removed_models_close_previous_ranges() {
        let mut db = Connection::open_in_memory().unwrap();
        crate::db::schema::migrate_to(&mut db, crate::db::schema::SCHEMA_VERSION).unwrap();
        provider(&db, "a", "A", "https://a.example.com");
        provider(&db, "b", "B", "https://b.example.com");
        let first = json!({"customModels":[{"id":"custom:m-0","model":"m","displayName":"m","baseUrl":"https://a.example.com"}]});
        assert_eq!(
            record_applied_configuration(&mut db, &first, 100, "v1").unwrap(),
            1
        );
        assert_eq!(
            record_applied_configuration(&mut db, &first, 200, "v2").unwrap(),
            0
        );
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM usage_model_identities WHERE effective_to IS NULL",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        let moved = json!({"customModels":[{"id":"custom:m-0","model":"m","displayName":"m","baseUrl":"https://b.example.com"}]});
        assert_eq!(
            record_applied_configuration(&mut db, &moved, 300, "v3").unwrap(),
            1
        );
        assert_eq!(
            mapping_at(&db, "custom:m-0", 250)
                .unwrap()
                .unwrap()
                .1
                .as_deref(),
            Some("a")
        );
        assert_eq!(
            mapping_at(&db, "custom:m-0", 350)
                .unwrap()
                .unwrap()
                .1
                .as_deref(),
            Some("b")
        );
        let empty = json!({"customModels":[]});
        record_applied_configuration(&mut db, &empty, 400, "v4").unwrap();
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM usage_model_identities WHERE effective_to IS NULL",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        assert!(mapping_at(&db, "custom:m-0", 450).unwrap().is_none());
    }
}
