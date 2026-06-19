---
name: image_generator
description: "AI Agent Skill for the generate_image tool — teaches the agent how to compose high-quality, premium PNG images using HTML, CSS, SVG, Tailwind, and custom selector snapshots via headless Chromium."
metadata:
5:   version: 2.0.0
---

# Image Generator Skill 🎨

This skill guides the agent in using the updated `generate_image` tool to produce premium, high-fidelity PNG images. Because the tool now runs a headless Chromium browser under the hood, you are no longer limited to pixel-based drawing. You can design beautiful, modern layouts using HTML, CSS, web fonts, vector SVGs, and libraries like Tailwind CSS.

---

## 1. Tool Overview & Parameters

`generate_image` accepts parameters to load and capture web content:

| Parameter | Type | Description |
|---|---|---|
| `html` | string | Raw HTML string to render. Can load CDN CSS (e.g. Tailwind), Google Fonts, canvas elements, animations, and gradients. |
| `html_path` | string | Local file path (e.g., `templates/invoice.html`) to load and capture. |
| `url` | string | Remote web URL (e.g. `https://news.ycombinator.com`) to load and capture. |
| `css` | string | Extra custom CSS to inject into the page before taking the screenshot. |
| `width` | integer | Viewport width in pixels (default: `800`). |
| `height` | integer | Viewport height in pixels (default: `800`). |
| `device_scale_factor` | number | DPI scale factor (default: `2.0` for Retina/high-DPI crisp screenshots). |
| `selector` | string | CSS selector of a specific element to capture (e.g., `#widget`, `.card`). Captures only that element's bounding rect. |
| `settle_ms` | integer | Delay in milliseconds to wait after load for layouts/fonts to settle (default: `300`). |
| `output_path` | string | Destination file path (default: `output.png`). |
| `shapes` | array | **Legacy Fallback:** Sequential geometric shapes. Automatically compiled to SVG and rendered crisp. |

---

## 2. Design Guidelines

### HTML/CSS is Preferred
Always prefer the `html` parameter for new images. It gives you access to modern UI styling:
- **Layouts**: Use Flexbox and CSS Grid for perfect alignment.
- **Styling**: Add gradients, rounded corners (`border-radius`), box-shadows, and transitions.
- **Typography**: Import Google Fonts or use high-quality system font stacks.
- **Frameworks**: Include Tailwind CSS via CDN inside the HTML `<head>` for rapid, premium styling.

### Bounding Rect Screenshotting
Use the `selector` parameter to screenshot only a specific element. This allows you to render a full page with margin, but capture only the element of interest:
- Render a beautiful dashboard card with `<div class="card" id="my-card">...</div>`.
- Set `"selector": "#my-card"` to get a crop containing only that card.

### High DPI Crispness
Keep `device_scale_factor` at `2.0` (default) for professional, retina-grade results. Reduce it to `1.0` if you need exact 1:1 pixel alignments or smaller file sizes.

---

## 3. Implementation Patterns & Recipes

### Premium Modern Card (using Tailwind CSS CDN)
```json
{
  "width": 800,
  "height": 600,
  "output_path": "card.png",
  "html": "<!DOCTYPE html><html><head><script src=\"https://cdn.tailwindcss.com\"></script><link href=\"https://fonts.googleapis.com/css2?family=Outfit:wght@400;600;800&display=swap\" rel=\"stylesheet\"><style>body { font-family: 'Outfit', sans-serif; }</style></head><body class=\"bg-slate-900 w-screen h-screen flex items-center justify-center p-8\"><div id=\"card\" class=\"bg-slate-800/80 backdrop-blur-md border border-slate-700/50 rounded-3xl p-8 shadow-2xl max-w-md text-white flex flex-col gap-4\"><div class=\"flex justify-between items-center\"><span class=\"text-xs font-semibold tracking-wider text-cyan-400 uppercase\">OpenZ Agent</span><span class=\"h-2 w-2 rounded-full bg-emerald-500 animate-pulse\"></span></div><h2 class=\"text-3xl font-extrabold bg-gradient-to-r from-cyan-400 to-blue-500 bg-clip-text text-transparent\">High-Fidelity Visuals</h2><p class=\"text-slate-400 text-sm leading-relaxed\">OpenZ can now render beautiful HTML templates, gradients, and custom components using a local Chromium instance.</p><div class=\"flex items-center gap-3 mt-2\"><div class=\"h-10 w-10 rounded-full bg-gradient-to-tr from-cyan-400 to-blue-500 flex items-center justify-center font-bold\">OZ</div><div><p class=\"text-xs font-semibold\">Antigravity AI</p><p class=\"text-[10px] text-slate-500\">Framework Developer</p></div></div></div></body></html>",
  "selector": "#card"
}
```

### Chart/Data Plot rendering (using CSS/SVG)
Instead of manual pixel drawing, write clean SVG structures in HTML:
```json
{
  "width": 600,
  "height": 400,
  "output_path": "chart.png",
  "html": "<!DOCTYPE html><html><body style=\"margin:0; background:#f8fafc; font-family:sans-serif; display:flex; flex-direction:column; justify-content:center; align-items:center; height:100vh;\"><div style=\"width:500px; background:white; padding:24px; border-radius:16px; border:1px solid #e2e8f0; box-shadow:0 4px 6px -1px rgb(0 0 0 / 0.1);\"><h3 style=\"margin-top:0; color:#0f172a;\">Monthly Metrics</h3><svg viewBox=\"0 0 400 200\" style=\"width:100%; height:200px;\"><rect x=\"10\" y=\"150\" width=\"40\" height=\"40\" fill=\"#3b82f6\" rx=\"4\" /><rect x=\"70\" y=\"110\" width=\"40\" height=\"80\" fill=\"#3b82f6\" rx=\"4\" /><rect x=\"130\" y=\"70\" width=\"40\" height=\"120\" fill=\"#3b82f6\" rx=\"4\" /><rect x=\"190\" y=\"30\" width=\"40\" height=\"160\" fill=\"#10b981\" rx=\"4\" /><line x1=\"0\" y1=\"190\" x2=\"400\" y2=\"190\" stroke=\"#cbd5e1\" stroke-width=\"2\" /></svg></div></body></html>"
}
```

---

## 4. Quality Checklist
Before calling `generate_image`, verify:
- [ ] You are using the `html` parameter for new/complex images
- [ ] If CSS layout classes or Tailwind CDN is loaded, the page has viewport dimensions (`width: 100vw; height: 100vh;` or `w-screen h-screen`)
- [ ] If screenshotting a specific component, it has a clear `id` or class passed into the `selector` parameter
- [ ] `output_path` ends with `.png`
- [ ] `settle_ms` is set higher (e.g. `500-1000`) if loading heavy external assets or Google Fonts
