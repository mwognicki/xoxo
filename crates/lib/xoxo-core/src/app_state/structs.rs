use crate::chat::structs::{ApiCompatibility, ApiProvider, ModelConfig};
use crate::config::Config;
use serde::{Deserialize, Serialize};

/// Mutable daemon-owned application state persisted across launches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppState {
    /// The currently selected provider used for new root chats.
    pub current_provider: ApiProvider,
    /// The currently selected model used for new root chats.
    pub current_model: ModelConfig,
}

impl AppState {
    /// Builds daemon-owned runtime state from the persisted config snapshot.
    pub fn from_config(config: &Config) -> Self {
        let current_provider = ApiProvider {
            name: config.current_provider().name.clone(),
            compatibility: parse_compatibility(&config.current_provider().compatibility),
        };

        Self {
            current_provider: current_provider.clone(),
            current_model: ModelConfig {
                model_name: config.current_model().model_name.clone(),
                provider: current_provider,
            },
        }
    }
}

fn parse_compatibility(raw: &str) -> ApiCompatibility {
    match raw {
        "open_router" | "openrouter" => ApiCompatibility::OpenRouter,
        "open_ai" | "openai" | "open_ai_like" => ApiCompatibility::OpenAiLike,
        "anthropic" | "anthropic_like" => ApiCompatibility::AnthropicLike,
        _ => ApiCompatibility::OpenAiLike,
    }
}
