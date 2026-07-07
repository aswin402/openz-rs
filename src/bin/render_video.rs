use anyhow::Result;
use openz::tools::html_video::HtmlToVideoTool;
use openz::tools::Tool;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let tool = HtmlToVideoTool;
    let args = json!({
        "html_path": "/home/aswin/openz_intro_video.html",
        "output_path": "/home/aswin/openz_intro.mp4",
        "width": 1920,
        "height": 1080,
        "fps": 30,
        "duration_seconds": 35.0,
        "tick_js": "if(window.setFrame) window.setFrame({frame});",
        "settle_ms": 30,
        "load_delay_ms": 1500
    });

    println!("Rendering HTML animation timeline to video '/home/aswin/openz_intro.mp4' (duration: 35s, 30fps)...");
    let res = tool.call(&args).await?;
    println!("Render completed! Result: {:?}", res);

    Ok(())
}
