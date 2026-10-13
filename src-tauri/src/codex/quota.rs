//! Read-only quota lookup through CPA. OAuth tokens stay inside CPA.
use super::{accounts, api, client, CodexService, BASE};
use reqwest::Method;
use serde::Serialize;
use serde_json::{json, Value};
use std::io::Read;
use tauri::Manager;

const MAX_RESPONSE: u64 = 512 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    label: String,
    remaining_percent: Option<f64>,
    window_seconds: Option<i64>,
    reset_at: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Quota {
    windows: Vec<Window>,
    allowed: Option<bool>,
    limit_reached: Option<bool>,
    fetched_at: i64,
}

fn request_body(account: &Value) -> Result<Value, String> {
    let index = account["auth_index"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("当前 CPA 未提供账号索引，无法查询额度")?;
    let account_id = account["id_token"]["chatgpt_account_id"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 256 && !s.chars().any(char::is_control))
        .ok_or("当前 CPA 未提供账号身份，无法安全查询额度")?;
    Ok(json!({
        "auth_index": index,
        "method": "GET",
        "url": "https://chatgpt.com/backend-api/wham/usage",
        "header": {
            "Authorization": "Bearer $TOKEN$",
            "ChatGPT-Account-Id": account_id,
            "Accept": "application/json",
            "User-Agent": "codex-cli",
            "OpenAI-Beta": "codex-1"
        }
    }))
}

fn parse_response(envelope: &Value, now: i64) -> Result<Quota, String> {
    match envelope["status_code"].as_u64() {
        Some(200) => {}
        Some(401 | 403) => return Err("额度查询授权失效或无权限，请检查登录状态".into()),
        Some(429) => return Err("额度接口请求过于频繁，请稍后重试".into()),
        _ => return Err("上游额度接口暂不可用，请稍后重试".into()),
    }
    let body: Value = serde_json::from_str(envelope["body"].as_str().ok_or("额度响应格式不兼容")?)
        .map_err(|_| "额度响应格式不兼容")?;
    let mut windows = Vec::new();
    for (field, prefix) in [
        ("rate_limit", "使用额度"),
        ("code_review_rate_limit", "代码审查额度"),
    ] {
        for (key, label) in [
            ("primary_window", "主要窗口"),
            ("secondary_window", "次要窗口"),
        ] {
            let window = &body[field][key];
            if !window.is_object() {
                continue;
            }
            let remaining_percent = window["used_percent"]
                .as_f64()
                .filter(|v| v.is_finite() && (0.0..=100.0).contains(v))
                .map(|used| 100.0 - used);
            let window_seconds = window["limit_window_seconds"].as_i64().filter(|v| *v > 0);
            let reset_at = window["reset_at"]
                .as_i64()
                .filter(|v| *v > 0 && *v <= 253402300799);
            windows.push(Window {
                label: format!("{prefix} · {label}"),
                remaining_percent,
                window_seconds,
                reset_at,
            });
        }
    }
    if windows.is_empty() {
        return Err("上游未提供可识别的额度窗口，剩余额度未知".into());
    }
    Ok(Quota {
        windows,
        allowed: body["rate_limit"]["allowed"].as_bool(),
        limit_reached: body["rate_limit"]["limit_reached"].as_bool(),
        fetched_at: now,
    })
}

#[tauri::command]
pub async fn codex_quota(app: tauri::AppHandle) -> Result<Quota, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let service = app.state::<CodexService>();
        let mut runtime = service
            .inner
            .try_lock()
            .map_err(|_| "Codex 服务繁忙，请稍后刷新额度")?;
        if !runtime.alive()? {
            return Err("请先启动 CPA 服务".into());
        }
        if runtime.session.is_some() {
            return Err("请先完成登录".into());
        }
        let keys = runtime.keys()?;
        let credentials = api(&keys, Method::GET, "credentials", &[])?;
        let entries = accounts(&credentials)?;
        let account = entries.first().ok_or("请先登录 ChatGPT 账号")?;
        let body = request_body(account)?;
        let response = client()?
            .post(format!("{BASE}/v8/management/requests/api-call"))
            .bearer_auth(&keys.management)
            .json(&body)
            .send()
            .map_err(|_| "额度查询超时或连接失败，请稍后重试")?;
        if !response.status().is_success() {
            return Err("CPA 额度查询失败或当前版本不支持，请稍后重试".into());
        }
        let mut bytes = Vec::new();
        response
            .take(MAX_RESPONSE + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "读取额度响应失败")?;
        if bytes.len() as u64 > MAX_RESPONSE {
            return Err("额度响应超出大小限制".into());
        }
        let envelope: Value =
            serde_json::from_slice(&bytes).map_err(|_| "CPA 额度响应格式不兼容")?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "系统时间无效")?
            .as_secs() as i64;
        parse_response(&envelope, now)
    })
    .await
    .map_err(|_| "额度查询任务失败")?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(body: Value) -> Value {
        json!({"status_code":200,"body":body.to_string()})
    }

    #[test]
    fn quota_request_is_fixed_and_never_copies_tokens() {
        let body = request_body(&json!({"auth_index":"index","id_token":{"chatgpt_account_id":"account"},"access_token":"secret"})).unwrap();
        assert_eq!(body["url"], "https://chatgpt.com/backend-api/wham/usage");
        assert_eq!(body["method"], "GET");
        assert_eq!(body["header"]["Authorization"], "Bearer $TOKEN$");
        assert!(!body.to_string().contains("secret"));
        assert!(request_body(&json!({"auth_index":"index","id_token":"raw-token"})).is_err());
        assert!(request_body(
            &json!({"auth_index":"index","id_token":{"chatgpt_account_id":"x\r\nInjected"}})
        )
        .is_err());
    }

    #[test]
    fn quota_windows_keep_zero_unknown_and_reset_distinct() {
        let parsed = parse_response(
            &response(json!({"rate_limit":{"allowed":false,"limit_reached":true,
            "primary_window":{"used_percent":100,"limit_window_seconds":18000,"reset_at":2000},
            "secondary_window":{"used_percent":25,"limit_window_seconds":604800}}})),
            1000,
        )
        .unwrap();
        assert_eq!(parsed.windows[0].remaining_percent, Some(0.0));
        assert_eq!(parsed.windows[0].reset_at, Some(2000));
        assert_eq!(parsed.windows[1].remaining_percent, Some(75.0));
        assert_eq!(parsed.limit_reached, Some(true));
        assert_eq!(parsed.fetched_at, 1000);
    }

    #[test]
    fn quota_missing_or_invalid_fields_never_become_full_balance() {
        for value in [Value::Null, json!(-1), json!(101), json!("10")] {
            let parsed = parse_response(
                &response(json!({"rate_limit":{"primary_window":{"used_percent":value}}})),
                1,
            )
            .unwrap();
            assert_eq!(parsed.windows[0].remaining_percent, None);
        }
        assert!(parse_response(&response(json!({})), 1).is_err());
    }

    #[test]
    fn quota_errors_never_return_upstream_body() {
        for code in [401, 403, 429, 500] {
            let error = parse_response(
                &json!({"status_code":code,"body":"sensitive upstream text"}),
                1,
            )
            .unwrap_err();
            assert!(!error.contains("sensitive"));
        }
    }
}
