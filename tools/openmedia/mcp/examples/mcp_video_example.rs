use openmedia_core::Config;
use openmedia_mcp::{OpenMediaServer, Parameters, VideoCreateRequest};
use serde_json::json;

#[tokio::main]
async fn main() {
    println!("Initializing OpenMediaServer...");
    let mut config = Config::default();
    
    // Set local paths
    let temp_dir = std::env::temp_dir();
    config.paths.model_dir = temp_dir.join("openmedia_models");
    config.paths.output_dir = std::env::current_dir().unwrap(); // Save output in current project directory
    config.paths.history_db = temp_dir.join("openmedia_history.db");

    let server = OpenMediaServer::new(config).await.unwrap();

    println!("Constructing VideoScene JSON for video_create MCP tool...");
    let scene_json = json!({
        "width": 1280,
        "height": 720,
        "fps": 15,
        "duration": 5.0,
        "background": "#1a1a2e",
        "scenes": [
            {
                "id": "slide_0",
                "start": 0.0,
                "end": 5.0,
                "elements": [
                    // Animated background shape
                    {
                        "type": "shape",
                        "shape": "rect",
                        "size": { "width": "100%", "height": "100%" },
                        "position": { "x": 0.0, "y": 0.0 },
                        "style": {
                            "fill": "#162447",
                            "opacity": 0.5
                        }
                    },
                    // Text title moving and fading in
                    {
                        "type": "text",
                        "content": "MCP Native Video Rendering",
                        "style": {
                            "font_family": "sans-serif",
                            "font_size": 42.0,
                            "font_weight": 700,
                            "color": "#00adb5",
                            "text_align": "center"
                        },
                        "position": { "x": 640.0, "y": 150.0 },
                        "anchor": "center",
                        "timeline": {
                            "keyframes": [
                                { "time": 0.0, "opacity": 0.0, "y": "-50" },
                                { "time": 1.5, "opacity": 1.0, "y": "0" }
                            ]
                        }
                    },
                    // Card background shape
                    {
                        "type": "shape",
                        "shape": "rounded_rect",
                        "size": { "width": 450.0, "height": 220.0 },
                        "position": { "x": 100.0, "y": 280.0 },
                        "style": {
                            "fill": "#252b48",
                            "stroke": "#00adb5",
                            "stroke_width": 2.0,
                            "border_radius": 12.0,
                            "opacity": 0.9
                        },
                        "timeline": {
                            "keyframes": [
                                { "time": 1.0, "opacity": 0.0, "scale": 0.8 },
                                { "time": 2.5, "opacity": 0.9, "scale": 1.0 }
                            ]
                        }
                    },
                    // Card text title
                    {
                        "type": "text",
                        "content": "Deterministic Animation",
                        "style": {
                            "font_family": "sans-serif",
                            "font_size": 24.0,
                            "font_weight": 700,
                            "color": "#00adb5",
                            "text_align": "center"
                        },
                        "position": { "x": 325.0, "y": 330.0 },
                        "anchor": "center",
                        "timeline": {
                            "keyframes": [
                                { "time": 1.2, "opacity": 0.0, "scale": 0.8 },
                                { "time": 2.7, "opacity": 1.0, "scale": 1.0 }
                            ]
                        }
                    },
                    // Card text body
                    {
                        "type": "text",
                        "content": "Driven frame-by-frame by the openmedia-rs engine.",
                        "style": {
                            "font_family": "sans-serif",
                            "font_size": 16.0,
                            "font_weight": 400,
                            "color": "#bbbbbb",
                            "text_align": "center"
                        },
                        "position": { "x": 325.0, "y": 410.0 },
                        "anchor": "center",
                        "timeline": {
                            "keyframes": [
                                { "time": 1.4, "opacity": 0.0 },
                                { "time": 2.9, "opacity": 1.0 }
                            ]
                        }
                    },
                    // Native vector chart
                    {
                        "type": "chart",
                        "chart_type": "bar",
                        "data": [
                            { "label": "Text", "value": 30.0 },
                            { "label": "Video", "value": 85.0 },
                            { "label": "Audio", "value": 45.0 }
                        ],
                        "position": { "x": 680.0, "y": 240.0 },
                        "size": { "width": 500.0, "height": 320.0 },
                        "theme": "dark",
                        "timeline": {
                            "keyframes": [
                                { "time": 1.5, "opacity": 0.0, "scale": 0.5 },
                                { "time": 3.0, "opacity": 1.0, "scale": 1.0 }
                            ]
                        }
                    }
                ]
            }
        ],
        "transitions": []
    });

    let request = VideoCreateRequest {
        scene: scene_json,
        output_path: Some("openmedia_mcp_example.mp4".to_string()),
    };

    println!("Invoking OpenMediaServer::video_create MCP method...");
    let result = server.video_create(Parameters(request)).await;

    match result {
        Ok(json_response) => {
            println!("SUCCESS: Example video created successfully!");
            println!("Response: {}", serde_json::to_string_pretty(&json_response.0).unwrap());
        }
        Err(err) => {
            eprintln!("ERROR: Failed to run video_create MCP method: {}", err);
        }
    }
}
