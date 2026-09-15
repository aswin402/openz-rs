use super::*;
use serde_json::json;

#[test]
fn format_tool_args_does_not_use_global_arg_normalizer() {
    let source = std::fs::read_to_string("src/agent/agent_loop/tool_execution.rs").unwrap();
    let production_source = source.split("#[cfg(test)]").next().unwrap_or(&source);
    assert!(
        !production_source.contains("normalize_tool_args(raw_args)"),
        "format_tool_args should not rely on global argument rewriting for display-only formatting"
    );
}

#[test]
fn filesystem_formatter_reads_native_aliases_directly() {
    let formatted = format_tool_args(
        "replace_lines",
        &json!({
            "filePath": "/tmp/example.rs",
            "startLine": 1,
            "endLine": 1,
            "content": "updated"
        }),
    );
    assert!(formatted.contains("example.rs"));
}

#[test]
fn html_to_video_formatter_shows_timeline_cost() {
    let formatted = format_tool_args(
        "html_to_video",
        &json!({
            "html_path": "/tmp/intro.html",
            "output_path": "/tmp/intro.mp4",
            "duration_seconds": 30,
            "fps": 30
        }),
    );
    assert!(formatted.contains("duration: 30s"));
    assert!(formatted.contains("fps: 30"));
    assert!(formatted.contains("frames: 900"));
}
