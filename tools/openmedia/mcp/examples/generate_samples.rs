use openmedia_svg::{ChartPoint, create_chart, render_mermaid, rasterize};
use std::fs;
use std::path::Path;

fn main() {
    let assets_dir = Path::new("assets");
    fs::create_dir_all(assets_dir).unwrap();

    // 1. Generate Sample Static SVG
    let sample_svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 200" width="400" height="200">
  <defs>
    <linearGradient id="blue-orange" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#00f2fe" />
      <stop offset="100%" stop-color="#ea580c" />
    </linearGradient>
  </defs>
  <rect width="100%" height="100%" fill="#09090b" rx="10"/>
  <rect x="20" y="20" width="360" height="160" fill="url(#blue-orange)" rx="8" opacity="0.15"/>
  <circle cx="100" cy="100" r="50" fill="url(#blue-orange)"/>
  <text x="200" y="95" fill="#ffffff" font-family="sans-serif" font-size="24" font-weight="bold">OpenMedia-RS</text>
  <text x="200" y="125" fill="#a1a1aa" font-family="sans-serif" font-size="14">AI Media Creation Engine</text>
</svg>"##;
    fs::write(assets_dir.join("sample_diagram.svg"), sample_svg).unwrap();
    println!("Generated assets/sample_diagram.svg");

    // 2. Generate Sample Animated SVG (SMIL rotation and pulse)
    let sample_animation = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 200" width="200" height="200">
  <rect width="100%" height="100%" fill="#09090b" rx="8"/>
  <!-- Rotating square -->
  <rect x="75" y="75" width="50" height="50" fill="none" stroke="#ea580c" stroke-width="3" rx="6">
    <animateTransform attributeName="transform" type="rotate" from="0 100 100" to="360 100 100" dur="4s" repeatCount="indefinite"/>
  </rect>
  <!-- Pulsing circle -->
  <circle cx="100" cy="100" r="15" fill="#00f2fe">
    <animate attributeName="r" values="10;20;10" dur="2s" repeatCount="indefinite"/>
    <animate attributeName="opacity" values="0.5;1;0.5" dur="2s" repeatCount="indefinite"/>
  </circle>
</svg>"##;
    fs::write(assets_dir.join("sample_animation.svg"), sample_animation).unwrap();
    println!("Generated assets/sample_animation.svg");

    // 3. Generate Sample Chart (Radar Chart)
    let chart_data = vec![
        ChartPoint { label: "Images".to_string(), value: 85.0 },
        ChartPoint { label: "Videos".to_string(), value: 90.0 },
        ChartPoint { label: "Animations".to_string(), value: 95.0 },
        ChartPoint { label: "Diagrams".to_string(), value: 80.0 },
        ChartPoint { label: "Performance".to_string(), value: 99.0 },
    ];
    let chart_svg = create_chart("radar", Some("Rendering Engines Performance"), &chart_data, 800, 600).unwrap();
    fs::write(assets_dir.join("sample_chart.svg"), chart_svg).unwrap();
    println!("Generated assets/sample_chart.svg");

    // 4. Generate Sample Mermaid Diagram
    let mermaid_code = "graph LR\n  Agent[AI Agent] -->|MCP Request| OpenMedia[OpenMedia-RS]\n  OpenMedia -->|SVG/SMIL| Animate[Animate Crate]\n  OpenMedia -->|Headless Chrome| Chrome[Browser Crate]\n  OpenMedia -->|WGSL Compute| Image[Image Crate]\n  Chrome -->|Render Frames| Output[MP4 Video]";
    let mermaid_svg = render_mermaid(mermaid_code, None, None).unwrap();
    fs::write(assets_dir.join("sample_mermaid.svg"), mermaid_svg).unwrap();
    println!("Generated assets/sample_mermaid.svg");

    // 5. Generate Sample Raster Image (by rasterizing the first SVG)
    rasterize(
        sample_svg,
        Some(800),
        Some(400),
        None,
        "png",
        &assets_dir.join("sample_image.png"),
    ).unwrap();
    println!("Generated assets/sample_image.png");

    println!("All sample assets generated successfully in assets/ folder!");
}
