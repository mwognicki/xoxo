5. Implement backend selection rules.
   Use rig-core for:
    - providers natively supported by rig-core
    - OpenAI-compatible providers
    - Anthropic-compatible providers
    - config-defined “other” compatible providers
      Use ai-lib only when rig-core cannot handle the provider.
6. Update inventory registrations and resolver logic.
   Built-in providers stay registered.
   Config-defined “other” providers are resolved directly from config, without hardcoded registration.
7. Add tests before cleanup.
   Cover:
    - rig-core precedence over ai-lib
    - custom compatible providers routing to rig-core
    - built-in provider resolution
    - ai-lib fallback
    - no backend crate types leaking through public xoxo-core APIs