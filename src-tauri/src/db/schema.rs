use rusqlite::{Connection, TransactionBehavior};

pub const SCHEMA_VERSION: u32 = 3;
struct Migration {
    version: u32,
    up: &'static str,
    down: &'static str,
}
const MIGRATIONS: [Migration; 3] = [
    Migration {
        version: 3,
        up: include_str!("migrations/003_up.sql"),
        down: include_str!("migrations/003_down.sql"),
    },
    Migration {
        version: 1,
        up: include_str!("migrations/001_up.sql"),
        down: include_str!("migrations/001_down.sql"),
    },
    Migration {
        version: 2,
        up: include_str!("migrations/002_up.sql"),
        down: include_str!("migrations/002_down.sql"),
    },
];

#[derive(Debug)]
pub enum MigrationError {
    Sql(rusqlite::Error),
    UnsupportedVersion(u32),
}
impl From<rusqlite::Error> for MigrationError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Sql(value)
    }
}
impl std::fmt::Display for MigrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sql(_) => write!(f, "database operation failed"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported database version: {version}")
            }
        }
    }
}
impl std::error::Error for MigrationError {}

/// Down migrations are maintenance-only and are never exposed to the UI.
pub fn migrate_to(connection: &mut Connection, target: u32) -> Result<(), MigrationError> {
    if target > SCHEMA_VERSION {
        return Err(MigrationError::UnsupportedVersion(target));
    }
    connection.pragma_update(None, "foreign_keys", true)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut current: u32 = transaction.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if current > SCHEMA_VERSION {
        return Err(MigrationError::UnsupportedVersion(current));
    }
    while current < target {
        let next = current + 1;
        let migration = MIGRATIONS
            .iter()
            .find(|m| m.version == next)
            .ok_or(MigrationError::UnsupportedVersion(next))?;
        transaction.execute_batch(migration.up)?;
        if next == 3 {
            // Convert decimal strings with integer arithmetic, never SQLite REAL.
            let rows = transaction.prepare("SELECT id,estimated_cost FROM usage_events WHERE estimated_cost IS NOT NULL AND cost_units IS NULL")?
                .query_map([], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<Result<Vec<_>,_>>()?;
            for (id, text) in rows {
                let units =
                    crate::usage::cost::parse_cost(&text).and_then(|v| i64::try_from(v).ok());
                transaction.execute(
                    "UPDATE usage_events SET cost_units=?2 WHERE id=?1",
                    rusqlite::params![id, units],
                )?;
            }
            // v1 costs were only emitted when all chargeable components were priced.
            transaction.execute("UPDATE usage_events SET cost_complete=1 WHERE cost_units IS NOT NULL AND model_selector IS NULL", [])?;
        }
        current = next;
    }
    while current > target {
        let migration = MIGRATIONS
            .iter()
            .find(|m| m.version == current)
            .ok_or(MigrationError::UnsupportedVersion(current))?;
        transaction.execute_batch(migration.down)?;
        current -= 1;
    }
    transaction.pragma_update(None, "user_version", target)?;
    transaction.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tables(db: &Connection) -> i64 {
        db.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            [],
            |r| r.get(0),
        )
        .unwrap()
    }
    #[test]
    fn schema_upgrades_from_v1_preserving_rows_and_reverts_to_v1() {
        let mut db = Connection::open_in_memory().unwrap();
        migrate_to(&mut db, 1).unwrap();
        db.execute("INSERT INTO provider_connections(id,name,base_url,protocol,credential_mode,credential_value,created_at,updated_at) VALUES ('p','N','https://example.com','openai','environment','KEY',0,0)", []).unwrap();
        db.execute("INSERT INTO model_prices(id,model_id,currency,input_per_million,cache_read_per_million,cache_write_per_million,output_per_million,source,effective_at) VALUES ('price','model','USD','1','0.5','2','3','manual',10)", []).unwrap();
        db.execute("INSERT INTO usage_events(id,source_file,event_key,session_id,occurred_at,source,input_tokens) VALUES ('e','log','k','s',5,'api',7)", []).unwrap();
        migrate_to(&mut db, 2).unwrap();
        assert_eq!(tables(&db), 10);
        assert_eq!(
            db.query_row("SELECT count(*) FROM usage_prices", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            db.query_row(
                "SELECT granularity FROM usage_events WHERE id='e'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "request"
        );
        assert!(db
            .query_row("SELECT provider_id FROM usage_prices", [], |r| r
                .get::<_, String>(0))
            .unwrap()
            .is_empty());
        migrate_to(&mut db, 1).unwrap();
        assert_eq!(tables(&db), 7);
        assert_eq!(
            db.query_row("SELECT count(*) FROM model_prices", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            db.query_row("SELECT count(*) FROM usage_events", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            db.query_row("SELECT count(*) FROM provider_connections", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        migrate_to(&mut db, 2).unwrap();
        assert_eq!(tables(&db), 10);
        assert_eq!(
            db.query_row("SELECT count(*) FROM usage_prices", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
    #[test]
    fn schema_rejects_future_versions_without_changes() {
        let mut db = Connection::open_in_memory().unwrap();
        db.pragma_update(None, "user_version", 77).unwrap();
        assert!(matches!(
            migrate_to(&mut db, SCHEMA_VERSION),
            Err(MigrationError::UnsupportedVersion(77))
        ));
        assert_eq!(tables(&db), 0);
    }
}
