use serde::{Deserialize, Serialize};

// Interactions API Request
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionsRequest {
    pub model: String,
    pub input: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GenerationConfig>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
}

// Interactions API Response
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionsResponse {
    pub id: String,
    pub steps: Vec<Step>,
    #[serde(default)]
    pub output_text: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Step {
    #[serde(rename = "model_output")]
    ModelOutput { content: Vec<Content> },
    #[serde(rename = "user_input")]
    UserInput { content: Vec<Content> },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Content {
    #[serde(rename = "text")]
    Text { text: String },
}

// Model list response
#[derive(Clone, Debug, Deserialize)]
pub struct ModelsResponse {
    pub models: Vec<ModelInfo>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub name: String,
    pub display_name: Option<String>,
}

/// Extract text output from Gemini Interactions API response
pub fn extract_output_text(response: &InteractionsResponse) -> Option<String> {
    // Prefer the convenience property if available
    if let Some(text) = &response.output_text {
        return Some(text.clone());
    }

    // Otherwise traverse steps in reverse to find the last model_output
    for step in response.steps.iter().rev() {
        if let Step::ModelOutput { content } = step {
            if let Some(c) = content.first() {
                let Content::Text { text } = c;
                return Some(text.clone());
            }
        }
    }
    None
}

/// Build a minimal test request for Gemini
pub fn build_test_request(model: &str) -> InteractionsRequest {
    InteractionsRequest {
        model: model.to_string(),
        input: "Reply OK.".to_string(),
        system_instruction: None,
        generation_config: Some(GenerationConfig {
            max_output_tokens: Some(32),
            temperature: None,
            top_p: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_interactions_request() {
        let req = InteractionsRequest {
            model: "gemini-3.8-flash".to_string(),
            input: "Hello".to_string(),
            system_instruction: None,
            generation_config: Some(GenerationConfig {
                temperature: Some(0.7),
                max_output_tokens: Some(100),
                top_p: None,
            }),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("gemini-3.8-flash"));
        assert!(json.contains("Hello"));
        assert!(json.contains("maxOutputTokens"));
    }

    #[test]
    fn deserialize_interactions_response() {
        let json = r#"{
            "id": "int-123",
            "steps": [
                {
                    "type": "model_output",
                    "content": [{"type": "text", "text": "Hi there!"}]
                }
            ],
            "outputText": "Hi there!"
        }"#;
        let response: InteractionsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, "int-123");
        assert_eq!(response.steps.len(), 1);
        assert_eq!(response.output_text, Some("Hi there!".to_string()));
    }

    #[test]
    fn deserialize_models_response() {
        let json = r#"{
            "models": [
                {"name": "models/gemini-3.8-flash", "displayName": "Gemini 3.8 Flash"},
                {"name": "models/gemini-3.1-pro"}
            ]
        }"#;
        let response: ModelsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.models.len(), 2);
        assert_eq!(response.models[0].name, "models/gemini-3.8-flash");
        assert_eq!(
            response.models[0].display_name,
            Some("Gemini 3.8 Flash".to_string())
        );
    }

    #[test]
    fn extract_output_from_convenience_property() {
        let response = InteractionsResponse {
            id: "test-123".into(),
            steps: vec![],
            output_text: Some("Hello!".into()),
        };
        assert_eq!(extract_output_text(&response), Some("Hello!".into()));
    }

    #[test]
    fn extract_output_from_steps() {
        let response = InteractionsResponse {
            id: "test-123".into(),
            steps: vec![Step::ModelOutput {
                content: vec![Content::Text {
                    text: "Hi there!".into(),
                }],
            }],
            output_text: None,
        };
        assert_eq!(extract_output_text(&response), Some("Hi there!".into()));
    }

    #[test]
    fn extract_output_returns_none_when_no_text() {
        let response = InteractionsResponse {
            id: "test-123".into(),
            steps: vec![],
            output_text: None,
        };
        assert_eq!(extract_output_text(&response), None);
    }

    #[test]
    fn test_request_structure() {
        let req = build_test_request("gemini-3.8-flash");
        assert_eq!(req.model, "gemini-3.8-flash");
        assert_eq!(req.input, "Reply OK.");
        assert!(req.generation_config.is_some());
        assert_eq!(req.generation_config.unwrap().max_output_tokens, Some(32));
    }
}
