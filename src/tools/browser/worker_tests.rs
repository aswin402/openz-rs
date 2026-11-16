use super::*;
use crate::tools::browser::webkit_ffi::probe_webkit_runtime;

#[test]
fn parse_size_spec_supports_presets_and_dimensions() {
    assert_eq!(parse_size_spec("desktop"), Some((1280, 800)));
    assert_eq!(parse_size_spec("mobile"), Some((412, 915)));
    assert_eq!(parse_size_spec("tablet"), Some((768, 1024)));
    assert_eq!(parse_size_spec("1440x900"), Some((1440, 900)));
    assert_eq!(parse_size_spec("800,600"), Some((800, 600)));
    assert!(parse_size_spec("invalid").is_none());
}

#[test]
fn sanitize_session_name_strips_special_chars() {
    assert_eq!(sanitize_session_name("my_session-1"), "my_session-1");
    assert_eq!(sanitize_session_name("../bad/path"), "___bad_path");
    assert_eq!(sanitize_session_name("   "), "default");
}

#[test]
fn normalize_cli_url_adds_https_when_schemeless() {
    assert_eq!(normalize_cli_url("example.com"), "https://example.com");
    assert_eq!(
        normalize_cli_url("http://localhost:3000"),
        "http://localhost:3000"
    );
    assert_eq!(normalize_cli_url("about:blank"), "about:blank");
}

#[test]
fn webkit_runtime_probe_executes_without_panicking() {
    let probe = probe_webkit_runtime();
    #[cfg(target_os = "linux")]
    assert!(probe.platform_supported);
}
