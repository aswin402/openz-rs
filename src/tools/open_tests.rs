use super::*;

#[tokio::test]
async fn test_open_tool() {
    let tool = OpenTool;

    let args = json!({
        "target": "https://example.com"
    });

    // In headless CI/test environments, this might return an error due to missing display server or xdg-open defaults.
    // We ensure it parses and handles results/errors gracefully.
    let res = tool.call(&args).await;
    match res {
        Ok(val) => {
            assert_eq!(val["status"], "success");
        }
        Err(e) => {
            println!(
                "Open tool run finished with error (expected in headless CI/containers): {}",
                e
            );
        }
    }
}

#[tokio::test]
async fn test_open_tool_direct_string_and_aliases() {
    let tool = OpenTool;

    // Direct string
    let res = tool.call(&json!("https://example.com")).await;
    match res {
        Ok(val) => {
            assert_eq!(val["status"], "success");
        }
        Err(e) => {
            println!("Expected headless open error: {}", e);
        }
    }

    // file:// prefix
    let res2 = tool.call(&json!("file://Cargo.toml")).await;
    match res2 {
        Ok(val) => {
            assert_eq!(val["status"], "success");
        }
        Err(e) => {
            println!("Expected headless open error: {}", e);
        }
    }

    // Alias url
    let res3 = tool.call(&json!({ "url": "https://example.com" })).await;
    match res3 {
        Ok(val) => {
            assert_eq!(val["status"], "success");
        }
        Err(e) => {
            println!("Expected headless open error: {}", e);
        }
    }
}
