pub mod schema;
use rusqlite::Connection;
use std::{fs::OpenOptions, io, path::Path, time::Duration};

pub fn open(path: &Path) -> Result<Connection, Box<dyn std::error::Error>> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if !meta.file_type().is_file() => {
            return Err(io::Error::other("regular database file required").into())
        }
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            options.open(path)?;
        }
        Err(e) => return Err(e.into()),
    }
    super::permissions::restrict_file(path)?;
    let mut connection = Connection::open(path)?;
    connection.busy_timeout(Duration::from_secs(3))?;
    connection.pragma_update(None, "foreign_keys", true)?;
    connection.pragma_update(None, "synchronous", "FULL")?;
    schema::migrate_to(&mut connection, schema::SCHEMA_VERSION)?;
    Ok(connection)
}

#[cfg(test)]
mod tests {
    use super::*;
    use schema::migrate_to;
    fn tables(db: &Connection) -> i64 {
        db.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            [],
            |r| r.get(0),
        )
        .unwrap()
    }
    fn provider(db: &Connection, id: &str) {
        db.execute("INSERT INTO provider_connections(id,name,base_url,protocol,credential_mode,credential_value,created_at,updated_at) VALUES (?1,'Name','https://example.com','openai','environment','API_KEY',0,0)", [id]).unwrap();
    }
    #[test]
    fn db_migrations_create_all_tables_idempotently_and_rollback() {
        let mut db = Connection::open_in_memory().unwrap();
        migrate_to(&mut db, 1).unwrap();
        assert_eq!(tables(&db), 7);
        provider(&db, "p1");
        migrate_to(&mut db, 1).unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM provider_connections", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            1
        );
        migrate_to(&mut db, 0).unwrap();
        assert_eq!(tables(&db), 0);
        migrate_to(&mut db, 1).unwrap();
        assert_eq!(tables(&db), 7);
    }
    #[test]
    fn db_failure_rolls_back_partial_ddl_and_version() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE model_entries (sentinel TEXT)")
            .unwrap();
        assert!(migrate_to(&mut db, 1).is_err());
        assert_eq!(tables(&db), 1);
        assert_eq!(
            db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            0
        );
    }
    #[test]
    fn db_future_schema_is_rejected_without_changes() {
        let mut db = Connection::open_in_memory().unwrap();
        db.pragma_update(None, "user_version", 99).unwrap();
        assert!(matches!(
            migrate_to(&mut db, 1),
            Err(schema::MigrationError::UnsupportedVersion(99))
        ));
        assert_eq!(tables(&db), 0);
        assert_eq!(
            db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            99
        );
    }
    #[test]
    fn db_foreign_keys_and_model_constraints() {
        let mut db = Connection::open_in_memory().unwrap();
        migrate_to(&mut db, 1).unwrap();
        provider(&db, "p1");
        provider(&db, "p2");
        let sql = "INSERT INTO model_entries(id,provider_id,api_model_id,display_name,max_output_tokens) VALUES (?1,?2,'same-model','Model',?3)";
        db.execute(sql, rusqlite::params!["m1", "p1", None::<u32>])
            .unwrap();
        db.execute(sql, rusqlite::params!["m2", "p2", 100]).unwrap();
        assert!(db.execute(sql, rusqlite::params!["m3", "p1", 100]).is_err());
        assert!(db
            .execute(sql, rusqlite::params!["m4", "missing", 100])
            .is_err());
        assert!(db
            .execute("UPDATE model_entries SET max_output_tokens=0", [])
            .is_err());
        assert!(db
            .execute("DELETE FROM provider_connections WHERE id='p1'", [])
            .is_err());
    }
    #[test]
    fn db_persistence_unknown_tokens_and_deduplication() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("switch.db");
        {
            let db = open(&path).unwrap();
            provider(&db, "p");
            db.execute("INSERT INTO drafts(id,target_path,payload,updated_at) VALUES ('draft','/test/settings.json','{}',0)", []).unwrap();
            let sql = "INSERT INTO usage_events(id,source_file,event_key,session_id,occurred_at,source) VALUES (?1,'log','event','session',0,'unknown')";
            db.execute(sql, ["e1"]).unwrap();
            assert!(db.execute(sql, ["e2"]).is_err());
            assert_eq!(
                db.query_row("SELECT input_tokens FROM usage_events", [], |r| r
                    .get::<_, Option<i64>>(0))
                    .unwrap(),
                None
            );
            assert!(db
                .execute("UPDATE usage_events SET output_tokens=-1", [])
                .is_err());
        }
        let db = open(&path).unwrap();
        assert_eq!(
            db.query_row("SELECT payload FROM drafts", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "{}"
        );
    }
}
