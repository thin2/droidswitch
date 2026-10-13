use super::{
    fetch::{endpoint, headers, network_error, status_error},
    ProviderConnection,
};
use crate::{
    model::{
        entry::{ModelEntry, Protocol},
        ids::InternalId,
    },
    net,
    workspace::Store,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    pub success: bool,
    pub message: String,
    pub elapsed_ms: u64,
    pub tested_at: u64,
}
pub fn test_request(
    p: &ProviderConnection,
    m: &ModelEntry,
    consent: bool,
) -> Result<net::HttpRequest, String> {
    if !consent {
        return Err("调用测试会消耗供应商额度，请先确认".into());
    }
    p.validate()?;
    let protocol = m.effective_protocol(p.protocol);
    let id = m.api_model_id.as_str();
    let (suffix, body) = match protocol {
        Protocol::AnthropicMessages => (
            "messages",
            json!({"model":id,"max_tokens":32,"messages":[{"role":"user","content":"Reply OK."}]}),
        ),
        Protocol::OpenAiResponses => (
            "responses",
            json!({"model":id,"max_output_tokens":32,"input":"Reply OK.","store":false}),
        ),
        Protocol::OpenAiChatCompletions => (
            "chat/completions",
            json!({"model":id,"max_tokens":32,"messages":[{"role":"user","content":"Reply OK."}]}),
        ),
        Protocol::GoogleGemini => (
            "interactions",
            json!({"model":id,"input":"Reply OK.","generationConfig":{"maxOutputTokens":32}}),
        ),
    };
    Ok(net::HttpRequest {
        url: endpoint(&p.base_url, suffix)?,
        method: net::HttpMethod::Post,
        headers: headers(p, protocol)?,
        body: Some(body),
        timeout_ms: 30000,
    })
}
pub fn response_valid(body: &str, protocol: Protocol) -> bool {
    let Ok(v) = serde_json::from_str::<Value>(body) else {
        return false;
    };
    if v.get("error").is_some_and(|v| !v.is_null()) {
        return false;
    }
    match protocol {
        Protocol::AnthropicMessages => {
            v.get("type").and_then(Value::as_str) == Some("message")
                && v.get("content").is_some_and(Value::is_array)
        }
        Protocol::OpenAiResponses => {
            v.get("object").and_then(Value::as_str) == Some("response")
                && matches!(
                    v.get("status").and_then(Value::as_str),
                    Some("completed" | "incomplete")
                )
                && v.get("output").is_some_and(Value::is_array)
        }
        Protocol::OpenAiChatCompletions => {
            v.get("choices").and_then(Value::as_array).is_some_and(|a| {
                a.iter()
                    .any(|c| c.get("message").is_some_and(Value::is_object))
            })
        }
        Protocol::GoogleGemini => {
            v.get("id").is_some()
                && v.get("steps").and_then(Value::as_array).is_some()
        }
    }
}
#[tauri::command]
pub async fn test_model(
    store: tauri::State<'_, Store>,
    id: InternalId,
    consent: bool,
) -> Result<TestResult, String> {
    let w = store.load()?;
    let m = w
        .catalog
        .models
        .iter()
        .find(|m| m.id == id)
        .ok_or("模型不存在")?;
    let p = w.catalog.provider(m.provider_id)?;
    let request = test_request(p, m, consent)?;
    let start = std::time::Instant::now();
    let response = net::execute(&net::client().map_err(network_error)?, request).await;
    let (success, message) = match response {
        Ok(r) if !(200..300).contains(&r.status) => (false, status_error(r.status)),
        Ok(r) if !response_valid(&r.body, m.effective_protocol(p.protocol)) => {
            (false, "响应与所选协议不符，不能确认调用成功".into())
        }
        Ok(_) => (true, "接口已响应；此结果不代表 Droid 配置已生效".into()),
        Err(e) => (false, network_error(e)),
    };
    let result = TestResult {
        success,
        message,
        elapsed_ms: start.elapsed().as_millis() as u64,
        tested_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };
    // Results are non-secret metadata. Revision stays unchanged, as this isn't a draft edit.
    let db = store.db.lock().map_err(|_| "数据库繁忙")?;
    db.execute("INSERT INTO sync_cursors(source_file,file_identity,byte_offset,state,updated_at) VALUES (?1,'connection-test',0,?2,unixepoch()) ON CONFLICT(source_file) DO UPDATE SET state=excluded.state,updated_at=excluded.updated_at",rusqlite::params![format!("connection-test:{id}"),serde_json::to_string(&result).map_err(|_|"测试结果序列化失败")?]).map_err(|_|"保存测试结果失败")?;
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conn_test_requires_consent_and_never_uses_extra_args() {
        let mut p = crate::provider::tests::sample();
        p.credential.mode = super::super::CredentialMode::Direct;
        p.credential.value = "test-only".into();
        let mut m = ModelEntry::new(
            p.id,
            crate::model::ids::ApiModelId::try_from("model".to_owned()).unwrap(),
            "Name".into(),
        );
        m.extra_args
            .insert("input".into(), json!("private project"));
        assert!(test_request(&p, &m, false).is_err());
        for protocol in [
            Protocol::AnthropicMessages,
            Protocol::OpenAiResponses,
            Protocol::OpenAiChatCompletions,
            Protocol::GoogleGemini,
        ] {
            m.protocol_override = Some(protocol);
            let r = test_request(&p, &m, true).unwrap();
            let text = r.body.unwrap().to_string();
            assert!(text.contains("Reply OK."));
            assert!(!text.contains("private project"));
        }
    }
    #[test]
    fn conn_test_classifies_status_and_rejects_false_success() {
        for code in [401, 403, 404, 429, 500] {
            assert!(!status_error(code).is_empty());
        }
        assert!(!response_valid("{}", Protocol::OpenAiResponses));
        assert!(!response_valid(
            r#"{"error":{"message":"private"}}"#,
            Protocol::OpenAiResponses
        ));
        assert!(response_valid(
            r#"{"object":"response","status":"incomplete","output":[]}"#,
            Protocol::OpenAiResponses
        ));
    }
    #[test]
    fn gemini_response_validation() {
        assert!(response_valid(
            r#"{"id":"int-123","steps":[{"type":"model_output","content":[{"type":"text","text":"Hi"}]}]}"#,
            Protocol::GoogleGemini
        ));
        assert!(!response_valid(
            r#"{"error":{"message":"Invalid key"}}"#,
            Protocol::GoogleGemini
        ));
        assert!(!response_valid(
            r#"{"id":"int-123"}"#,
            Protocol::GoogleGemini
        ));
        assert!(!response_valid(
            r#"{"steps":[]}"#,
            Protocol::GoogleGemini
        ));
    }
}
#[tauri::command]
pub fn latest_tests(
    store: tauri::State<'_, Store>,
) -> Result<std::collections::BTreeMap<String, TestResult>, String> {
    let db = store.db.lock().map_err(|_| "数据库繁忙")?;
    let mut statement = db
        .prepare("SELECT source_file,state FROM sync_cursors WHERE file_identity='connection-test'")
        .map_err(|_| "无法读取测试记录")?;
    let rows = statement
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|_| "无法读取测试记录")?;
    let mut out = std::collections::BTreeMap::new();
    for row in rows {
        let (k, v) = row.map_err(|_| "测试记录无效")?;
        if let Some(id) = k.strip_prefix("connection-test:") {
            out.insert(
                id.into(),
                serde_json::from_str(&v).map_err(|_| "测试记录结构未知")?,
            );
        }
    }
    Ok(out)
}
#[tauri::command]
pub fn validate_workspace(store: tauri::State<'_, Store>) -> Result<String, String> {
    let w = store.load()?;
    w.catalog.validate()?;
    w.scheme.validate(&w.catalog)?;
    let missing = w
        .catalog
        .providers
        .iter()
        .filter(|p| p.credential.resolve().is_err())
        .count();
    Ok(format!(
        "字段、JSON 与模型引用校验通过。{missing} 个连接的凭据在当前进程不可用；未发送网络请求。"
    ))
}
