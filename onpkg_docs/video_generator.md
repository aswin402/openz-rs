---
name: video_generator
description: "AI Agent Skill for the generate_video tool — teaches the agent how to compose timeline-based MP4 videos using the wavyte crate's Composition API, including tracks, clips, assets, keyframe animations, transitions, and effects."
metadata:
  version: 1.0.0
---

# Video Generator Skill 🎬

This skill guides the agent in using the `generate_video` tool to create programmatic MP4 videos using the **wavyte** crate. Wavyte is a Rust composition engine that renders vector-path based timelines to MP4 via a CPU rasterizer and FFmpeg.

---

## 1. Core Concepts

### Composition Model

A `wavyte::Composition` describes everything needed to render a video:

```
Composition
├── fps        — frame rate (e.g. 30/1 = 30fps)
├── canvas     — width × height in pixels
├── duration   — total length in frames (FrameIndex)
├── assets     — named library of reusable visual primitives
│   ├── "rect"   → Asset::Path { svg_path_d: "M0,0 L100,0 L100,100 L0,100 Z" }
│   └── "text"   → Asset::Text { content: "Hello", font_size: 32.0, ... }
└── tracks     — ordered list of Track objects (z-ordered, composited)
    └── Track
        ├── name
        ├── z_base        — base z-order for all clips in this track
        ├── layout_mode   — Absolute | Row | Column | Grid
        └── clips         — Vec<Clip>
            └── Clip
                ├── id          — unique clip id
                ├── asset       — key into assets map
                ├── range       — FrameRange(start..end)
                └── props       — ClipProps
                    ├── transform: Anim<Transform2D>  — animated position/scale/rotate
                    ├── opacity:   Anim<f32>           — animated opacity 0.0-1.0
                    └── blend:     BlendMode           — Normal | Multiply | Screen | ...
```

### Keyframe Animation (`Anim<T>`)

`Anim<T>` is either:
- **Constant**: `Anim::constant(value)` — no change over time
- **Keyframed**: `Anim::keyframes(vec![(frame, value, easing), ...])` — interpolated

```rust
// Slide in from left: x goes from -200 to 50 over frames 0-30
transform: Anim::keyframes(vec![
    (FrameIndex(0),  Transform2D { translate: Vec2::new(-200.0, 100.0), ..Default::default() }, Easing::EaseOut),
    (FrameIndex(30), Transform2D { translate: Vec2::new(50.0, 100.0), ..Default::default() },   Easing::Linear),
]),
```

### Asset Types

```rust
// Vector path (SVG path data)
Asset::Path(PathAsset {
    svg_path_d: "M0,0 L200,0 L200,60 L0,60 Z".to_string(),
})

// Text label
Asset::Text(TextAsset {
    content: "Hello World".to_string(),
    font_size: 36.0,
    color: [255, 255, 255, 255], // RGBA
    font_family: None, // uses default
})
```

---

## 2. JSON Composition Format

The `generate_video` tool accepts the composition as a **serialized JSON string** via the `composition_json` parameter. Build the full Rust `Composition` struct, serialize it with `serde_json::to_string(&comp)`, then pass that string.

### Minimal example (static rect, 3 seconds @ 30fps)

```json
{
  "fps": { "numerator": 30, "denominator": 1 },
  "canvas": { "width": 1280, "height": 720 },
  "duration": 90,
  "seed": 42,
  "assets": {
    "bg_rect": {
      "Path": {
        "svg_path_d": "M0,0 L1280,0 L1280,720 L0,720 Z"
      }
    },
    "label": {
      "Text": {
        "content": "OpenZ",
        "font_size": 72.0,
        "color": [255, 255, 255, 255],
        "font_family": null
      }
    }
  },
  "tracks": [
    {
      "name": "background",
      "z_base": 0,
      "layout_mode": "Absolute",
      "layout_gap_px": 0.0,
      "layout_padding": { "top": 0.0, "right": 0.0, "bottom": 0.0, "left": 0.0 },
      "layout_align_x": "Start",
      "layout_align_y": "Start",
      "layout_grid_columns": 2,
      "clips": [
        {
          "id": "bg",
          "asset": "bg_rect",
          "range": { "start": 0, "end": 90 },
          "props": {
            "transform": { "Constant": { "translate": [0.0, 0.0], "scale": [1.0, 1.0], "rotate": 0.0, "shear": [0.0, 0.0] } },
            "opacity": { "Constant": 1.0 },
            "blend": "Normal"
          },
          "z_offset": 0,
          "effects": [],
          "transition_in": null,
          "transition_out": null
        }
      ]
    }
  ]
}
```

---

## 3. Common Animation Patterns

### Fade-In
```
opacity: Anim::keyframes(vec![
    (FrameIndex(0),  0.0, Easing::Linear),
    (FrameIndex(30), 1.0, Easing::Linear),
])
```

### Slide-In From Left
```
transform: Anim::keyframes(vec![
    (FrameIndex(0),  Transform2D { translate: Vec2::new(-300.0, 200.0), scale: Vec2::new(1.0,1.0), ..default }, Easing::EaseOut),
    (FrameIndex(30), Transform2D { translate: Vec2::new(100.0, 200.0),  scale: Vec2::new(1.0,1.0), ..default }, Easing::EaseOut),
])
```

### Zoom-In (Scale Up)
```
transform: Anim::keyframes(vec![
    (FrameIndex(0),  Transform2D { scale: Vec2::new(0.1, 0.1), ..default }, Easing::EaseOut),
    (FrameIndex(45), Transform2D { scale: Vec2::new(1.0, 1.0), ..default }, Easing::EaseOut),
])
```

### Fade-Out Exit
```
opacity: Anim::keyframes(vec![
    (FrameIndex(60), 1.0, Easing::Linear),
    (FrameIndex(90), 0.0, Easing::Linear),
])
```

---

## 4. Layout Modes

| Mode | Description |
|---|---|
| `Absolute` | Clips positioned by transform.translate (pixel-precise) |
| `Row` | Clips flow left-to-right with `layout_gap_px` between them |
| `Column` | Clips flow top-to-bottom |
| `Grid` | Grid layout with `layout_grid_columns` columns |

---

## 5. Rendering Parameters

| Parameter | Description | Default |
|---|---|---|
| `bg_rgba` | Background fill `[r,g,b,a]` | `[18, 20, 28, 255]` (dark navy) |
| `output_path` | Where to write the MP4 | `output.mp4` |
| `composition_json` | Full JSON-serialized `Composition` | *required* |

**Requirements**: FFmpeg must be installed on the host (`ffmpeg -version`). The tool uses CPU-based rasterization via the `wavyte` crate's built-in backend.

---

## 6. Design Recipes

### Animated Logo Reveal (90 frames @ 30fps = 3s)
1. Frame 0-10: Black screen (empty track)
2. Frame 0-30: Logo SVG path fades + scales in
3. Frame 30-60: Tagline text slides up from below
4. Frame 60-90: Everything holds at full opacity

### Data Visualization Fly-In
1. Create bar assets as rect paths sized to values
2. Use a `Column` layout track to auto-position bars
3. Animate each clip with staggered `opacity` fade-ins (offset by 10 frames each)
4. Add a title text clip with `EaseOut` slide-in from top

### Loading Spinner Loop
1. Create a ring path asset
2. Single clip spanning all frames
3. Use rotate transform keyframes: `0° → 360°` over 60 frames
4. Set clip range to full duration for seamless loop

---

## 7. Quality Checklist

Before generating a video, verify:
- [ ] `duration` matches intended length (frames = seconds × fps)
- [ ] All `asset` keys in clips exist in the `assets` map
- [ ] `FrameRange::start < end` and both are within `[0, duration]`
- [ ] Track `z_base` values are unique to control layering
- [ ] Background is set via `bg_rgba` parameter or a full-canvas path asset
- [ ] `output_path` ends with `.mp4`
- [ ] FFmpeg is available on the system (`which ffmpeg`)
