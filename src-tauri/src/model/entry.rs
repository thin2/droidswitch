use super::ids::{ApiModelId, DroidSelectorId, InternalId};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Protocol {
    #[serde(rename = "anthropic")]
    AnthropicMessages,
    #[serde(rename = "openai")]
    OpenAiResponses,
    #[serde(rename = "generic-chat-completion-api")]
    OpenAiChatCompletions,
    #[serde(rename = "google-gemini")]
    GoogleGemini,
}

/// Local management representation, never serialize directly into customModels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelEntry {
    pub id: InternalId,
    pub provider_id: InternalId,
    pub api_model_id: ApiModelId,
    pub droid_selector_id: Option<DroidSelectorId>,
    pub display_name: String,
    pub manufacturer: Option<String>,
    pub protocol_override: Option<Protocol>,
    pub enabled: bool,
    pub max_output_tokens: Option<NonZeroU32>,
    pub image_input: bool,
    #[serde(default)]
    pub extra_args: serde_json::Map<String, serde_json::Value>,
}

impl ModelEntry {
    pub fn new(provider_id: InternalId, api_model_id: ApiModelId, display_name: String) -> Self {
        Self {
            id: InternalId::new(),
            provider_id,
            api_model_id,
            droid_selector_id: None,
            display_name,
            manufacturer: None,
            protocol_override: None,
            enabled: true,
            max_output_tokens: None,
            image_input: false,
            extra_args: Default::default(),
        }
    }
    pub fn effective_protocol(&self, provider_protocol: Protocol) -> Protocol {
        self.protocol_override.unwrap_or(provider_protocol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_identity_survives_rename_and_same_remote_id_across_providers() {
        let api = ApiModelId::try_from("shared-model".to_owned()).unwrap();
        let mut first = ModelEntry::new(InternalId::new(), api.clone(), "First".into());
        let second = ModelEntry::new(InternalId::new(), api, "First".into());
        let original_id = first.id;
        first.display_name = "Renamed".into();
        assert_eq!(first.id, original_id);
        assert_ne!(first.id, second.id);
        assert_ne!(first.provider_id, second.provider_id);
        assert_eq!(first.api_model_id, second.api_model_id);
        assert!(first.droid_selector_id.is_none());
    }
    #[test]
    fn model_token_limit_is_optional_positive_and_protocol_is_explicit() {
        let mut entry = ModelEntry::new(
            InternalId::new(),
            ApiModelId::try_from("claude-name-does-not-decide-protocol".to_owned()).unwrap(),
            "Name".into(),
        );
        assert_eq!(
            entry.effective_protocol(Protocol::OpenAiResponses),
            Protocol::OpenAiResponses
        );
        entry.protocol_override = Some(Protocol::AnthropicMessages);
        assert_eq!(
            entry.effective_protocol(Protocol::OpenAiResponses),
            Protocol::AnthropicMessages
        );
        let mut json = serde_json::to_value(&entry).unwrap();
        assert!(serde_json::from_value::<ModelEntry>(json.clone())
            .unwrap()
            .max_output_tokens
            .is_none());
        for invalid in [0, -1] {
            json["maxOutputTokens"] = invalid.into();
            assert!(serde_json::from_value::<ModelEntry>(json.clone()).is_err());
        }
        json["maxOutputTokens"] = 1024.into();
        assert_eq!(
            serde_json::from_value::<ModelEntry>(json)
                .unwrap()
                .max_output_tokens
                .unwrap()
                .get(),
            1024
        );
    }
}
