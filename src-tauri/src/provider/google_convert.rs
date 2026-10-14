//! Native generateContent bridge. Interactions (connection testing) is a separate API.
use serde_json::{json, Value};
use std::collections::HashMap;

fn object_keys(value: &Value, allowed: &[&str]) -> Result<(), String> {
    let obj = value.as_object().ok_or("Expected an object")?;
    if obj.keys().any(|k| !allowed.contains(&k.as_str())) {
        return Err("Unsupported request field".into());
    }
    Ok(())
}
fn text_parts(content: &Value, images: bool) -> Result<Vec<Value>, String> {
    if content.is_null() {
        return Ok(vec![]);
    }
    if let Some(s) = content.as_str() {
        return Ok(vec![json!({"text":s})]);
    }
    let mut parts = vec![];
    for part in content.as_array().ok_or("Unsupported message content")? {
        match part["type"].as_str() {
            Some("text") => {
                object_keys(part, &["type", "text"])?;
                parts.push(json!({"text":part["text"].as_str().ok_or("Invalid text")?}));
            }
            Some("image_url") if images => {
                object_keys(part, &["type", "image_url"])?;
                object_keys(&part["image_url"], &["url", "detail"])?;
                if part["image_url"]["detail"]
                    .as_str()
                    .is_some_and(|d| d != "auto")
                {
                    return Err("Image detail selection is unsupported".into());
                }
                let url = part["image_url"]["url"].as_str().ok_or("Invalid image")?;
                let (mime, data) = url
                    .strip_prefix("data:")
                    .and_then(|s| s.split_once(";base64,"))
                    .ok_or(
                        "Only inline base64 images are supported; remote URLs are not fetched",
                    )?;
                if ![
                    "image/png",
                    "image/jpeg",
                    "image/webp",
                    "image/heic",
                    "image/heif",
                ]
                .contains(&mime)
                    || data.is_empty()
                    || data.len() % 4 != 0
                    || !data
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b))
                {
                    return Err("Invalid inline image".into());
                }
                parts.push(json!({"inlineData":{"mimeType":mime,"data":data}}));
            }
            _ => return Err("Unsupported content part".into()),
        }
    }
    Ok(parts)
}
// Encode opaque thought signatures into call IDs, so clients that discard extension
// fields still return them with tool history. JSON serialization is reversible.
fn call_id(name: &str, signature: Option<&str>, native_id: Option<&str>) -> String {
    let bytes = serde_json::to_vec(&json!([
        name,
        signature,
        native_id,
        uuid::Uuid::new_v4().to_string()
    ]))
    .unwrap();
    format!(
        "call_g_{}",
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )
}
fn call_metadata(id: &str) -> Option<Value> {
    let s = id.strip_prefix("call_g_")?;
    if s.len() % 2 != 0 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect();
    serde_json::from_slice(&bytes?).ok()
}

pub fn openai_to_gemini_request(req: &Value) -> Result<Value, String> {
    object_keys(
        req,
        &[
            "model",
            "messages",
            "stream",
            "stream_options",
            "temperature",
            "top_p",
            "max_tokens",
            "max_completion_tokens",
            "stop",
            "tools",
            "tool_choice",
            "parallel_tool_calls",
            "n",
            "response_format",
            "reasoning_effort",
        ],
    )?;
    if req.get("stream").is_some_and(|v| !v.is_boolean()) {
        return Err("Invalid stream flag".into());
    }
    if let Some(options) = req.get("stream_options").filter(|v| !v.is_null()) {
        object_keys(options, &["include_usage"])?;
        if options
            .get("include_usage")
            .is_some_and(|v| !v.is_boolean())
        {
            return Err("Invalid stream options".into());
        }
    }
    if req.get("n").is_some_and(|v| v != 1) {
        return Err("Only n=1 is supported".into());
    }
    if req.get("parallel_tool_calls").is_some_and(|v| v != true) {
        return Err("Disabling parallel tools is unsupported".into());
    }
    let messages = req["messages"]
        .as_array()
        .filter(|a| !a.is_empty())
        .ok_or("Messages must not be empty")?;
    let mut contents: Vec<Value> = vec![];
    let mut system = vec![];
    let mut pending: HashMap<String, (String, Option<String>)> = HashMap::new();
    for msg in messages {
        object_keys(msg, &["role", "content", "tool_calls", "tool_call_id"])?;
        let role = msg["role"].as_str().ok_or("Missing role")?;
        let mut parts = text_parts(&msg["content"], role == "user")?;
        match role {
            "system" | "developer" => {
                if !contents.is_empty() || msg.get("tool_calls").is_some() {
                    return Err("System instructions must precede conversation".into());
                }
                system.extend(parts);
                continue;
            }
            "assistant" => {
                if !pending.is_empty() {
                    return Err("Missing tool results".into());
                }
                if let Some(calls) = msg.get("tool_calls").filter(|v| !v.is_null()) {
                    for call in calls.as_array().ok_or("Invalid tool calls")? {
                        object_keys(call, &["id", "type", "function", "extra_content"])?;
                        if call["type"] != "function" {
                            return Err("Unsupported tool call".into());
                        }
                        object_keys(&call["function"], &["name", "arguments"])?;
                        let id = call["id"].as_str().ok_or("Missing tool ID")?;
                        let name = call["function"]["name"]
                            .as_str()
                            .ok_or("Missing function name")?;
                        let args: Value = serde_json::from_str(
                            call["function"]["arguments"]
                                .as_str()
                                .ok_or("Invalid arguments")?,
                        )
                        .map_err(|_| "Invalid function arguments JSON")?;
                        if !args.is_object() {
                            return Err("Function arguments must be an object".into());
                        }
                        let meta = call_metadata(id);
                        if meta.as_ref().is_some_and(|m| m[0] != name) {
                            return Err("Tool signature/name mismatch".into());
                        }
                        let signature = meta.as_ref().and_then(|m| m[1].as_str()).or_else(|| {
                            call["extra_content"]["google"]["thought_signature"].as_str()
                        });
                        let native_id =
                            meta.as_ref().and_then(|m| m[2].as_str()).map(str::to_owned);
                        let mut part = json!({"functionCall":{"name":name,"args":args}});
                        if let Some(s) = signature {
                            part["thoughtSignature"] = json!(s);
                        }
                        if let Some(s) = &native_id {
                            part["functionCall"]["id"] = json!(s);
                        }
                        if pending
                            .insert(id.into(), (name.into(), native_id))
                            .is_some()
                        {
                            return Err("Duplicate tool ID".into());
                        }
                        parts.push(part);
                    }
                }
            }
            "tool" => {
                if msg.get("tool_calls").is_some() {
                    return Err("Invalid tool message".into());
                }
                let id = msg["tool_call_id"]
                    .as_str()
                    .ok_or("Missing tool result ID")?;
                let (name, native_id) = pending
                    .remove(id)
                    .ok_or("Unknown or duplicate tool result ID")?;
                let text = parts
                    .iter()
                    .map(|p| p["text"].as_str().unwrap_or(""))
                    .collect::<Vec<_>>()
                    .join("\n");
                let output = serde_json::from_str::<Value>(&text).unwrap_or(json!(text));
                let mut part =
                    json!({"functionResponse":{"name":name,"response":{"output":output}}});
                if let Some(id) = native_id {
                    part["functionResponse"]["id"] = json!(id);
                }
                parts = vec![part];
            }
            "user" => {
                if !pending.is_empty() || msg.get("tool_calls").is_some() {
                    return Err("Missing tool results or invalid user tools".into());
                }
            }
            _ => return Err("Unsupported message role".into()),
        }
        if role != "tool" && msg.get("tool_call_id").is_some() {
            return Err("Unexpected tool result ID".into());
        }
        if parts.is_empty() {
            return Err("Empty message".into());
        }
        let native_role = if role == "assistant" { "model" } else { "user" };
        if let Some(last) = contents.last_mut().filter(|c| c["role"] == native_role) {
            last["parts"].as_array_mut().unwrap().extend(parts);
        } else {
            contents.push(json!({"role":native_role,"parts":parts}));
        }
    }
    if !pending.is_empty() || contents.is_empty() {
        return Err("Missing conversation or tool results".into());
    }
    let mut out = json!({"contents":contents});
    if !system.is_empty() {
        out["systemInstruction"] = json!({"parts":system});
    }
    let mut config = serde_json::Map::new();
    for (from, to) in [
        ("temperature", "temperature"),
        ("top_p", "topP"),
        ("max_tokens", "maxOutputTokens"),
        ("max_completion_tokens", "maxOutputTokens"),
    ] {
        if let Some(v) = req.get(from).filter(|v| !v.is_null()) {
            if !v.is_number() || (from.starts_with("max_") && v.as_u64().is_none_or(|n| n == 0)) {
                return Err("Invalid generation setting".into());
            }
            if config.insert(to.into(), v.clone()).is_some() {
                return Err("Conflicting token limits".into());
            }
        }
    }
    // Droid adds this field for Gemini model IDs even without extraArgs.
    // Gemini 3 uses named thinking levels, not OpenAI's top-level field.
    if let Some(effort) = req.get("reasoning_effort").filter(|v| !v.is_null()) {
        let level = match effort.as_str() {
            Some("minimal") => "MINIMAL",
            Some("low") => "LOW",
            Some("medium") => "MEDIUM",
            Some("high") => "HIGH",
            _ => return Err("Unsupported Gemini reasoning effort".into()),
        };
        config.insert("thinkingConfig".into(), json!({"thinkingLevel":level}));
    }
    if let Some(stop) = req.get("stop").filter(|v| !v.is_null()) {
        let stops = if stop.is_string() {
            vec![stop.clone()]
        } else {
            stop.as_array().ok_or("Invalid stop sequences")?.clone()
        };
        if stops.len() > 5 || stops.iter().any(|s| !s.is_string()) {
            return Err("Invalid stop sequences".into());
        }
        config.insert("stopSequences".into(), json!(stops));
    }
    if let Some(format) = req.get("response_format") {
        object_keys(format, &["type"])?;
        match format["type"].as_str() {
            Some("json_object") => {
                config.insert("responseMimeType".into(), json!("application/json"));
            }
            Some("text") => (),
            _ => return Err("Unsupported response format".into()),
        }
    }
    if !config.is_empty() {
        out["generationConfig"] = json!(config);
    }
    let mut names = vec![];
    if let Some(tools) = req.get("tools").filter(|v| !v.is_null()) {
        let mut declarations = vec![];
        for tool in tools.as_array().ok_or("Invalid tools")? {
            object_keys(tool, &["type", "function"])?;
            if tool["type"] != "function" {
                return Err("Unsupported tool type".into());
            }
            let f = &tool["function"];
            object_keys(f, &["name", "description", "parameters", "strict"])?;
            if f.get("strict").is_some_and(|s| s != false && !s.is_null()) {
                return Err("Strict tools are unsupported".into());
            }
            let name = f["name"].as_str().ok_or("Missing tool name")?;
            if names.contains(&name) {
                return Err("Duplicate tool name".into());
            }
            names.push(name);
            let mut declaration = json!({"name":name});
            if let Some(d) = f.get("description") {
                declaration["description"] = d.clone();
            }
            if let Some(p) = f.get("parameters") {
                if !p.is_object() {
                    return Err("Invalid tool schema".into());
                }
                declaration["parametersJsonSchema"] = p.clone();
            }
            declarations.push(declaration);
        }
        if !declarations.is_empty() {
            out["tools"] = json!([{"functionDeclarations":declarations}]);
        }
    }
    if let Some(choice) = req.get("tool_choice") {
        let mode = match choice.as_str() {
            Some("auto") => "AUTO",
            Some("none") => "NONE",
            Some("required") if !names.is_empty() => "ANY",
            None if choice["type"] == "function" => {
                object_keys(choice, &["type", "function"])?;
                object_keys(&choice["function"], &["name"])?;
                let name = choice["function"]["name"]
                    .as_str()
                    .ok_or("Invalid tool choice")?;
                if !names.contains(&name) {
                    return Err("Unknown selected tool".into());
                }
                out["toolConfig"] =
                    json!({"functionCallingConfig":{"mode":"ANY","allowedFunctionNames":[name]}});
                ""
            }
            _ => return Err("Unsupported tool choice".into()),
        };
        if !mode.is_empty() {
            out["toolConfig"] = json!({"functionCallingConfig":{"mode":mode}});
        }
    }
    Ok(out)
}

pub fn gemini_to_openai_response(response: &Value, model: &str) -> Result<Value, String> {
    let candidate = response["candidates"].as_array().and_then(|a| a.first());
    let blocked = response["promptFeedback"]["blockReason"].as_str().is_some();
    let c = candidate.cloned().unwrap_or(Value::Null);
    if candidate.is_none() && !blocked {
        return Err("Missing Gemini candidate".into());
    }
    let mut text = String::new();
    let mut calls = vec![];
    for part in c["content"]["parts"].as_array().into_iter().flatten() {
        if part["thought"] == true {
            continue;
        } // Never expose private reasoning.
        if let Some(t) = part["text"].as_str() {
            text.push_str(t);
        } else if let Some(call) = part.get("functionCall") {
            let name = call["name"].as_str().ok_or("Invalid native tool call")?;
            let args = call.get("args").cloned().unwrap_or(json!({}));
            if !args.is_object() {
                return Err("Invalid native tool arguments".into());
            }
            let signature = part["thoughtSignature"].as_str();
            calls.push(json!({"id":call_id(name,signature,call["id"].as_str()),"type":"function","function":{"name":name,"arguments":args.to_string()}}));
        } else {
            return Err("Unsupported Gemini output part".into());
        }
    }
    let reason = if blocked {
        "content_filter"
    } else {
        match c["finishReason"].as_str() {
            Some("STOP") if !calls.is_empty() => "tool_calls",
            Some("STOP") => "stop",
            Some("MAX_TOKENS") => "length",
            Some(
                "SAFETY" | "RECITATION" | "BLOCKLIST" | "PROHIBITED_CONTENT" | "SPII"
                | "IMAGE_SAFETY",
            ) => "content_filter",
            _ => return Err("Unsupported or missing Gemini finish reason".into()),
        }
    };
    let mut message = json!({"role":"assistant","content":if text.is_empty() { Value::Null } else { json!(text) }});
    if !calls.is_empty() {
        message["tool_calls"] = json!(calls);
    }
    let u = &response["usageMetadata"];
    let prompt = u["promptTokenCount"].as_u64().unwrap_or(0);
    let thoughts = u["thoughtsTokenCount"].as_u64().unwrap_or(0);
    let completion = u["candidatesTokenCount"]
        .as_u64()
        .unwrap_or(0)
        .checked_add(thoughts)
        .ok_or("Invalid usage counts")?;
    let total = u["totalTokenCount"]
        .as_u64()
        .map(Ok)
        .unwrap_or_else(|| prompt.checked_add(completion).ok_or("Invalid usage counts"))?;
    Ok(
        json!({"id":format!("chatcmpl-{}",uuid::Uuid::new_v4()),"object":"chat.completion","created":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),"model":model,
        "choices":[{"index":0,"message":message,"finish_reason":reason}],
        "usage":{"prompt_tokens":prompt,"completion_tokens":completion,"total_tokens":total,"prompt_tokens_details":{"cached_tokens":u["cachedContentTokenCount"].as_u64().unwrap_or(0)},"completion_tokens_details":{"reasoning_tokens":thoughts}}}),
    )
}

/// Buffered SSE, not token-by-token upstream streaming.
pub fn to_sse(response: &Value, include_usage: bool) -> String {
    let mut result = String::new();
    let mut emit = |delta: Value, finish: Value, usage: bool| {
        let mut chunk = json!({"id":response["id"],"object":"chat.completion.chunk","created":response["created"],"model":response["model"],"choices":if usage { json!([]) } else { json!([{"index":0,"delta":delta,"finish_reason":finish}]) }});
        if usage {
            chunk["usage"] = response["usage"].clone();
        }
        result.push_str(&format!("data: {}\n\n", chunk));
    };
    emit(json!({"role":"assistant"}), Value::Null, false);
    let msg = &response["choices"][0]["message"];
    if msg["content"].is_string() {
        emit(json!({"content":msg["content"]}), Value::Null, false);
    }
    for (index, call) in msg["tool_calls"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let mut call = call.clone();
        call["index"] = json!(index);
        emit(json!({"tool_calls":[call]}), Value::Null, false);
    }
    emit(
        json!({}),
        response["choices"][0]["finish_reason"].clone(),
        false,
    );
    if include_usage {
        emit(json!({}), Value::Null, true);
    }
    result.push_str("data: [DONE]\n\n");
    result
}
