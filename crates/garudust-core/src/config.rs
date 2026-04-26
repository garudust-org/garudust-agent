use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub home_dir:        PathBuf,
    pub model:           String,
    pub max_iterations:  u32,
    pub tool_delay_ms:   u64,
    pub provider:        String,
    pub base_url:        Option<String>,
    pub api_key:         Option<String>,
    pub compression:     CompressionConfig,
    pub network:         NetworkConfig,
}

impl Default for AgentConfig {
    fn default() -> Self {
        let home_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".garudust");
        Self {
            home_dir,
            model:          "anthropic/claude-sonnet-4-6".into(),
            max_iterations: 90,
            tool_delay_ms:  0,
            provider:       "openrouter".into(),
            base_url:       None,
            api_key:        None,
            compression:    CompressionConfig::default(),
            network:        NetworkConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    pub enabled:            bool,
    pub threshold_fraction: f32,
    pub model:              Option<String>,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self { enabled: true, threshold_fraction: 0.8, model: None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkConfig {
    pub force_ipv4:  bool,
    pub proxy:       Option<String>,
}
