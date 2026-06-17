---
name: svg_animator
description: "AI Agent Skill for the create_animated_svg tool — teaches how to compose rich, animated SVG files using declarative JSON with SMIL animations, gradients, filters, and complex shape hierarchies."
metadata:
  version: 1.0.0
---

# SVG Animator Skill ✨

This skill guides the agent in using the `create_animated_svg` tool to produce beautiful, self-animating SVG files that work natively in any browser. The tool generates pure SVG with **SMIL (Synchronized Multimedia Integration Language)** animations — no JavaScript or CSS required.

---

## 1. Core Concepts

### Why SVG + SMIL?
- **Self-contained**: A single `.svg` file plays in any browser, email client, or Markdown renderer
- **Scalable**: Vector graphics at any resolution without quality loss
- **Performant**: Native browser rendering — zero JS overhead
- **Composable**: Supports gradients, filters, clip-paths, and nested groups

### Animation Model
Each element can carry an `animations` array with SMIL animation directives. These inject `<animate>`, `<animateTransform>`, `<animateMotion>`, or `<set>` child elements inside the SVG element.

---

## 2. Available Animation Types

| Type | What it does | Key params |
|---|---|---|
| `fade_in` | Opacity 0 → 1 | `dur`, `repeat` |
| `fade_out` | Opacity 1 → 0 | `dur`, `repeat` |
| `blink` | Opacity 1 → 0 loop | `dur`, `repeat` |
| `rotate` | Rotation around a point | `from` (e.g. `"0 150 150"`), `to` (e.g. `"360 150 150"`), `dur`, `repeat` |
| `translate` | Move X,Y | `from` (e.g. `"0 0"`), `to` (e.g. `"100 50"`), `dur`, `repeat` |
| `scale` | Scale X,Y | `from` (e.g. `"0.5 0.5"`), `to` (e.g. `"1.5 1.5"`), `dur`, `repeat` |
| `pulse` | Scale up/down rhythmically | `from_scale`, `to_scale`, `dur`, `repeat` |
| `color_cycle` | Animate fill/stroke color | `attr` (fill/stroke), `from`, `to`, `dur`, `repeat` |
| `motion_path` | Move along an SVG path | `path` (SVG path string), `dur`, `repeat` |
| `stroke_dash` | Draw-on path animation | `length` (total path length), `dur`, `repeat` |
| `typewriter` | Reveal text via visibility | `begin` (delay), `dur`, `repeat` |
| *(generic)* | Animate any SVG attribute | `attr`, `from`, `to`, `dur`, `repeat` |

### `repeat` values
- `"indefinite"` — loops forever
- `"3"` — plays 3 times then stops
- `"1"` — plays once

---

## 3. Element Reference

### Shapes
| Shape | Required attrs | Optional attrs |
|---|---|---|
| `rect` | `x, y, width, height` | `rx` (radius), `fill`, `stroke`, `stroke_width`, `opacity` |
| `circle` | `cx, cy, r` | `fill`, `stroke`, `opacity` |
| `ellipse` | `cx, cy, rx, ry` | `fill`, `opacity` |
| `line` | `x1, y1, x2, y2, stroke` | `stroke_width` |
| `polyline` | `points` | `stroke`, `fill`, `stroke_width` |
| `polygon` | `points` | `fill`, `stroke` |
| `path` | `d` | `fill`, `stroke`, `stroke_dasharray`, `stroke_dashoffset` |
| `text` | `x, y, content` | `font_size`, `font_family`, `fill`, `text_anchor` |
| `group` | *(none)* | `transform`, `opacity`, `children[]` |

### Defs (reusable definitions)
| Type | Purpose | Key fields |
|---|---|---|
| `linearGradient` | Linear color gradient | `id`, `x1,y1,x2,y2`, `stops[]` |
| `radialGradient` | Radial color gradient | `id`, `cx,cy,r`, `stops[]` |
| `filter` blur | Gaussian blur effect | `id`, `filter_type: "blur"`, `stdDeviation` |
| `filter` shadow | Drop shadow effect | `id`, `filter_type: "shadow"`, `dx, dy, blur, color` |
| `clipPath` | Clip region | `id`, `shape: "rect"|"circle"`, shape attrs |

Reference a def by setting `fill: "url(#id)"` or `filter: "url(#id)"` on an element.

---

## 4. Composition Patterns

### Pulsing Gradient Logo
```json
{
  "width": 400, "height": 400,
  "background": "#0f0f1a",
  "title": "Pulsing Logo",
  "defs": [
    {
      "type": "radialGradient", "id": "glow",
      "cx": "50%", "cy": "50%", "r": "50%",
      "stops": [
        { "offset": "0%", "color": "#4A90E2", "opacity": "1" },
        { "offset": "100%", "color": "#0f0f1a", "opacity": "0" }
      ]
    }
  ],
  "elements": [
    {
      "shape": "circle", "cx": "200", "cy": "200", "r": "150",
      "fill": "url(#glow)",
      "animations": [{ "type": "pulse", "dur": "2s", "repeat": "indefinite", "from_scale": "0.85", "to_scale": "1.0" }]
    },
    {
      "shape": "text", "x": "200", "y": "215", "content": "AI",
      "font_size": "96", "fill": "#ffffff", "text_anchor": "middle",
      "animations": [{ "type": "fade_in", "dur": "1.5s", "repeat": "1" }]
    }
  ]
}
```

### Rotating Gear / Spinner
```json
{
  "width": 200, "height": 200, "background": "#1a1a2e",
  "elements": [
    {
      "shape": "circle", "cx": "100", "cy": "100", "r": "70",
      "fill": "none", "stroke": "#4A90E2", "stroke_width": "8",
      "animations": [{ "type": "rotate", "from": "0 100 100", "to": "360 100 100", "dur": "2s", "repeat": "indefinite" }]
    },
    {
      "shape": "circle", "cx": "100", "cy": "100", "r": "50",
      "fill": "none", "stroke": "#E74C3C", "stroke_width": "5",
      "animations": [{ "type": "rotate", "from": "360 100 100", "to": "0 100 100", "dur": "3s", "repeat": "indefinite" }]
    }
  ]
}
```

### Path Draw-On Animation (signature/logo reveal)
```json
{
  "width": 500, "height": 200, "background": "#ffffff",
  "elements": [
    {
      "shape": "path",
      "d": "M30,100 C100,20 200,180 300,100 S450,20 480,100",
      "stroke": "#E74C3C", "stroke_width": "4", "fill": "none",
      "stroke_dasharray": "600", "stroke_dashoffset": "600",
      "animations": [{ "type": "stroke_dash", "dur": "3s", "repeat": "1", "length": "600" }]
    }
  ]
}
```

### Floating Particle System
```json
{
  "width": 400, "height": 400, "background": "#0a0a1a",
  "elements": [
    { "shape": "circle", "cx": "80",  "cy": "200", "r": "8", "fill": "#4A90E2", "opacity": "0.8",
      "animations": [{ "type": "translate", "from": "0 0", "to": "20 -80", "dur": "3s", "repeat": "indefinite" }] },
    { "shape": "circle", "cx": "200", "cy": "300", "r": "5", "fill": "#E74C3C", "opacity": "0.7",
      "animations": [{ "type": "translate", "from": "0 0", "to": "-30 -120", "dur": "4s", "repeat": "indefinite" }] },
    { "shape": "circle", "cx": "320", "cy": "250", "r": "10", "fill": "#27AE60", "opacity": "0.9",
      "animations": [{ "type": "translate", "from": "0 0", "to": "10 -60", "dur": "2.5s", "repeat": "indefinite" }] }
  ]
}
```

### Glowing Text Banner
```json
{
  "width": 600, "height": 150, "background": "#0f0f1a",
  "defs": [
    { "type": "filter", "id": "glow_filter", "filter_type": "blur", "stdDeviation": "4" }
  ],
  "elements": [
    {
      "shape": "text", "x": "300", "y": "90",
      "content": "HELLO WORLD", "font_size": "52",
      "fill": "#4A90E2", "text_anchor": "middle",
      "filter": "url(#glow_filter)",
      "animations": [{ "type": "color_cycle", "attr": "fill", "from": "#4A90E2", "to": "#E74C3C", "dur": "3s", "repeat": "indefinite" }]
    },
    {
      "shape": "text", "x": "300", "y": "90",
      "content": "HELLO WORLD", "font_size": "52",
      "fill": "#ffffff", "text_anchor": "middle",
      "animations": [{ "type": "fade_in", "dur": "1s", "repeat": "1" }]
    }
  ]
}
```

---

## 5. Advanced Techniques

### Layered Glow Effect
Stack two identical elements — one with a `blur` filter and a bright color (the glow layer), one sharp on top (the crisp layer). Animate both together for a neon glow effect.

### Counter-Rotating Rings
Use multiple circles with `rotate` animations at different speeds (`dur`) and directions (`from/to` reversed) to create complex orbital patterns.

### Staggered Reveal
Use multiple text or rect elements with `fade_in` animations where each has a different `begin` time (via a `typewriter` or generic `set` animation) to create a sequential reveal effect.

### Motion Along a Path
Use `motion_path` with a well-designed SVG `path` to make a circle or shape follow a curve:
```json
{ "type": "motion_path", "path": "M0,100 Q200,0 400,100 T800,100", "dur": "4s", "repeat": "indefinite" }
```

---

## 6. Quality Checklist

Before calling `create_animated_svg`, verify:
- [ ] All gradient/filter/clip `id`s referenced in elements actually exist in `defs`
- [ ] `rotate` animation `from`/`to` include the center point: `"0 cx cy"` not just `"0"`
- [ ] `stroke_dasharray` is set on paths that use `stroke_dash` animation (must match `length`)
- [ ] Background is explicitly set (default is white which may clash with dark elements)
- [ ] Groups with children have the `children` array populated
- [ ] `output_path` ends with `.svg`
- [ ] Text elements have `text_anchor: "middle"` when centering on a coordinate
- [ ] Animation `dur` values have units: `"2s"` not `"2"`
