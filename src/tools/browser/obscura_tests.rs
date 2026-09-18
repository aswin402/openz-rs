use super::*;

#[tokio::test]
async fn test_obscura_browser_tool_metadata() -> Result<()> {
    let tool = ObscuraBrowserTool::new();
    assert_eq!(tool.name(), "obscura_browser");
    let params = tool.parameters();
    assert!(params.get("properties").is_some());
    Ok(())
}

#[test]
fn test_obscura_action_recognition() {
    let eval_actions = ["eval", "eval_js", "eval-js", "evaluate", "js", "EVAL_JS", " Eval "];
    for act in eval_actions {
        let norm = act.trim().to_lowercase().replace('-', "_");
        let is_eval = matches!(norm.as_str(), "eval_js" | "eval" | "evaluate" | "js");
        assert!(is_eval, "action '{act}' should be recognized as eval");
    }

    let render_actions = ["render", "RENDER", "markdown", ""];
    for act in render_actions {
        let norm = act.trim().to_lowercase().replace('-', "_");
        let is_eval = matches!(norm.as_str(), "eval_js" | "eval" | "evaluate" | "js");
        assert!(!is_eval, "action '{act}' should not be eval");
    }
}

#[test]
fn test_obscura_timeout_and_aliases() {
    let args1 = json!({ "timeout": "25" });
    let timeout1 = args1.get("timeout").or_else(|| args1.get("timeout_secs")).and_then(|v| {
        v.as_u64().or_else(|| v.as_str().and_then(|s| s.trim().parse::<u64>().ok()))
    }).unwrap_or(15);
    assert_eq!(timeout1, 25);

    let args2 = json!({ "timeout_secs": 42 });
    let timeout2 = args2.get("timeout").or_else(|| args2.get("timeout_secs")).and_then(|v| {
        v.as_u64().or_else(|| v.as_str().and_then(|s| s.trim().parse::<u64>().ok()))
    }).unwrap_or(15);
    assert_eq!(timeout2, 42);
}
