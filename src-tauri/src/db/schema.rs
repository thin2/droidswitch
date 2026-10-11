use rusqlite::{Connection, TransactionBehavior};

pub const SCHEMA_VERSION: u32 = 1;
const UP: &str = include_str!("migrations/001_up.sql");
const DOWN: &str = include_str!("migrations/001_down.sql");

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
    let current: u32 = transaction.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if current > SCHEMA_VERSION {
        return Err(MigrationError::UnsupportedVersion(current));
    }
    if current == 0 && target == 1 {
        transaction.execute_batch(UP)?;
    }
    if current == 1 && target == 0 {
        transaction.execute_batch(DOWN)?;
    }
    transaction.pragma_update(None, "user_version", target)?;
    transaction.commit()?;
    Ok(())
}
