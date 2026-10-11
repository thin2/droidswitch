use super::catalog::ObservedModel;
use crate::{
    model::ids::InternalId,
    provider::Catalog,
    workspace::{Store, Workspace},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Official,
    Api,
}
#[derive(Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OfficialPlan {
    pub model: Option<String>,
    pub reasoning: Option<String>,
}
#[derive(Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApiPlan {
    pub model_id: Option<InternalId>,
    pub reasoning: Option<String>,
}
#[derive(Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Scheme {
    pub source: Option<Source>,
    pub official: OfficialPlan,
    pub api: ApiPlan,
}
impl Scheme {
    pub fn validate(&self, catalog: &Catalog) -> Result<(), String> {
        if let Some(id) = self.api.model_id {
            if !catalog.models.iter().any(|m| m.id == id && m.enabled) {
                return Err("API 方案引用了不存在或已停用的模型，请先更换或清除默认选择".into());
            }
        }
        Ok(())
    }
}
pub fn apply_scheme(
    document: &mut Value,
    scheme: &Scheme,
    catalog: &Catalog,
    observed: &[ObservedModel],
) -> Result<Vec<String>, String> {
    scheme.validate(catalog)?;
    let Some(source) = scheme.source else {
        return Ok(vec![]);
    };
    let (matches, reasoning): (Vec<&ObservedModel>, _) = match source {
        Source::Official => {
            let id = scheme
                .official
                .model
                .as_deref()
                .ok_or("请先选择官方默认模型")?;
            (
                observed
                    .iter()
                    .filter(|m| m.source == "official" && m.id == id)
                    .collect(),
                &scheme.official.reasoning,
            )
        }
        Source::Api => {
            let id = scheme.api.model_id.ok_or("请先选择 API 默认模型")?;
            let model = catalog
                .models
                .iter()
                .find(|m| m.id == id)
                .ok_or("API 默认模型不存在")?;
            // The name must identify exactly one entry in both the generated config and actual catalog.
            let matching = document
                .get("customModels")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter(|v| {
                            v.get("displayName").and_then(Value::as_str)
                                == Some(&model.display_name)
                        })
                        .count()
                })
                .unwrap_or(0);
            if matching != 1 {
                return Err("默认模型显示名有歧义，请为它设置唯一显示名称".into());
            }
            (
                observed
                    .iter()
                    .filter(|m| m.source == "api" && m.name == model.display_name)
                    .collect(),
                &scheme.api.reasoning,
            )
        }
    };
    if matches.len() != 1 {
        return Err("Droid 未提供唯一有效的模型选择器，未写入配置".into());
    }
    let selected = matches[0];
    let mut warnings = vec![];
    if document.get("sessionDefaultSettings").is_none() {
        document["sessionDefaultSettings"] = json!({});
    }
    let settings = document["sessionDefaultSettings"]
        .as_object_mut()
        .ok_or("sessionDefaultSettings 必须为对象")?;
    settings.insert("model".into(), json!(selected.id));
    if let Some(value) = reasoning
        .as_ref()
        .filter(|r| selected.reasoning.contains(r))
    {
        settings.insert("reasoningEffort".into(), json!(value));
    } else {
        settings.remove("reasoningEffort");
        if reasoning.is_some() {
            warnings.push("所选思考参数不受目标模型支持，已恢复模型默认值".into());
        }
    }
    if document.get("model").is_some() {
        warnings.push("保留了原有顶层 model；请在新会话用 /model 核对默认设置优先级".into());
    }
    Ok(warnings)
}
#[tauri::command]
pub fn save_scheme(
    store: tauri::State<'_, Store>,
    scheme: Scheme,
    revision: u64,
) -> Result<Workspace, String> {
    store.edit(revision, |w| {
        scheme.validate(&w.catalog)?;
        w.scheme = scheme;
        Ok(())
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_scheme_switch_roundtrip_preserves_both_plans_and_explicit_overrides() {
        let mut s = Scheme {
            source: Some(Source::Official),
            official: OfficialPlan {
                model: Some("official-id".into()),
                reasoning: Some("unsupported".into()),
            },
            api: ApiPlan::default(),
        };
        let copy = s.clone();
        s.source = Some(Source::Api);
        s.source = Some(Source::Official);
        assert!(s == copy);
        let mut d = json!({"customModels":[{"external":true}],"specModel":"keep","sessionDefaultSettings":{"autonomyMode":"normal","specModel":"keep-spec"}});
        let observed = vec![ObservedModel {
            id: "official-id".into(),
            name: "Official".into(),
            source: "official".into(),
            reasoning: vec!["low".into()],
            default_reasoning: Some("low".into()),
        }];
        let warnings = apply_scheme(&mut d, &s, &Catalog::default(), &observed).unwrap();
        assert_eq!(warnings.len(), 1);
        assert_eq!(d["customModels"][0]["external"], true);
        assert_eq!(d["sessionDefaultSettings"]["specModel"], "keep-spec");
        assert!(d["sessionDefaultSettings"].get("reasoningEffort").is_none());
        assert!(apply_scheme(&mut d, &s, &Catalog::default(), &[]).is_err());
    }
    #[test]
    fn dangling_default_cannot_be_deleted_or_disabled() {
        let s = Scheme {
            api: ApiPlan {
                model_id: Some(InternalId::new()),
                reasoning: None,
            },
            ..Default::default()
        };
        assert!(s.validate(&Catalog::default()).is_err());
    }
}
