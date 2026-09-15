use super::*;

#[test]
fn update_provider_key_uses_builtin_alias_default_bases() {
    let mut config = Config::default();
    config.providers.z_ai = None;
    config.providers.opencode_zen = None;
    config.providers.google_ai_studio = None;

    update_provider_key(&mut config, "z_ai", "z-key".to_string());
    assert_eq!(
        config
            .providers
            .z_ai
            .as_ref()
            .and_then(|provider| provider.api_base.as_deref()),
        Some("https://api.z.ai/api/paas/v4/")
    );

    update_provider_key(&mut config, "opencode-zen", "zen-key".to_string());
    assert_eq!(
        config
            .providers
            .opencode_zen
            .as_ref()
            .and_then(|provider| provider.api_base.as_deref()),
        Some("https://opencode.ai/zen/v1")
    );

    update_provider_key(&mut config, "google-ai-studio", "google-key".to_string());
    assert_eq!(
        config
            .providers
            .google_ai_studio
            .as_ref()
            .and_then(|provider| provider.api_base.as_deref()),
        Some("https://generativelanguage.googleapis.com/v1beta/openai/")
    );
}
