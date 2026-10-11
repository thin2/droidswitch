use serde_json::Value;
fn sensitive(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.contains("key")
        || key.contains("token") && key != "maxoutputtokens"
        || key.contains("secret")
        || key.contains("password")
        || key.contains("credential")
        || key.contains("authorization")
        || key == "extraheaders"
}
fn secrets(value: &Value, hidden: bool, out: &mut Vec<String>) {
    match value {
        Value::Object(o) => {
            for (k, v) in o {
                secrets(v, hidden || sensitive(k), out);
            }
        }
        Value::Array(a) => {
            for v in a {
                secrets(v, hidden, out);
            }
        }
        Value::String(s) if hidden && !s.is_empty() => out.push(s.clone()),
        _ => {}
    }
}
fn scrub(value: Value, known: &[String]) -> Value {
    match value {
        Value::String(mut s) => {
            for secret in known {
                s = s.replace(secret, "[已脱敏]");
            }
            Value::String(s)
        }
        Value::Array(a) => Value::Array(a.into_iter().map(|v| scrub(v, known)).collect()),
        Value::Object(o) => Value::Object(
            o.into_iter()
                .map(|(mut k, v)| {
                    for secret in known {
                        k = k.replace(secret, "[已脱敏]");
                    }
                    (k, scrub(v, known))
                })
                .collect(),
        ),
        _ => value,
    }
}
pub fn pair(before: &Value, after: &Value) -> (Value, Value, bool) {
    let mut a = vec![];
    let mut b = vec![];
    secrets(before, false, &mut a);
    secrets(after, false, &mut b);
    let changed = a != b;
    a.extend(b);
    a.sort_by_key(|s| std::cmp::Reverse(s.len()));
    a.dedup();
    (
        scrub(redact_fields(before), &a),
        scrub(redact_fields(after), &a),
        changed,
    )
}
pub fn redact(value: &Value) -> Value {
    pair(value, value).0
}

fn redact_fields(value: &Value) -> Value {
    match value {
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, v)| {
                    let key = k.to_ascii_lowercase();
                    let hidden = key.contains("key")
                        || key.contains("token") && key != "maxoutputtokens"
                        || key.contains("secret")
                        || key.contains("password")
                        || key.contains("credential")
                        || key.contains("authorization")
                        || key == "extraheaders";
                    (
                        k.clone(),
                        if hidden {
                            Value::String("[已脱敏]".into())
                        } else {
                            redact_fields(v)
                        },
                    )
                })
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(redact_fields).collect()),
        Value::String(s) => {
            if let Ok(mut u) = reqwest::Url::parse(s) {
                if matches!(u.scheme(), "http" | "https")
                    && (u.query().is_some() || !u.username().is_empty() || u.password().is_some())
                {
                    let _ = u.set_username("");
                    let _ = u.set_password(None);
                    u.set_query(Some("redacted"));
                    return Value::String(u.to_string());
                }
            }
            Value::String(s.clone())
        }
        _ => value.clone(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn redact_nested_secrets_and_query() {
        let v = redact(
            &json!({"customModels":[{"apiKey":"secret","extraHeaders":{"X-Test":"private"},"maxOutputTokens":32,"baseUrl":"https://a.test/v1?key=private"}],"nested":{"access_token":"hidden"}}),
        );
        let s = v.to_string();
        for secret in ["secret", "private", "hidden"] {
            assert!(!s.contains(secret));
        }
        assert_eq!(v["customModels"][0]["maxOutputTokens"], 32);
    }
}
#[cfg(test)]
mod preview_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn preview_credential_changes_redact_echoed_values_in_both_sides() {
        let (a, b, changed) = pair(
            &json!({"apiKey":"old-secret","note":"echo new-secret"}),
            &json!({"apiKey":"new-secret","note":"echo old-secret","name":"changed"}),
        );
        assert!(changed);
        for value in [a, b] {
            let s = value.to_string();
            assert!(!s.contains("old-secret"));
            assert!(!s.contains("new-secret"));
        }
    }
}
