use std::fs;
use std::path::Path;

fn main() {
    let assets_dir = Path::new("assets");
    let icons_dir = assets_dir.join("icons");
    fs::create_dir_all(&icons_dir).unwrap();

    // 1. SVG Background System (animated network background mockup)
    let animated_network = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 600" width="800" height="600">
  <style>
    .node { fill: #2563eb; filter: drop-shadow(0 0 5px #2563eb); }
    .node-orange { fill: #ea580c; filter: drop-shadow(0 0 5px #ea580c); }
    .connection { stroke: rgba(37, 99, 235, 0.15); stroke-width: 1; }
    .pulse-node { animation: pulse 3s infinite ease-in-out; }
    @keyframes pulse {
      0%, 100% { r: 4; opacity: 0.6; }
      50% { r: 8; opacity: 1; }
    }
  </style>
  <rect width="100%" height="100%" fill="#020306"/>
  <!-- connections -->
  <line x1="100" y1="100" x2="300" y2="150" class="connection"/>
  <line x1="300" y1="150" x2="400" y2="350" class="connection"/>
  <line x1="400" y1="350" x2="200" y2="450" class="connection"/>
  <line x1="200" y1="450" x2="100" y2="100" class="connection"/>
  <line x1="300" y1="150" x2="600" y2="200" class="connection"/>
  <line x1="600" y1="200" x2="700" y2="400" class="connection"/>
  <line x1="700" y1="400" x2="400" y2="350" class="connection"/>
  <line x1="600" y1="200" x2="500" y2="100" class="connection"/>
  <line x1="500" y1="100" x2="300" y2="150" class="connection"/>
  <!-- nodes -->
  <circle cx="100" cy="100" r="6" class="node pulse-node"/>
  <circle cx="300" cy="150" r="5" class="node-orange"/>
  <circle cx="400" cy="350" r="6" class="node pulse-node"/>
  <circle cx="200" cy="450" r="5" class="node-orange"/>
  <circle cx="600" cy="200" r="7" class="node pulse-node"/>
  <circle cx="700" cy="400" r="5" class="node-orange"/>
  <circle cx="500" cy="100" r="6" class="node pulse-node"/>
</svg>"##;
    fs::write(assets_dir.join("animated_network.svg"), animated_network).unwrap();
    println!("Generated assets/animated_network.svg");

    // 2. MCP Flow SVG
    let mcp_flow = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 300" width="800" height="300">
  <defs>
    <linearGradient id="box-grad" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#1e293b" stop-opacity="0.8" />
      <stop offset="100%" stop-color="#0f172a" stop-opacity="0.8" />
    </linearGradient>
    <linearGradient id="orange-blue" x1="0%" y1="0%" x2="100%" y2="0%">
      <stop offset="0%" stop-color="#ea580c" />
      <stop offset="100%" stop-color="#2563eb" />
    </linearGradient>
  </defs>
  <rect width="100%" height="100%" fill="#090d16" rx="12"/>
  
  <!-- Flow Lines -->
  <path d="M120 150 L200 150" stroke="url(#orange-blue)" stroke-width="4" fill="none"/>
  <path d="M320 150 L400 150" stroke="#2563eb" stroke-width="3" fill="none"/>
  <path d="M520 150 L580 100" stroke="#2563eb" stroke-width="2" fill="none"/>
  <path d="M520 150 L580 200" stroke="#2563eb" stroke-width="2" fill="none"/>
  <path d="M520 150 L600 150" stroke="#2563eb" stroke-width="2" fill="none"/>

  <!-- AI Agent -->
  <rect x="20" y="100" width="100" height="100" rx="8" fill="url(#box-grad)" stroke="#ea580c" stroke-width="1.5"/>
  <text x="70" y="155" fill="#ffffff" font-family="sans-serif" font-size="14" font-weight="bold" text-anchor="middle">AI Agent</text>

  <!-- MCP -->
  <rect x="200" y="100" width="120" height="100" rx="8" fill="url(#box-grad)" stroke="#2563eb" stroke-width="1.5"/>
  <text x="260" y="155" fill="#ffffff" font-family="sans-serif" font-size="14" font-weight="bold" text-anchor="middle">OpenMedia MCP</text>

  <!-- Engines -->
  <rect x="400" y="100" width="120" height="100" rx="8" fill="url(#box-grad)" stroke="#38bdf8" stroke-width="1.5"/>
  <text x="460" y="155" fill="#ffffff" font-family="sans-serif" font-size="14" font-weight="bold" text-anchor="middle">Vector Engine</text>

  <!-- Output badging -->
  <rect x="600" y="70" width="150" height="40" rx="6" fill="#1e1b4b" stroke="#8b5cf6" stroke-width="1"/>
  <text x="675" y="95" fill="#c084fc" font-family="sans-serif" font-size="12" font-weight="bold" text-anchor="middle">Images &amp; SVGs</text>

  <rect x="600" y="130" width="150" height="40" rx="6" fill="#1c1917" stroke="#ea580c" stroke-width="1"/>
  <text x="675" y="155" fill="#fdba74" font-family="sans-serif" font-size="12" font-weight="bold" text-anchor="middle">Video &amp; Audio</text>

  <rect x="600" y="190" width="150" height="40" rx="6" fill="#064e3b" stroke="#10b981" stroke-width="1"/>
  <text x="675" y="215" fill="#6ee7b7" font-family="sans-serif" font-size="12" font-weight="bold" text-anchor="middle">Animations &amp; Charts</text>
</svg>"##;
    fs::write(assets_dir.join("mcp_flow.svg"), mcp_flow).unwrap();
    println!("Generated assets/mcp_flow.svg");

    // 3. Architecture Diagram
    let architecture_diagram = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 600 400" width="600" height="400">
  <rect width="100%" height="100%" fill="#0a0f1d" rx="10"/>
  <text x="300" y="40" fill="#ffffff" font-family="sans-serif" font-size="18" font-weight="bold" text-anchor="middle">Architecture Diagram</text>

  <g transform="translate(50, 80)">
    <!-- Arrows -->
    <path d="M120 100 H200" stroke="#ea580c" stroke-width="2"/>
    <path d="M320 100 L420 40" stroke="#2563eb" stroke-width="2"/>
    <path d="M320 100 L420 80" stroke="#2563eb" stroke-width="2"/>
    <path d="M320 100 L420 120" stroke="#2563eb" stroke-width="2"/>
    <path d="M320 100 L420 160" stroke="#2563eb" stroke-width="2"/>
    <path d="M320 100 L420 200" stroke="#2563eb" stroke-width="2"/>

    <!-- Nodes -->
    <rect x="0" y="75" width="120" height="50" rx="5" fill="#1e293b" stroke="#ea580c" stroke-width="2"/>
    <text x="60" y="105" fill="#ffffff" font-family="sans-serif" font-size="14" text-anchor="middle">AI Agent</text>

    <rect x="200" y="75" width="120" height="50" rx="5" fill="#1e293b" stroke="#2563eb" stroke-width="2"/>
    <text x="260" y="105" fill="#ffffff" font-family="sans-serif" font-size="14" text-anchor="middle">OpenMedia MCP</text>

    <rect x="420" y="15" width="110" height="35" rx="4" fill="#0f172a" stroke="#38bdf8" stroke-width="1.5"/>
    <text x="475" y="37" fill="#38bdf8" font-family="sans-serif" font-size="12" text-anchor="middle">Images</text>

    <rect x="420" y="60" width="110" height="35" rx="4" fill="#0f172a" stroke="#38bdf8" stroke-width="1.5"/>
    <text x="475" y="82" fill="#38bdf8" font-family="sans-serif" font-size="12" text-anchor="middle">SVG</text>

    <rect x="420" y="100" width="110" height="35" rx="4" fill="#0f172a" stroke="#38bdf8" stroke-width="1.5"/>
    <text x="475" y="122" fill="#38bdf8" font-family="sans-serif" font-size="12" text-anchor="middle">Animation</text>

    <rect x="420" y="140" width="110" height="35" rx="4" fill="#0f172a" stroke="#38bdf8" stroke-width="1.5"/>
    <text x="475" y="162" fill="#38bdf8" font-family="sans-serif" font-size="12" text-anchor="middle">Charts</text>

    <rect x="420" y="180" width="110" height="35" rx="4" fill="#0f172a" stroke="#38bdf8" stroke-width="1.5"/>
    <text x="475" y="202" fill="#38bdf8" font-family="sans-serif" font-size="12" text-anchor="middle">Video</text>
  </g>
</svg>"##;
    fs::write(assets_dir.join("architecture_diagram.svg"), architecture_diagram).unwrap();
    println!("Generated assets/architecture_diagram.svg");

    // 4. Animated SVG
    let animated_svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 200" width="200" height="200">
  <rect width="100%" height="100%" fill="#09090b" rx="8"/>
  <circle cx="100" cy="100" r="40" fill="none" stroke="#2563eb" stroke-width="4" stroke-dasharray="250" stroke-dashoffset="250">
    <animate attributeName="stroke-dashoffset" values="250;0" dur="3s" repeatCount="indefinite" fill="freeze"/>
  </circle>
  <circle cx="100" cy="100" r="10" fill="#ea580c">
    <animate attributeName="opacity" values="0.3;1;0.3" dur="2s" repeatCount="indefinite"/>
  </circle>
</svg>"##;
    fs::write(assets_dir.join("animated_svg.svg"), animated_svg).unwrap();
    println!("Generated assets/animated_svg.svg");

    // 5. Animated Chart
    let animated_chart = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 500 300" width="500" height="300">
  <rect width="100%" height="100%" fill="#09090b" rx="8"/>
  <text x="250" y="35" fill="#ffffff" font-family="sans-serif" font-size="14" font-weight="bold" text-anchor="middle">Performance Growth</text>
  <path d="M 50 250 L 150 200 L 250 220 L 350 120 L 450 80" fill="none" stroke="#2563eb" stroke-width="4" stroke-dasharray="1000" stroke-dashoffset="1000">
    <animate attributeName="stroke-dashoffset" from="1000" to="0" dur="4s" fill="freeze" repeatCount="indefinite"/>
  </path>
  <circle cx="450" cy="80" r="6" fill="#ea580c">
    <animate attributeName="r" values="5;9;5" dur="1.5s" repeatCount="indefinite"/>
  </circle>
</svg>"##;
    fs::write(assets_dir.join("animated_chart.svg"), animated_chart).unwrap();
    println!("Generated assets/animated_chart.svg");

    // 6. Loading Sequence
    let loading_spinner = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="100" height="100">
  <rect width="100%" height="100%" fill="#09090b" rx="10"/>
  <circle cx="50" cy="50" r="30" fill="none" stroke="#1e293b" stroke-width="6"/>
  <circle cx="50" cy="50" r="30" fill="none" stroke="#2563eb" stroke-width="6" stroke-dasharray="188" stroke-dashoffset="120">
    <animateTransform attributeName="transform" type="rotate" from="0 50 50" to="360 50 50" dur="1.5s" repeatCount="indefinite"/>
  </circle>
</svg>"##;
    fs::write(assets_dir.join("loading_spinner.svg"), loading_spinner).unwrap();
    println!("Generated assets/loading_spinner.svg");

    // 7. Logo Reveal Timeline
    let timeline_json = r##"{
  "timeline": {
    "sequence": [
      { "name": "particle_assemble", "start": 0.0, "duration": 2.0 },
      { "name": "glow_pulse", "start": 2.0, "duration": 1.5 },
      { "name": "path_draw", "start": 3.0, "duration": 1.5 },
      { "name": "camera_push", "start": 4.0, "duration": 1.0 }
    ],
    "total_duration": 5.0
  }
}"##;
    fs::write(assets_dir.join("logo_reveal_timeline.json"), timeline_json).unwrap();
    println!("Generated assets/logo_reveal_timeline.json");

    // 8. 10 Icons Set
    let icon_defs = vec![
        ("video", r##"<path d="M23 7l-7 5 7 5V7z"></path><rect x="1" y="5" width="15" height="14" rx="2" ry="2"></rect>"##),
        ("image", r##"<rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline>"##),
        ("play", r##"<polygon points="5 3 19 12 5 21 5 3"></polygon>"##),
        ("cpu", r##"<rect x="4" y="4" width="16" height="16" rx="2"></rect><rect x="9" y="9" width="6" height="6"></rect><path d="M9 1v3M15 1v3M9 20v3M15 20v3M20 9h3M20 15h3M1 9h3M1 15h3"></path>"##),
        ("sparkles", r##"<path d="M12 3v18M3 12h18M12 3l3 3-3 3-3-3 3-3zm-6 9l3 3-3 3-3-3 3-3zm12 0l3 3-3 3-3-3 3-3z"></path>"##),
        ("workflow", r##"<rect x="3" y="3" width="6" height="6" rx="1"></rect><rect x="15" y="15" width="6" height="6" rx="1"></rect><rect x="15" y="3" width="6" height="6" rx="1"></rect><path d="M6 9v6h9M21 9v6"></path>"##),
        ("chart", r##"<line x1="18" y1="20" x2="18" y2="10"></line><line x1="12" y1="20" x2="12" y2="4"></line><line x1="6" y1="20" x2="6" y2="14"></line>"##),
        ("terminal", r##"<polyline points="4 17 10 11 4 5"></polyline><line x1="12" y1="19" x2="20" y2="19"></line>"##),
        ("settings", r##"<circle cx="12" cy="12" r="3"></circle><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"></path>"##),
        ("database", r##"<ellipse cx="12" cy="5" rx="9" ry="3"></ellipse><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"></path><path d="M3 12c0 1.66 4 3 9 3s9-1.34 9-3"></path>"##),
    ];

    for (name, path) in icon_defs {
        let icon_svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512" viewBox="0 0 24 24" fill="none" stroke="#2563EB" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="filter: drop-shadow(0 0 8px rgba(37,99,235,0.4))">
  {}
</svg>"##,
            path
        );
        fs::write(icons_dir.join(format!("{}.svg", name)), icon_svg).unwrap();
    }
    println!("Generated 10 custom Lucide icons in assets/icons/");
}
