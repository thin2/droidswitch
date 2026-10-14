use crate::{model::entry::Protocol, workspace::Workspace};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, RwLock,
};

// Deliberately not Debug: this contains resolved credentials.
#[derive(Clone)]
pub struct ProviderConfig {
    pub base_url: String,
    pub api_key: String,
    pub models: HashSet<String>,
    pub extra_headers: BTreeMap<String, String>,
}
#[derive(Clone, Default)]
pub struct ProxyRegistry {
    providers: Arc<RwLock<HashMap<String, ProviderConfig>>>,
    running: Arc<AtomicBool>,
}
impl ProxyRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }
    pub(super) fn set_running(&self, running: bool) {
        self.running.store(running, Ordering::Release);
    }
    #[cfg(test)]
    pub fn register(&self, id: String, config: ProviderConfig) {
        self.providers.write().unwrap().insert(id, config);
    }
    pub fn get(&self, id: &str) -> Option<ProviderConfig> {
        self.providers.read().ok()?.get(id).cloned()
    }
    pub fn load_from_workspace(&self, workspace: &Workspace) -> Result<(), String> {
        let mut next = HashMap::new();
        let mut unavailable = false;
        for provider in &workspace.applied_catalog.providers {
            let models: HashSet<_> = workspace
                .applied_catalog
                .models
                .iter()
                .filter(|m| {
                    m.enabled
                        && m.provider_id == provider.id
                        && m.effective_protocol(provider.protocol) == Protocol::GoogleGemini
                })
                .map(|m| m.api_model_id.as_str().to_owned())
                .collect();
            if models.is_empty() {
                continue;
            }
            let key = match provider.credential.resolve() {
                Ok(key) if !key.trim().is_empty() => key,
                _ => {
                    unavailable = true;
                    continue;
                }
            };
            if provider.validate().is_err() {
                unavailable = true;
                continue;
            }
            next.insert(
                provider.id.to_string(),
                ProviderConfig {
                    base_url: provider.base_url.clone(),
                    api_key: key,
                    models,
                    extra_headers: provider.extra_headers.clone(),
                },
            );
        }
        // Replace atomically, including removals and unresolved credentials. Never
        // retain a previously valid credential after a failed resolution.
        *self
            .providers
            .write()
            .map_err(|_| "Proxy registry unavailable")? = next;
        if unavailable {
            Err("Some applied Gemini connections are unavailable; check credentials in the app environment".into())
        } else {
            Ok(())
        }
    }
    pub fn refresh(&self, workspace: &Workspace) -> Result<(), String> {
        self.load_from_workspace(workspace)
    }
}
