pub mod connection;
pub mod fetch;
pub mod google;
use crate::model::{
    entry::{ModelEntry, Protocol},
    ids::InternalId,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConnection {
    pub id: InternalId,
    pub name: String,
    pub base_url: String,
    pub protocol: Protocol,
    pub credential: Credential,
    pub model_list_url: Option<String>,
    #[serde(default)]
    pub extra_headers: BTreeMap<String, String>,
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Credential {
    pub mode: CredentialMode,
    pub value: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialMode {
    Environment,
    Direct,
}
impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credential")
            .field("mode", &self.mode)
            .field("value", &"[REDACTED]")
            .finish()
    }
}
impl Credential {
    pub fn validate(&self) -> Result<(), String> {
        if self.value.is_empty() {
            return Err("凭据不能为空".into());
        }
        if self.mode == CredentialMode::Environment && !valid_env_name(&self.value) {
            return Err("环境变量名只能包含字母、数字和下划线，且不能以数字开头".into());
        }
        Ok(())
    }
    pub fn settings_value(&self) -> String {
        match self.mode {
            CredentialMode::Environment => format!("${{{}}}", self.value),
            CredentialMode::Direct => self.value.clone(),
        }
    }
    pub fn resolve(&self) -> Result<String, String> {
        self.validate()?;
        match self.mode {
            CredentialMode::Direct => Ok(self.value.clone()),
            CredentialMode::Environment => std::env::var(&self.value)
                .ok()
                .filter(|v| !v.is_empty())
                .ok_or("当前进程未读取到该环境变量".into()),
        }
    }
}
pub fn valid_env_name(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}
pub fn validate_url(value: &str) -> Result<(), String> {
    let url = reqwest::Url::parse(value).map_err(|_| "接口地址无效")?;
    if !matches!(url.scheme(), "https" | "http")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err("接口地址需为不含用户名、密码或片段的 HTTP(S) URL".into());
    }
    Ok(())
}
impl ProviderConnection {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("连接名称不能为空".into());
        }
        validate_url(&self.base_url)?;
        if let Some(url) = &self.model_list_url {
            if !url.is_empty() {
                validate_url(url)?;
            }
        }
        self.credential.validate()?;
        for (name, value) in &self.extra_headers {
            reqwest::header::HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| "附加请求头名称无效")?;
            reqwest::header::HeaderValue::from_str(value).map_err(|_| "附加请求头值无效")?;
        }
        Ok(())
    }
}
#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub providers: Vec<ProviderConnection>,
    pub models: Vec<ModelEntry>,
}
impl Catalog {
    pub fn validate(&self) -> Result<(), String> {
        let mut ids = std::collections::HashSet::new();
        for provider in &self.providers {
            provider.validate()?;
            if !ids.insert(provider.id) {
                return Err("供应商内部 ID 重复".into());
            }
        }
        let mut model_ids = std::collections::HashSet::new();
        let mut identities = std::collections::HashSet::new();
        for model in &self.models {
            if !ids.contains(&model.provider_id) {
                return Err("模型引用的供应商不存在".into());
            }
            if model.display_name.trim().is_empty() {
                return Err("模型显示名不能为空".into());
            }
            if !model_ids.insert(model.id)
                || !identities.insert((model.provider_id, model.api_model_id.clone()))
            {
                return Err("模型记录重复".into());
            }
        }
        Ok(())
    }
    pub fn provider(&self, id: InternalId) -> Result<&ProviderConnection, String> {
        self.providers
            .iter()
            .find(|p| p.id == id)
            .ok_or("供应商不存在".into())
    }
}
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub fn sample() -> ProviderConnection {
        ProviderConnection {
            id: InternalId::new(),
            name: "Connection".into(),
            base_url: "https://example.com/custom/v1".into(),
            protocol: Protocol::OpenAiResponses,
            credential: Credential {
                mode: CredentialMode::Environment,
                value: "PROVIDER_KEY".into(),
            },
            model_list_url: None,
            extra_headers: Default::default(),
        }
    }
    #[test]
    fn provider_same_site_distinct_ids_and_credential_redaction() {
        let p = sample();
        let mut p2 = p.clone();
        p2.id = InternalId::new();
        let catalog = Catalog {
            providers: vec![p, p2],
            models: vec![],
        };
        assert!(catalog.validate().is_ok());
        let c = Credential {
            mode: CredentialMode::Direct,
            value: "secret-token".into(),
        };
        assert!(!format!("{c:?}").contains("secret-token"));
    }
    #[test]
    fn provider_validation_keeps_base_url_and_rejects_invalid_env() {
        let p = sample();
        assert!(p.validate().is_ok());
        assert_eq!(p.base_url, "https://example.com/custom/v1");
        let bad = Credential {
            mode: CredentialMode::Environment,
            value: "A-B".into(),
        };
        assert!(bad.validate().is_err());
    }
}
