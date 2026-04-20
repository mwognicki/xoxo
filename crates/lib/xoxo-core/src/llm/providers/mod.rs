use crate::chat::structs::ApiCompatibility;
use crate::llm::provider::{
    ProviderCapabilities, ProviderRegistration, RegisteredProvider,
};

fn registered_provider(
    id: &'static str,
    name: &'static str,
    compatibility: ApiCompatibility,
    capabilities: ProviderCapabilities,
) -> RegisteredProvider {
    RegisteredProvider {
        id: id.to_string(),
        name: name.to_string(),
        compatibility,
        capabilities,
    }
}

// --- Rig-native providers -------------------------------------------------

inventory::submit! {
    ProviderRegistration {
        id: "openai",
        factory: || registered_provider(
            "openai",
            "OpenAI",
            ApiCompatibility::OpenAi,
            ProviderCapabilities::hybrid(),
        ),
    }
}

inventory::submit! {
    ProviderRegistration {
        id: "anthropic",
        factory: || registered_provider(
            "anthropic",
            "Anthropic",
            ApiCompatibility::Anthropic,
            ProviderCapabilities::hybrid(),
        ),
    }
}

inventory::submit! {
    ProviderRegistration {
        id: "gemini",
        factory: || registered_provider(
            "gemini",
            "Gemini",
            ApiCompatibility::Gemini,
            ProviderCapabilities::hybrid(),
        ),
    }
}

inventory::submit! {
    ProviderRegistration {
        id: "cohere",
        factory: || registered_provider(
            "cohere",
            "Cohere",
            ApiCompatibility::Cohere,
            ProviderCapabilities::rig_only(),
        ),
    }
}

inventory::submit! {
    ProviderRegistration {
        id: "xai",
        factory: || registered_provider(
            "xai",
            "xAI",
            ApiCompatibility::XAi,
            ProviderCapabilities::rig_only(),
        ),
    }
}

inventory::submit! {
    ProviderRegistration {
        id: "azure",
        factory: || registered_provider(
            "azure",
            "Azure OpenAI",
            ApiCompatibility::AzureOpenAi,
            ProviderCapabilities::rig_only(),
        ),
    }
}

inventory::submit! {
    ProviderRegistration {
        id: "deepseek",
        factory: || registered_provider(
            "deepseek",
            "DeepSeek",
            ApiCompatibility::DeepSeek,
            ProviderCapabilities::hybrid(),
        ),
    }
}

inventory::submit! {
    ProviderRegistration {
        id: "ollama",
        factory: || registered_provider(
            "ollama",
            "Ollama",
            ApiCompatibility::Ollama,
            ProviderCapabilities::rig_only(),
        ),
    }
}

inventory::submit! {
    ProviderRegistration {
        id: "openrouter",
        factory: || registered_provider(
            "openrouter",
            "OpenRouter",
            ApiCompatibility::OpenRouter,
            ProviderCapabilities::hybrid(),
        ),
    }
}

inventory::submit! {
    ProviderRegistration {
        id: "groq",
        factory: || registered_provider(
            "groq",
            "Groq",
            ApiCompatibility::Groq,
            ProviderCapabilities::hybrid(),
        ),
    }
}

inventory::submit! {
    ProviderRegistration {
        id: "hyperbolic",
        factory: || registered_provider(
            "hyperbolic",
            "Hyperbolic",
            ApiCompatibility::Hyperbolic,
            ProviderCapabilities::rig_only(),
        ),
    }
}

inventory::submit! {
    ProviderRegistration {
        id: "together",
        factory: || registered_provider(
            "together",
            "Together AI",
            ApiCompatibility::Together,
            ProviderCapabilities::rig_only(),
        ),
    }
}

inventory::submit! {
    ProviderRegistration {
        id: "galadriel",
        factory: || registered_provider(
            "galadriel",
            "Galadriel",
            ApiCompatibility::Galadriel,
            ProviderCapabilities::rig_only(),
        ),
    }
}

inventory::submit! {
    ProviderRegistration {
        id: "mira",
        factory: || registered_provider(
            "mira",
            "Mira",
            ApiCompatibility::Mira,
            ProviderCapabilities::rig_only(),
        ),
    }
}

// --- OpenAI-compatible providers wired through rig (no native rig client) -

inventory::submit! {
    ProviderRegistration {
        id: "perplexity",
        factory: || registered_provider(
            "perplexity",
            "Perplexity",
            ApiCompatibility::Perplexity,
            ProviderCapabilities::hybrid(),
        ),
    }
}

inventory::submit! {
    ProviderRegistration {
        id: "moonshot",
        factory: || registered_provider(
            "moonshot",
            "Moonshot",
            ApiCompatibility::Moonshot,
            ProviderCapabilities::rig_only(),
        ),
    }
}

// --- Ai-lib-only providers ------------------------------------------------
//
// Retained so users who point `provider_id` at them keep working through the
// ai-lib fallback. No rig-core native client exists for these.

inventory::submit! {
    ProviderRegistration {
        id: "qwen",
        factory: || registered_provider(
            "qwen",
            "Qwen",
            ApiCompatibility::Custom {
                name: "custom".to_string(),
            },
            ProviderCapabilities::ai_lib_only(),
        ),
    }
}

inventory::submit! {
    ProviderRegistration {
        id: "huggingface",
        factory: || registered_provider(
            "huggingface",
            "HuggingFace",
            ApiCompatibility::Custom {
                name: "custom".to_string(),
            },
            ProviderCapabilities::ai_lib_only(),
        ),
    }
}

inventory::submit! {
    ProviderRegistration {
        id: "replicate",
        factory: || registered_provider(
            "replicate",
            "Replicate",
            ApiCompatibility::Custom {
                name: "custom".to_string(),
            },
            ProviderCapabilities::ai_lib_only(),
        ),
    }
}

inventory::submit! {
    ProviderRegistration {
        id: "ai21",
        factory: || registered_provider(
            "ai21",
            "AI21",
            ApiCompatibility::Custom {
                name: "custom".to_string(),
            },
            ProviderCapabilities::ai_lib_only(),
        ),
    }
}

inventory::submit! {
    ProviderRegistration {
        id: "baidu_ernie",
        factory: || registered_provider(
            "baidu_ernie",
            "Baidu ERNIE",
            ApiCompatibility::Custom {
                name: "custom".to_string(),
            },
            ProviderCapabilities::ai_lib_only(),
        ),
    }
}

inventory::submit! {
    ProviderRegistration {
        id: "tencent_hunyuan",
        factory: || registered_provider(
            "tencent_hunyuan",
            "Tencent Hunyuan",
            ApiCompatibility::Custom {
                name: "custom".to_string(),
            },
            ProviderCapabilities::ai_lib_only(),
        ),
    }
}

inventory::submit! {
    ProviderRegistration {
        id: "iflytek_spark",
        factory: || registered_provider(
            "iflytek_spark",
            "iFlytek Spark",
            ApiCompatibility::Custom {
                name: "custom".to_string(),
            },
            ProviderCapabilities::ai_lib_only(),
        ),
    }
}

inventory::submit! {
    ProviderRegistration {
        id: "z.ai",
        factory: || registered_provider(
            "z.ai",
            "Z.AI",
            ApiCompatibility::Custom {
                name: "custom".to_string(),
            },
            ProviderCapabilities::ai_lib_only(),
        ),
    }
}

inventory::submit! {
    ProviderRegistration {
        id: "minimax",
        factory: || registered_provider(
            "minimax",
            "MiniMax",
            ApiCompatibility::Custom {
                name: "custom".to_string(),
            },
            ProviderCapabilities::ai_lib_only(),
        ),
    }
}
