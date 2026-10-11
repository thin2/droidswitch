use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Application identity; independent of display names, API IDs and Droid selectors.
///
/// ```compile_fail
/// use droid_switch_lib::model::ids::{InternalId, ApiModelId};
/// let id: ApiModelId = InternalId::new();
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InternalId(Uuid);
impl InternalId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}
impl Default for InternalId {
    fn default() -> Self {
        Self::new()
    }
}
impl std::fmt::Display for InternalId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Opaque ID sent to the provider API. Kept exactly as returned by the provider.
///
/// ```compile_fail
/// use droid_switch_lib::model::ids::{ApiModelId, DroidSelectorId};
/// let api = ApiModelId::try_from("model".to_owned()).unwrap();
/// let selector: DroidSelectorId = api;
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ApiModelId(String);

/// Only construct from an actually observed selector. No inferred prefix conversion exists.
///
/// ```compile_fail
/// use droid_switch_lib::model::ids::{DroidSelectorId, InternalId};
/// let selector = DroidSelectorId::try_from("observed-value".to_owned()).unwrap();
/// let internal: InternalId = selector;
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DroidSelectorId(String);

macro_rules! opaque_id {
    ($name:ident) => {
        impl TryFrom<String> for $name {
            type Error = &'static str;
            fn try_from(value: String) -> Result<Self, Self::Error> {
                if value.trim().is_empty() {
                    return Err("identifier must not be empty");
                }
                Ok(Self(value))
            }
        }
        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }
        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}
opaque_id!(ApiModelId);
opaque_id!(DroidSelectorId);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_api_id_roundtrip_is_exact() {
        let input = " provider/model:版本-01 ";
        let id = ApiModelId::try_from(input.to_owned()).unwrap();
        let json = serde_json::to_string(&id).unwrap();
        let decoded: ApiModelId = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.as_str(), input);
    }
    #[test]
    fn model_id_deserialization_validates_all_types() {
        assert!(serde_json::from_str::<InternalId>("\"display name\"").is_err());
        for input in ["\"\"", "\"  \""] {
            assert!(serde_json::from_str::<ApiModelId>(input).is_err());
            assert!(serde_json::from_str::<DroidSelectorId>(input).is_err());
        }
        let id = InternalId::new();
        assert_ne!(id, InternalId::new());
        assert_eq!(
            id,
            serde_json::from_str(&serde_json::to_string(&id).unwrap()).unwrap()
        );
    }
}
