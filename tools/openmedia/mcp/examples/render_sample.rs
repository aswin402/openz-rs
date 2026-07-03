use openmedia_video::{
    VideoScene, Scene, SceneElement, Position, DimensionValue,
    CustomFontSpec, SceneTransition, TransitionType, Keyframe, ElementTimeline, Size
};
use std::path::Path;

#[tokio::main]
async fn main() {
    let output_path = Path::new("openmedia_sample.mp4");
    
    // Construct Custom Font Spec
    let fonts = vec![CustomFontSpec {
        family: "RobotoRegular".to_string(),
        src: "https://github.com/google/fonts/raw/main/ofl/roboto/Roboto-Regular.ttf".to_string(),
    }];

    // Read and base64 encode logo.svg
    let logo_svg_bytes = std::fs::read("assets/logo.svg").expect("Failed to read assets/logo.svg");
    let logo_base64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, logo_svg_bytes);

    // Global Floating Glow Matte Black Background
    let animated_background = SceneElement::Html {
        content: r#"
<style>
  @keyframes glow-move-1 {
    0%, 100% { transform: translate(0px, 0px) scale(1); }
    50% { transform: translate(300px, 150px) scale(1.3); }
  }
  @keyframes glow-move-2 {
    0%, 100% { transform: translate(0px, 0px) scale(1.2); }
    50% { transform: translate(-250px, -200px) scale(0.9); }
  }
  .bg-canvas {
    position: absolute;
    width: 1280px;
    height: 720px;
    background: #030303;
    overflow: hidden;
    top: 0;
    left: 0;
    z-index: -10;
  }
  .glow-spot {
    position: absolute;
    border-radius: 50%;
    filter: blur(120px);
    opacity: 0.18;
  }
  .glow-spot-1 {
    width: 500px;
    height: 500px;
    background: #00f2fe;
    top: -100px;
    left: -100px;
    animation: glow-move-1 20s ease-in-out infinite;
  }
  .glow-spot-2 {
    width: 600px;
    height: 600px;
    background: #ea580c;
    bottom: -150px;
    right: -150px;
    animation: glow-move-2 25s ease-in-out infinite;
  }
</style>
<div class="bg-canvas">
  <div class="glow-spot glow-spot-1"></div>
  <div class="glow-spot glow-spot-2"></div>
</div>
"#.to_string(),
        position: Position {
            x: DimensionValue::Pixels(0.0),
            y: DimensionValue::Pixels(0.0),
        },
        size: Size {
            width: DimensionValue::Pixels(1280.0),
            height: DimensionValue::Pixels(720.0),
        },
        timeline: None,
    };

    // --- Slide 0: Dependency Hell (0s - 5s) ---
    let slide0 = Scene {
        id: "slide_0".to_string(),
        start: 0.0,
        end: 5.0,
        elements: vec![
            animated_background.clone(),
            SceneElement::Html {
                content: r#"
<style>
  .hell-container {
    width: 100%;
    height: 100%;
    color: #f4f4f5;
    font-family: 'RobotoRegular', monospace;
    padding: 30px;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
  }
  .window {
    background: #09090b;
    border: 1px solid #27272a;
    border-radius: 12px;
    box-shadow: 0 10px 40px rgba(0,0,0,0.8);
    flex-grow: 1;
    margin-bottom: 20px;
    padding: 20px;
    display: flex;
    flex-direction: column;
    font-size: 15px;
    line-height: 1.45;
    overflow: hidden;
  }
  .window-header {
    display: flex;
    align-items: center;
    margin-bottom: 15px;
    border-bottom: 1px solid #18181b;
    padding-bottom: 10px;
  }
  .dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    margin-right: 8px;
  }
  .dot.red { background: #ef4444; }
  .dot.yellow { background: #f59e0b; }
  .dot.green { background: #10b981; }
  .terminal-title {
    margin-left: 10px;
    color: #71717a;
    font-size: 13px;
  }
  .console {
    flex-grow: 1;
    color: #a1a1aa;
  }
  .cmd { color: #f4f4f5; }
  .err { color: #f87171; font-weight: bold; }
  .warn { color: #fbbf24; }
  .progress {
    color: #00f2fe;
  }
  .footer-text {
    display: flex;
    justify-content: space-around;
    font-weight: 800;
    font-size: 32px;
    letter-spacing: 2px;
    text-transform: uppercase;
  }
  .text-red {
    color: #ef4444;
    text-shadow: 0 0 10px rgba(239, 68, 68, 0.4);
  }
</style>
<div class="hell-container">
  <div class="window">
    <div class="window-header">
      <div class="dot red"></div>
      <div class="dot yellow"></div>
      <div class="dot green"></div>
      <div class="terminal-title">bash - dependency-hell</div>
    </div>
    <div class="console">
      <div><span class="cmd">$ pip install torch diffusers</span></div>
      <div>Downloading torch-2.1.2-cp310-manylinux1_x86_64.whl (755.4 MB)</div>
      <div class="progress">[████████████░░░░░░░░░░░] 34% (12.3 MB/s)</div>
      <div><span class="cmd">$ npm install remotion</span></div>
      <div class="warn">npm WARN deprecated inflight@1.0.6: Please look at glob...</div>
      <div class="err">npm ERR! code ERESOLVE</div>
      <div class="err">npm ERR! dependency conflict: peer conflicts detected!</div>
      <div><span class="cmd">$ python setup.py</span></div>
      <div class="err">Traceback (most recent call last):</div>
      <div class="err">  File "setup.py", line 12, in &lt;module&gt; import torch</div>
      <div class="err">ImportError: libgomp.so.1: cannot open shared object file</div>
    </div>
  </div>
  <div class="footer-text">
    <span class="text-red">Too Heavy.</span>
    <span class="text-red">Too Complex.</span>
    <span class="text-red">Too Slow.</span>
  </div>
</div>
"#.to_string(),
                position: Position {
                    x: DimensionValue::Pixels(140.0),
                    y: DimensionValue::Pixels(80.0),
                },
                size: Size {
                    width: DimensionValue::Pixels(1000.0),
                    height: DimensionValue::Pixels(560.0),
                },
                timeline: Some(ElementTimeline {
                    keyframes: vec![
                        Keyframe {
                            time: 0.0,
                            opacity: Some(0.0),
                            x: Some("0".to_string()),
                            y: Some("50".to_string()),
                            scale: None,
                            scale_x: None,
                            scale_y: None,
                            rotation: None,
                            easing: Some("ease_out".to_string()),
                        },
                        Keyframe {
                            time: 1.0,
                            opacity: Some(1.0),
                            x: Some("0".to_string()),
                            y: Some("0".to_string()),
                            scale: None,
                            scale_x: None,
                            scale_y: None,
                            rotation: None,
                            easing: None,
                        },
                    ],
                }),
            },
        ],
    };

    // --- Slide 1: Logo Emerge (5s - 10s) ---
    let slide1 = Scene {
        id: "slide_1".to_string(),
        start: 5.0,
        end: 10.0,
        elements: vec![
            animated_background.clone(),
            SceneElement::Html {
                content: format!(
                    r#"
<style>
  @keyframes rotate-slow {{
    0% {{ transform: rotate(0deg); }}
    100% {{ transform: rotate(360deg); }}
  }}
  .emerge-container {{
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    font-family: 'RobotoRegular', sans-serif;
    color: #ffffff;
    box-sizing: border-box;
    position: relative;
  }}
  .particle-grid {{
    position: absolute;
    width: 400px;
    height: 400px;
    border: 2px dashed rgba(234, 88, 12, 0.2);
    border-radius: 50%;
    animation: rotate-slow 30s linear infinite;
    z-index: 1;
  }}
  .particle-grid-inner {{
    position: absolute;
    width: 300px;
    height: 300px;
    border: 1px dashed rgba(0, 242, 254, 0.25);
    border-radius: 50%;
    animation: rotate-slow 15s linear infinite reverse;
    z-index: 1;
  }}
  .logo-wrapper {{
    z-index: 5;
    display: flex;
    flex-direction: column;
    align-items: center;
  }}
  .logo-img {{
    width: 140px;
    height: 140px;
    margin-bottom: 25px;
    filter: drop-shadow(0 0 20px rgba(0, 242, 254, 0.5)) drop-shadow(0 0 10px rgba(234, 88, 12, 0.4));
  }}
  .brand-title {{
    font-size: 52px;
    font-weight: 800;
    letter-spacing: 2px;
    background: linear-gradient(90deg, #00f2fe, #ea580c);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    margin-bottom: 10px;
  }}
  .brand-sub {{
    font-size: 20px;
    color: #e4e4e7;
    letter-spacing: 1px;
    font-weight: 300;
  }}
</style>
<div class="emerge-container">
  <div class="particle-grid"></div>
  <div class="particle-grid-inner"></div>
  <div class="logo-wrapper">
    <img class="logo-img" src="data:image/svg+xml;base64,{}" />
    <div class="brand-title">OpenMedia-RS</div>
    <div class="brand-sub">AI Media Creation Engine</div>
  </div>
</div>
"#,
                    logo_base64
                ),
                position: Position {
                    x: DimensionValue::Pixels(240.0),
                    y: DimensionValue::Pixels(80.0),
                },
                size: Size {
                    width: DimensionValue::Pixels(800.0),
                    height: DimensionValue::Pixels(560.0),
                },
                timeline: Some(ElementTimeline {
                    keyframes: vec![
                        Keyframe {
                            time: 0.0,
                            opacity: Some(0.0),
                            x: None,
                            y: Some("50".to_string()),
                            scale: Some(0.85),
                            scale_x: None,
                            scale_y: None,
                            rotation: None,
                            easing: Some("ease_out".to_string()),
                        },
                        Keyframe {
                            time: 1.5,
                            opacity: Some(1.0),
                            x: None,
                            y: Some("0".to_string()),
                            scale: Some(1.0),
                            scale_x: None,
                            scale_y: None,
                            rotation: None,
                            easing: None,
                        },
                    ],
                }),
            },
        ],
    };

    // --- Slide 2: Tool Cards Executing (10s - 15s) ---
    let slide2 = Scene {
        id: "slide_2".to_string(),
        start: 10.0,
        end: 15.0,
        elements: vec![
            animated_background.clone(),
            // Left Panel (MCP Tool Cards)
            SceneElement::Html {
                content: r#"
<style>
  .cards-panel {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 15px;
    font-family: 'RobotoRegular', sans-serif;
    justify-content: center;
  }
  .tool-card {
    background: rgba(24, 24, 27, 0.7);
    border: 1px solid rgba(63, 63, 70, 0.4);
    border-radius: 8px;
    padding: 15px 20px;
    display: flex;
    align-items: center;
    color: #e4e4e7;
    font-weight: 600;
    font-size: 15px;
    box-shadow: 0 4px 10px rgba(0,0,0,0.3);
  }
  .tool-card.active-blue {
    border-color: #00f2fe;
    background: rgba(0, 242, 254, 0.08);
    box-shadow: 0 0 15px rgba(0, 242, 254, 0.2);
    color: #00f2fe;
  }
  .tool-card.active-orange {
    border-color: #ea580c;
    background: rgba(234, 88, 12, 0.08);
    box-shadow: 0 0 15px rgba(234, 88, 12, 0.2);
    color: #ea580c;
  }
  .bullet-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    margin-right: 15px;
    background: #52525b;
  }
  .tool-card.active-blue .bullet-dot {
    background: #00f2fe;
    box-shadow: 0 0 8px #00f2fe;
  }
  .tool-card.active-orange .bullet-dot {
    background: #ea580c;
    box-shadow: 0 0 8px #ea580c;
  }
</style>
<div class="cards-panel">
  <div class="tool-card active-blue">
    <div class="bullet-dot"></div>
    generate_image
  </div>
  <div class="tool-card active-orange">
    <div class="bullet-dot"></div>
    create_svg
  </div>
  <div class="tool-card active-blue">
    <div class="bullet-dot"></div>
    create_chart
  </div>
  <div class="tool-card active-orange">
    <div class="bullet-dot"></div>
    animate_svg
  </div>
  <div class="tool-card active-blue">
    <div class="bullet-dot"></div>
    video_create
  </div>
  <div class="tool-card active-orange">
    <div class="bullet-dot"></div>
    diagram_generate_mermaid
  </div>
</div>
"#.to_string(),
                position: Position {
                    x: DimensionValue::Pixels(80.0),
                    y: DimensionValue::Pixels(80.0),
                },
                size: Size {
                    width: DimensionValue::Pixels(480.0),
                    height: DimensionValue::Pixels(560.0),
                },
                timeline: Some(ElementTimeline {
                    keyframes: vec![
                        Keyframe {
                            time: 0.0,
                            opacity: Some(0.0),
                            x: Some("-100".to_string()),
                            y: None,
                            scale: None,
                            scale_x: None,
                            scale_y: None,
                            rotation: None,
                            easing: Some("ease_out".to_string()),
                        },
                        Keyframe {
                            time: 1.0,
                            opacity: Some(1.0),
                            x: Some("0".to_string()),
                            y: None,
                            scale: None,
                            scale_x: None,
                            scale_y: None,
                            rotation: None,
                            easing: None,
                        },
                    ],
                }),
            },
            // Right Panel (Visual Preview Frame)
            SceneElement::Html {
                content: r#"
<style>
  .preview-panel {
    width: 100%;
    height: 100%;
    background: rgba(24, 24, 27, 0.4);
    border: 1px solid rgba(63, 63, 70, 0.3);
    border-radius: 12px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 20px;
    box-sizing: border-box;
    position: relative;
    box-shadow: 0 10px 30px rgba(0,0,0,0.5);
  }
  .preview-box {
    width: 100%;
    height: 100%;
    border-radius: 8px;
    background: #030303;
    border: 1px solid #27272a;
    box-shadow: inset 0 0 20px rgba(0,0,0,0.8);
  }
  .label {
    position: absolute;
    top: 15px;
    left: 20px;
    font-size: 13px;
    color: #71717a;
    font-family: sans-serif;
    text-transform: uppercase;
    letter-spacing: 1px;
  }
</style>
<div class="preview-panel">
  <div class="label">Interactive Output Preview</div>
  <div class="preview-box"></div>
</div>
"#.to_string(),
                position: Position {
                    x: DimensionValue::Pixels(620.0),
                    y: DimensionValue::Pixels(80.0),
                },
                size: Size {
                    width: DimensionValue::Pixels(580.0),
                    height: DimensionValue::Pixels(560.0),
                },
                timeline: Some(ElementTimeline {
                    keyframes: vec![
                        Keyframe {
                            time: 0.5,
                            opacity: Some(0.0),
                            x: Some("100".to_string()),
                            y: None,
                            scale: None,
                            scale_x: None,
                            scale_y: None,
                            rotation: None,
                            easing: Some("ease_out".to_string()),
                        },
                        Keyframe {
                            time: 1.5,
                            opacity: Some(1.0),
                            x: Some("0".to_string()),
                            y: None,
                            scale: None,
                            scale_x: None,
                            scale_y: None,
                            rotation: None,
                            easing: None,
                        },
                    ],
                }),
            },
            // Overlay Radar Chart inside Preview Box
            SceneElement::Chart {
                chart_type: "radar".to_string(),
                data: serde_json::json!([
                    { "label": "Images", "value": 75.0 },
                    { "label": "Videos", "value": 85.0 },
                    { "label": "SVGs", "value": 90.0 },
                    { "label": "Charts", "value": 95.0 },
                    { "label": "Diagrams", "value": 80.0 }
                ]),
                position: Position {
                    x: DimensionValue::Pixels(660.0),
                    y: DimensionValue::Pixels(150.0),
                },
                size: Size {
                    width: DimensionValue::Pixels(500.0),
                    height: DimensionValue::Pixels(420.0),
                },
                theme: "dark".to_string(),
                timeline: Some(ElementTimeline {
                    keyframes: vec![
                        Keyframe {
                            time: 0.8,
                            opacity: Some(0.0),
                            x: None,
                            y: None,
                            scale: Some(0.5),
                            scale_x: None,
                            scale_y: None,
                            rotation: None,
                            easing: None,
                        },
                        Keyframe {
                            time: 1.8,
                            opacity: Some(1.0),
                            x: None,
                            y: None,
                            scale: Some(1.0),
                            scale_x: None,
                            scale_y: None,
                            rotation: None,
                            easing: Some("ease_out".to_string()),
                        },
                    ],
                }),
            },
        ],
    };

    // --- Slide 3: Tell Your Agent (15s - 20s) ---
    let slide3 = Scene {
        id: "slide_3".to_string(),
        start: 15.0,
        end: 20.0,
        elements: vec![
            animated_background.clone(),
            SceneElement::Html {
                content: r#"
<style>
  .agent-container {
    width: 100%;
    height: 100%;
    padding: 30px;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    font-family: 'RobotoRegular', sans-serif;
    color: #ffffff;
    justify-content: space-between;
  }
  .prompt-bar {
    background: #09090b;
    border: 1px solid #27272a;
    border-radius: 10px;
    padding: 15px 25px;
    font-size: 18px;
    font-weight: 500;
    color: #e4e4e7;
    box-shadow: 0 4px 15px rgba(0,0,0,0.5);
    display: flex;
    align-items: center;
  }
  .prompt-prefix {
    color: #00f2fe;
    margin-right: 15px;
    font-weight: bold;
  }
  .flow-nodes {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-grow: 1;
    margin: 30px 0;
    padding: 0 40px;
    position: relative;
  }
  .flow-line {
    position: absolute;
    height: 2px;
    background: linear-gradient(90deg, #00f2fe, #ea580c);
    left: 100px;
    right: 100px;
    top: 50%;
    z-index: 1;
  }
  .node {
    width: 130px;
    height: 130px;
    border-radius: 50%;
    background: #18181b;
    border: 2px solid #3f3f46;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    font-size: 13px;
    font-weight: bold;
    color: #a1a1aa;
    z-index: 2;
    box-shadow: 0 8px 25px rgba(0,0,0,0.5);
  }
  .node.active {
    border-color: #00f2fe;
    color: #00f2fe;
    box-shadow: 0 0 25px rgba(0, 242, 254, 0.4);
    background: rgba(0, 242, 254, 0.05);
  }
  .node.active-orange {
    border-color: #ea580c;
    color: #ea580c;
    box-shadow: 0 0 25px rgba(234, 88, 12, 0.4);
    background: rgba(234, 88, 12, 0.05);
  }
  .node-icon {
    width: 32px;
    height: 32px;
    margin-bottom: 8px;
    fill: currentColor;
  }
  .footer-banner {
    font-size: 28px;
    font-weight: 800;
    text-align: center;
    background: linear-gradient(90deg, #ffffff, #a1a1aa);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
  }
</style>
<div class="agent-container">
  <div class="prompt-bar">
    <span class="prompt-prefix">Agent:</span>
    Create a product launch video
  </div>
  <div class="flow-nodes">
    <div class="flow-line"></div>
    <div class="node active">
      <svg class="node-icon" viewBox="0 0 24 24"><path d="M19 3H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zm0 16H5V5h14v14z"/></svg>
      html_to_image
    </div>
    <div class="node active-orange">
      <svg class="node-icon" viewBox="0 0 24 24"><path d="M16.5 12c1.38 0 2.49-1.12 2.49-2.5S17.88 7 16.5 7C15.12 7 14 8.12 14 9.5s1.12 2.5 2.5 2.5zM9 11c1.66 0 3-1.34 3-3s-1.34-3-3-3-3 1.34-3 3 1.34 3 3 3zm7.5 3c-1.83 0-5.5.92-5.5 2.75V19h11v-2.25c0-1.83-3.67-2.75-5.5-2.75z"/></svg>
      animate_svg
    </div>
    <div class="node active">
      <svg class="node-icon" viewBox="0 0 24 24"><path d="M17 10.5V7c0-.55-.45-1-1-1H4c-.55 0-1 .45-1 1v10c0 .55.45 1 1 1h12c.55 0 1-.45 1-1v-3.5l4 4v-11l-4 4z"/></svg>
      video_create
    </div>
    <div class="node active-orange">
      <svg class="node-icon" viewBox="0 0 24 24"><path d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"/></svg>
      video_audio
    </div>
  </div>
  <div class="footer-banner">
    Tell your agent what you want. It handles the rest.
  </div>
</div>
"#.to_string(),
                position: Position {
                    x: DimensionValue::Pixels(140.0),
                    y: DimensionValue::Pixels(80.0),
                },
                size: Size {
                    width: DimensionValue::Pixels(1000.0),
                    height: DimensionValue::Pixels(560.0),
                },
                timeline: Some(ElementTimeline {
                    keyframes: vec![
                        Keyframe {
                            time: 0.0,
                            opacity: Some(0.0),
                            x: None,
                            y: Some("50".to_string()),
                            scale: Some(0.9),
                            scale_x: None,
                            scale_y: None,
                            rotation: None,
                            easing: Some("ease_out".to_string()),
                        },
                        Keyframe {
                            time: 1.0,
                            opacity: Some(1.0),
                            x: None,
                            y: Some("0".to_string()),
                            scale: Some(1.0),
                            scale_x: None,
                            scale_y: None,
                            rotation: None,
                            easing: None,
                        },
                    ],
                }),
            },
        ],
    };

    // --- Slide 4: Comparison (20s - 25s) ---
    let slide4 = Scene {
        id: "slide_4".to_string(),
        start: 20.0,
        end: 25.0,
        elements: vec![
            animated_background.clone(),
            SceneElement::Html {
                content: r#"
<style>
  .comp-container {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    font-family: 'RobotoRegular', sans-serif;
    color: #ffffff;
  }
  .columns {
    display: flex;
    justify-content: space-between;
    flex-grow: 1;
    gap: 30px;
    margin-bottom: 20px;
  }
  .col {
    width: 50%;
    background: rgba(24, 24, 27, 0.4);
    border: 1px solid rgba(63, 63, 70, 0.3);
    border-radius: 12px;
    padding: 25px 30px;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    box-shadow: 0 10px 30px rgba(0,0,0,0.5);
  }
  .col.highlight {
    border: 1px solid #00f2fe;
    background: rgba(0, 242, 254, 0.03);
    box-shadow: 0 0 25px rgba(0, 242, 254, 0.15);
  }
  .col-title {
    font-size: 22px;
    font-weight: 700;
    margin-bottom: 20px;
    letter-spacing: 1px;
    text-transform: uppercase;
  }
  .col.highlight .col-title {
    color: #00f2fe;
  }
  .col.normal .col-title {
    color: #f87171;
  }
  .item-list {
    display: flex;
    flex-direction: column;
    gap: 15px;
    flex-grow: 1;
  }
  .item {
    font-size: 17px;
    color: #d4d4d8;
    display: flex;
    align-items: center;
  }
  .item::before {
    content: "•";
    margin-right: 15px;
    font-size: 20px;
  }
  .col.highlight .item::before {
    color: #ea580c;
  }
  .col.normal .item::before {
    color: #ef4444;
  }
  .mem-box {
    margin-top: 15px;
    padding-top: 15px;
    border-top: 1px solid rgba(63, 63, 70, 0.4);
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 19px;
    font-weight: bold;
  }
  .col.highlight .mem-box {
    color: #ea580c;
  }
  .col.normal .mem-box {
    color: #f87171;
  }
  .comp-footer {
    font-size: 26px;
    font-weight: bold;
    text-align: center;
    color: #e4e4e7;
    letter-spacing: 0.5px;
  }
</style>
<div class="comp-container">
  <div class="columns">
    <div class="col normal">
      <div class="col-title">Traditional Stack</div>
      <div class="item-list">
        <div class="item">Python & PyTorch</div>
        <div class="item">Diffusers & Models (5GB+)</div>
        <div class="item">Node.js & Remotion</div>
        <div class="item">System FFmpeg installations</div>
        <div class="item">Complex Virtual Envs</div>
      </div>
      <div class="mem-box">
        <span>Disk / Memory:</span>
        <span>8GB+</span>
      </div>
    </div>
    <div class="col highlight">
      <div class="col-title">OpenMedia-RS</div>
      <div class="item-list">
        <div class="item">Single Compiled Binary</div>
        <div class="item">Rust Native Architecture</div>
        <div class="item">100% Offline Rendering</div>
        <div class="item">Standard MCP Interface</div>
        <div class="item">Zero-Configuration Setup</div>
      </div>
      <div class="mem-box">
        <span>Disk / Memory:</span>
        <span>&lt; 1GB</span>
      </div>
    </div>
  </div>
  <div class="comp-footer">
    Built in Rust. Offline. Lightweight.
  </div>
</div>
"#.to_string(),
                position: Position {
                    x: DimensionValue::Pixels(140.0),
                    y: DimensionValue::Pixels(80.0),
                },
                size: Size {
                    width: DimensionValue::Pixels(1000.0),
                    height: DimensionValue::Pixels(560.0),
                },
                timeline: Some(ElementTimeline {
                    keyframes: vec![
                        Keyframe {
                            time: 0.0,
                            opacity: Some(0.0),
                            x: Some("0".to_string()),
                            y: Some("50".to_string()),
                            scale: Some(0.95),
                            scale_x: None,
                            scale_y: None,
                            rotation: None,
                            easing: Some("ease_out".to_string()),
                        },
                        Keyframe {
                            time: 1.0,
                            opacity: Some(1.0),
                            x: Some("0".to_string()),
                            y: Some("0".to_string()),
                            scale: Some(1.0),
                            scale_x: None,
                            scale_y: None,
                            rotation: None,
                            easing: None,
                        },
                    ],
                }),
            },
        ],
    };

    // --- Slide 5: Outro Hero Card (25s - 30s) ---
    let slide5 = Scene {
        id: "slide_5".to_string(),
        start: 25.0,
        end: 30.0,
        elements: vec![
            animated_background.clone(),
            SceneElement::Html {
                content: format!(
                    r#"
<style>
  .outro-container {{
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    font-family: 'RobotoRegular', sans-serif;
    color: #ffffff;
    text-align: center;
    box-sizing: border-box;
  }}
  .outro-logo {{
    width: 90px;
    height: 90px;
    margin-bottom: 20px;
    filter: drop-shadow(0 0 15px rgba(0, 242, 254, 0.4));
  }}
  .outro-title {{
    font-size: 46px;
    font-weight: 800;
    margin-bottom: 8px;
    background: linear-gradient(90deg, #00f2fe, #ea580c);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    letter-spacing: 2px;
  }}
  .outro-subtitle {{
    font-size: 19px;
    color: #a1a1aa;
    margin-bottom: 25px;
    letter-spacing: 0.5px;
  }}
  .features-grid {{
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 15px;
    width: 100%;
    max-width: 760px;
    margin-bottom: 30px;
  }}
  .feature-item {{
    background: rgba(24, 24, 27, 0.6);
    border: 1px solid rgba(63, 63, 70, 0.4);
    border-radius: 8px;
    padding: 12px;
    font-size: 13px;
    font-weight: 600;
    color: #e4e4e7;
  }}
  .feature-item.accent {{
    border-color: #ea580c;
    color: #ea580c;
    background: rgba(234, 88, 12, 0.05);
  }}
  .feature-item.accent-blue {{
    border-color: #00f2fe;
    color: #00f2fe;
    background: rgba(0, 242, 254, 0.05);
  }}
  .github-cta {{
    font-size: 22px;
    font-weight: bold;
    color: #00f2fe;
    margin-bottom: 10px;
  }}
  .license-info {{
    font-size: 13px;
    color: #71717a;
  }}
</style>
<div class="outro-container">
  <img class="outro-logo" src="data:image/svg+xml;base64,{}" />
  <div class="outro-title">OpenMedia-RS</div>
  <div class="outro-subtitle">The media engine for AI agents.</div>
  
  <div class="features-grid">
    <div class="feature-item accent-blue">Images</div>
    <div class="feature-item accent">Videos</div>
    <div class="feature-item accent-blue">Animations</div>
    <div class="feature-item accent">Diagrams</div>
    <div class="feature-item">Offline</div>
    <div class="feature-item">Fast</div>
    <div class="feature-item">Rust Native</div>
    <div class="feature-item">MCP Ready</div>
  </div>
  
  <div class="github-cta">github.com/openmedia-rs</div>
  <div class="license-info">Open Source • MIT / Apache 2.0</div>
</div>
"#,
                    logo_base64
                ),
                position: Position {
                    x: DimensionValue::Pixels(140.0),
                    y: DimensionValue::Pixels(80.0),
                },
                size: Size {
                    width: DimensionValue::Pixels(1000.0),
                    height: DimensionValue::Pixels(560.0),
                },
                timeline: Some(ElementTimeline {
                    keyframes: vec![
                        Keyframe {
                            time: 0.0,
                            opacity: Some(0.0),
                            x: Some("0".to_string()),
                            y: Some("-50".to_string()),
                            scale: None,
                            scale_x: None,
                            scale_y: None,
                            rotation: None,
                            easing: Some("ease_out".to_string()),
                        },
                        Keyframe {
                            time: 1.0,
                            opacity: Some(1.0),
                            x: Some("0".to_string()),
                            y: Some("0".to_string()),
                            scale: None,
                            scale_x: None,
                            scale_y: None,
                            rotation: None,
                            easing: None,
                        },
                    ],
                }),
            },
        ],
    };

    // Transitions definitions
    let transitions = vec![
        SceneTransition {
            from: "slide_0".to_string(),
            to: "slide_1".to_string(),
            transition_type: TransitionType::Blur,
            duration: 0.5,
            easing: Some("ease_in_out".to_string()),
        },
        SceneTransition {
            from: "slide_1".to_string(),
            to: "slide_2".to_string(),
            transition_type: TransitionType::Glitch,
            duration: 0.5,
            easing: Some("ease_in_out".to_string()),
        },
        SceneTransition {
            from: "slide_2".to_string(),
            to: "slide_3".to_string(),
            transition_type: TransitionType::RadialWipe,
            duration: 0.5,
            easing: Some("ease_in_out".to_string()),
        },
        SceneTransition {
            from: "slide_3".to_string(),
            to: "slide_4".to_string(),
            transition_type: TransitionType::SlideLeft,
            duration: 0.5,
            easing: Some("ease_in_out".to_string()),
        },
        SceneTransition {
            from: "slide_4".to_string(),
            to: "slide_5".to_string(),
            transition_type: TransitionType::SlideUp,
            duration: 0.5,
            easing: Some("ease_in_out".to_string()),
        },
    ];

    let scene = VideoScene {
        width: 1280,
        height: 720,
        fps: 15,
        duration: 30.0,
        background: "#030303".to_string(),
        scenes: vec![slide0, slide1, slide2, slide3, slide4, slide5],
        transitions,
        audio: None,
        custom_fonts: Some(fonts),
    };

    println!("Starting render of openmedia_sample.mp4 (30s, 15fps, 1280x720)...");
    let spec = openmedia_video::render_video_scene(&scene, output_path).await.unwrap();
    println!("SUCCESS: Video generated at {:?}", spec.path);
}
