use super::*;
use serde_json::json;

fn test_tool() -> DeviceInventoryTool {
    let path = std::env::temp_dir().join(format!(
        "openz_device_inventory_{}.json",
        uuid::Uuid::new_v4()
    ));
    DeviceInventoryTool::with_path(path)
}

#[tokio::test]
async fn record_successful_default_open_learns_target_type() -> Result<()> {
    let path = std::env::temp_dir().join(format!(
        "openz_device_inventory_open_{}.json",
        uuid::Uuid::new_v4()
    ));

    let id = record_successful_default_open_at(path.clone(), "/tmp/render.png")?;
    assert_eq!(id.as_deref(), Some("system-default-image_viewer-png"));

    let tool = DeviceInventoryTool::with_path(path);
    let result = tool
        .call(&json!({
            "action": "suggest",
            "category": "image_viewer",
            "target": "/tmp/another.png"
        }))
        .await?;

    assert_eq!(
        result["suggestions"][0]["capability"]["command"],
        "system-default"
    );
    assert_eq!(result["suggestions"][0]["capability"]["success_count"], 1);
    Ok(())
}

#[tokio::test]
async fn suggest_prefers_successful_matching_capability() -> Result<()> {
    let tool = test_tool();

    tool.call(&json!({
        "action": "add",
        "id": "firefox-image",
        "category": "image_viewer",
        "name": "Firefox",
        "command": "firefox",
        "args": ["{path}"],
        "works_for": [".png", ".jpg"]
    }))
    .await?;
    tool.call(&json!({
        "action": "add",
        "id": "vlc-video",
        "category": "video_player",
        "name": "VLC",
        "command": "vlc",
        "args": ["{path}"],
        "works_for": [".mp4"]
    }))
    .await?;
    tool.call(&json!({
        "action": "record_success",
        "id": "firefox-image"
    }))
    .await?;

    let result = tool
        .call(&json!({
            "action": "suggest",
            "category": "image_viewer",
            "target": "/tmp/render.png"
        }))
        .await?;

    assert_eq!(result["status"], "success");
    assert_eq!(
        result["suggestions"][0]["capability"]["id"],
        "firefox-image"
    );
    Ok(())
}

#[tokio::test]
async fn update_and_delete_capability() -> Result<()> {
    let tool = test_tool();
    tool.call(&json!({
        "action": "add",
        "id": "viewer",
        "category": "image_viewer",
        "name": "Viewer",
        "command": "xdg-open"
    }))
    .await?;

    let updated = tool
        .call(&json!({
            "action": "update",
            "id": "viewer",
            "works_for": [".webp"],
            "enabled": false
        }))
        .await?;
    assert_eq!(updated["capability"]["enabled"], false);
    assert_eq!(updated["capability"]["works_for"][0], ".webp");

    let deleted = tool
        .call(&json!({
            "action": "delete",
            "id": "viewer"
        }))
        .await?;
    assert_eq!(deleted["deleted"], "viewer");
    Ok(())
}
