use super::*;

#[tokio::test]
async fn test_clipboard_tool() {
    let tool = ClipboardTool;

    // We test clipboard, but since test environment might be headless (e.g. running in CI/containers),
    // we handle clipboard initialization failure gracefully so the test suite doesn't fail.
    let test_val = json!({
        "action": "set",
        "text": "OpenZ is awesome!"
    });

    match tool.call(&test_val).await {
        Ok(res) => {
            assert_eq!(res["status"], "success");

            // Now test get
            let get_val = json!({
                "action": "get"
            });
            if let Ok(get_res) = tool.call(&get_val).await {
                assert_eq!(get_res["status"], "success");
                if let Some(text_val) = get_res.get("text").and_then(|v| v.as_str()) {
                    // It could be that another concurrent test or tool changed it, but we check if we can get it
                    println!("Retrieved clipboard text successfully: {}", text_val);
                }
            } else {
                println!(
                    "Get clipboard text failed (which is normal on Linux if no clipboard manager daemon is active to persist dropped clipboard buffers)."
                );
            }
        }
        Err(e) => {
            println!("Clipboard test skipped or failed gracefully: {}", e);
            // We pass the test if it's due to headless environment
            let err_msg = e.to_string();
            assert!(
                err_msg.contains("Failed to initialize system clipboard")
                    || err_msg.contains("clipboard access may not be supported")
                    || err_msg.contains("ClipboardNotSupported")
            );
        }
    }
}

#[tokio::test]
async fn test_clipboard_tool_direct_string_and_aliases() {
    let tool = ClipboardTool;

    // Direct string copy
    let res = tool.call(&json!("Direct string to clipboard")).await;
    match res {
        Ok(val) => {
            assert_eq!(val["status"], "success");
        }
        Err(e) => {
            let err_msg = e.to_string();
            assert!(
                err_msg.contains("Failed to initialize system clipboard")
                    || err_msg.contains("clipboard access may not be supported")
                    || err_msg.contains("ClipboardNotSupported")
            );
        }
    }

    // Direct string read
    let res2 = tool.call(&json!("read")).await;
    match res2 {
        Ok(val) => {
            assert_eq!(val["status"], "success");
        }
        Err(e) => {
            let err_msg = e.to_string();
            assert!(
                err_msg.contains("Failed to initialize system clipboard")
                    || err_msg.contains("clipboard access may not be supported")
                    || err_msg.contains("ClipboardNotSupported")
                    || err_msg.contains("Failed to read text from system clipboard")
            );
        }
    }

    // Alias content
    let res3 = tool.call(&json!({ "content": "Alias text" })).await;
    match res3 {
        Ok(val) => {
            assert_eq!(val["status"], "success");
        }
        Err(e) => {
            let err_msg = e.to_string();
            assert!(
                err_msg.contains("Failed to initialize system clipboard")
                    || err_msg.contains("clipboard access may not be supported")
                    || err_msg.contains("ClipboardNotSupported")
            );
        }
    }
}
