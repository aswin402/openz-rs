use openmedia_video::{
    VideoScene, Scene, SceneElement, Position, DimensionValue,
    CustomFontSpec, SceneTransition, TransitionType, Keyframe, ElementTimeline, Size
};
use std::path::Path;

#[tokio::main]
async fn main() {
    let output_path = Path::new("openmedia_promo.mp4");
    
    // Construct Custom Font Spec
    let fonts = vec![CustomFontSpec {
        family: "RobotoRegular".to_string(),
        src: "https://github.com/google/fonts/raw/main/ofl/roboto/Roboto-Regular.ttf".to_string(),
    }];

    // Read and base64 encode logo.svg
    let logo_svg_bytes = std::fs::read("assets/logo.svg").expect("Failed to read assets/logo.svg");
    let logo_base64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, logo_svg_bytes);

    // Render Mermaid Graph for Slide 3
    let mermaid_code = "graph TD\n  Agent[LLM Agent] --> Engine[openmedia-rs]\n  Engine --> Vector[Vector Engine]\n  Engine --> Audio[Rayon Mixer]\n  Vector --> Render[MP4 Output]\n  Audio --> Render";
    let mermaid_svg = openmedia_svg::render_mermaid(mermaid_code, None, None)
        .unwrap_or_else(|e| format!("<svg><text y=\"20\">Mermaid Error: {}</text></svg>", e));

    let animated_background = SceneElement::Html {
        content: r#"
<style>
  @keyframes gradient-shift {
    0% { background-position: 0% 50%; }
    50% { background-position: 100% 50%; }
    100% { background-position: 0% 50%; }
  }
  .background-container {
    position: absolute;
    width: 1280px;
    height: 720px;
    background: linear-gradient(-45deg, #070a13, #0f172a, #1e1b4b, #0d111d);
    background-size: 400% 400%;
    animation: gradient-shift 15s ease infinite;
    top: 0;
    left: 0;
    z-index: -10;
  }
</style>
<div class="background-container"></div>
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

    let slide1_card = SceneElement::Html {
        content: r#"
<style>
  @keyframes pulse-glow {
    0%, 100% { filter: drop-shadow(0 0 2px #00f2fe); opacity: 0.8; }
    50% { filter: drop-shadow(0 0 8px #00f2fe); opacity: 1; }
  }
  .card {
    width: 100%;
    height: 100%;
    background: rgba(15, 23, 42, 0.4);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    border: 1px solid rgba(0, 242, 254, 0.15);
    border-radius: 20px;
    padding: 35px;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
    color: #eeeeee;
    font-family: 'RobotoRegular', sans-serif;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    justify-content: center;
  }
  .title {
    font-size: 32px;
    font-weight: 700;
    margin-bottom: 30px;
    background: linear-gradient(90deg, #00f2fe, #4facfe);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    letter-spacing: 1px;
  }
  .bullet-item {
    display: flex;
    align-items: center;
    margin-bottom: 25px;
    font-size: 20px;
    line-height: 1.5;
  }
  .icon-container {
    width: 44px;
    height: 44px;
    margin-right: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 242, 254, 0.1);
    border: 1px solid rgba(0, 242, 254, 0.3);
    border-radius: 50%;
    animation: pulse-glow 3s infinite ease-in-out;
    flex-shrink: 0;
  }
  .icon-svg {
    width: 22px;
    height: 22px;
    fill: #00f2fe;
  }
</style>
<div class="card">
  <div class="title">1. Vector Diagram Engine</div>
  <div class="bullet-item">
    <div class="icon-container">
      <svg class="icon-svg" viewBox="0 0 24 24"><path d="M19 3H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zm-2 10h-4v4h-2v-4H7v-2h4V7h2v4h4v2z"/></svg>
    </div>
    <div>Compile raw JSON arrays to styled vector shape canvas</div>
  </div>
  <div class="bullet-item">
    <div class="icon-container" style="animation-delay: 0.5s;">
      <svg class="icon-svg" viewBox="0 0 24 24"><path d="M19 3H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zm-2 10H7v-2h10v2z"/></svg>
    </div>
    <div>Dynamic mathematical bar, bezier-line, and pie charts</div>
  </div>
  <div class="bullet-item">
    <div class="icon-container" style="animation-delay: 1.0s;">
      <svg class="icon-svg" viewBox="0 0 24 24"><path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 17h-2v-2h2v2zm2.07-7.75l-.9.92C13.45 12.9 13 13.5 13 15h-2v-.5c0-1.1.45-2.1 1.17-2.83l1.24-1.26c.37-.36.59-.86.59-1.41 0-1.1-.9-2-2-2s-2 .9-2 2H7c0-2.76 2.24-5 5-5s5 2.24 5 5c0 1.04-.42 1.99-1.07 2.75z"/></svg>
    </div>
    <div>Render native Mermaid text graphs completely offline</div>
  </div>
</div>
"#.to_string(),
        position: Position {
            x: DimensionValue::Pixels(80.0),
            y: DimensionValue::Pixels(80.0),
        },
        size: Size {
            width: DimensionValue::Pixels(580.0),
            height: DimensionValue::Pixels(560.0),
        },
        timeline: Some(ElementTimeline {
            keyframes: vec![
                Keyframe {
                    time: 0.0,
                    opacity: Some(0.0),
                    x: Some("-150".to_string()),
                    y: Some("0".to_string()),
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
    };

    let slide1_right_card = SceneElement::Html {
        content: r#"
<style>
  .card-right {
    width: 100%;
    height: 100%;
    background: rgba(15, 23, 42, 0.4);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    border: 1px solid rgba(0, 242, 254, 0.15);
    border-radius: 20px;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
    box-sizing: border-box;
  }
</style>
<div class="card-right"></div>
"#.to_string(),
        position: Position {
            x: DimensionValue::Pixels(710.0),
            y: DimensionValue::Pixels(80.0),
        },
        size: Size {
            width: DimensionValue::Pixels(490.0),
            height: DimensionValue::Pixels(560.0),
        },
        timeline: Some(ElementTimeline {
            keyframes: vec![
                Keyframe {
                    time: 0.5,
                    opacity: Some(0.0),
                    x: None,
                    y: None,
                    scale: Some(0.2),
                    scale_x: None,
                    scale_y: None,
                    rotation: Some(-45.0),
                    easing: None,
                },
                Keyframe {
                    time: 2.0,
                    opacity: Some(1.0),
                    x: None,
                    y: None,
                    scale: Some(1.0),
                    scale_x: None,
                    scale_y: None,
                    rotation: Some(0.0),
                    easing: Some("ease_out".to_string()),
                },
            ],
        }),
    };

    let slide2_card = SceneElement::Html {
        content: r#"
<style>
  @keyframes pulse-glow-pink {
    0%, 100% { filter: drop-shadow(0 0 2px #f857a6); opacity: 0.8; }
    50% { filter: drop-shadow(0 0 8px #f857a6); opacity: 1; }
  }
  .card-pink {
    width: 100%;
    height: 100%;
    background: rgba(15, 23, 42, 0.4);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    border: 1px solid rgba(248, 87, 166, 0.15);
    border-radius: 20px;
    padding: 35px;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
    color: #eeeeee;
    font-family: 'RobotoRegular', sans-serif;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    justify-content: center;
  }
  .title-pink {
    font-size: 32px;
    font-weight: 700;
    margin-bottom: 30px;
    background: linear-gradient(90deg, #f857a6, #ff5858);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    letter-spacing: 1px;
  }
  .bullet-item {
    display: flex;
    align-items: center;
    margin-bottom: 25px;
    font-size: 20px;
    line-height: 1.5;
  }
  .icon-container-pink {
    width: 44px;
    height: 44px;
    margin-right: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(248, 87, 166, 0.1);
    border: 1px solid rgba(248, 87, 166, 0.3);
    border-radius: 50%;
    animation: pulse-glow-pink 3s infinite ease-in-out;
    flex-shrink: 0;
  }
  .icon-svg-pink {
    width: 22px;
    height: 22px;
    fill: #f857a6;
  }
</style>
<div class="card-pink">
  <div class="title-pink">2. Image Processing & Shaders</div>
  <div class="bullet-item">
    <div class="icon-container-pink">
      <svg class="icon-svg-pink" viewBox="0 0 24 24"><path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 17.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93zm6.9-2.54c-.26-.81-1-1.39-1.9-1.39h-1v-3c0-.55-.45-1-1-1H8v-2h2c.55 0 1-.45 1-1V7h2c1.1 0 2-.9 2-2v-.41c2.93 1.19 5 4.06 5 7.41 0 2.08-.8 3.97-2.1 5.39z"/></svg>
    </div>
    <div>GPU-accelerated WGSL compute shaders for ultra-fast filtering</div>
  </div>
  <div class="bullet-item">
    <div class="icon-container-pink" style="animation-delay: 0.5s;">
      <svg class="icon-svg-pink" viewBox="0 0 24 24"><path d="M12 22c5.52 0 10-4.48 10-10S17.52 2 12 2 2 6.48 2 12s4.48 10 10 10zm1-15h-2v6h2V7zm0 8h-2v2h2v-2z"/></svg>
    </div>
    <div>Multi-threaded CPU fallback processor using Rayon</div>
  </div>
  <div class="bullet-item">
    <div class="icon-container-pink" style="animation-delay: 1.0s;">
      <svg class="icon-svg-pink" viewBox="0 0 24 24"><path d="M19.35 10.04C18.67 6.59 15.64 4 12 4 9.11 4 6.6 5.64 5.35 8.04 2.34 8.36 0 10.91 0 14c0 3.31 2.69 6 6 6h13c2.76 0 5-2.24 5-5 0-2.64-2.05-4.78-4.65-4.96zM19 18H6c-2.21 0-4-1.79-4-4 0-2.05 1.53-3.76 3.56-3.97l1.07-.11.5-.95C8.08 7.14 9.94 6 12 6c2.62 0 4.88 1.86 5.39 4.43l.3 1.5 1.53.11c1.56.1 2.78 1.41 2.78 2.96 0 1.65-1.35 3-3 3z"/></svg>
    </div>
    <div>Format converters supporting AVIF, WebP, PNG, JPEG</div>
  </div>
</div>
"#.to_string(),
        position: Position {
            x: DimensionValue::Pixels(80.0),
            y: DimensionValue::Pixels(80.0),
        },
        size: Size {
            width: DimensionValue::Pixels(580.0),
            height: DimensionValue::Pixels(560.0),
        },
        timeline: Some(ElementTimeline {
            keyframes: vec![
                Keyframe {
                    time: 0.0,
                    opacity: Some(0.0),
                    x: Some("-150".to_string()),
                    y: Some("0".to_string()),
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
    };

    let slide2_right_card = SceneElement::Html {
        content: r#"
<style>
  .card-right-pink {
    width: 100%;
    height: 100%;
    background: rgba(15, 23, 42, 0.4);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    border: 1px solid rgba(248, 87, 166, 0.15);
    border-radius: 20px;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
    box-sizing: border-box;
  }
</style>
<div class="card-right-pink"></div>
"#.to_string(),
        position: Position {
            x: DimensionValue::Pixels(710.0),
            y: DimensionValue::Pixels(80.0),
        },
        size: Size {
            width: DimensionValue::Pixels(490.0),
            height: DimensionValue::Pixels(560.0),
        },
        timeline: Some(ElementTimeline {
            keyframes: vec![
                Keyframe {
                    time: 0.5,
                    opacity: Some(0.0),
                    x: None,
                    y: Some("100".to_string()),
                    scale: Some(0.8),
                    scale_x: None,
                    scale_y: None,
                    rotation: None,
                    easing: None,
                },
                Keyframe {
                    time: 2.0,
                    opacity: Some(1.0),
                    x: None,
                    y: Some("0".to_string()),
                    scale: Some(1.0),
                    scale_x: None,
                    scale_y: None,
                    rotation: None,
                    easing: Some("ease_out".to_string()),
                },
            ],
        }),
    };

    let slide3_card = SceneElement::Html {
        content: r#"
<style>
  @keyframes pulse-glow-violet {
    0%, 100% { filter: drop-shadow(0 0 2px #a855f7); opacity: 0.8; }
    50% { filter: drop-shadow(0 0 8px #a855f7); opacity: 1; }
  }
  .card-violet {
    width: 100%;
    height: 100%;
    background: rgba(15, 23, 42, 0.4);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    border: 1px solid rgba(168, 85, 247, 0.15);
    border-radius: 20px;
    padding: 35px;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
    color: #eeeeee;
    font-family: 'RobotoRegular', sans-serif;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    justify-content: center;
  }
  .title-violet {
    font-size: 32px;
    font-weight: 700;
    margin-bottom: 30px;
    background: linear-gradient(90deg, #a855f7, #ec4899);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    letter-spacing: 1px;
  }
  .bullet-item {
    display: flex;
    align-items: center;
    margin-bottom: 25px;
    font-size: 20px;
    line-height: 1.5;
  }
  .icon-container-violet {
    width: 44px;
    height: 44px;
    margin-right: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(168, 85, 247, 0.1);
    border: 1px solid rgba(168, 85, 247, 0.3);
    border-radius: 50%;
    animation: pulse-glow-violet 3s infinite ease-in-out;
    flex-shrink: 0;
  }
  .icon-svg-violet {
    width: 22px;
    height: 22px;
    fill: #a855f7;
  }
</style>
<div class="card-violet">
  <div class="title-violet">3. Video DSL & Audio Mixer</div>
  <div class="bullet-item">
    <div class="icon-container-violet">
      <svg class="icon-svg-violet" viewBox="0 0 24 24"><path d="M17 10.5V7c0-.55-.45-1-1-1H4c-.55 0-1 .45-1 1v10c0 .55.45 1 1 1h12c.55 0 1-.45 1-1v-3.5l4 4v-11l-4 4zM14 13h-3v3H9v-3H6v-2h3V8h2v3h3v2z"/></svg>
    </div>
    <div>Complex Video Scene DSL layer compiler</div>
  </div>
  <div class="bullet-item">
    <div class="icon-container-violet" style="animation-delay: 0.5s;">
      <svg class="icon-svg-violet" viewBox="0 0 24 24"><path d="M12 5.83L15.17 9l.06-.06L12 5.83zm0 12.34L8.83 15l-.06.06L12 18.17zM12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8z"/></svg>
    </div>
    <div>Pixel-level frames transitions (crossfades, slide wipes)</div>
  </div>
  <div class="bullet-item">
    <div class="icon-container-violet" style="animation-delay: 1.0s;">
      <svg class="icon-svg-violet" viewBox="0 0 24 24"><path d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"/></svg>
    </div>
    <div>Multi-track audio mixer (delays, volumes, and fades)</div>
  </div>
</div>
"#.to_string(),
        position: Position {
            x: DimensionValue::Pixels(80.0),
            y: DimensionValue::Pixels(80.0),
        },
        size: Size {
            width: DimensionValue::Pixels(580.0),
            height: DimensionValue::Pixels(560.0),
        },
        timeline: Some(ElementTimeline {
            keyframes: vec![
                Keyframe {
                    time: 0.0,
                    opacity: Some(0.0),
                    x: Some("-150".to_string()),
                    y: Some("0".to_string()),
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
    };

    let slide3_right_card = SceneElement::Html {
        content: r#"
<style>
  .card-right-violet {
    width: 100%;
    height: 100%;
    background: rgba(15, 23, 42, 0.4);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    border: 1px solid rgba(168, 85, 247, 0.15);
    border-radius: 20px;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
    box-sizing: border-box;
  }
</style>
<div class="card-right-violet"></div>
"#.to_string(),
        position: Position {
            x: DimensionValue::Pixels(710.0),
            y: DimensionValue::Pixels(80.0),
        },
        size: Size {
            width: DimensionValue::Pixels(490.0),
            height: DimensionValue::Pixels(560.0),
        },
        timeline: Some(ElementTimeline {
            keyframes: vec![
                Keyframe {
                    time: 0.5,
                    opacity: Some(0.0),
                    x: Some("150".to_string()),
                    y: None,
                    scale: None,
                    scale_x: None,
                    scale_y: None,
                    rotation: None,
                    easing: None,
                },
                Keyframe {
                    time: 2.0,
                    opacity: Some(1.0),
                    x: Some("0".to_string()),
                    y: None,
                    scale: None,
                    scale_x: None,
                    scale_y: None,
                    rotation: None,
                    easing: Some("ease_out".to_string()),
                },
            ],
        }),
    };

    // Slide 0: Title Card
    let slide0 = Scene {
        id: "slide_0".to_string(),
        start: 0.0,
        end: 6.0,
        elements: vec![
            animated_background.clone(),
            SceneElement::Html {
                content: format!(
                    r#"
<style>
  .intro-card {{
    width: 100%;
    height: 100%;
    background: rgba(15, 23, 42, 0.4);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    border: 1px solid rgba(0, 242, 254, 0.15);
    border-radius: 24px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 40px;
    text-align: center;
    box-shadow: 0 15px 50px rgba(0,0,0,0.6);
    box-sizing: border-box;
    font-family: 'RobotoRegular', sans-serif;
  }}
  .intro-logo-container {{
    width: 200px;
    height: 200px;
    margin-bottom: 25px;
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
  }}
  .intro-logo {{
    width: 160px;
    height: 160px;
    filter: drop-shadow(0 0 20px rgba(0, 242, 254, 0.5));
    z-index: 2;
  }}
  .intro-title {{
    font-size: 54px;
    font-weight: 800;
    margin-bottom: 15px;
    background: linear-gradient(90deg, #00f2fe, #4facfe);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    letter-spacing: 2px;
  }}
  .intro-sub1 {{
    font-size: 24px;
    color: #eeeeee;
    margin-bottom: 15px;
    font-weight: 400;
  }}
  .intro-sub2 {{
    font-size: 18px;
    color: #888888;
    font-weight: 400;
    letter-spacing: 1px;
  }}
  @keyframes rotate-ring {{
    0% {{ transform: rotate(0deg); }}
    100% {{ transform: rotate(360deg); }}
  }}
  .tech-ring {{
    position: absolute;
    width: 200px;
    height: 200px;
    border: 2px dashed rgba(0, 242, 254, 0.4);
    border-radius: 50%;
    animation: rotate-ring 25s linear infinite;
    z-index: 1;
  }}
</style>
<div class="intro-card">
  <div class="intro-logo-container">
    <div class="tech-ring"></div>
    <img class="intro-logo" src="data:image/svg+xml;base64,{}" />
  </div>
  <div class="intro-title">OpenMedia-RS</div>
  <div class="intro-sub1">Local Media Generation MCP Server</div>
  <div class="intro-sub2">Local • Free • Parallel • Agent-Ready</div>
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
                            x: Some("0".to_string()),
                            y: Some("50".to_string()),
                            scale: Some(0.8),
                            scale_x: None,
                            scale_y: None,
                            rotation: None,
                            easing: Some("ease_out".to_string()),
                        },
                        Keyframe {
                            time: 1.5,
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

    // Slide 1: Diagrams
    let slide1 = Scene {
        id: "slide_1".to_string(),
        start: 6.0,
        end: 12.0,
        elements: vec![
            animated_background.clone(),
            slide1_card,
            slide1_right_card,
            SceneElement::Chart {
                chart_type: "radar".to_string(),
                data: serde_json::json!([
                    { "label": "Charts", "value": 35.0 },
                    { "label": "Flows", "value": 45.0 },
                    { "label": "Shapes", "value": 20.0 }
                ]),
                position: Position {
                    x: DimensionValue::Pixels(730.0),
                    y: DimensionValue::Pixels(140.0),
                },
                size: Size {
                    width: DimensionValue::Pixels(450.0),
                    height: DimensionValue::Pixels(450.0),
                },
                theme: "dark".to_string(),
                timeline: Some(ElementTimeline {
                    keyframes: vec![
                        Keyframe {
                            time: 0.5,
                            opacity: Some(0.0),
                            x: None,
                            y: None,
                            scale: Some(0.2),
                            scale_x: None,
                            scale_y: None,
                            rotation: Some(-45.0),
                            easing: None,
                        },
                        Keyframe {
                            time: 2.0,
                            opacity: Some(1.0),
                            x: None,
                            y: None,
                            scale: Some(1.0),
                            scale_x: None,
                            scale_y: None,
                            rotation: Some(0.0),
                            easing: Some("ease_out".to_string()),
                        },
                    ],
                }),
            },
        ],
    };

    // Slide 2: Image Filters & Shaders
    let slide2 = Scene {
        id: "slide_2".to_string(),
        start: 12.0,
        end: 18.0,
        elements: vec![
            animated_background.clone(),
            slide2_card,
            slide2_right_card,
            SceneElement::Chart {
                chart_type: "area".to_string(),
                data: serde_json::json!([
                    { "label": "1 Thread", "value": 1.0 },
                    { "label": "4 Threads", "value": 3.4 },
                    { "label": "8 Threads", "value": 6.2 },
                    { "label": "WGSL GPU", "value": 12.5 }
                ]),
                position: Position {
                    x: DimensionValue::Pixels(730.0),
                    y: DimensionValue::Pixels(140.0),
                },
                size: Size {
                    width: DimensionValue::Pixels(450.0),
                    height: DimensionValue::Pixels(450.0),
                },
                theme: "dark".to_string(),
                timeline: Some(ElementTimeline {
                    keyframes: vec![
                        Keyframe {
                            time: 0.5,
                            opacity: Some(0.0),
                            x: None,
                            y: Some("100".to_string()),
                            scale: Some(0.8),
                            scale_x: None,
                            scale_y: None,
                            rotation: None,
                            easing: None,
                        },
                        Keyframe {
                            time: 2.0,
                            opacity: Some(1.0),
                            x: None,
                            y: Some("0".to_string()),
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

    // Slide 3: Video Scene & Audio
    let slide3 = Scene {
        id: "slide_3".to_string(),
        start: 18.0,
        end: 24.0,
        elements: vec![
            animated_background.clone(),
            slide3_card,
            slide3_right_card,
            SceneElement::Svg {
                content: mermaid_svg,
                position: Position {
                    x: DimensionValue::Pixels(730.0),
                    y: DimensionValue::Pixels(140.0),
                },
                size: Some(Size {
                    width: DimensionValue::Pixels(450.0),
                    height: DimensionValue::Pixels(440.0),
                }),
                timeline: Some(ElementTimeline {
                    keyframes: vec![
                        Keyframe {
                            time: 0.5,
                            opacity: Some(0.0),
                            x: Some("150".to_string()),
                            y: None,
                            scale: None,
                            scale_x: None,
                            scale_y: None,
                            rotation: None,
                            easing: None,
                        },
                        Keyframe {
                            time: 2.0,
                            opacity: Some(1.0),
                            x: Some("0".to_string()),
                            y: None,
                            scale: None,
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

    // Slide 4: Outro
    let slide4 = Scene {
        id: "slide_4".to_string(),
        start: 24.0,
        end: 30.0,
        elements: vec![
            animated_background.clone(),
            SceneElement::Html {
                content: format!(
                    r#"
<style>
  .outro-card {{
    width: 100%;
    height: 100%;
    background: rgba(15, 23, 42, 0.4);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    border: 1px solid rgba(0, 242, 254, 0.15);
    border-radius: 24px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 40px;
    text-align: center;
    box-shadow: 0 15px 50px rgba(0,0,0,0.6);
    box-sizing: border-box;
    font-family: 'RobotoRegular', sans-serif;
  }}
  .outro-logo {{
    width: 130px;
    height: 130px;
    margin-bottom: 25px;
    filter: drop-shadow(0 0 15px rgba(0, 242, 254, 0.4));
  }}
  .outro-title {{
    font-size: 44px;
    font-weight: 800;
    margin-bottom: 15px;
    background: linear-gradient(90deg, #00f2fe, #4facfe);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
  }}
  .outro-sub1 {{
    font-size: 22px;
    color: #eeeeee;
    margin-bottom: 15px;
  }}
  .outro-sub2 {{
    font-size: 18px;
    color: #888888;
  }}
</style>
<div class="outro-card">
  <img class="outro-logo" src="data:image/svg+xml;base64,{}" />
  <div class="outro-title">Powered by OpenMedia-RS</div>
  <div class="outro-sub1">Exposing 33 robust tools for LLM agent integration.</div>
  <div class="outro-sub2">Built on Rust. Engineered for Speed.</div>
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
                            x: Some("0".to_string()),
                            y: Some("-50".to_string()),
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

    // Transitions
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
        background: "#1a1a2e".to_string(),
        scenes: vec![slide0, slide1, slide2, slide3, slide4],
        transitions,
        audio: None,
        custom_fonts: Some(fonts),
    };

    println!("Starting render of openmedia_promo.mp4 (30s, 15fps, 1280x720)...");
    let spec = openmedia_video::render_video_scene(&scene, output_path).await.unwrap();
    println!("SUCCESS: Video generated at {:?}", spec.path);
}
