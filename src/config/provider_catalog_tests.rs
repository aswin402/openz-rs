use super::*;

#[test]
fn provider_catalog_matches_resolver_aliases() {
    assert_eq!(find_provider("anthropic").unwrap().canonical_name, "anthropic");
    assert_eq!(find_provider("z_ai").unwrap().canonical_name, "z.ai");
    assert_eq!(
        find_provider("google-ai-studio").unwrap().canonical_name,
        "google_ai_studio"
    );
    assert_eq!(
        find_provider("opencode zen").unwrap().canonical_name,
        "opencode_zen"
    );

    let (provider, prefix) = provider_prefix_for_model("CEREBRES/foo").unwrap();
    assert_eq!(provider.canonical_name, "cerebras");
    assert_eq!(prefix, "cerebres/");

    assert_eq!(
        environment_keys_for_provider("z.ai"),
        &["Z_AI_API_KEY"]
    );
    assert_eq!(
        keyword_provider_candidates("claude-3-5-sonnet"),
        &["anthropic", "opencode_zen", "openrouter"]
    );
    assert!(provider_descriptors().iter().all(|descriptor| {
        descriptor
            .aliases
            .iter()
            .all(|alias| find_provider(alias).is_some())
    }));
}
