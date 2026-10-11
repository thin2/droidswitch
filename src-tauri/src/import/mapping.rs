use super::ccswitch::{self, SourceRecord};
use crate::{
    model::{
        entry::{ModelEntry, Protocol},
        ids::{ApiModelId, InternalId},
    },
    provider::{Credential, CredentialMode, ProviderConnection},
    workspace::{Store, Workspace},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Imported {
    pub provider_id: InternalId,
    pub hash: String,
}
struct Mapped {
    url: Option<String>,
    protocol: Option<Protocol>,
    key: Option<String>,
    models: Vec<String>,
    issue: Option<String>,
}
fn str_at(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}
fn map(r: &SourceRecord) -> Mapped {
    let mut out = Mapped {
        url: None,
        protocol: None,
        key: None,
        models: vec![],
        issue: None,
    };
    match r.app.as_str() {
        "claude" => {
            let env = &r.settings["env"];
            out.url = str_at(env, "ANTHROPIC_BASE_URL");
            out.key =
                str_at(env, "ANTHROPIC_AUTH_TOKEN").or_else(|| str_at(env, "ANTHROPIC_API_KEY"));
            out.protocol = Some(Protocol::AnthropicMessages);
            for key in [
                "ANTHROPIC_MODEL",
                "ANTHROPIC_DEFAULT_OPUS_MODEL",
                "ANTHROPIC_DEFAULT_SONNET_MODEL",
                "ANTHROPIC_DEFAULT_HAIKU_MODEL",
            ] {
                if let Some(model) = str_at(env, key) {
                    if !out.models.contains(&model) {
                        out.models.push(model);
                    }
                }
            }
        }
        "codex" => {
            out.key = r
                .settings
                .get("auth")
                .and_then(|v| str_at(v, "OPENAI_API_KEY"));
            if let Some(config) = r.settings.get("config").and_then(Value::as_str) {
                match config.parse::<toml::Value>() {
                    Ok(config) => {
                        if let Some(m) = config.get("model").and_then(toml::Value::as_str) {
                            out.models.push(m.into());
                        }
                        let name = config.get("model_provider").and_then(toml::Value::as_str);
                        if let Some(provider) =
                            name.and_then(|n| config.get("model_providers").and_then(|v| v.get(n)))
                        {
                            out.url = provider
                                .get("base_url")
                                .and_then(toml::Value::as_str)
                                .map(str::to_owned);
                            out.protocol =
                                match provider.get("wire_api").and_then(toml::Value::as_str) {
                                    Some("responses") => Some(Protocol::OpenAiResponses),
                                    Some("chat") => Some(Protocol::OpenAiChatCompletions),
                                    _ => None,
                                };
                        }
                    }
                    Err(_) => out.issue = Some("Codex TOML 无效，请修复源配置后重试".into()),
                }
            }
        }
        _ => out.issue = Some("此来源应用不支持直接导入".into()),
    }
    if out.issue.is_none() && (out.url.is_none() || out.protocol.is_none()) {
        out.issue =
            Some("未找到可直接使用的 API 接口，请补充 Droid 兼容接口；订阅登录凭据不会导入".into());
    }
    out
}
fn source_key(r: &SourceRecord) -> String {
    crate::config::pipeline::hash(format!("{}\0{}\0{}", r.location, r.app, r.id).as_bytes())
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub key: String,
    pub hash: String,
    pub app: String,
    pub name: String,
    pub base_url: Option<String>,
    pub protocol: Option<Protocol>,
    pub models: Vec<String>,
    pub has_credential: bool,
    pub issue: Option<String>,
    pub duplicate: String,
}
#[tauri::command]
pub async fn preview_ccswitch(
    store: tauri::State<'_, Store>,
    path: String,
) -> Result<Vec<Candidate>, String> {
    let w = store.load()?;
    Ok(ccswitch::read(&path)?
        .iter()
        .map(|r| {
            let m = map(r);
            let key = source_key(r);
            let duplicate = match w
                .imports
                .get(&key)
                .filter(|old| w.catalog.providers.iter().any(|p| p.id == old.provider_id))
            {
                Some(old) if old.hash == r.hash => "same",
                Some(_) => "changed",
                None => "new",
            };
            Candidate {
                key,
                hash: r.hash.clone(),
                app: r.app.clone(),
                name: r.name.clone(),
                base_url: m.url.map(|u| {
                    crate::config::redact::redact(&Value::String(u))
                        .as_str()
                        .unwrap_or("[已脱敏]")
                        .into()
                }),
                protocol: m.protocol,
                models: m.models,
                has_credential: m.key.is_some(),
                issue: m.issue,
                duplicate: duplicate.into(),
            }
        })
        .collect())
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    pub key: String,
    pub hash: String,
    pub action: String,
    pub credential_mode: CredentialMode,
    pub environment: Option<String>,
    pub base_url: Option<String>,
    pub protocol: Option<Protocol>,
}
pub fn import(
    w: &mut Workspace,
    records: &[SourceRecord],
    choices: Vec<Choice>,
    allow_plaintext: bool,
) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    for choice in choices {
        if !seen.insert(choice.key.clone()) {
            return Err("导入选择重复".into());
        }
        if choice.action == "skip" {
            continue;
        }
        if !["add", "update", "copy"].contains(&choice.action.as_str()) {
            return Err("导入策略无效".into());
        }
        let r = records
            .iter()
            .find(|r| source_key(r) == choice.key)
            .ok_or("所选源记录已不存在，请刷新")?;
        if choice.hash != r.hash {
            return Err("源记录在预览后改变，请重新检查".into());
        }
        if !["claude", "codex"].contains(&r.app.as_str()) {
            return Err("不支持此来源应用".into());
        }
        let m = map(r);
        let old = w
            .imports
            .get(&choice.key)
            .filter(|old| w.catalog.providers.iter().any(|p| p.id == old.provider_id));
        if let Some(old) = old {
            if old.hash == r.hash && choice.action != "copy" {
                continue;
            }
            if choice.action == "add" {
                return Err("已有导入记录，请选择更新或另存".into());
            }
        }
        let id = if choice.action == "update" {
            old.map(|o| o.provider_id).ok_or("找不到待更新的目标记录")?
        } else {
            InternalId::new()
        };
        let credential = match choice.credential_mode {
            CredentialMode::Environment => Credential {
                mode: CredentialMode::Environment,
                value: choice
                    .environment
                    .filter(|s| !s.is_empty())
                    .ok_or("请填写要引用的环境变量名")?,
            },
            CredentialMode::Direct => {
                if !allow_plaintext {
                    return Err("明文导入需要显式确认".into());
                }
                Credential {
                    mode: CredentialMode::Direct,
                    value: m.key.ok_or("源记录没有可导入的 API Key，请改用环境变量")?,
                }
            }
        };
        let p = ProviderConnection {
            id,
            name: r.name.clone(),
            base_url: choice
                .base_url
                .filter(|s| !s.is_empty())
                .or(m.url)
                .ok_or("请补充兼容 API 接口")?,
            protocol: choice
                .protocol
                .or(m.protocol)
                .ok_or("请选择兼容 API 协议")?,
            credential,
            model_list_url: None,
            extra_headers: Default::default(),
        };
        p.validate()?;
        if let Some(existing) = w.catalog.providers.iter_mut().find(|p| p.id == id) {
            *existing = p;
        } else {
            w.catalog.providers.push(p);
        }
        for api in m.models {
            let api = ApiModelId::try_from(api).map_err(|_| "源模型 ID 无效")?;
            if !w
                .catalog
                .models
                .iter()
                .any(|m| m.provider_id == id && m.api_model_id == api)
            {
                let name = api.as_str().to_owned();
                w.catalog.models.push(ModelEntry::new(id, api, name));
            }
        }
        w.imports.insert(
            choice.key,
            Imported {
                provider_id: id,
                hash: r.hash.clone(),
            },
        );
    }
    Ok(())
}
#[tauri::command]
pub async fn import_ccswitch(
    store: tauri::State<'_, Store>,
    path: String,
    choices: Vec<Choice>,
    allow_plaintext: bool,
    revision: u64,
) -> Result<Workspace, String> {
    let records = ccswitch::read(&path)?;
    store.edit(revision, |w| import(w, &records, choices, allow_plaintext))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn import_edge_empty_model_list_and_repeat_do_not_inflate() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(d.path()).unwrap();
        let r = SourceRecord {
            id: "a".into(),
            app: "claude".into(),
            name: "Name".into(),
            settings: serde_json::json!({"env":{"ANTHROPIC_BASE_URL":"https://example.com","ANTHROPIC_AUTH_TOKEN":"private"}}),
            hash: "hash".into(),
            location: "source".into(),
        };
        let choice = || Choice {
            key: source_key(&r),
            hash: "hash".into(),
            action: "add".into(),
            credential_mode: CredentialMode::Environment,
            environment: Some("IMPORTED_KEY".into()),
            base_url: None,
            protocol: None,
        };
        s.edit(0, |w| {
            import(w, std::slice::from_ref(&r), vec![choice()], false)?;
            import(w, std::slice::from_ref(&r), vec![choice()], false)?;
            assert_eq!(w.catalog.providers.len(), 1);
            assert!(w.catalog.models.is_empty());
            assert_eq!(w.catalog.providers[0].credential.value, "IMPORTED_KEY");
            Ok(())
        })
        .unwrap();
        let db = s.db.lock().unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM usage_events", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
}
