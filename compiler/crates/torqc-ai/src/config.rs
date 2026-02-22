use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// AI provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub provider: ProviderKind,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default)]
    pub endpoint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Anthropic,
    OpenAI,
    Local,
    Custom,
}

fn default_model() -> String {
    "claude-sonnet-4-6".to_string()
}

fn default_max_retries() -> u32 {
    3
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: ProviderKind::Anthropic,
            key: None,
            model: default_model(),
            max_retries: default_max_retries(),
            endpoint: None,
        }
    }
}

/// Wrapper for the YAML config file structure.
#[derive(Debug, Serialize, Deserialize)]
struct ConfigFile {
    ai: Option<AiConfig>,
}

impl AiConfig {
    /// Load config from `~/.torqc/config.yaml`, falling back to defaults.
    pub fn load_global() -> Self {
        let path = global_config_path();
        Self::load_from(&path).unwrap_or_default()
    }

    /// Load config from a specific file path.
    pub fn load_from(path: &PathBuf) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        let file: ConfigFile = serde_yaml::from_str(&content).ok()?;
        file.ai
    }

    /// Merge a project-level config (from torq.yaml) over the global config.
    /// Project values override global values where present.
    pub fn merge_project(&mut self, project_path: &PathBuf) {
        if let Some(project) = Self::load_from(project_path) {
            self.provider = project.provider;
            if project.key.is_some() {
                self.key = project.key;
            }
            self.model = project.model;
            self.max_retries = project.max_retries;
            if project.endpoint.is_some() {
                self.endpoint = project.endpoint;
            }
        }
    }

    /// Save config to `~/.torqc/config.yaml`.
    pub fn save_global(&self) -> Result<(), String> {
        let path = global_config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create config dir: {}", e))?;
        }
        let wrapper = serde_yaml::to_string(&ConfigFile { ai: Some(self.clone()) })
            .map_err(|e| format!("failed to serialize config: {}", e))?;
        std::fs::write(&path, wrapper)
            .map_err(|e| format!("failed to write config: {}", e))?;
        Ok(())
    }
}

fn global_config_path() -> PathBuf {
    dirs_path().join("config.yaml")
}

fn dirs_path() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".torqc")
    } else {
        PathBuf::from(".torqc")
    }
}

