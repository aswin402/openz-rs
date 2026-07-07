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

    println!("Constructing VideoScene JSON for promo video...");

    // HTML Content for Scene 1: Dependency Hell
    let scene_1_html = r##"
<div style="position: relative; width: 1280px; height: 720px; background: #07070e; color: #ef4444; overflow: hidden; font-family: monospace; box-sizing: border-box;">
  <canvas id="canvas1" width="1280" height="720" style="position: absolute; top: 0; left: 0; width: 100%; height: 100%;"></canvas>
  
  <style>
    @keyframes local-shake {
      0%, 100% { transform: translate(0, 0); }
      10% { transform: translate(-5px, 3px) rotate(-0.5deg); }
      30% { transform: translate(4px, -4px) rotate(0.5deg); }
      50% { transform: translate(-3px, 5px) rotate(-1deg); }
      70% { transform: translate(5px, 3px) rotate(1deg); }
      90% { transform: translate(-4px, -3px) rotate(-0.5deg); }
    }
    .glitch-title {
      font-size: 80px;
      font-weight: 900;
      color: #ef4444;
      text-shadow: 0 0 20px rgba(239, 68, 68, 0.7);
      letter-spacing: 6px;
      margin: 0;
      display: inline-block;
      animation: local-shake 0.12s infinite;
    }
    .scanlines {
      position: absolute;
      top: 0;
      left: 0;
      width: 100%;
      height: 100%;
      background: linear-gradient(rgba(18, 16, 16, 0) 50%, rgba(0, 0, 0, 0.25) 50%), linear-gradient(90deg, rgba(255, 0, 0, 0.06), rgba(0, 255, 0, 0.02), rgba(0, 255, 0, 0.06));
      background-size: 100% 4px, 6px 100%;
      z-index: 20;
      pointer-events: none;
    }
  </style>

  <div class="scanlines"></div>

  <div style="position: absolute; top: 50%; left: 50%; width: 100%; text-align: center; z-index: 10; transform: translate(-50%, -50%);">
    <h1 class="glitch-title">DEPENDENCY HELL</h1>
    <p id="log-text" style="font-size: 20px; color: #a1a1aa; margin-top: 20px; font-weight: bold;"></p>
  </div>

  <script>
    (function() {
      const canvas = document.getElementById('canvas1');
      const ctx = canvas.getContext('2d');
      const w = canvas.width;
      const h = canvas.height;
      const wrapper = canvas.closest('[data-scene-time]');
      const t = wrapper ? parseFloat(wrapper.getAttribute('data-scene-time')) : 0.0;
      
      function hash(x) {
        const s = Math.sin(x) * 1e4;
        return s - Math.floor(s);
      }

      // Draw cyber matrix grid background
      ctx.strokeStyle = 'rgba(239, 68, 68, 0.04)';
      ctx.lineWidth = 1;
      const gridSize = 40;
      for (let x = 0; x < w; x += gridSize) {
        ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, h); ctx.stroke();
      }
      for (let y = 0; y < h; y += gridSize) {
        ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(w, y); ctx.stroke();
      }

      // Dynamic warning logs text typing
      const logText = document.getElementById('log-text');
      const logs = [
        "npm ERR! code ELIFECYCLE",
        "npm ERR! errno 137 (Out Of Memory)",
        "Compiling cargo dependencies... FAILED",
        "fatal: dependency chain exploded",
        "Process terminated with exit code 1"
      ];
      if (logText) {
        const logIdx = Math.min(logs.length - 1, Math.floor(t * 1.5));
        logText.innerHTML = " > " + logs[logIdx];
      }

      // Code rain
      ctx.font = '11px monospace';
      const cols = 30;
      for (let col = 0; col < cols; col++) {
        const cx = col * (w / cols) + 15;
        const seed = col * 43.1;
        const speed = 250 + hash(seed) * 200;
        const startY = -150 + hash(seed + 1) * 300;
        const y = (startY + t * speed) % (h + 200) - 100;
        
        for (let charIdx = 0; charIdx < 12; charIdx++) {
          const charCode = 33 + Math.floor(hash(seed + charIdx + t) * 93);
          const char = String.fromCharCode(charCode);
          const charY = y - charIdx * 12;
          const alpha = (1.0 - charIdx / 12) * 0.35;
          ctx.fillStyle = `rgba(239, 68, 68, ${alpha})`;
          ctx.fillText(char, cx, charY);
        }
      }

      // Exploding dependency nodes
      const numNodes = 15;
      const nodes = [];
      const centerX = w / 2;
      const centerY = h / 2;
      for (let i = 0; i < numNodes; i++) {
        const seed = i * 73.9;
        const angle = hash(seed) * Math.PI * 2;
        const radius = 120 + hash(seed + 1) * 150;
        let px = centerX + Math.cos(angle) * radius;
        let py = centerY + Math.sin(angle) * radius;

        if (t > 1.8) {
          const expT = t - 1.8;
          const expDist = expT * 700;
          px += Math.cos(angle) * expDist;
          py += Math.sin(angle) * expDist;
        }
        nodes.push({ x: px, y: py, r: 8 + hash(seed + 2) * 8 });
      }

      // Draw connections
      ctx.strokeStyle = `rgba(239, 68, 68, ${t < 1.8 ? 0.25 : Math.max(0, 0.25 - (t - 1.8) * 2.0)})`;
      ctx.lineWidth = 2;
      for (let i = 0; i < numNodes; i++) {
        for (let j = i + 1; j < numNodes; j++) {
          if (hash(i * j) > 0.5) {
            ctx.beginPath();
            ctx.moveTo(nodes[i].x, nodes[i].y);
            ctx.lineTo(nodes[j].x, nodes[j].y);
            ctx.stroke();
          }
        }
      }

      // Draw node particles
      nodes.forEach((n) => {
        const alpha = t < 1.8 ? 0.7 : Math.max(0, 0.7 - (t - 1.8) * 1.5);
        ctx.fillStyle = `rgba(239, 68, 68, ${alpha})`;
        ctx.beginPath();
        ctx.arc(n.x, n.y, n.r, 0, Math.PI * 2);
        ctx.fill();
        ctx.strokeStyle = `rgba(255, 255, 255, ${alpha * 0.4})`;
        ctx.stroke();
      });
    })();
  </script>
</div>
"##;

    // HTML Content for Scene 2: OpenMedia Emerges
    let scene_2_html = r##"
<div style="position: relative; width: 1280px; height: 720px; background: #020308; overflow: hidden; box-sizing: border-box;">
  <canvas id="canvas2" width="1280" height="720" style="position: absolute; top: 0; left: 0; width: 100%; height: 100%; z-index: 1;"></canvas>
  
  <!-- Logo container -->
  <div id="logo-container" style="position: absolute; top: 45%; left: 50%; transform: translate(-50%, -50%); width: 340px; height: 340px; z-index: 10; opacity: 0; transition: opacity 0.8s ease-out;">
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 320 320" width="100%" height="100%">
      <defs>
        <linearGradient id="cyber-cyan-pink" x1="0%" y1="0%" x2="100%" y2="100%">
          <stop offset="0%" stop-color="#00f2fe" />
          <stop offset="50%" stop-color="#9b51e0" />
          <stop offset="100%" stop-color="#4facfe" />
        </linearGradient>
        <linearGradient id="neon-glow-grad" x1="0%" y1="100%" x2="100%" y2="0%">
          <stop offset="0%" stop-color="#f857a6" />
          <stop offset="100%" stop-color="#ff5858" />
        </linearGradient>
        <radialGradient id="space-glow" cx="50%" cy="50%" r="50%">
          <stop offset="0%" stop-color="#9b51e0" stop-opacity="0.3" />
          <stop offset="50%" stop-color="#00f2fe" stop-opacity="0.08" />
          <stop offset="100%" stop-color="#0b0f19" stop-opacity="0" />
        </radialGradient>
        <filter id="ultra-glow" x="-50%" y="-50%" width="200%" height="200%">
          <feGaussianBlur stdDeviation="8" result="blur1" />
          <feGaussianBlur stdDeviation="3" result="blur2" />
          <feMerge>
            <feMergeNode in="blur1" />
            <feMergeNode in="blur2" />
            <feMergeNode in="SourceGraphic" />
          </feMerge>
        </filter>
      </defs>
      <circle cx="160" cy="160" r="140" fill="url(#space-glow)" />
      <!-- Rings -->
      <circle id="ring-outer" cx="160" cy="160" r="135" fill="none" stroke="url(#cyber-cyan-pink)" stroke-width="1.5" stroke-dasharray="4 8 16 8 64 12" stroke-opacity="0.4" />
      <circle id="ring-inner" cx="160" cy="160" r="122" fill="none" stroke="url(#neon-glow-grad)" stroke-width="2" stroke-dasharray="120 40 80 40" stroke-opacity="0.6" />
      <!-- Waveforms -->
      <path d="M 50 160 C 85 110, 115 210, 160 160 C 205 110, 235 210, 270 160" fill="none" stroke="url(#cyber-cyan-pink)" stroke-width="3" stroke-linecap="round" stroke-opacity="0.8" filter="url(#ultra-glow)" />
      <path d="M 50 160 C 85 210, 115 110, 160 160 C 205 210, 235 110, 270 160" fill="none" stroke="url(#neon-glow-grad)" stroke-width="2" stroke-linecap="round" stroke-opacity="0.7" filter="url(#ultra-glow)" />
      <!-- Glowing Core -->
      <circle cx="160" cy="160" r="48" fill="none" stroke="url(#cyber-cyan-pink)" stroke-width="3.5" filter="url(#ultra-glow)" />
      <path d="M 148 132 L 186 160 L 148 188 Z" fill="url(#neon-glow-grad)" stroke="#ffffff" stroke-width="1.5" stroke-linejoin="round" filter="url(#ultra-glow)" />
    </svg>
  </div>

  <!-- Glowing Title -->
  <div id="title-text" style="position: absolute; bottom: 8%; left: 50%; transform: translateX(-50%); text-align: center; z-index: 10; opacity: 0; transition: opacity 0.8s ease-out;">
    <h2 style="font-size: 42px; font-weight: 800; color: #ffffff; text-shadow: 0 0 25px rgba(0, 242, 254, 0.7); font-family: 'Inter', sans-serif; letter-spacing: 8px; margin: 0; text-transform: uppercase;">
      OPENMEDIA-RS
    </h2>
    <p style="font-size: 16px; color: #94a3b8; font-family: 'Inter', sans-serif; font-weight: 600; letter-spacing: 4px; margin: 8px 0 0 0; text-transform: uppercase;">
      A Rust-native MCP Media Engine
    </p>
  </div>

  <script>
    (function() {
      const canvas = document.getElementById('canvas2');
      const ctx = canvas.getContext('2d');
      const w = canvas.width;
      const h = canvas.height;
      const wrapper = canvas.closest('[data-scene-time]');
      const t = wrapper ? parseFloat(wrapper.getAttribute('data-scene-time')) : 0.0;
      
      // Control opacity programmatically based on time
      const logo = document.getElementById('logo-container');
      const title = document.getElementById('title-text');
      if (logo && title) {
        logo.style.opacity = Math.min(1.0, t / 1.0);
        title.style.opacity = t > 0.5 ? Math.min(1.0, (t - 0.5) / 1.0) : 0.0;
      }

      // Rotate tech rings in SVG
      const ringOuter = document.getElementById('ring-outer');
      const ringInner = document.getElementById('ring-inner');
      if (ringOuter && ringInner) {
        const outerRot = t * 15; // 15 degrees per second clockwise
        const innerRot = -t * 22; // counter-clockwise
        ringOuter.setAttribute('transform', `rotate(${outerRot} 160 160)`);
        ringInner.setAttribute('transform', `rotate(${innerRot} 160 160)`);
      }

      function hash(x) {
        const s = Math.sin(x) * 1e4;
        return s - Math.floor(s);
      }

      // Space particles and volumetric fog
      const numParticles = 80;
      for (let i = 0; i < numParticles; i++) {
        const seed = i * 137.9;
        const initialX = hash(seed) * w;
        const initialY = hash(seed + 1) * h;
        const speedX = -20 + hash(seed + 2) * 40;
        const speedY = -15 + hash(seed + 3) * 30;
        const x = (initialX + speedX * t + w) % w;
        const y = (initialY + speedY * t + h) % h;
        const size = 1 + hash(seed + 4) * 3.5;
        const alpha = (0.2 + hash(seed + 5) * 0.6) * Math.min(1.0, t / 0.5);
        const isOrange = hash(seed + 6) > 0.55;
        
        ctx.fillStyle = isOrange ? `rgba(234, 88, 12, ${alpha})` : `rgba(0, 242, 254, ${alpha})`;
        ctx.beginPath();
        ctx.arc(x, y, size, 0, Math.PI * 2);
        ctx.fill();
      }

      // Draw light rays radiating from center
      ctx.save();
      ctx.translate(w / 2, h / 2);
      ctx.rotate(t * 0.05);
      const numRays = 8;
      for (let i = 0; i < numRays; i++) {
        const angle = (i / numRays) * Math.PI * 2;
        ctx.fillStyle = `rgba(155, 81, 224, ${0.015 + Math.sin(t * 2 + i) * 0.005})`;
        ctx.beginPath();
        ctx.moveTo(0, 0);
        ctx.lineTo(Math.cos(angle - 0.1) * 800, Math.sin(angle - 0.1) * 800);
        ctx.lineTo(Math.cos(angle + 0.1) * 800, Math.sin(angle + 0.1) * 800);
        ctx.closePath();
        ctx.fill();
      }
      ctx.restore();
    })();
  </script>
</div>
"##;

    // HTML Content for Scene 3: MCP Tool Showcase
    let scene_3_html = r##"
<div style="position: relative; width: 1280px; height: 720px; background: #030510; color: #ffffff; overflow: hidden; font-family: 'Inter', sans-serif; box-sizing: border-box;">
  <canvas id="canvas3" width="1280" height="720" style="position: absolute; top: 0; left: 0; width: 100%; height: 100%; z-index: 1;"></canvas>
  
  <!-- Header -->
  <div style="position: absolute; top: 8%; width: 100%; text-align: center; z-index: 10;">
    <p style="font-size: 14px; font-weight: 700; color: #3b82f6; letter-spacing: 5px; text-transform: uppercase; margin: 0;">
      One Server, Full Capability
    </p>
    <h2 style="font-size: 40px; font-weight: 800; color: #ffffff; margin: 5px 0 0 0; letter-spacing: -1px; text-shadow: 0 0 15px rgba(255,255,255,0.15);">
      Native MCP Tool Showcase
    </h2>
  </div>

  <style>
    .glass-card {
      position: absolute;
      width: 210px;
      height: 280px;
      background: rgba(15, 23, 42, 0.6);
      backdrop-filter: blur(15px);
      border: 1px solid rgba(255, 255, 255, 0.08);
      border-radius: 18px;
      display: flex;
      flex-direction: column;
      justify-content: space-between;
      align-items: center;
      padding: 30px 20px;
      box-sizing: border-box;
      box-shadow: 0 15px 35px rgba(0,0,0,0.4), inset 0 0 15px rgba(255, 255, 255, 0.03);
      z-index: 10;
      transition: all 0.3s ease;
    }
    
    .card-title {
      font-family: monospace;
      font-size: 15px;
      font-weight: bold;
      color: #60a5fa;
      background: rgba(59, 130, 246, 0.1);
      padding: 4px 8px;
      border-radius: 6px;
      border: 1px solid rgba(59, 130, 246, 0.2);
    }
    .card-icon {
      font-size: 32px;
      margin: 15px 0;
    }
    .card-desc {
      font-size: 13px;
      color: #94a3b8;
      line-height: 1.5;
      text-align: center;
    }
  </style>

  <!-- Floating tool cards symmetrically spaced -->
  <div class="glass-card" id="card-0" style="left: 55px; top: 220px; border-color: rgba(59, 130, 246, 0.25);">
    <div class="card-title">generate_image</div>
    <div class="card-icon">🎨</div>
    <div class="card-desc">AI Image synthesis, inpainting, and upscaling models.</div>
  </div>

  <div class="glass-card" id="card-1" style="left: 295px; top: 220px; border-color: rgba(236, 72, 153, 0.25);">
    <div class="card-title">animate_svg</div>
    <div class="card-icon">✨</div>
    <div class="card-desc">Preset vector animations, spinner, & Lottie converter.</div>
  </div>

  <div class="glass-card" id="card-2" style="left: 535px; top: 220px; border-color: rgba(16, 185, 129, 0.25);">
    <div class="card-title">video_create</div>
    <div class="card-icon">🎬</div>
    <div class="card-desc">Renders rich JSON DSL structures to MP4 locally.</div>
  </div>

  <div class="glass-card" id="card-3" style="left: 775px; top: 220px; border-color: rgba(245, 158, 11, 0.25);">
    <div class="card-title">create_chart</div>
    <div class="card-icon">📊</div>
    <div class="card-desc">Compiles line, bar, pie, and radar charts.</div>
  </div>

  <div class="glass-card" id="card-4" style="left: 1015px; top: 220px; border-color: rgba(139, 92, 246, 0.25);">
    <div class="card-title">diagram_mermaid</div>
    <div class="card-icon">🧬</div>
    <div class="card-desc">Renders flowchart and ER markdown offline.</div>
  </div>

  <script>
    (function() {
      const canvas = document.getElementById('canvas3');
      const ctx = canvas.getContext('2d');
      const w = canvas.width;
      const h = canvas.height;
      const wrapper = canvas.closest('[data-scene-time]');
      const t = wrapper ? parseFloat(wrapper.getAttribute('data-scene-time')) : 0.0;
      
      function hash(x) {
        const s = Math.sin(x) * 1e4;
        return s - Math.floor(s);
      }

      // Draw neural network backdrop
      const numNodes = 30;
      const nodes = [];
      for (let i = 0; i < numNodes; i++) {
        const seed = i * 149.3;
        const ix = hash(seed) * w;
        const iy = hash(seed + 1) * h;
        const dx = -35 + hash(seed + 2) * 70;
        const dy = -35 + hash(seed + 3) * 70;
        const x = (ix + dx * t + w) % w;
        const y = (iy + dy * t + h) % h;
        nodes.push({ x, y });
      }

      ctx.strokeStyle = 'rgba(59, 130, 246, 0.08)';
      ctx.lineWidth = 1;
      for (let i = 0; i < numNodes; i++) {
        for (let j = i + 1; j < numNodes; j++) {
          const d = Math.hypot(nodes[i].x - nodes[j].x, nodes[i].y - nodes[j].y);
          if (d < 240) {
            ctx.beginPath();
            ctx.moveTo(nodes[i].x, nodes[i].y);
            ctx.lineTo(nodes[j].x, nodes[j].y);
            ctx.stroke();
          }
        }
      }

      nodes.forEach(n => {
        ctx.fillStyle = 'rgba(59, 130, 246, 0.15)';
        ctx.beginPath();
        ctx.arc(n.x, n.y, 3, 0, Math.PI * 2);
        ctx.fill();
      });

      // Animate cards pop-in & floating
      for (let i = 0; i < 5; i++) {
        const card = document.getElementById(`card-${i}`);
        if (!card) continue;
        
        // Stagger entrance trigger times
        const trigger = i * 0.25;
        if (t < trigger) {
          card.style.opacity = 0;
          card.style.transform = 'scale(0.85) translateY(40px) perspective(800px) rotateY(15deg)';
        } else {
          const enterT = Math.min(1.0, (t - trigger) * 2.5); // bounce pop-in
          
          // Hover-like cinematic floating offset
          const floatOffset = Math.sin(t * 2.2 + i * 1.6) * 12;
          const tiltY = 10 + Math.cos(t * 1.5 + i) * 3;
          const tiltX = Math.sin(t * 1.8 + i) * 2;
          
          card.style.opacity = enterT;
          card.style.transform = `scale(${0.85 + enterT * 0.15}) translateY(${floatOffset}px) perspective(800px) rotateY(${tiltY}deg) rotateX(${tiltX}deg)`;
        }
      }
    })();
  </script>
</div>
"##;

    // HTML Content for Scene 4: AI Agent Creates Everything
    let scene_4_html = r##"
<div style="position: relative; width: 1280px; height: 720px; background: #020308; color: #ffffff; overflow: hidden; font-family: 'Inter', sans-serif; box-sizing: border-box;">
  <canvas id="canvas4" width="1280" height="720" style="position: absolute; top: 0; left: 0; width: 100%; height: 100%; z-index: 1;"></canvas>
  
  <!-- Header -->
  <div style="position: absolute; top: 6%; width: 100%; text-align: center; z-index: 10;">
    <p style="font-size: 13px; font-weight: 700; color: #10b981; letter-spacing: 5px; text-transform: uppercase; margin: 0;">
      Autonomous Creation Loop
    </p>
    <h2 style="font-size: 38px; font-weight: 800; color: #ffffff; margin: 5px 0 0 0; letter-spacing: -1px; text-shadow: 0 0 15px rgba(255,255,255,0.15);">
      AI Agents Scaffold & Draw Instantly
    </h2>
  </div>

  <style>
    .window-card {
      position: absolute;
      width: 320px;
      height: 290px;
      background: rgba(15, 23, 42, 0.65);
      border: 1px solid rgba(255, 255, 255, 0.08);
      border-radius: 14px;
      overflow: hidden;
      box-shadow: 0 12px 30px rgba(0,0,0,0.4);
      z-index: 10;
    }
    .window-header {
      height: 32px;
      background: rgba(30, 41, 59, 0.5);
      border-bottom: 1px solid rgba(255, 255, 255, 0.06);
      display: flex;
      align-items: center;
      padding: 0 12px;
      font-size: 12px;
      color: #94a3b8;
      font-family: monospace;
    }
    .window-dots {
      display: flex;
      gap: 6px;
      margin-right: 12px;
    }
    .dot {
      width: 9px;
      height: 9px;
      border-radius: 50%;
    }
    .window-content {
      padding: 16px;
      height: calc(100% - 32px);
      box-sizing: border-box;
      overflow: hidden;
    }
  </style>

  <!-- Left: Typewriter Terminal -->
  <div class="window-card" id="win-left" style="left: 80px; top: 180px; border-color: rgba(59, 130, 246, 0.25);">
    <div class="window-header">
      <div class="window-dots">
        <div class="dot" style="background: #ef4444;"></div>
        <div class="dot" style="background: #f59e0b;"></div>
        <div class="dot" style="background: #10b981;"></div>
      </div>
      main.rs
    </div>
    <div class="window-content" style="background: #090d16;">
      <pre id="typewriter-code" style="margin: 0; font-family: monospace; font-size: 11.5px; color: #a7f3d0; line-height: 1.45; white-space: pre-wrap;"></pre>
    </div>
  </div>

  <!-- Middle: Growing Chart (Canvas resized to 280x220 to avoid content overflow) -->
  <div class="window-card" id="win-mid" style="left: 480px; top: 220px; border-color: rgba(16, 185, 129, 0.25); transform: scale(1.08);">
    <div class="window-header">
      <div class="window-dots">
        <div class="dot" style="background: #ef4444;"></div>
        <div class="dot" style="background: #f59e0b;"></div>
        <div class="dot" style="background: #10b981;"></div>
      </div>
      chart_gen.svg
    </div>
    <div class="window-content" style="display: flex; justify-content: center; align-items: center;">
      <canvas id="chart-canvas" width="280" height="220"></canvas>
    </div>
  </div>

  <!-- Right: Drawing SVG -->
  <div class="window-card" id="win-right" style="left: 880px; top: 180px; border-color: rgba(139, 92, 246, 0.25);">
    <div class="window-header">
      <div class="window-dots">
        <div class="dot" style="background: #ef4444;"></div>
        <div class="dot" style="background: #f59e0b;"></div>
        <div class="dot" style="background: #10b981;"></div>
      </div>
      network.svg
    </div>
    <div class="window-content" id="svg-diag-container" style="display: flex; justify-content: center; align-items: center; background: #08070d;">
      <!-- Drawing vector diagram paths -->
      <svg viewBox="0 0 200 200" width="160" height="160">
        <circle cx="100" cy="40" r="16" fill="none" stroke="#8b5cf6" stroke-width="2.5" />
        <circle cx="40" cy="140" r="16" fill="none" stroke="#06b6d4" stroke-width="2.5" />
        <circle cx="160" cy="140" r="16" fill="none" stroke="#ec4899" stroke-width="2.5" />
        
        <!-- Connecting paths -->
        <path d="M 100 56 L 40 124" fill="none" stroke="#ffffff" stroke-width="2" stroke-linecap="round" />
        <path d="M 100 56 L 160 124" fill="none" stroke="#ffffff" stroke-width="2" stroke-linecap="round" />
        <path d="M 56 140 L 144 140" fill="none" stroke="#ffffff" stroke-width="1.5" stroke-dasharray="4 4" />
      </svg>
    </div>
  </div>

  <script>
    (function() {
      const canvas = document.getElementById('canvas4');
      const ctx = canvas.getContext('2d');
      const w = canvas.width;
      const h = canvas.height;
      const wrapper = canvas.closest('[data-scene-time]');
      const t = wrapper ? parseFloat(wrapper.getAttribute('data-scene-time')) : 0.0;

      // Glow lines connecting windows
      const win1 = { x: 240, y: 320 };
      const win2 = { x: 640, y: 360 };
      const win3 = { x: 1040, y: 320 };
      
      ctx.strokeStyle = 'rgba(16, 185, 129, 0.15)';
      ctx.lineWidth = 2.5;
      ctx.setLineDash([6, 12]);
      ctx.lineDashOffset = -t * 40;
      ctx.beginPath();
      ctx.moveTo(win1.x, win1.y);
      ctx.lineTo(win2.x, win2.y);
      ctx.lineTo(win3.x, win3.y);
      ctx.stroke();
      ctx.setLineDash([]);

      // Code typewriter
      const codeEl = document.getElementById('typewriter-code');
      if (codeEl) {
        const fullText = "let mcp = OpenMediaServer::new().await?;\n\n// generate interactive video\nlet scene = json!({\n  \"width\": 1280,\n  \"height\": 720,\n  \"scenes\": [ ... ]\n});\n\nmcp.video_create(scene).await?;";
        const chars = Math.floor(t * 36);
        codeEl.textContent = fullText.slice(0, chars) + (chars % 2 === 0 ? '_' : ' ');
      }

      // Chart Drawing logic (cw=280, ch=220)
      const chartCanvas = document.getElementById('chart-canvas');
      if (chartCanvas) {
        const cctx = chartCanvas.getContext('2d');
        const cw = chartCanvas.width;
        const ch = chartCanvas.height;
        cctx.clearRect(0, 0, cw, ch);
        
        const scale = Math.min(1.0, t / 2.0); // complete growth in 2s
        const barHeights = [45, 110, 70, 130, 95];
        const colors = ['#3b82f6', '#10b981', '#f59e0b', '#ec4899', '#8b5cf6'];
        const barW = 30;
        const gap = 15;
        const startX = 30;

        // Draw horizontal grid lines
        cctx.strokeStyle = 'rgba(255, 255, 255, 0.06)';
        cctx.lineWidth = 1;
        for (let y = 30; y < ch - 30; y += 40) {
          cctx.beginPath();
          cctx.moveTo(15, y);
          cctx.lineTo(cw - 15, y);
          cctx.stroke();
        }

        // Draw bars
        barHeights.forEach((hVal, idx) => {
          const x = startX + idx * (barW + gap);
          const drawH = hVal * scale;
          const y = ch - 40 - drawH;
          
          cctx.fillStyle = colors[idx];
          cctx.beginPath();
          cctx.roundRect(x, y, barW, drawH, [4, 4, 0, 0]);
          cctx.fill();
        });
      }

      // Self-drawing SVG Paths
      const svgDiag = document.getElementById('svg-diag-container');
      if (svgDiag) {
        const paths = svgDiag.querySelectorAll('path, circle');
        paths.forEach((path) => {
          let length = path.getAttribute('data-len');
          if (!length) {
            length = path.getTotalLength ? path.getTotalLength() : 250;
            path.setAttribute('data-len', length);
            path.style.strokeDasharray = length;
          }
          const progress = Math.min(1.0, t / 3.0); // draw completely in 3s
          path.style.strokeDashoffset = length * (1.0 - progress);
        });
      }
    })();
  </script>
</div>
"##;

    // HTML Content for Scene 5: Performance Comparison
    let scene_5_html = r##"
<div style="position: relative; width: 1280px; height: 720px; overflow: hidden; font-family: 'Inter', sans-serif; display: flex; box-sizing: border-box; background: #000;">
  
  <style>
    @keyframes local-shake {
      0%, 100% { transform: translate(0, 0); }
      10% { transform: translate(-3px, 2px) rotate(-0.2deg); }
      30% { transform: translate(3px, -2px) rotate(0.2deg); }
      50% { transform: translate(-2px, 3px) rotate(-0.5deg); }
      70% { transform: translate(3px, 2px) rotate(0.5deg); }
      90% { transform: translate(-3px, -2px) rotate(-0.2deg); }
    }
    .local-shake-anim {
      display: inline-block;
      animation: local-shake 0.12s infinite;
    }
  </style>

  <!-- Left Side: Legacy Chaos -->
  <div style="width: 50%; height: 100%; background: radial-gradient(circle at center, #1b0a0a 0%, #030101 100%); border-right: 2px solid #ef4444; position: relative; box-sizing: border-box; display: flex; flex-direction: column; justify-content: center; align-items: center; padding: 40px;">
    <div style="position: absolute; top: 10%; text-align: center;">
      <p style="font-size: 12px; font-weight: 700; color: #ef4444; letter-spacing: 4px; text-transform: uppercase; margin: 0;">Legacy AI Tools</p>
      <h3 style="font-size: 32px; font-weight: 800; color: #fca5a5; margin: 5px 0 0 0; text-transform: uppercase;">Python / PyTorch Stack</h3>
    </div>

    <!-- Gauge (explicit content-box model to align absolute circle correctly) -->
    <div style="width: 280px; height: 280px; border-radius: 50%; border: 6px solid #374151; position: relative; display: flex; flex-direction: column; justify-content: center; align-items: center; background: rgba(0,0,0,0.3); box-sizing: content-box;">
      <div id="left-memory-val" style="font-size: 40px; font-weight: 900; color: #ef4444; text-shadow: 0 0 15px rgba(239,68,68,0.5);">12.5 GB</div>
      <div style="font-size: 13px; color: #9ca3af; margin-top: 5px; text-transform: uppercase; font-weight: bold; letter-spacing: 1px;">RAM FOOTPRINT</div>
      
      <!-- Exploding progress indicator outer circle -->
      <svg style="position: absolute; top: -6px; left: -6px; width: 280px; height: 280px; transform: rotate(-90deg);">
        <circle cx="140" cy="140" r="134" fill="none" stroke="#ef4444" stroke-width="6" id="left-circle" stroke-dasharray="841" stroke-dashoffset="300" stroke-opacity="0.8" />
      </svg>
    </div>
    
    <div style="margin-top: 30px; font-family: monospace; font-size: 14px; color: #fca5a5; background: rgba(239, 68, 68, 0.1); border: 1px solid rgba(239, 68, 68, 0.2); padding: 12px 20px; border-radius: 8px; width: 280px; text-align: center; height: 46px; box-sizing: border-box; display: flex; justify-content: center; align-items: center;">
      <div id="left-oom-msg" style="font-weight: bold;" class="local-shake-anim">SPIKING CPU & RUNTIME OVERHEAD</div>
    </div>
  </div>

  <!-- Right Side: OpenMedia Rust -->
  <div style="width: 50%; height: 100%; background: radial-gradient(circle at center, #0b1a29 0%, #010307 100%); position: relative; box-sizing: border-box; display: flex; flex-direction: column; justify-content: center; align-items: center; padding: 40px;">
    <div style="position: absolute; top: 10%; text-align: center;">
      <p style="font-size: 12px; font-weight: 700; color: #10b981; letter-spacing: 4px; text-transform: uppercase; margin: 0;">OpenMedia-RS</p>
      <h3 style="font-size: 32px; font-weight: 800; color: #a7f3d0; margin: 5px 0 0 0; text-transform: uppercase;">Rust CPU-Engine</h3>
    </div>

    <!-- Gauge -->
    <div style="width: 280px; height: 280px; border-radius: 50%; border: 6px solid #1e293b; position: relative; display: flex; flex-direction: column; justify-content: center; align-items: center; background: rgba(0,0,0,0.3); box-shadow: 0 0 30px rgba(16, 185, 129, 0.1); box-sizing: content-box;">
      <div id="right-memory-val" style="font-size: 40px; font-weight: 900; color: #10b981; text-shadow: 0 0 15px rgba(16,185,129,0.5);">42.3 MB</div>
      <div style="font-size: 13px; color: #9ca3af; margin-top: 5px; text-transform: uppercase; font-weight: bold; letter-spacing: 1px;">RAM FOOTPRINT</div>
      
      <!-- Clean progress indicator outer circle -->
      <svg style="position: absolute; top: -6px; left: -6px; width: 280px; height: 280px; transform: rotate(-90deg);">
        <circle cx="140" cy="140" r="134" fill="none" stroke="#10b981" stroke-width="6" id="right-circle" stroke-dasharray="841" stroke-dashoffset="800" stroke-opacity="0.8" />
      </svg>
    </div>

    <div style="margin-top: 30px; font-family: monospace; font-size: 14px; color: #a7f3d0; background: rgba(16, 185, 129, 0.1); border: 1px solid rgba(16, 185, 129, 0.2); padding: 12px 20px; border-radius: 8px; width: 280px; text-align: center; height: 46px; box-sizing: border-box; display: flex; justify-content: center; align-items: center;">
      <div style="font-weight: bold;">LIGHTWEIGHT VECTOR CORE &lt; 50MB</div>
    </div>
  </div>

  <script>
    (function() {
      const leftMem = document.getElementById('left-memory-val');
      const rightMem = document.getElementById('right-memory-val');
      const leftCirc = document.getElementById('left-circle');
      const rightCirc = document.getElementById('right-circle');
      const leftOom = document.getElementById('left-oom-msg');
      
      if (!leftMem) return;
      const wrapper = leftMem.closest('[data-scene-time]');
      const t = wrapper ? parseFloat(wrapper.getAttribute('data-scene-time')) : 0.0;
      
      // Calculate dynamic memory footprint
      let leftVal = 4.2 + t * 4.8 + Math.sin(t * 12) * 0.4;
      if (t > 2.5) {
        leftVal = 16.0;
        leftMem.innerHTML = "OOM CRASH!";
        leftMem.style.color = "#f87171";
        leftCirc.setAttribute('stroke', '#ef4444');
        leftCirc.style.strokeDashoffset = "0";
        if (leftOom) {
          leftOom.innerHTML = "FATAL: HEAP EXPLODED";
          leftOom.style.color = "#ef4444";
          leftOom.parentElement.style.background = "rgba(239, 68, 68, 0.25)";
          leftOom.parentElement.style.borderColor = "#ef4444";
        }
      } else {
        leftMem.innerHTML = leftVal.toFixed(1) + " GB";
        const offset = 841 - (leftVal / 16.0) * 841;
        leftCirc.style.strokeDashoffset = offset;
      }

      // Right remains tiny and constant
      const rightVal = 42.1 + Math.sin(t * 2) * 0.8;
      rightMem.innerHTML = rightVal.toFixed(1) + " MB";
      const offsetRight = 841 - (rightVal / 100.0) * 841;
      rightCirc.style.strokeDashoffset = offsetRight;
    })();
  </script>
</div>
"##;

    // HTML Content for Scene 6: Hero Ending
    let scene_6_html = r##"
<div style="position: relative; width: 1280px; height: 720px; background: #020306; overflow: hidden; font-family: 'Inter', sans-serif; box-sizing: border-box;">
  <canvas id="canvas6" width="1280" height="720" style="position: absolute; top: 0; left: 0; width: 100%; height: 100%; z-index: 1;"></canvas>

  <!-- Central Logo Emerging -->
  <div id="end-logo-container" style="position: absolute; top: 40%; left: 50%; transform: translate(-50%, -50%) scale(0.6); width: 260px; height: 260px; z-index: 10; opacity: 0;">
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 320 320" width="100%" height="100%">
      <defs>
        <linearGradient id="cyber-cyan-pink" x1="0%" y1="0%" x2="100%" y2="100%">
          <stop offset="0%" stop-color="#00f2fe" />
          <stop offset="50%" stop-color="#9b51e0" />
          <stop offset="100%" stop-color="#4facfe" />
        </linearGradient>
        <linearGradient id="neon-glow-grad" x1="0%" y1="100%" x2="100%" y2="0%">
          <stop offset="0%" stop-color="#f857a6" />
          <stop offset="100%" stop-color="#ff5858" />
        </linearGradient>
        <filter id="ultra-glow" x="-50%" y="-50%" width="200%" height="200%">
          <feGaussianBlur stdDeviation="8" result="blur1" />
          <feGaussianBlur stdDeviation="3" result="blur2" />
          <feMerge>
            <feMergeNode in="blur1" />
            <feMergeNode in="blur2" />
            <feMergeNode in="SourceGraphic" />
          </feMerge>
        </filter>
      </defs>
      <!-- Rings -->
      <circle id="end-ring-outer" cx="160" cy="160" r="135" fill="none" stroke="url(#cyber-cyan-pink)" stroke-width="1.5" stroke-dasharray="4 8 16 8 64 12" stroke-opacity="0.4" />
      <circle id="end-ring-inner" cx="160" cy="160" r="122" fill="none" stroke="url(#neon-glow-grad)" stroke-width="2" stroke-dasharray="120 40 80 40" stroke-opacity="0.6" />
      <!-- Waveforms -->
      <path d="M 50 160 C 85 110, 115 210, 160 160 C 205 110, 235 210, 270 160" fill="none" stroke="url(#cyber-cyan-pink)" stroke-width="3" stroke-linecap="round" stroke-opacity="0.8" filter="url(#ultra-glow)" />
      <path d="M 50 160 C 85 210, 115 110, 160 160 C 205 210, 235 110, 270 160" fill="none" stroke="url(#neon-glow-grad)" stroke-width="2" stroke-linecap="round" stroke-opacity="0.7" filter="url(#ultra-glow)" />
      <!-- Glowing Core -->
      <circle cx="160" cy="160" r="48" fill="none" stroke="url(#cyber-cyan-pink)" stroke-width="3.5" filter="url(#ultra-glow)" />
      <path d="M 148 132 L 186 160 L 148 188 Z" fill="url(#neon-glow-grad)" stroke="#ffffff" stroke-width="1.5" stroke-linejoin="round" filter="url(#ultra-glow)" />
    </svg>
  </div>

  <!-- Flying mockups wrapping during consolidation -->
  <div class="mockup" id="mock-0" style="position: absolute; width: 150px; height: 100px; background: rgba(59,130,246,0.15); border: 1px solid rgba(59,130,246,0.3); border-radius: 8px; z-index: 5; display: flex; justify-content: center; align-items: center; color: #60a5fa; font-family: monospace; font-size: 13px; font-weight: bold; box-shadow: 0 0 15px rgba(59,130,246,0.15);">VIDEO COMPOSER</div>
  <div class="mockup" id="mock-1" style="position: absolute; width: 150px; height: 100px; background: rgba(16,185,129,0.15); border: 1px solid rgba(16,185,129,0.3); border-radius: 8px; z-index: 5; display: flex; justify-content: center; align-items: center; color: #34d399; font-family: monospace; font-size: 13px; font-weight: bold; box-shadow: 0 0 15px rgba(16,185,129,0.15);">SVG ANIMATOR</div>
  <div class="mockup" id="mock-2" style="position: absolute; width: 150px; height: 100px; background: rgba(245,158,11,0.15); border: 1px solid rgba(245,158,11,0.3); border-radius: 8px; z-index: 5; display: flex; justify-content: center; align-items: center; color: #fbbf24; font-family: monospace; font-size: 13px; font-weight: bold; box-shadow: 0 0 15px rgba(245,158,11,0.15);">DATA CHARTS</div>
  <div class="mockup" id="mock-3" style="position: absolute; width: 150px; height: 100px; background: rgba(236,72,153,0.15); border: 1px solid rgba(236,72,153,0.3); border-radius: 8px; z-index: 5; display: flex; justify-content: center; align-items: center; color: #f472b6; font-family: monospace; font-size: 13px; font-weight: bold; box-shadow: 0 0 15px rgba(236,72,153,0.15);">AI IMAGES</div>

  <!-- End Text Titles -->
  <div id="end-titles" style="position: absolute; bottom: 8%; left: 50%; transform: translateX(-50%); text-align: center; z-index: 10; opacity: 0;">
    <h2 id="final-main-text" style="font-size: 44px; font-weight: 900; color: #ffffff; text-shadow: 0 0 25px rgba(0, 242, 254, 0.7); letter-spacing: 8px; margin: 0; text-transform: uppercase;">OPENMEDIA-RS</h2>
    <p id="final-sub-text" style="font-size: 16px; color: #94a3b8; font-weight: 600; letter-spacing: 4px; margin: 8px 0 0 0; text-transform: uppercase;">The Media Engine for AI Agents</p>
  </div>

  <script>
    (function() {
      const canvas = document.getElementById('canvas6');
      const ctx = canvas.getContext('2d');
      const w = canvas.width;
      const h = canvas.height;
      const wrapper = canvas.closest('[data-scene-time]');
      const t = wrapper ? parseFloat(wrapper.getAttribute('data-scene-time')) : 0.0;
      
      const cx = w / 2;
      const cy = h / 2;
      
      // Control UI timings
      const endLogo = document.getElementById('end-logo-container');
      const endTitles = document.getElementById('end-titles');
      const finalMain = document.getElementById('final-main-text');
      const finalSub = document.getElementById('final-sub-text');

      if (endLogo) {
        // Logo appears after 2.0s
        if (t >= 2.0) {
          const logoProgress = Math.min(1.0, (t - 2.0) * 1.5);
          endLogo.style.opacity = logoProgress;
          const scale = 0.6 + logoProgress * 0.5 + Math.sin(t * 3.5) * 0.03;
          endLogo.style.transform = `translate(-50%, -50%) scale(${scale})`;
        } else {
          endLogo.style.opacity = 0;
        }
      }

      // Rotate logo rings
      const ringOuter = document.getElementById('end-ring-outer');
      const ringInner = document.getElementById('end-ring-inner');
      if (ringOuter && ringInner) {
        ringOuter.setAttribute('transform', `rotate(${t * 20} 160 160)`);
        ringInner.setAttribute('transform', `rotate(${-t * 30} 160 160)`);
      }

      if (endTitles) {
        // Texts fade-in at 2.6s
        if (t >= 2.6) {
          endTitles.style.opacity = Math.min(1.0, (t - 2.6) * 1.5);
        } else {
          endTitles.style.opacity = 0;
        }
        
        // Final punchy tagline replacement at 4.6s
        if (t >= 4.6) {
          if (finalMain) {
            finalMain.innerHTML = "ONE MCP";
            finalMain.style.color = "#10b981";
            finalMain.style.textShadow = "0 0 25px rgba(16, 185, 129, 0.7)";
          }
          if (finalSub) {
            finalSub.innerHTML = "ENDLESS CREATION.";
            finalSub.style.color = "#34d399";
          }
        }
      }

      function hash(x) {
        const s = Math.sin(x) * 1e4;
        return s - Math.floor(s);
      }

      // 4 Mockup components flying to center
      const initialPositions = [
        { x: 100, y: 100 },
        { x: 1030, y: 100 },
        { x: 100, y: 520 },
        { x: 1030, y: 520 }
      ];

      for (let i = 0; i < 4; i++) {
        const mock = document.getElementById(`mock-${i}`);
        if (!mock) continue;
        
        if (t < 2.0) {
          const progress = t / 2.0;
          const currentX = initialPositions[i].x + (cx - 75 - initialPositions[i].x) * progress;
          const currentY = initialPositions[i].y + (cy - 50 - initialPositions[i].y) * progress;
          const scale = 1.0 - progress * 0.5;
          const rotate = progress * 180 * (i % 2 === 0 ? 1 : -1);
          
          mock.style.left = `${currentX}px`;
          mock.style.top = `${currentY}px`;
          mock.style.transform = `scale(${scale}) rotate(${rotate}deg)`;
          mock.style.opacity = 1.0 - progress;
        } else {
          mock.style.opacity = 0;
        }
      }

      // Particle universe
      ctx.fillStyle = '#020306';
      ctx.fillRect(0, 0, w, h);

      // Radial background flare
      if (t >= 2.0) {
        const flareSize = Math.min(300, (t - 2.0) * 300);
        const grad = ctx.createRadialGradient(cx, cy, 20, cx, cy, flareSize);
        grad.addColorStop(0, `rgba(0, 242, 254, ${0.25 * Math.max(0, 1.0 - (t - 2.0) * 0.3)})`);
        grad.addColorStop(0.6, `rgba(155, 81, 224, ${0.08 * Math.max(0, 1.0 - (t - 2.0) * 0.3)})`);
        grad.addColorStop(1, 'rgba(0,0,0,0)');
        ctx.fillStyle = grad;
        ctx.beginPath();
        ctx.arc(cx, cy, flareSize, 0, Math.PI * 2);
        ctx.fill();
      }

      // Particles contracting then pulsing outwards
      const numParticles = 140;
      for (let i = 0; i < numParticles; i++) {
        const seed = i * 69.2;
        const angle = hash(seed) * Math.PI * 2;
        const initialRadius = 380 + hash(seed + 1) * 320;
        
        let radius;
        let alpha;
        if (t < 2.0) {
          const progress = t / 2.0;
          radius = initialRadius * (1.0 - progress);
          alpha = 0.5 * (1.0 - progress);
        } else {
          const expT = t - 2.0;
          radius = expT * 600 + hash(seed + 3) * 50;
          alpha = Math.max(0, 0.6 - expT * 0.8);
        }
        
        const px = cx + Math.cos(angle) * radius;
        const py = cy + Math.sin(angle) * radius;
        const size = 1.5 + hash(seed + 2) * 3.0;

        ctx.fillStyle = `rgba(0, 242, 254, ${alpha})`;
        ctx.beginPath();
        ctx.arc(px, py, size, 0, Math.PI * 2);
        ctx.fill();
      }
    })();
  </script>
</div>
"##;

    let scene_json = json!({
        "width": 1280,
        "height": 720,
        "fps": 24,
        "duration": 28.0,
        "background": "#020306",
        "scenes": [
            {
                "id": "scene_1",
                "start": 0.0,
                "end": 4.0,
                "elements": [
                    {
                        "type": "html",
                        "content": scene_1_html,
                        "position": { "x": 0.0, "y": 0.0 },
                        "size": { "width": "100%", "height": "100%" },
                        "timeline": {
                            "keyframes": [
                                { "time": 0.0, "scale": 1.0 },
                                { "time": 4.0, "scale": 1.05 }
                            ]
                        }
                    }
                ]
            },
            {
                "id": "scene_2",
                "start": 4.0,
                "end": 8.0,
                "elements": [
                    {
                        "type": "html",
                        "content": scene_2_html,
                        "position": { "x": 0.0, "y": 0.0 },
                        "size": { "width": "100%", "height": "100%" },
                        "timeline": {
                            "keyframes": [
                                { "time": 0.0, "scale": 0.95, "opacity": 0.0 },
                                { "time": 0.8, "scale": 1.0, "opacity": 1.0 },
                                { "time": 4.0, "scale": 1.1, "opacity": 1.0 }
                            ]
                        }
                    }
                ]
            },
            {
                "id": "scene_3",
                "start": 8.0,
                "end": 13.0,
                "elements": [
                    {
                        "type": "html",
                        "content": scene_3_html,
                        "position": { "x": 0.0, "y": 0.0 },
                        "size": { "width": "100%", "height": "100%" },
                        "timeline": {
                            "keyframes": [
                                { "time": 0.0, "scale": 1.0, "opacity": 0.0 },
                                { "time": 0.6, "scale": 1.01, "opacity": 1.0 },
                                { "time": 5.0, "scale": 1.08, "opacity": 1.0 }
                            ]
                        }
                    }
                ]
            },
            {
                "id": "scene_4",
                "start": 13.0,
                "end": 18.0,
                "elements": [
                    {
                        "type": "html",
                        "content": scene_4_html,
                        "position": { "x": 0.0, "y": 0.0 },
                        "size": { "width": "100%", "height": "100%" },
                        "timeline": {
                            "keyframes": [
                                { "time": 0.0, "scale": 1.05, "opacity": 0.0 },
                                { "time": 0.6, "scale": 1.03, "opacity": 1.0 },
                                { "time": 5.0, "scale": 0.96, "opacity": 1.0 }
                            ]
                        }
                    }
                ]
            },
            {
                "id": "scene_5",
                "start": 18.0,
                "end": 22.0,
                "elements": [
                    {
                        "type": "html",
                        "content": scene_5_html,
                        "position": { "x": 0.0, "y": 0.0 },
                        "size": { "width": "100%", "height": "100%" },
                        "timeline": {
                            "keyframes": [
                                { "time": 0.0, "scale": 1.0, "opacity": 0.0 },
                                { "time": 0.6, "scale": 1.01, "opacity": 1.0 },
                                { "time": 4.0, "scale": 1.05, "opacity": 1.0 }
                            ]
                        }
                    }
                ]
            },
            {
                "id": "scene_6",
                "start": 22.0,
                "end": 28.0,
                "elements": [
                    {
                        "type": "html",
                        "content": scene_6_html,
                        "position": { "x": 0.0, "y": 0.0 },
                        "size": { "width": "100%", "height": "100%" },
                        "timeline": {
                            "keyframes": [
                                { "time": 0.0, "scale": 0.95, "opacity": 0.0 },
                                { "time": 0.6, "scale": 1.0, "opacity": 1.0 },
                                { "time": 6.0, "scale": 1.15, "opacity": 1.0 }
                            ]
                        }
                    }
                ]
            }
        ],
        "transitions": [
            {
                "from": "scene_1",
                "to": "scene_2",
                "type": "glitch",
                "duration": 0.8
            },
            {
                "from": "scene_2",
                "to": "scene_3",
                "type": "crossfade",
                "duration": 0.6
            },
            {
                "from": "scene_3",
                "to": "scene_4",
                "type": "blur",
                "duration": 0.6
            },
            {
                "from": "scene_4",
                "to": "scene_5",
                "type": "glitch",
                "duration": 0.6
            },
            {
                "from": "scene_5",
                "to": "scene_6",
                "type": "radial_wipe",
                "duration": 0.8
            }
        ]
    });

    let request = VideoCreateRequest {
        scene: scene_json,
        output_path: Some("openmedia_promo.mp4".to_string()),
    };

    println!("Invoking OpenMediaServer::video_create MCP method to render promo video...");
    let result = server.video_create(Parameters(request)).await;

    match result {
        Ok(json_response) => {
            println!("SUCCESS: Promo video rendered successfully!");
            println!(
                "Response: {}",
                serde_json::to_string_pretty(&json_response.0).unwrap()
            );
        }
        Err(err) => {
            eprintln!("ERROR: Failed to render promo video: {}", err);
        }
    }
}
