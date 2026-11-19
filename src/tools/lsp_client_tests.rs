use super::*;

#[tokio::test]
async fn test_lsp_frame_roundtrip_and_normalizers() -> Result<()> {
    let payload = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "contents": {
                "kind": "markdown",
                "value": "```rust\npub fn add(a: i32, b: i32) -> i32\n```"
            }
        }
    });

    let encoded = encode_lsp_frame(&payload)?;
    let mut reader = BufReader::new(std::io::Cursor::new(encoded));
    let decoded = read_lsp_frame(&mut reader).await?;
    assert_eq!(decoded["id"], 1);
    assert_eq!(
        extract_hover_markdown(&decoded["result"]),
        "```rust\npub fn add(a: i32, b: i32) -> i32\n```"
    );

    let ws_root = std::env::temp_dir().join("openz_lsp_uri_test");
    let sample_file = ws_root.join("src").join("lib.rs");
    let uri = url::Url::from_file_path(&sample_file).unwrap().to_string();
    let loc_payload = json!([
        {
            "targetUri": uri,
            "targetSelectionRange": {
                "start": { "line": 9, "character": 4 },
                "end": { "line": 9, "character": 15 }
            }
        }
    ]);
    let normalized = normalize_lsp_locations(&loc_payload, &ws_root);
    assert_eq!(normalized.len(), 1);
    assert_eq!(normalized[0]["file"], "src/lib.rs");
    assert_eq!(normalized[0]["line"], 10);
    assert_eq!(normalized[0]["character"], 5);

    let tool = LspQueryTool;
    let servers_res = tool.call(&json!({ "action": "servers" })).await?;
    assert_eq!(servers_res["status"], "success");
    assert!(servers_res["servers"].is_array());
    Ok(())
}
