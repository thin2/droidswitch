use crate::provider::google_convert::{
    gemini_to_openai_response, openai_to_gemini_request, to_sse,
};
use crate::proxy::ProxyRegistry;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const MAX_BODY: usize = 8 * 1024 * 1024;
#[derive(Clone)]
pub struct AppState {
    pub registry: ProxyRegistry,
    pub client: reqwest::Client,
}

pub(crate) fn endpoint(base: &str, model: &str) -> Result<reqwest::Url, AppError> {
    let mut url =
        reqwest::Url::parse(base).map_err(|_| AppError::bad("Invalid configured endpoint"))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(AppError::bad("Invalid configured endpoint"));
    }
    let model = model.strip_prefix("models/").unwrap_or(model);
    if model.is_empty()
        || !model
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        || model == "."
        || model == ".."
    {
        return Err(AppError::bad("Unsupported Gemini model ID"));
    }
    let path = url.path().trim_end_matches('/');
    // A host-only endpoint defaults to v1beta; otherwise preserve the configured
    // API version/prefix (including reverse proxy prefixes), never append /v1 twice.
    let prefix = if path.is_empty() { "/v1beta" } else { path };
    url.set_path(&format!("{prefix}/models/{model}:generateContent"));
    Ok(url)
}

pub async fn proxy_handler(
    State(state): State<AppState>,
    Path(provider_id): Path<String>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Response, AppError> {
    if headers.contains_key("origin") || headers.contains_key("sec-fetch-site") {
        return Err(AppError(
            StatusCode::FORBIDDEN,
            "Browser requests are not supported",
        ));
    }
    let config = state.registry.get(&provider_id).ok_or(AppError(
        StatusCode::NOT_FOUND,
        "Unknown or unavailable provider",
    ))?;
    let supplied = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .unwrap_or("");
    // Compare fixed-sized digests rather than early-exiting on secret characters.
    let a = Sha256::digest(supplied.as_bytes());
    let b = Sha256::digest(config.api_key.as_bytes());
    let mismatch = a
        .iter()
        .zip(b.iter())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b));
    if mismatch != 0 || supplied.is_empty() {
        return Err(AppError(
            StatusCode::UNAUTHORIZED,
            "Invalid proxy credentials",
        ));
    }
    let req: Value =
        serde_json::from_slice(&body).map_err(|_| AppError::bad("Invalid JSON request"))?;
    let model = req["model"]
        .as_str()
        .ok_or(AppError::bad("Missing model"))?;
    if !config.models.contains(model) {
        return Err(AppError(
            StatusCode::NOT_FOUND,
            "Model is not enabled for this applied Gemini connection",
        ));
    }
    let native = openai_to_gemini_request(&req).map_err(|_| {
        AppError::bad(
            "Unsupported or invalid Chat Completions request; see native proxy limitations",
        )
    })?;
    let target = endpoint(&config.base_url, model)?;
    let mut request = state.client.post(target);
    for (name, value) in &config.extra_headers {
        // Transport and native API key headers are bridge-owned. A configured
        // Authorization header may authenticate an upstream gateway; the local
        // client's Authorization header is never forwarded.
        if [
            "host",
            "content-length",
            "transfer-encoding",
            "connection",
            "content-type",
            "x-goog-api-key",
        ]
        .iter()
        .any(|n| name.eq_ignore_ascii_case(n))
        {
            continue;
        }
        request = request.header(name, value);
    }
    let mut upstream = request
        .header("x-goog-api-key", &config.api_key)
        .json(&native)
        .send()
        .await
        .map_err(|_| AppError(StatusCode::BAD_GATEWAY, "Gemini transport failed"))?;
    if !upstream.status().is_success() {
        // Never echo upstream bodies, URLs, or credentials.
        let message = match upstream.status().as_u16() {
            401 => "Gemini upstream authentication failed (401); check the provider key and gateway authentication headers",
            403 => "Gemini upstream access denied (403); check model and key permissions",
            429 => "Gemini upstream rate limit or quota exceeded (429)",
            400 => "Gemini upstream rejected the request format (400)",
            404 => "Gemini upstream endpoint or model not found (404)",
            _ => "Gemini rejected the request",
        };
        return Err(AppError(StatusCode::BAD_GATEWAY, message));
    }
    if upstream
        .content_length()
        .is_some_and(|n| n > MAX_BODY as u64)
    {
        return Err(AppError(
            StatusCode::BAD_GATEWAY,
            "Gemini response exceeds size limit",
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = upstream
        .chunk()
        .await
        .map_err(|_| AppError(StatusCode::BAD_GATEWAY, "Gemini response read failed"))?
    {
        if bytes.len() + chunk.len() > MAX_BODY {
            return Err(AppError(
                StatusCode::BAD_GATEWAY,
                "Gemini response exceeds size limit",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    let response: Value = serde_json::from_slice(&bytes)
        .map_err(|_| AppError(StatusCode::BAD_GATEWAY, "Invalid Gemini response"))?;
    let output = gemini_to_openai_response(&response, model)
        .map_err(|_| AppError(StatusCode::BAD_GATEWAY, "Unsupported Gemini response"))?;
    if req["stream"] == true {
        let sse = to_sse(&output, req["stream_options"]["include_usage"] == true);
        if sse.len() > MAX_BODY {
            return Err(AppError(
                StatusCode::BAD_GATEWAY,
                "Converted response exceeds size limit",
            ));
        }
        Ok((
            [
                ("content-type", "text/event-stream"),
                ("cache-control", "no-cache"),
                ("x-accel-buffering", "no"),
            ],
            sse,
        )
            .into_response())
    } else {
        let body = output.to_string();
        if body.len() > MAX_BODY {
            return Err(AppError(
                StatusCode::BAD_GATEWAY,
                "Converted response exceeds size limit",
            ));
        }
        Ok(([("content-type", "application/json")], body).into_response())
    }
}

pub struct AppError(pub StatusCode, pub &'static str);
impl AppError {
    fn bad(message: &'static str) -> Self {
        Self(StatusCode::BAD_REQUEST, message)
    }
}
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            self.0,
            Json(json!({"error":{"message":self.1,"type":"proxy_error"}})),
        )
            .into_response()
    }
}
