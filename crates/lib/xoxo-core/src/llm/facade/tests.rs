#[cfg(test)]
mod tests {
    use crate::chat::structs::ApiCompatibility;
    use crate::config::{CustomProviderCompatibility, ProviderConfig};
    use crate::llm::{LlmBackendKind, LlmFacade};

    #[test]
    fn resolve_built_in_provider_reuses_registered_metadata_and_selects_rig() {
        let facade = LlmFacade::new();
        let resolved = facade
            .resolve(&ProviderConfig::built_in("openai", None, "secret"), "gpt-4o")
            .expect("provider resolves");

        assert_eq!(resolved.model().provider.name, "OpenAI");
        assert_eq!(resolved.model().provider.compatibility, ApiCompatibility::OpenAi);
        assert_eq!(resolved.backend_kind(), LlmBackendKind::Rig);
        assert!(resolved.supports_tool_calls());
    }

    #[test]
    fn resolve_ai_lib_only_provider_selects_ai_lib_backend_family() {
        let facade = LlmFacade::new();
        let resolved = facade
            .resolve(&ProviderConfig::built_in("qwen", None, "secret"), "qwen-max")
            .expect("provider resolves");

        assert_eq!(resolved.model().provider.name, "Qwen");
        assert_eq!(resolved.backend_kind(), LlmBackendKind::AiLib);
        assert!(resolved.supports_tool_calls());
    }

    #[test]
    fn resolve_gemini_hybrid_provider_prefers_rig() {
        let facade = LlmFacade::new();
        let resolved = facade
            .resolve(
                &ProviderConfig::built_in("gemini", None, "secret"),
                "gemini-1.5-flash",
            )
            .expect("provider resolves");

        assert_eq!(resolved.model().provider.name, "Gemini");
        assert_eq!(
            resolved.model().provider.compatibility,
            ApiCompatibility::Gemini
        );
        assert_eq!(resolved.backend_kind(), LlmBackendKind::Rig);
        assert!(resolved.supports_tool_calls());
    }

    #[test]
    fn resolve_custom_openai_compatible_provider_selects_rig_backend_family() {
        let facade = LlmFacade::new();
        let resolved = facade
            .resolve(
                &ProviderConfig::other(
                    "Acme Gateway",
                    "https://gateway.example.com/v1",
                    CustomProviderCompatibility::OpenAi,
                    "secret",
                ),
                "acme-chat-1",
            )
            .expect("custom provider resolves");

        assert_eq!(resolved.model().provider.name, "Acme Gateway");
        assert_eq!(
            resolved.model().provider.compatibility,
            ApiCompatibility::OpenAiLike
        );
        assert_eq!(resolved.backend_kind(), LlmBackendKind::Rig);
        assert_eq!(resolved.base_url(), Some("https://gateway.example.com/v1"));
        assert!(resolved.supports_tool_calls());
    }

    #[test]
    fn resolve_custom_anthropic_compatible_provider_selects_rig_backend_family() {
        let facade = LlmFacade::new();
        let resolved = facade
            .resolve(
                &ProviderConfig::other(
                    "Acme Anthropic Gateway",
                    "https://anthropic-gw.example.com",
                    CustomProviderCompatibility::Anthropic,
                    "secret",
                ),
                "acme-anthropic-1",
            )
            .expect("custom provider resolves");

        assert_eq!(resolved.model().provider.name, "Acme Anthropic Gateway");
        assert_eq!(
            resolved.model().provider.compatibility,
            ApiCompatibility::AnthropicLike
        );
        assert_eq!(resolved.backend_kind(), LlmBackendKind::Rig);
        assert_eq!(resolved.base_url(), Some("https://anthropic-gw.example.com"));
        assert!(resolved.supports_tool_calls());
    }
}
