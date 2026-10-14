use super::ProviderConnection;
use crate::{
    model::{
        entry::Protocol,
        ids::{ApiModelId, InternalId},
    },
    net,
    workspace::Store,
};
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};

pub fn endpoint(base: &str, suffix: &str) -> Result<String, String> {
    let mut url = reqwest::Url::parse(base).map_err(|_| "接口地址无效")?;
    url.set_path(&format!("{}/{}", url.path().trim_end_matches('/'), suffix));
    Ok(url.to_string())
}
pub fn headers(
    p: &ProviderConnection,
    protocol: Protocol,
) -> Result<BTreeMap<String, String>, String> {
    let mut headers: BTreeMap<String, String> = p
        .extra_headers
        .iter()
        .map(|(k, v)| (k.to_ascii_lowercase(), v.clone()))
        .collect();
    let secret = p.credential.resolve()?;
    match protocol {
        Protocol::AnthropicMessages => {
            headers.insert("x-api-key".into(), secret);
            headers
                .entry("anthropic-version".into())
                .or_insert("2023-06-01".into());
        }
        Protocol::GoogleGemini => {
            headers.insert("x-goog-api-key".into(), secret);
        }
        _ => {
            headers.insert("authorization".into(), format!("Bearer {secret}"));
        }
    }
    Ok(headers)
}
pub fn network_error(e: net::NetworkError) -> String {
    match e {
        net::NetworkError::Timeout => "请求超时",
        net::NetworkError::Transport => "连接失败，请检查网络、证书及代理",
        net::NetworkError::ResponseTooLarge => "响应超过大小限制",
        _ => "请求或响应格式无效",
    }
    .into()
}
pub fn status_error(status: u16) -> String {
    match status {
        401 | 403 => "凭据无效或无访问权限".into(),
        404 => "接口不存在，请检查完整地址与协议".into(),
        429 => "接口限流或额度不足".into(),
        _ => format!("接口返回 HTTP {status}"),
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteModels {
    pub ids: Vec<ApiModelId>,
    pub elapsed_ms: u64,
    pub incomplete: bool,
}
pub fn parse_models(body: &str, protocol: Protocol) -> Result<(Vec<ApiModelId>, bool), String> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|_| "模型列表不是有效 JSON")?;

    match protocol {
        Protocol::GoogleGemini => {
            // Gemini: {"models": [{"name": "models/gemini-3.8-flash", ...}]}
            let models = value
                .get("models")
                .and_then(|v| v.as_array())
                .ok_or("模型列表缺少 models 数组")?;

            let mut seen = HashSet::new();
            let mut ids = vec![];
            for item in models {
                let raw = item
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or("模型列表含无效 name")?;

                // Strip "models/" prefix if present
                let id_str = raw.strip_prefix("models/").unwrap_or(raw);
                let id = ApiModelId::try_from(id_str.to_owned()).map_err(|_| "模型列表含空 ID")?;

                if seen.insert(id.clone()) {
                    ids.push(id);
                }
            }
            Ok((ids, false)) // Gemini does not use pagination
        }
        _ => {
            // OpenAI-compatible format
            let data = value
                .get("data")
                .and_then(|v| v.as_array())
                .ok_or("模型列表缺少 data 数组，请手动添加模型")?;
            let mut seen = HashSet::new();
            let mut ids = vec![];
            for item in data {
                let raw = item
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or("模型列表含无效 ID")?;
                let id = ApiModelId::try_from(raw.to_owned()).map_err(|_| "模型列表含空 ID")?;
                if seen.insert(id.clone()) {
                    ids.push(id);
                }
            }
            Ok((
                ids,
                value
                    .get("has_more")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            ))
        }
    }
}
#[tauri::command]
pub async fn fetch_models(
    store: tauri::State<'_, Store>,
    provider_id: InternalId,
) -> Result<RemoteModels, String> {
    let p = store.load()?.catalog.provider(provider_id)?.clone();
    let url = match p.model_list_url.as_ref().filter(|s| !s.is_empty()) {
        Some(u) => u.clone(),
        None => endpoint(&p.base_url, "models")?,
    };
    let request = net::HttpRequest {
        url,
        method: net::HttpMethod::Get,
        headers: headers(&p, p.protocol)?,
        body: None,
        timeout_ms: 15000,
    };
    let r = net::execute(&net::client().map_err(network_error)?, request)
        .await
        .map_err(network_error)?;
    if !(200..300).contains(&r.status) {
        return Err(status_error(r.status));
    }
    let (ids, incomplete) = parse_models(&r.body, p.protocol)?;
    Ok(RemoteModels {
        ids,
        elapsed_ms: r.elapsed_ms,
        incomplete,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fetch_preserves_exact_ids_deduplicates_and_reports_pagination() {
        let (ids, more) = parse_models(
            r#"{"data":[{"id":"a/b:1"},{"id":"a/b:1"},{"id":" A "}],"has_more":true}"#,
            Protocol::OpenAiResponses,
        )
        .unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[1].as_str(), " A ");
        assert!(more);
        assert!(parse_models("{}", Protocol::OpenAiResponses).is_err());
        assert!(parse_models(r#"{"data":[{"id":3}]}"#, Protocol::OpenAiResponses).is_err());
    }
    #[test]
    fn fetch_endpoint_keeps_custom_path_and_query() {
        assert_eq!(
            endpoint("https://example.com/custom/v2/?region=a", "models").unwrap(),
            "https://example.com/custom/v2/models?region=a"
        );
    }
    #[test]
    fn fetch_gemini_models() {
        let (ids, more) = parse_models(
            r#"{"models":[{"name":"models/gemini-3.8-flash"},{"name":"models/gemini-3.1-pro"}]}"#,
            Protocol::GoogleGemini,
        )
        .unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0].as_str(), "gemini-3.8-flash");
        assert_eq!(ids[1].as_str(), "gemini-3.1-pro");
        assert!(!more);
    }
    #[test]
    fn fetch_gemini_strips_models_prefix() {
        let (ids, _) = parse_models(
            r#"{"models":[{"name":"models/gemini-test"},{"name":"no-prefix"}]}"#,
            Protocol::GoogleGemini,
        )
        .unwrap();
        assert_eq!(ids[0].as_str(), "gemini-test");
        assert_eq!(ids[1].as_str(), "no-prefix");
    }
}
