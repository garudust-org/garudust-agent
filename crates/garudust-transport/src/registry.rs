use std::sync::Arc;

use garudust_core::transport::ProviderTransport;
use garudust_core::config::AgentConfig;

use crate::chat_completions::ChatCompletionsTransport;
use crate::anthropic::AnthropicTransport;

pub fn build_transport(config: &AgentConfig) -> Arc<dyn ProviderTransport> {
    let base_url = config.base_url.clone();
    let api_key  = config.api_key.clone().unwrap_or_default();

    match config.provider.as_str() {
        "anthropic" => Arc::new(AnthropicTransport::new(api_key)),
        _ => Arc::new(ChatCompletionsTransport::new(
            base_url.unwrap_or_else(|| "https://openrouter.ai/api/v1".into()),
            api_key,
        )),
    }
}
