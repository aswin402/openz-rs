---
name: image_generator
description: "AI Agent Skill for the generate_image tool — teaches the agent how to compose high-quality, expressive PNG images using geometric shapes, gradients, layouts, and text on a pixel canvas."
metadata:
  version: 1.0.0
---

# Image Generator Skill 🎨

This skill guides the agent in using the `generate_image` tool to produce polished, intentional PNG images purely from geometric primitives and text. Because the tool operates at the pixel level with Rust's `imageproc` crate, you must think in terms of coordinates and layered drawing operations.

---

## 1. Tool Overview

`generate_image` accepts a **declarative list of shapes** drawn sequentially on an `ImageBuffer<Rgb<u8>>` canvas using the following operations:

| Shape type | Key params | Notes |
|---|---|---|
| `rect` | `x, y, w, h, fill, color` | Filled or hollow rectangle |
| `circle` | `cx, cy, r, fill, color` | Filled or hollow circle |
| `line` | `x1, y1, x2, y2, color` | Anti-aliased line |
| `text` | `x, y, text, size, color` | Uses DejaVu Sans if available |

Colors are hex strings (`#rrggbb` or `#rgb`).

---

## 2. Design Principles

### Think in Layers
Shapes are drawn **top-to-bottom** in the `shapes` array — later shapes paint over earlier ones. Use this to:
- Draw a solid background color first with a full-canvas `rect`
- Add structural panels/cards
- Add content shapes on top
- Finish with labels or annotations

### Coordinate System
- Origin `(0, 0)` is the **top-left** corner
- X increases to the right, Y increases downward
- Center a circle at `(width/2, height/2)` for centered compositions

### Color Strategy
Always use a **curated palette** rather than arbitrary colors:
```
Background:  #0f0f1a (near-black navy) or #f5f5f5 (off-white)
Accent 1:    #4A90E2 (vibrant blue)
Accent 2:    #E74C3C (coral red)
Highlight:   #F39C12 (amber gold)
Success:     #27AE60 (emerald)
Text:        #ECF0F1 (white-ish) on dark, #2C3E50 (charcoal) on light
```

---

## 3. Composition Patterns

### Hero Card (dark background with centered circle)
```json
{
  "width": 800,
  "height": 500,
  "output_path": "hero.png",
  "background_color": "#0f0f1a",
  "shapes": [
    { "type": "rect", "x": 50, "y": 50, "w": 700, "h": 400, "color": "#1a1a2e", "fill": true },
    { "type": "circle", "cx": 400, "cy": 250, "r": 150, "color": "#4A90E2", "fill": true },
    { "type": "circle", "cx": 400, "cy": 250, "r": 110, "color": "#0f0f1a", "fill": true },
    { "type": "text", "x": 360, "y": 265, "text": "AI", "size": 48.0, "color": "#E74C3C" }
  ]
}
```

### Bar Chart
```json
{
  "width": 600,
  "height": 400,
  "output_path": "chart.png",
  "background_color": "#f5f5f5",
  "shapes": [
    { "type": "rect", "x": 50, "y": 300, "w": 80, "h": 80, "color": "#4A90E2", "fill": true },
    { "type": "rect", "x": 160, "y": 200, "w": 80, "h": 180, "color": "#E74C3C", "fill": true },
    { "type": "rect", "x": 270, "y": 100, "w": 80, "h": 280, "color": "#27AE60", "fill": true },
    { "type": "line", "x1": 30, "y1": 380, "x2": 570, "y2": 380, "color": "#2C3E50" },
    { "type": "text", "x": 20, "y": 395, "text": "Category", "size": 14.0, "color": "#2C3E50" }
  ]
}
```

### Profile Badge
```json
{
  "width": 400,
  "height": 200,
  "output_path": "badge.png",
  "background_color": "#1ABC9C",
  "shapes": [
    { "type": "circle", "cx": 80, "cy": 100, "r": 60, "color": "#ffffff", "fill": true },
    { "type": "circle", "cx": 80, "cy": 100, "r": 55, "color": "#16A085", "fill": true },
    { "type": "text", "x": 155, "y": 90, "text": "OpenZ Agent", "size": 22.0, "color": "#ffffff" },
    { "type": "text", "x": 155, "y": 120, "text": "AI Assistant", "size": 16.0, "color": "#d4efeb" }
  ]
}
```

---

## 4. Tips & Gotchas

- **Text rendering**: Only works if `/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf` exists on the system. Always include fallback text color matching the background in case font is missing.
- **No alpha blending**: The tool uses `Rgb<u8>` (no alpha channel). Transparency is not supported — overlap to simulate layering.
- **Hollow shapes**: Set `"fill": false` for `rect` and `circle` to get outlines.
- **Image size**: Keep width × height under 4096 × 4096 to avoid memory pressure. For large images, prefer simple patterns.
- **Output path**: Use absolute paths or paths relative to the workspace root. The tool saves a PNG directly.
- **Sequential drawing**: All shapes in the `shapes` array are drawn in order. A shape can cover earlier ones.

---

## 5. Quality Checklist

Before calling `generate_image`, verify:
- [ ] Background color is set and appropriate
- [ ] Text is positioned within canvas bounds (x + text_width < canvas_width)
- [ ] Color palette is harmonious (not random colors)
- [ ] Shapes are sized proportionally to the canvas
- [ ] The composition has a clear visual hierarchy (background → structure → content → labels)
