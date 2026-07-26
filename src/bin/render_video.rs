use anyhow::Result;
use clap::Parser;
use openz::tools::html_video::HtmlToVideoTool;
use openz::tools::Tool;
use serde_json::json;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(about = "Render an HTML animation timeline to an MP4 video")]
struct Args {
    /// HTML file to render.
    #[arg(long, default_value = "openz_intro_video.html")]
    html: PathBuf,

    /// MP4 output path.
    #[arg(long, default_value = "openz_intro.mp4")]
    output: PathBuf,

    /// Output video width in pixels.
    #[arg(long, default_value_t = 1920)]
    width: u32,

    /// Output video height in pixels.
    #[arg(long, default_value_t = 1080)]
    height: u32,

    /// Frames per second.
    #[arg(long, default_value_t = 30)]
    fps: u32,

    /// Duration in seconds.
    #[arg(long, default_value_t = 35.0)]
    duration_seconds: f64,

    /// JavaScript executed before each frame capture. Use {frame} for frame index.
    #[arg(long, default_value = "if(window.setFrame) window.setFrame({frame});")]
    tick_js: String,

    /// Milliseconds to wait after each frame tick before capture.
    #[arg(long, default_value_t = 30)]
    settle_ms: u64,

    /// Milliseconds to wait after loading the page before rendering.
    #[arg(long, default_value_t = 1500)]
    load_delay_ms: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let cli = Args::parse();
    let html_path = cli.html.to_string_lossy().into_owned();
    let output_path = cli.output.to_string_lossy().into_owned();

    let tool = HtmlToVideoTool;
    let args = json!({
        "html_path": html_path,
        "output_path": output_path,
        "width": cli.width,
        "height": cli.height,
        "fps": cli.fps,
        "duration_seconds": cli.duration_seconds,
        "tick_js": cli.tick_js,
        "settle_ms": cli.settle_ms,
        "load_delay_ms": cli.load_delay_ms
    });

    println!(
        "Rendering HTML animation timeline to video '{}' (duration: {}s, {}fps)...",
        output_path, cli.duration_seconds, cli.fps
    );
    let res = tool.call(&args).await?;
    println!("Render completed! Result: {:?}", res);

    Ok(())
}
