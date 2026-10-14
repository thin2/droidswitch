pub mod versions;
use crate::{model::entry::ModelEntry, provider::ProviderConnection};
use serde_json::{json, Value};

pub fn custom_model(
    model: &ModelEntry,
    provider: &ProviderConnection,
    version: &str,
) -> Result<Value, String> {
    versions::require_supported(version)?;
    provider.validate()?;
    if model.provider_id != provider.id {
        return Err("模型与供应商关联不匹配".into());
    }

    // Native Gemini is bridged through Droid Chat Completions, never Responses.
    let effective_protocol = model.effective_protocol(provider.protocol);
    let droid_protocol = match effective_protocol {
        crate::model::entry::Protocol::GoogleGemini => "generic-chat-completion-api",
        crate::model::entry::Protocol::AnthropicMessages => "anthropic",
        crate::model::entry::Protocol::OpenAiResponses => "openai",
        crate::model::entry::Protocol::OpenAiChatCompletions => "generic-chat-completion-api",
    };

    // Rewrite baseUrl for GoogleGemini to point to local proxy
    let base_url = if effective_protocol == crate::model::entry::Protocol::GoogleGemini {
        format!("http://127.0.0.1:18318/proxy/{}/v1", provider.id)
    } else {
        provider.base_url.clone()
    };

    let mut output = json!({"model":model.api_model_id.as_str(),"displayName":model.display_name,"baseUrl":base_url,"provider":droid_protocol,"apiKey":provider.credential.settings_value(),"noImageSupport":!model.image_input});
    if let Some(max) = model.max_output_tokens {
        output["maxOutputTokens"] = json!(max.get());
    }
    if !model.extra_args.is_empty() {
        output["extraArgs"] = json!(model.extra_args);
    }
    if effective_protocol != crate::model::entry::Protocol::GoogleGemini
        && !provider.extra_headers.is_empty()
    {
        output["extraHeaders"] = json!(provider.extra_headers);
    }
    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::{entry::ModelEntry, ids::ApiModelId},
        provider::tests::sample,
    };
    #[test]
    fn adapter_custom_models_only_contains_supported_fields() {
        let p = sample();
        let m = ModelEntry::new(
            p.id,
            ApiModelId::try_from("api/id".to_owned()).unwrap(),
            "Name".into(),
        );
        let result = custom_model(&m, &p, "0.238.0").unwrap();
        for forbidden in ["id", "providerId", "enabled", "droidSelectorId", "lastTest"] {
            assert!(result.get(forbidden).is_none());
        }
        assert_eq!(result["apiKey"], "${PROVIDER_KEY}");
        assert!(result.get("maxOutputTokens").is_none());
        assert!(custom_model(&m, &p, "unknown").is_err());
    }
}

#[cfg(test)]
mod fixture_tests {
    use super::*;
    use crate::{
        model::{
            entry::Protocol,
            ids::{ApiModelId, InternalId},
        },
        provider::{Credential, CredentialMode},
    };
    #[test]
    fn adapter_maps_all_protocols_from_observed_cli_input() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../planning/verification/fixtures/probe/models.json"
        ))
        .unwrap();
        for item in fixture["customModels"].as_array().unwrap() {
            let provider = ProviderConnection {
                id: InternalId::new(),
                name: "Probe".into(),
                base_url: item["baseUrl"].as_str().unwrap().into(),
                protocol: serde_json::from_value::<Protocol>(item["provider"].clone()).unwrap(),
                credential: Credential {
                    mode: CredentialMode::Direct,
                    value: item["apiKey"].as_str().unwrap().into(),
                },
                model_list_url: None,
                extra_headers: Default::default(),
            };
            let model = ModelEntry::new(
                provider.id,
                ApiModelId::try_from(item["model"].as_str().unwrap().to_owned()).unwrap(),
                item["displayName"].as_str().unwrap().into(),
            );
            let actual = custom_model(&model, &provider, "0.238.0").unwrap();
            for (key, value) in item.as_object().unwrap() {
                assert_eq!(&actual[key], value);
            }
        }
    }
}
#[cfg(test)]
mod management_field_tests {
    #[test]
    #[allow(non_snake_case)]
    fn customModels_never_contains_local_fields() {
        let p = crate::provider::tests::sample();
        let m = crate::model::entry::ModelEntry::new(
            p.id,
            crate::model::ids::ApiModelId::try_from("id".to_owned()).unwrap(),
            "Name".into(),
        );
        let output = super::custom_model(&m, &p, "0.238.0").unwrap();
        for key in [
            "id",
            "providerId",
            "enabled",
            "manufacturer",
            "droidSelectorId",
            "lastTest",
            "price",
        ] {
            assert!(output.get(key).is_none());
        }
    }
}
