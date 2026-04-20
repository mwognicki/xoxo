use crate::chat::structs::ApiCompatibility;
use crate::config::{CustomProviderCompatibility, ProviderConfig};
use crate::llm::backends::ai_lib::AiLibBackendAdapter;
use crate::llm::backends::rig::RigBackendAdapter;
use crate::llm::facade::RuntimeBackend;
use crate::llm::RegisteredProvider;

/// Selects the runtime backend for a registered built-in provider.
///
/// Preference order is:
/// 1. `rig-core`
/// 2. `ai-lib`
/// 3. fallback custom backend marker
pub(crate) fn select_registered_backend(
    registration: &RegisteredProvider,
    provider_config: &ProviderConfig,
) -> RuntimeBackend {
    if registration.capabilities.supports_rig {
        let base_url = provider_config.effective_base_url();
        return match registration.compatibility {
            // OpenAI-compatible wire format, no dedicated rig-native client.
            ApiCompatibility::OpenAiLike
            | ApiCompatibility::OpenAiAndAnthropic
            | ApiCompatibility::Perplexity
            | ApiCompatibility::Moonshot => {
                RuntimeBackend::Rig(RigBackendAdapter::compatible_openai(base_url))
            }
            // Anthropic-compatible wire format, no dedicated rig-native client.
            ApiCompatibility::AnthropicLike => {
                RuntimeBackend::Rig(RigBackendAdapter::compatible_anthropic(base_url))
            }
            // Providers with a dedicated rig-native client → native adapter.
            ApiCompatibility::OpenAi
            | ApiCompatibility::Anthropic
            | ApiCompatibility::Gemini
            | ApiCompatibility::Cohere
            | ApiCompatibility::XAi
            | ApiCompatibility::AzureOpenAi
            | ApiCompatibility::DeepSeek
            | ApiCompatibility::Ollama
            | ApiCompatibility::OpenRouter
            | ApiCompatibility::Groq
            | ApiCompatibility::Hyperbolic
            | ApiCompatibility::Together
            | ApiCompatibility::Galadriel
            | ApiCompatibility::Mira
            | ApiCompatibility::Custom { .. } => {
                RuntimeBackend::Rig(RigBackendAdapter::native(base_url))
            }
        };
    }

    if registration.capabilities.supports_ai_lib {
        if let Ok(adapter) =
            AiLibBackendAdapter::for_provider(registration.id.as_str(), provider_config)
        {
            return RuntimeBackend::AiLib(adapter);
        }
    }

    RuntimeBackend::Custom {
        base_url: provider_config.effective_base_url(),
    }
}

/// Selects the runtime backend for a config-defined compatible provider.
///
/// Always routes through rig-core, using the OpenAI- or Anthropic-compatible client
/// according to the declared wire format.
pub(crate) fn select_custom_compatible_backend(
    provider_config: &ProviderConfig,
    compatibility: &CustomProviderCompatibility,
) -> RuntimeBackend {
    let base_url = provider_config.effective_base_url();
    match compatibility {
        CustomProviderCompatibility::OpenAi => {
            RuntimeBackend::Rig(RigBackendAdapter::compatible_openai(base_url))
        }
        CustomProviderCompatibility::Anthropic => {
            RuntimeBackend::Rig(RigBackendAdapter::compatible_anthropic(base_url))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::chat::structs::ApiCompatibility;
    use crate::config::{CustomProviderCompatibility, ProviderConfig};
    use crate::llm::LlmBackendKind;
    use crate::llm::{ProviderCapabilities, RegisteredProvider};
    use crate::llm::backends::rig::RigAdapterKind;
    use super::*;

    fn kind_of(backend: RuntimeBackend) -> LlmBackendKind {
        backend.kind()
    }

    #[test]
    fn selector_prefers_rig_when_both_backends_are_available() {
        let registration = RegisteredProvider {
            id: "openai".to_string(),
            name: "OpenAI".to_string(),
            compatibility: ApiCompatibility::OpenAi,
            capabilities: ProviderCapabilities::hybrid(),
        };
        let config = ProviderConfig::built_in("openai", None, "secret");

        let backend = select_registered_backend(&registration, &config);

        assert_eq!(kind_of(backend), LlmBackendKind::Rig);
    }

    #[test]
    fn selector_falls_back_to_ai_lib_when_rig_is_unavailable() {
        let registration = RegisteredProvider {
            id: "gemini".to_string(),
            name: "Gemini".to_string(),
            compatibility: ApiCompatibility::Custom {
                name: "custom".to_string(),
            },
            capabilities: ProviderCapabilities::ai_lib_only(),
        };
        let config = ProviderConfig::built_in("gemini", None, "secret");

        let backend = select_registered_backend(&registration, &config);

        assert_eq!(kind_of(backend), LlmBackendKind::AiLib);
    }

    #[test]
    fn selector_routes_custom_openai_compat_provider_to_rig_openai_adapter() {
        let config = ProviderConfig::other(
            "Acme Gateway",
            "https://gateway.example.com/v1",
            CustomProviderCompatibility::OpenAi,
            "secret",
        );

        let backend =
            select_custom_compatible_backend(&config, &CustomProviderCompatibility::OpenAi);

        let RuntimeBackend::Rig(adapter) = backend else {
            panic!("expected rig backend, got {backend:?}");
        };
        assert_eq!(adapter.kind(), RigAdapterKind::CompatibleOpenAi);
        assert_eq!(adapter.base_url(), Some("https://gateway.example.com/v1"));
    }

    #[test]
    fn selector_routes_custom_anthropic_compat_provider_to_rig_anthropic_adapter() {
        let config = ProviderConfig::other(
            "Acme Anthropic Gateway",
            "https://anthropic-gw.example.com",
            CustomProviderCompatibility::Anthropic,
            "secret",
        );

        let backend =
            select_custom_compatible_backend(&config, &CustomProviderCompatibility::Anthropic);

        let RuntimeBackend::Rig(adapter) = backend else {
            panic!("expected rig backend, got {backend:?}");
        };
        assert_eq!(adapter.kind(), RigAdapterKind::CompatibleAnthropic);
        assert_eq!(adapter.base_url(), Some("https://anthropic-gw.example.com"));
    }

    #[test]
    fn selector_uses_openai_compatible_adapter_for_openai_like_tags() {
        let registration = RegisteredProvider {
            id: "perplexity".to_string(),
            name: "Perplexity".to_string(),
            compatibility: ApiCompatibility::Perplexity,
            capabilities: ProviderCapabilities::rig_only(),
        };
        let config = ProviderConfig::built_in("perplexity", None, "secret");

        let backend = select_registered_backend(&registration, &config);

        let RuntimeBackend::Rig(adapter) = backend else {
            panic!("expected rig backend, got {backend:?}");
        };
        assert_eq!(adapter.kind(), RigAdapterKind::CompatibleOpenAi);
    }

    #[test]
    fn selector_uses_anthropic_compatible_adapter_for_anthropic_like_tags() {
        let registration = RegisteredProvider {
            id: "imaginary".to_string(),
            name: "Imaginary Anthropic-compat".to_string(),
            compatibility: ApiCompatibility::AnthropicLike,
            capabilities: ProviderCapabilities::rig_only(),
        };
        let config = ProviderConfig::built_in("imaginary", None, "secret");

        let backend = select_registered_backend(&registration, &config);

        let RuntimeBackend::Rig(adapter) = backend else {
            panic!("expected rig backend, got {backend:?}");
        };
        assert_eq!(adapter.kind(), RigAdapterKind::CompatibleAnthropic);
    }

    #[test]
    fn selector_uses_native_adapter_for_rig_native_providers() {
        let registration = RegisteredProvider {
            id: "anthropic".to_string(),
            name: "Anthropic".to_string(),
            compatibility: ApiCompatibility::Anthropic,
            capabilities: ProviderCapabilities::hybrid(),
        };
        let config = ProviderConfig::built_in("anthropic", None, "secret");

        let backend = select_registered_backend(&registration, &config);

        assert_eq!(kind_of(backend), LlmBackendKind::Rig);
    }
}
