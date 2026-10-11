use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::path::{Path, PathBuf};
pub struct SourceRecord {
    pub id: String,
    pub app: String,
    pub name: String,
    pub settings: Value,
    pub hash: String,
    pub location: String,
}
pub fn locate(input: &str) -> Result<PathBuf, String> {
    let path = if input.trim().is_empty() {
        crate::workspace::default_target()?
            .parent()
            .and_then(Path::parent)
            .ok_or("无法定位用户目录")?
            .join(".cc-switch/cc-switch.db")
    } else {
        crate::config::path::resolve(input)?
    };
    Ok(if path.is_dir() {
        path.join("cc-switch.db")
    } else {
        path
    })
}
pub fn read(input: &str) -> Result<Vec<SourceRecord>, String> {
    let path = locate(input)?;
    let metadata = std::fs::symlink_metadata(&path)
        .map_err(|_| "未找到 CC Switch 数据库，请选择文件或数据目录")?;
    if !metadata.is_file() {
        return Err("请选择普通数据库文件，不能使用符号链接".into());
    }
    let location = std::fs::canonicalize(&path)
        .map_err(|_| "无法定位数据库")?
        .to_string_lossy()
        .to_string();
    let mut db = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| "无法只读打开数据库，请关闭 CC Switch 后重试或选择完整备份")?;
    db.busy_timeout(std::time::Duration::from_secs(3))
        .map_err(|_| "无法设置数据库读取超时")?;
    let tx = db.transaction().map_err(|_| "无法建立一致性读取事务")?;
    let mut fields = tx
        .prepare("PRAGMA table_info(providers)")
        .map_err(|_| "数据库结构读取失败")?;
    let columns: Result<Vec<String>, _> = fields
        .query_map([], |r| r.get(1))
        .map_err(|_| "数据库结构读取失败")?
        .collect();
    let columns = columns.map_err(|_| "数据库结构读取失败")?;
    for expected in ["id", "app_type", "name", "settings_config"] {
        if !columns.iter().any(|c| c == expected) {
            return Err("CC Switch 数据库结构不受支持，未将读取失败当作空库".into());
        }
    }
    let mut statement=tx.prepare("SELECT id,app_type,name,settings_config FROM providers ORDER BY app_type,id LIMIT 10001").map_err(|_|"无法读取供应商表")?;
    let rows = statement
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|_| "读取供应商失败")?;
    let mut out = vec![];
    let mut total = 0;
    for row in rows {
        let (id, app, name, raw) = row.map_err(|_| "供应商记录字段格式未知")?;
        total += raw.len();
        if raw.len() > 1024 * 1024 || total > 16 * 1024 * 1024 || out.len() >= 10000 {
            return Err("供应商数据超过读取限制".into());
        }
        let settings =
            serde_json::from_str(&raw).map_err(|_| "供应商配置包含无效 JSON，未导入任何内容")?;
        out.push(SourceRecord {
            id,
            app,
            name,
            settings,
            hash: crate::config::pipeline::hash(raw.as_bytes()),
            location: location.clone(),
        });
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ccswitch_read_reads_wal_and_rejects_unknown_schema_without_writing() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("cc-switch.db");
        let db = Connection::open(&p).unwrap();
        db.execute_batch("PRAGMA journal_mode=WAL;CREATE TABLE providers(id TEXT,app_type TEXT,name TEXT,settings_config TEXT);INSERT INTO providers VALUES('a','claude','Name','{}');").unwrap();
        let before = std::fs::read(&p).unwrap();
        let wal = std::fs::read(p.with_extension("db-wal")).unwrap();
        assert_eq!(read(p.to_str().unwrap()).unwrap().len(), 1);
        assert_eq!(std::fs::read(&p).unwrap(), before);
        assert_eq!(std::fs::read(p.with_extension("db-wal")).unwrap(), wal);
        db.execute("DROP TABLE providers", []).unwrap();
        assert!(read(p.to_str().unwrap()).is_err());
    }
}
#[cfg(test)]
mod local_smoke {
    #[test]
    #[ignore = "requires the developer's real CC Switch database"]
    fn ccswitch_read_local_database_without_printing_credentials() {
        let records = super::read("").unwrap();
        assert!(!records.is_empty());
        assert!(records
            .iter()
            .any(|r| r.app == "claude" || r.app == "codex"));
    }
}
