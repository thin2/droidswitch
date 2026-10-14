use super::{handler, server, ProviderConfig, ProxyRegistry};
use crate::{
    model::{
        entry::{ModelEntry, Protocol},
        ids::ApiModelId,
    },
    provider::{google_convert::*, CredentialMode},
    workspace::Store,
};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode, Uri},
    response::IntoResponse,
    Router,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

type Observed = Arc<Mutex<Vec<(String, HeaderMap, Value)>>>;
#[derive(Clone)]
struct MockState {
    observed: Observed,
    response: Arc<Mutex<(StatusCode, String)>>,
}
struct Harness {
    url: String,
    native_url: String,
    mock: MockState,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}
impl Drop for Harness {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}
async fn capture(
    State(s): State<MockState>,
    uri: Uri,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    s.observed.lock().unwrap().push((
        uri.to_string(),
        headers,
        serde_json::from_slice(&body).unwrap(),
    ));
    let (status, body) = s.response.lock().unwrap().clone();
    (status, [("location", "/must-not-follow")], body)
}
async fn serve(app: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    (
        url,
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        }),
    )
}
impl Harness {
    async fn new(response: Value) -> Self {
        let mock = MockState {
            observed: Default::default(),
            response: Arc::new(Mutex::new((StatusCode::OK, response.to_string()))),
        };
        let (native_url, native_task) =
            serve(Router::new().fallback(capture).with_state(mock.clone())).await;
        let registry = ProxyRegistry::new();
        registry.register(
            "fixture".into(),
            ProviderConfig {
                base_url: format!("{native_url}/custom/v1beta"),
                api_key: "synthetic-key".into(),
                models: ["gemini-fixture".into()].into(),
                extra_headers: [
                    ("x-fixture".into(), "configured".into()),
                    ("authorization".into(), "Bearer gateway-fixture".into()),
                ]
                .into(),
            },
        );
        let (url, proxy_task) = serve(server::router(registry).unwrap()).await;
        Self {
            url,
            native_url,
            mock,
            tasks: vec![native_task, proxy_task],
        }
    }
    fn request(&self) -> reqwest::RequestBuilder {
        reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .post(format!("{}/proxy/fixture/v1/chat/completions", self.url))
            .bearer_auth("synthetic-key")
    }
}
fn request() -> Value {
    json!({"model":"gemini-fixture","messages":[{"role":"user","content":"Hi"}]})
}
fn response() -> Value {
    json!({"candidates":[{"content":{"role":"model","parts":[{"text":"Hello"}]},"finishReason":"STOP"}],
        "usageMetadata":{"promptTokenCount":7,"candidatesTokenCount":3,"thoughtsTokenCount":2,"totalTokenCount":12,"cachedContentTokenCount":4}})
}

#[tokio::test]
async fn droid_reasoning_effort_reaches_native_thinking_config() {
    let h = Harness::new(response()).await;
    // Shape observed from Droid exec with a Gemini-named custom model.
    let mut req = json!({"model":"gemini-fixture","messages":[
        {"role":"system","content":"Test assistant"},
        {"role":"user","content":[{"type":"text","text":"Synthetic context"}]},
        {"role":"user","content":"Reply OK"}
    ],"stream":true,"stream_options":{"include_usage":true},"temperature":1,
      "max_tokens":32000,"reasoning_effort":"high",
      "tools":[{"type":"function","function":{"name":"Read","description":"Read fixture",
      "parameters":{"type":"object","properties":{"file_path":{"type":"string"}},"required":["file_path"]}}}]});
    for (effort, level) in [
        ("minimal", "MINIMAL"),
        ("low", "LOW"),
        ("medium", "MEDIUM"),
        ("high", "HIGH"),
    ] {
        req["reasoning_effort"] = json!(effort);
        let result = h.request().json(&req).send().await.unwrap();
        assert_eq!(result.status(), 200);
        assert!(result.text().await.unwrap().ends_with("data: [DONE]\n\n"));
        let records = h.mock.observed.lock().unwrap();
        let native = &records.last().unwrap().2;
        assert_eq!(
            native["generationConfig"]["thinkingConfig"]["thinkingLevel"],
            level
        );
        assert!(native.get("reasoning_effort").is_none());
    }
    for invalid in [json!("unknown"), json!(false), json!(123), json!({})] {
        req["reasoning_effort"] = invalid;
        assert_eq!(h.request().json(&req).send().await.unwrap().status(), 400);
    }
    assert_eq!(h.mock.observed.lock().unwrap().len(), 4);
    for omit in [false, true] {
        req["reasoning_effort"] = Value::Null;
        if omit {
            req.as_object_mut().unwrap().remove("reasoning_effort");
        }
        let native = openai_to_gemini_request(&req).unwrap();
        assert!(native["generationConfig"].get("thinkingConfig").is_none());
    }
}

#[tokio::test]
async fn native_http_preserves_history_images_config_headers_and_usage() {
    let h = Harness::new(response()).await;
    let req = json!({"model":"gemini-fixture","messages":[
        {"role":"system","content":"Be helpful"},{"role":"developer","content":"Be brief"},
        {"role":"user","content":"First"},{"role":"assistant","content":"Previous"},
        {"role":"user","content":[{"type":"text","text":"Inspect"},{"type":"image_url","image_url":{"url":"data:image/png;base64,AAAA"}}]}
    ],"temperature":0.5,"top_p":0.9,"max_tokens":50,"stop":["END"]});
    let result = h.request().json(&req).send().await.unwrap();
    assert_eq!(result.status(), 200);
    let result: Value = result.json().await.unwrap();
    assert_eq!(result["choices"][0]["message"]["content"], "Hello");
    assert_eq!(result["usage"]["completion_tokens"], 5);
    assert_eq!(result["usage"]["total_tokens"], 12);
    assert_eq!(result["usage"]["prompt_tokens_details"]["cached_tokens"], 4);
    let records = h.mock.observed.lock().unwrap();
    let (uri, headers, body) = &records[0];
    assert_eq!(uri, "/custom/v1beta/models/gemini-fixture:generateContent");
    assert_eq!(headers["x-goog-api-key"], "synthetic-key");
    assert_eq!(headers["x-fixture"], "configured");
    assert_eq!(headers["authorization"], "Bearer gateway-fixture");
    assert_eq!(body["contents"].as_array().unwrap().len(), 3);
    assert_eq!(body["contents"][1]["role"], "model");
    assert_eq!(
        body["contents"][2]["parts"][1]["inlineData"]["data"],
        "AAAA"
    );
    assert_eq!(
        body["systemInstruction"]["parts"].as_array().unwrap().len(),
        2
    );
    assert_eq!(body["generationConfig"]["maxOutputTokens"], 50);
    assert!(body.get("model").is_none());
    assert!(body.get("input").is_none());
}

#[tokio::test]
async fn streamed_parallel_tools_roundtrip_retains_ids_and_thought_signatures() {
    let native = json!({"candidates":[{"content":{"parts":[
        {"thought":true,"text":"private reasoning"},{"text":"Checking"},
        {"functionCall":{"name":"lookup","args":{"q":"a"},"id":"native-a"},"thoughtSignature":"opaque-signature"},
        {"functionCall":{"name":"lookup","args":{"q":"b"}}}
    ]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":4,"totalTokenCount":14}});
    let h = Harness::new(native).await;
    let mut req = request();
    req["stream"] = json!(true);
    req["stream_options"] = json!({"include_usage":true});
    req["tools"] = json!([{"type":"function","function":{"name":"lookup","description":"Search","parameters":{"type":"object","properties":{"q":{"type":"string"}},"required":["q"]}}}]);
    req["tool_choice"] = json!({"type":"function","function":{"name":"lookup"}});
    let result = h.request().json(&req).send().await.unwrap();
    assert_eq!(result.status(), 200);
    assert_eq!(result.headers()["content-type"], "text/event-stream");
    let sse = result.text().await.unwrap();
    assert!(sse.ends_with("data: [DONE]\n\n"));
    assert!(!sse.contains("private reasoning"));
    let chunks: Vec<Value> = sse
        .lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .filter(|s| *s != "[DONE]")
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(chunks[0]["choices"][0]["delta"]["role"], "assistant");
    assert_eq!(
        chunks[chunks.len() - 2]["choices"][0]["finish_reason"],
        "tool_calls"
    );
    assert_eq!(chunks.last().unwrap()["usage"]["total_tokens"], 14);
    assert_eq!(chunks.last().unwrap()["choices"], json!([]));
    let calls: Vec<Value> = chunks
        .iter()
        .filter_map(|c| c["choices"][0]["delta"]["tool_calls"][0].as_object())
        .map(|c| {
            let mut c = c.clone();
            c.remove("index");
            json!(c)
        })
        .collect();
    assert_eq!(calls.len(), 2);
    req["stream"] = json!(false);
    req["messages"] = json!([
        {"role":"user","content":"Hi"},
        {"role":"assistant","content":"Checking","tool_calls":calls},
        {"role":"tool","tool_call_id":calls[1]["id"],"content":"second"},
        {"role":"tool","tool_call_id":calls[0]["id"],"content":"{\"found\":true}"}
    ]);
    *h.mock.response.lock().unwrap() = (StatusCode::OK, response().to_string());
    assert_eq!(h.request().json(&req).send().await.unwrap().status(), 200);
    let records = h.mock.observed.lock().unwrap();
    let initial = &records[0].2;
    assert_eq!(
        initial["tools"][0]["functionDeclarations"][0]["parametersJsonSchema"]["type"],
        "object"
    );
    assert_eq!(
        initial["toolConfig"]["functionCallingConfig"]["allowedFunctionNames"],
        json!(["lookup"])
    );
    let native = &records[1].2;
    assert_eq!(
        native["contents"][1]["parts"][1]["thoughtSignature"],
        "opaque-signature"
    );
    assert_eq!(
        native["contents"][1]["parts"][1]["functionCall"]["id"],
        "native-a"
    );
    assert_eq!(
        native["contents"][2]["parts"][1]["functionResponse"]["id"],
        "native-a"
    );
    assert_eq!(
        native["contents"][2]["parts"][1]["functionResponse"]["response"]["output"]["found"],
        true
    );
}

#[tokio::test]
async fn authentication_origins_routes_models_and_bad_requests_never_reach_upstream() {
    let h = Harness::new(response()).await;
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    for path in [
        "/proxy/fixture/v1/responses",
        "/proxy/fixture/v1/chat/completions/extra",
        "/proxy/unknown/v1/chat/completions",
        "/health",
    ] {
        assert_eq!(
            client
                .post(format!("{}{path}", h.url))
                .json(&request())
                .send()
                .await
                .unwrap()
                .status(),
            404
        );
    }
    let route = format!("{}/proxy/fixture/v1/chat/completions", h.url);
    for auth in ["", "Bearer wrong", "Basic synthetic-key"] {
        assert_eq!(
            client
                .post(&route)
                .header("authorization", auth)
                .json(&request())
                .send()
                .await
                .unwrap()
                .status(),
            401
        );
    }
    assert_eq!(
        h.request()
            .header("origin", "https://example.invalid")
            .json(&request())
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        client
            .request(reqwest::Method::OPTIONS, &route)
            .send()
            .await
            .unwrap()
            .status(),
        405
    );
    let mut req = request();
    req["model"] = json!("unapplied");
    assert_eq!(h.request().json(&req).send().await.unwrap().status(), 404);
    req = request();
    req["messages"][0]["content"] = json!([{"type":"input_audio","data":"unsupported"}]);
    assert_eq!(h.request().json(&req).send().await.unwrap().status(), 400);
    let oversize = "x".repeat(handler::MAX_BODY + 1);
    assert_eq!(
        h.request().body(oversize).send().await.unwrap().status(),
        413
    );
    assert!(h.mock.observed.lock().unwrap().is_empty());
}

#[tokio::test]
async fn redirects_errors_and_oversized_responses_are_bounded_and_redacted() {
    let h = Harness::new(response()).await;
    for (status, body) in [
        (StatusCode::FOUND, "synthetic-secret upstream body".into()),
        (
            StatusCode::UNAUTHORIZED,
            "synthetic-secret upstream body".into(),
        ),
        (StatusCode::OK, "synthetic-secret malformed response".into()),
        (StatusCode::OK, "x".repeat(handler::MAX_BODY + 1)),
    ] {
        *h.mock.response.lock().unwrap() = (status, body);
        let result = h.request().json(&request()).send().await.unwrap();
        assert_eq!(result.status(), 502);
        let body = result.text().await.unwrap();
        assert!(!body.contains("synthetic-secret"));
        assert!(!body.contains(&h.native_url));
    }
    assert_eq!(h.mock.observed.lock().unwrap().len(), 4);
}

#[test]
fn endpoint_selection_preserves_explicit_prefix_and_defaults_only_host() {
    for (base, model, expected) in [
        (
            "https://example.invalid",
            "g",
            "https://example.invalid/v1beta/models/g:generateContent",
        ),
        (
            "https://example.invalid/v1/",
            "models/g",
            "https://example.invalid/v1/models/g:generateContent",
        ),
        (
            "https://example.invalid/prefix/v1beta",
            "g",
            "https://example.invalid/prefix/v1beta/models/g:generateContent",
        ),
    ] {
        assert_eq!(
            handler::endpoint(base, model).ok().unwrap().as_str(),
            expected
        );
    }
    for model in ["../bad", "g?key=secret", "a/b", "models/", ".."] {
        assert!(handler::endpoint("https://example.invalid", model).is_err());
    }
    assert!(handler::endpoint("https://example.invalid?key=secret", "g").is_err());
}

#[test]
fn conversion_rejects_unsupported_data_and_maps_finish_reasons() {
    for (field, value) in [
        ("logprobs", json!(true)),
        ("n", json!(2)),
        ("parallel_tool_calls", json!(false)),
    ] {
        let mut req = request();
        req[field] = value;
        assert!(openai_to_gemini_request(&req).is_err());
    }
    let mut req = request();
    req["messages"][0]["content"] =
        json!([{"type":"image_url","image_url":{"url":"https://example.invalid/private.png"}}]);
    assert!(openai_to_gemini_request(&req).is_err());
    req["messages"] = json!([{"role":"tool","tool_call_id":"unknown","content":"x"}]);
    assert!(openai_to_gemini_request(&req).is_err());
    for (native, expected) in [
        ("STOP", "stop"),
        ("MAX_TOKENS", "length"),
        ("SAFETY", "content_filter"),
    ] {
        let mut resp = response();
        resp["candidates"][0]["finishReason"] = json!(native);
        assert_eq!(
            gemini_to_openai_response(&resp, "g").unwrap()["choices"][0]["finish_reason"],
            expected
        );
    }
    assert_eq!(
        gemini_to_openai_response(&json!({"promptFeedback":{"blockReason":"SAFETY"}}), "g")
            .unwrap()["choices"][0]["finish_reason"],
        "content_filter"
    );
    assert!(gemini_to_openai_response(
        &json!({"id":"interaction","outputText":"wrong protocol"}),
        "g"
    )
    .is_err());
}

#[test]
fn committed_applied_registry_ignores_drafts_refreshes_removals_and_overrides() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let mut p = crate::provider::tests::sample();
    p.credential.mode = CredentialMode::Direct;
    p.credential.value = "synthetic-key".into();
    let mut model = ModelEntry::new(
        p.id,
        ApiModelId::try_from("gemini-fixture".to_owned()).unwrap(),
        "Fixture".into(),
    );
    model.protocol_override = Some(Protocol::GoogleGemini);
    let w = store
        .edit(0, |w| {
            w.catalog.providers.push(p.clone());
            w.catalog.models.push(model.clone());
            Ok(())
        })
        .unwrap();
    assert!(store.proxy_registry.get(&p.id.to_string()).is_none());
    let w = store
        .edit(w.revision, |w| {
            w.applied_catalog = w.catalog.clone();
            Ok(())
        })
        .unwrap();
    assert!(store
        .proxy_registry
        .get(&p.id.to_string())
        .unwrap()
        .models
        .contains("gemini-fixture"));
    let w = store
        .edit(w.revision, |w| {
            w.catalog.providers[0].credential.value = "draft-key".into();
            Ok(())
        })
        .unwrap();
    assert_eq!(
        store.proxy_registry.get(&p.id.to_string()).unwrap().api_key,
        "synthetic-key"
    );
    let w = store
        .edit(w.revision, |w| {
            w.applied_catalog.providers[0].protocol = Protocol::GoogleGemini;
            w.applied_catalog.models[0].protocol_override = Some(Protocol::OpenAiChatCompletions);
            Ok(())
        })
        .unwrap();
    assert!(store.proxy_registry.get(&p.id.to_string()).is_none());
    let w = store
        .edit(w.revision, |w| {
            w.applied_catalog = w.catalog.clone();
            w.applied_catalog.models[0].enabled = false;
            Ok(())
        })
        .unwrap();
    assert!(store.proxy_registry.get(&p.id.to_string()).is_none());
    store
        .edit(w.revision, |w| {
            w.applied_catalog = Default::default();
            Ok(())
        })
        .unwrap();
    assert!(store.proxy_registry.get(&p.id.to_string()).is_none());
}

#[test]
fn environment_credentials_resolve_without_literal_variable_fallback() {
    // Unique synthetic variable, never read or overwrite a user's variable.
    let name = format!("DROID_PROXY_TEST_{}", uuid::Uuid::new_v4().simple());
    std::env::set_var(&name, "synthetic-env-key");
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let mut p = crate::provider::tests::sample();
    p.protocol = Protocol::GoogleGemini;
    p.credential.value = name.clone();
    let model = ModelEntry::new(
        p.id,
        ApiModelId::try_from("g".to_owned()).unwrap(),
        "Fixture".into(),
    );
    let w = store
        .edit(0, |w| {
            w.applied_catalog.providers.push(p.clone());
            w.applied_catalog.models.push(model);
            Ok(())
        })
        .unwrap();
    assert_eq!(
        store.proxy_registry.get(&p.id.to_string()).unwrap().api_key,
        "synthetic-env-key"
    );
    let reopened = Store::open(dir.path()).unwrap();
    assert_eq!(
        reopened
            .proxy_registry
            .get(&p.id.to_string())
            .unwrap()
            .api_key,
        "synthetic-env-key"
    );
    std::env::remove_var(&name);
    assert!(store.proxy_registry.refresh(&w).is_err());
    assert!(store.proxy_registry.get(&p.id.to_string()).is_none());
}

#[test]
fn adapter_maps_only_effective_gemini_to_chat_proxy() {
    let mut p = crate::provider::tests::sample();
    let mut m = ModelEntry::new(
        p.id,
        ApiModelId::try_from("g".to_owned()).unwrap(),
        "Fixture".into(),
    );
    m.protocol_override = Some(Protocol::GoogleGemini);
    p.extra_headers
        .insert("x-fixture".into(), "upstream-only".into());
    let output = crate::adapter::custom_model(&m, &p, "0.238.0").unwrap();
    assert_eq!(output["provider"], "generic-chat-completion-api");
    assert_eq!(
        output["baseUrl"],
        format!("http://127.0.0.1:18318/proxy/{}/v1", p.id)
    );
    assert_eq!(output["apiKey"], "${PROVIDER_KEY}");
    assert!(output.get("extraHeaders").is_none());
    p.protocol = Protocol::GoogleGemini;
    for (protocol, expected) in [
        (Protocol::OpenAiResponses, "openai"),
        (
            Protocol::OpenAiChatCompletions,
            "generic-chat-completion-api",
        ),
        (Protocol::AnthropicMessages, "anthropic"),
    ] {
        m.protocol_override = Some(protocol);
        let output = crate::adapter::custom_model(&m, &p, "0.238.0").unwrap();
        assert_eq!(output["provider"], expected);
        assert_eq!(output["baseUrl"], p.base_url);
        assert_eq!(output["extraHeaders"]["x-fixture"], "upstream-only");
    }
}

#[test]
fn occupied_port_is_nonfatal_without_runtime_and_never_reused() {
    let owner = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let registry = ProxyRegistry::new();
    server::start_on_port(registry.clone(), owner.local_addr().unwrap().port());
    assert!(!registry.is_running());
    assert!(owner.local_addr().unwrap().ip().is_loopback());
}

#[tokio::test]
async fn upstream_timeout_is_bounded_and_sanitized() {
    let (native_url, native_task) = serve(Router::new().fallback(|| async {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        "never returned"
    }))
    .await;
    let registry = ProxyRegistry::new();
    registry.register(
        "fixture".into(),
        ProviderConfig {
            base_url: native_url,
            api_key: "synthetic-key".into(),
            models: ["gemini-fixture".into()].into(),
            extra_headers: Default::default(),
        },
    );
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_millis(25))
        .build()
        .unwrap();
    let (url, proxy_task) = serve(
        Router::new()
            .route(
                "/proxy/:provider_id/v1/chat/completions",
                axum::routing::post(handler::proxy_handler),
            )
            .with_state(handler::AppState { registry, client }),
    )
    .await;
    let result = reqwest::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .post(format!("{url}/proxy/fixture/v1/chat/completions"))
        .bearer_auth("synthetic-key")
        .json(&request())
        .send()
        .await
        .unwrap();
    assert_eq!(result.status(), 502);
    assert_eq!(
        result.json::<Value>().await.unwrap()["error"]["message"],
        "Gemini transport failed"
    );
    native_task.abort();
    proxy_task.abort();
}
