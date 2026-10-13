//! Generate a synthetic, real backend-shaped IPC fixture for UI regression tests.
use droid_switch_lib::{db, usage};
fn main() {
    let output = std::env::args().nth(1).expect("output path required");
    let mut db = rusqlite::Connection::open_in_memory().unwrap();
    db::schema::migrate_to(&mut db, db::schema::SCHEMA_VERSION).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    db.execute("INSERT INTO usage_endpoints(id,endpoint,label,created_at,updated_at) VALUES ('qa','https://qa.example.invalid','QA',0,0)",[]).unwrap();
    for i in 0..20 {
        db.execute("INSERT INTO usage_events(id,source_file,event_key,session_id,occurred_at,source,provider_id_at_event,api_model_id,input_tokens,cache_read_tokens,cache_write_tokens,output_tokens,thinking_tokens,estimated_cost,cost_units,currency,cost_complete) VALUES (?1,'synthetic',?1,'session',?2,'api','qa','claude-test',1200,400,0,100,0,'0.012',12000000000,'USD',1)",rusqlite::params![format!("e{i}"),now-i*3600]).unwrap();
    }
    let filter:usage::aggregate::Filter=serde_json::from_value(serde_json::json!({"from":now-7*86400,"to":now+86400,"includeUnknownTime":false,"offsetMinutes":480})).unwrap();
    let page = usage::aggregate::page(&db, &filter, 100, 0).unwrap();
    let payload = usage::commands::UsagePayload {
        settings: usage::commands::UsageSettings::default(),
        notice: None,
        report: None,
        page,
        scanning: false,
    };
    std::fs::write(output, serde_json::to_string_pretty(&payload).unwrap()).unwrap();
}
