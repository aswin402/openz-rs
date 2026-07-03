use serde::{Deserialize, Serialize};
use openmedia_core::{Result, OpenMediaError, ImageOutput};
use std::path::Path;
use futures::StreamExt;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::handler::viewport::Viewport;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoScene {
    /// Video width in pixels
    pub width: u32,
    /// Video height in pixels
    pub height: u32,
    /// Frames per second
    pub fps: u32,
    /// Total duration in seconds
    pub duration: f64,
    /// Background color (hex)
    pub background: String,
    /// Ordered list of scenes
    pub scenes: Vec<Scene>,
    /// Transitions between scenes
    pub transitions: Vec<SceneTransition>,
    /// Audio tracks
    pub audio: Option<AudioConfig>,
    /// Custom fonts to load
    pub custom_fonts: Option<Vec<CustomFontSpec>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct CustomFontSpec {
    pub family: String,
    pub src: String,
}

/// A single scene within a video
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    /// Unique scene identifier
    pub id: String,
    /// Start time in seconds
    pub start: f64,
    /// End time in seconds
    pub end: f64,
    /// Elements within this scene
    pub elements: Vec<SceneElement>,
}

/// An element within a video scene
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SceneElement {
    Text {
        content: String,
        style: TextStyle,
        position: Position,
        anchor: Anchor,
        timeline: Option<ElementTimeline>,
    },
    Image {
        src: String,
        position: Position,
        size: Size,
        fit: ObjectFit,
        timeline: Option<ElementTimeline>,
    },
    Shape {
        shape: ShapeType,
        size: Size,
        position: Position,
        style: ShapeStyle,
        timeline: Option<ElementTimeline>,
    },
    Svg {
        content: String,
        position: Position,
        size: Option<Size>,
        timeline: Option<ElementTimeline>,
    },
    Group {
        elements: Vec<SceneElement>,
        position: Position,
        transform: Option<Transform>,
        timeline: Option<ElementTimeline>,
    },
    Html {
        content: String,
        position: Position,
        size: Size,
        timeline: Option<ElementTimeline>,
    },
    Code {
        content: String,
        language: String,
        theme: String,
        position: Position,
        size: Size,
        font_size: f32,
        timeline: Option<ElementTimeline>,
    },
    Chart {
        chart_type: String,
        data: serde_json::Value,
        position: Position,
        size: Size,
        theme: String,
        timeline: Option<ElementTimeline>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: DimensionValue,
    pub y: DimensionValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DimensionValue {
    Pixels(f64),
    Percentage(String),  // e.g., "50%"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Size {
    pub width: DimensionValue,
    pub height: DimensionValue,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Anchor {
    TopLeft, TopCenter, TopRight,
    CenterLeft, Center, CenterRight,
    BottomLeft, BottomCenter, BottomRight,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectFit {
    Cover, Contain, Fill, ScaleDown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeType {
    Rect, RoundedRect, Circle, Ellipse, Polygon, Line,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextStyle {
    pub font_family: String,
    pub font_size: f32,
    pub font_weight: u16,
    pub color: String,
    pub text_align: String,
    pub line_height: Option<f32>,
    pub letter_spacing: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShapeStyle {
    pub fill: Option<String>,
    pub stroke: Option<String>,
    pub stroke_width: Option<f32>,
    pub border_radius: Option<f32>,
    pub opacity: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transform {
    pub translate: Option<(f64, f64)>,
    pub rotate: Option<f64>,
    pub scale: Option<(f64, f64)>,
}

/// Animation timeline for a scene element
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementTimeline {
    pub keyframes: Vec<Keyframe>,
}

/// A single keyframe in an element's animation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keyframe {
    /// Time in seconds (relative to scene start)
    pub time: f64,
    /// Opacity (0.0–1.0)
    pub opacity: Option<f64>,
    /// X position offset
    pub x: Option<String>,
    /// Y position offset
    pub y: Option<String>,
    /// Uniform scale
    pub scale: Option<f64>,
    /// Horizontal scale
    pub scale_x: Option<f64>,
    /// Vertical scale
    pub scale_y: Option<f64>,
    /// Rotation in degrees
    pub rotation: Option<f64>,
    /// Easing function to reach this keyframe
    pub easing: Option<String>,
}

/// Transition between two scenes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneTransition {
    /// Source scene ID
    pub from: String,
    /// Target scene ID
    pub to: String,
    /// Transition type
    #[serde(rename = "type")]
    pub transition_type: TransitionType,
    /// Duration in seconds
    pub duration: f64,
    /// Easing function
    pub easing: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransitionType {
    None,
    Crossfade,
    SlideLeft,
    SlideRight,
    SlideUp,
    SlideDown,
    ZoomIn,
    ZoomOut,
    WipeLeft,
    WipeRight,
    WipeUp,
    WipeDown,
    Dissolve,
    IrisIn,
    IrisOut,
    Blur,
    Glitch,
    RadialWipe,
}

/// Audio configuration for a video
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub tracks: Vec<AudioTrack>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioTrack {
    pub src: String,
    pub start: f64,
    pub volume: f32,
    pub fade_in: Option<f64>,
    pub fade_out: Option<f64>,
}

/// Trait for rendering a single video frame from scene elements
#[async_trait::async_trait]
pub trait FrameRenderer: Send + Sync {
    /// Render a single frame at the given time
    async fn render_frame(
        &self,
        scene: &VideoScene,
        time: f64,
        width: u32,
        height: u32,
    ) -> Result<image::RgbaImage>;

    fn name(&self) -> &str;
}

pub struct DummyFrameRenderer;

impl DummyFrameRenderer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DummyFrameRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl FrameRenderer for DummyFrameRenderer {
    async fn render_frame(
        &self,
        _scene: &VideoScene,
        _time: f64,
        _width: u32,
        _height: u32,
    ) -> Result<image::RgbaImage> {
        Err(OpenMediaError::BackendUnavailable("Dummy frame renderer".into()))
    }

    fn name(&self) -> &str {
        "dummy"
    }
}

// === Native SVG Frame Renderer ===
pub struct SvgFrameRenderer;

#[async_trait::async_trait]
impl FrameRenderer for SvgFrameRenderer {
    async fn render_frame(
        &self,
        scene: &VideoScene,
        time: f64,
        width: u32,
        height: u32,
    ) -> Result<image::RgbaImage> {
        // Check for active transition
        let mut active_trans = None;
        for trans in &scene.transitions {
            if let Some(from_s) = scene.scenes.iter().find(|s| s.id == trans.from) {
                let trans_start = from_s.end - trans.duration;
                if time >= trans_start && time <= from_s.end {
                    if let Some(to_s) = scene.scenes.iter().find(|s| s.id == trans.to) {
                        active_trans = Some((from_s, to_s, trans_start, trans));
                        break;
                    }
                }
            }
        }

        if let Some((from_s, to_s, trans_start, trans)) = active_trans {
            let progress = if trans.duration <= 0.0 {
                1.0
            } else {
                apply_transition_easing((time - trans_start) / trans.duration, trans.easing.as_deref())
            };
            
            let mut from_scene = scene.clone();
            from_scene.scenes = vec![from_s.clone()];
            from_scene.transitions = vec![];
            let img_from = self.render_frame(&from_scene, time, width, height).await?;
            
            let mut to_scene = scene.clone();
            to_scene.scenes = vec![to_s.clone()];
            to_scene.transitions = vec![];
            let img_to = self.render_frame(&to_scene, time, width, height).await?;
            
            return Ok(blend_frames(&img_from, &img_to, progress, &trans.transition_type));
        }

        let svg_str = compile_scene_to_svg(scene, time, width, height)?;
        
        static USVG_FONTDB_CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<Vec<CustomFontSpec>, std::sync::Arc<resvg::usvg::fontdb::Database>>>> = std::sync::OnceLock::new();
        let cache = USVG_FONTDB_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
        
        let custom_fonts = scene.custom_fonts.clone().unwrap_or_default();
        
        let fontdb_arc = {
            let mut found = None;
            if let Ok(guard) = cache.lock() {
                if let Some(cached_db) = guard.get(&custom_fonts) {
                    found = Some(cached_db.clone());
                }
            }
            if let Some(db) = found {
                db
            } else {
                static BASE_SYSTEM_FONTS: std::sync::OnceLock<resvg::usvg::fontdb::Database> = std::sync::OnceLock::new();
                let base_db = BASE_SYSTEM_FONTS.get_or_init(|| {
                    let mut db = resvg::usvg::fontdb::Database::new();
                    db.load_system_fonts();
                    db
                });
                let mut fontdb = base_db.clone();
                if let Some(ref fonts) = scene.custom_fonts {
                    let resolved = resolve_custom_fonts(fonts).await;
                    for (_, bytes) in resolved {
                        fontdb.load_font_data(bytes);
                    }
                }
                let arc_db = std::sync::Arc::new(fontdb);
                if let Ok(mut guard) = cache.lock() {
                    guard.insert(custom_fonts, arc_db.clone());
                }
                arc_db
            }
        };

        let mut opt = resvg::usvg::Options::default();
        opt.fontdb = fontdb_arc;
        let tree = resvg::usvg::Tree::from_str(&svg_str, &opt)
            .map_err(|e| OpenMediaError::InvalidSvgInput(e.to_string()))?;
            
        let mut pixmap = tiny_skia::Pixmap::new(width, height)
            .ok_or_else(|| OpenMediaError::InvalidDimensions {
                width,
                height,
                reason: "Failed to allocate pixmap".to_string(),
            })?;
            
        let transform = tiny_skia::Transform::default();
        resvg::render(&tree, transform, &mut pixmap.as_mut());
        
        let mut pixels = pixmap.data().to_vec();
        for chunk in pixels.chunks_exact_mut(4) {
            let a = chunk[3];
            if a > 0 && a < 255 {
                let alpha_factor = 255.0 / a as f32;
                chunk[0] = (chunk[0] as f32 * alpha_factor).min(255.0) as u8;
                chunk[1] = (chunk[1] as f32 * alpha_factor).min(255.0) as u8;
                chunk[2] = (chunk[2] as f32 * alpha_factor).min(255.0) as u8;
            }
        }

        let buffer = image::ImageBuffer::from_raw(width, height, pixels)
            .ok_or_else(|| OpenMediaError::Internal("Failed to build RgbaImage".to_string()))?;
            
        Ok(buffer)
    }

    fn name(&self) -> &str {
        "svg"
    }
}

fn resolve_dimension(val: &DimensionValue, total: f64) -> f64 {
    match val {
        DimensionValue::Pixels(pixels) => *pixels,
        DimensionValue::Percentage(pct) => {
            let clean = pct.trim_end_matches('%');
            if let Ok(pct_val) = clean.parse::<f64>() {
                (pct_val / 100.0) * total
            } else {
                0.0
            }
        }
    }
}

fn interpolate_f64(t: f64, keyframes: &[Keyframe], get_val: impl Fn(&Keyframe) -> Option<f64>, default: f64) -> f64 {
    if keyframes.is_empty() {
        return default;
    }
    
    let mut sorted = keyframes.to_vec();
    sorted.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    
    if t <= sorted[0].time {
        return get_val(&sorted[0]).unwrap_or(default);
    }
    if t >= sorted[sorted.len() - 1].time {
        return get_val(&sorted[sorted.len() - 1]).unwrap_or(default);
    }
    
    for window in sorted.windows(2) {
        let k1 = &window[0];
        let k2 = &window[1];
        if t >= k1.time && t <= k2.time {
            let v1 = get_val(k1).unwrap_or(default);
            let v2 = get_val(k2).unwrap_or(default);
            let duration = k2.time - k1.time;
            if duration == 0.0 {
                return v2;
            }
            let mut progress = (t - k1.time) / duration;
            if let Some(easing) = &k2.easing {
                progress = match easing.to_lowercase().as_str() {
                    "ease_in" | "ease-in" => progress * progress,
                    "ease_out" | "ease-out" => progress * (2.0 - progress),
                    "ease_in_out" | "ease-in-out" => {
                        if progress < 0.5 {
                            2.0 * progress * progress
                        } else {
                            -1.0 + (4.0 - 2.0 * progress) * progress
                        }
                    }
                    _ => progress,
                };
            }
            return v1 + (v2 - v1) * progress;
        }
    }
    default
}

fn interpolate_string_dimension(
    t: f64,
    keyframes: &[Keyframe],
    get_str: impl Fn(&Keyframe) -> Option<&String>,
    default_str: &str,
    total: f64,
) -> f64 {
    let parse_dim = |s: &str| -> f64 {
        if s.ends_with('%') {
            let clean = s.trim_end_matches('%');
            if let Ok(p) = clean.parse::<f64>() {
                (p / 100.0) * total
            } else {
                0.0
            }
        } else {
            s.parse::<f64>().unwrap_or(0.0)
        }
    };
    
    if keyframes.is_empty() {
        return parse_dim(default_str);
    }
    
    let mut sorted = keyframes.to_vec();
    sorted.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    
    if t <= sorted[0].time {
        let s = get_str(&sorted[0]).map(|x| x.as_str()).unwrap_or(default_str);
        return parse_dim(s);
    }
    if t >= sorted[sorted.len() - 1].time {
        let s = get_str(&sorted[sorted.len() - 1]).map(|x| x.as_str()).unwrap_or(default_str);
        return parse_dim(s);
    }
    
    for window in sorted.windows(2) {
        let k1 = &window[0];
        let k2 = &window[1];
        if t >= k1.time && t <= k2.time {
            let s1 = get_str(k1).map(|x| x.as_str()).unwrap_or(default_str);
            let s2 = get_str(k2).map(|x| x.as_str()).unwrap_or(default_str);
            let v1 = parse_dim(s1);
            let v2 = parse_dim(s2);
            let duration = k2.time - k1.time;
            if duration == 0.0 {
                return v2;
            }
            let mut progress = (t - k1.time) / duration;
            if let Some(easing) = &k2.easing {
                progress = match easing.to_lowercase().as_str() {
                    "ease_in" | "ease-in" => progress * progress,
                    "ease_out" | "ease-out" => progress * (2.0 - progress),
                    "ease_in_out" | "ease-in-out" => {
                        if progress < 0.5 {
                            2.0 * progress * progress
                        } else {
                            -1.0 + (4.0 - 2.0 * progress) * progress
                        }
                    }
                    _ => progress,
                };
            }
            return v1 + (v2 - v1) * progress;
        }
    }
    parse_dim(default_str)
}

fn compile_scene_to_svg(scene: &VideoScene, time: f64, width: u32, height: u32) -> Result<String> {
    let mut active_scene = None;
    for s in &scene.scenes {
        if time >= s.start && time <= s.end {
            active_scene = Some(s);
            break;
        }
    }

    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}">"#,
        width, height, width, height
    );
    svg.push_str(&format!(
        r#"<rect width="100%" height="100%" fill="{}"/>"#,
        scene.background
    ));

    if let Some(s) = active_scene {
        let local_t = time - s.start;
        for el in &s.elements {
            let el_svg = render_element_to_svg(el, local_t, width as f64, height as f64)?;
            svg.push_str(&el_svg);
        }
    }

    svg.push_str("</svg>");
    Ok(svg)
}

fn render_element_to_svg(el: &SceneElement, t: f64, total_w: f64, total_h: f64) -> Result<String> {
    match el {
        SceneElement::Shape { shape, size, position, style, timeline } => {
            let (opacity, x_offset, y_offset, scale_x, scale_y, rotation) = if let Some(tl) = timeline {
                let op = interpolate_f64(t, &tl.keyframes, |k| k.opacity, 1.0);
                let x = interpolate_string_dimension(t, &tl.keyframes, |k| k.x.as_ref(), "0", total_w);
                let y = interpolate_string_dimension(t, &tl.keyframes, |k| k.y.as_ref(), "0", total_h);
                let scale = interpolate_f64(t, &tl.keyframes, |k| k.scale, 1.0);
                let sx = interpolate_f64(t, &tl.keyframes, |k| k.scale_x, scale);
                let sy = interpolate_f64(t, &tl.keyframes, |k| k.scale_y, scale);
                let rot = interpolate_f64(t, &tl.keyframes, |k| k.rotation, 0.0);
                (op, x, y, sx, sy, rot)
            } else {
                (1.0, 0.0, 0.0, 1.0, 1.0, 0.0)
            };

            let base_x = resolve_dimension(&position.x, total_w);
            let base_y = resolve_dimension(&position.y, total_h);
            let final_x = base_x + x_offset;
            let final_y = base_y + y_offset;

            let w = resolve_dimension(&size.width, total_w);
            let h = resolve_dimension(&size.height, total_h);
            let fill_str = style.fill.as_deref().unwrap_or("none");
            let stroke_str = style.stroke.as_deref().unwrap_or("none");
            let stroke_w = style.stroke_width.unwrap_or(0.0);

            match shape {
                ShapeType::Rect => {
                    let rx = style.border_radius.unwrap_or(0.0);
                    Ok(format!(
                        r#"<rect x="0" y="0" width="{}" height="{}" rx="{}" fill="{}" stroke="{}" stroke-width="{}" opacity="{}" transform="translate({}, {}) rotate({}) scale({}, {}) translate({}, {})"/>"#,
                        w, h, rx, fill_str, stroke_str, stroke_w, opacity,
                        final_x + w / 2.0, final_y + h / 2.0,
                        rotation,
                        scale_x, scale_y,
                        -w / 2.0, -h / 2.0
                    ))
                }
                ShapeType::Circle => {
                    let r = w / 2.0;
                    Ok(format!(
                        r#"<circle cx="0" cy="0" r="{}" fill="{}" stroke="{}" stroke-width="{}" opacity="{}" transform="translate({}, {}) rotate({}) scale({}, {})"/>"#,
                        r, fill_str, stroke_str, stroke_w, opacity,
                        final_x + r, final_y + r,
                        rotation,
                        scale_x, scale_y
                    ))
                }
                _ => Ok(String::new()),
            }
        }
        SceneElement::Text { content, style, position, anchor, timeline } => {
            let (opacity, x_offset, y_offset, scale_x, scale_y, rotation) = if let Some(tl) = timeline {
                let op = interpolate_f64(t, &tl.keyframes, |k| k.opacity, 1.0);
                let x = interpolate_string_dimension(t, &tl.keyframes, |k| k.x.as_ref(), "0", total_w);
                let y = interpolate_string_dimension(t, &tl.keyframes, |k| k.y.as_ref(), "0", total_h);
                let scale = interpolate_f64(t, &tl.keyframes, |k| k.scale, 1.0);
                let sx = interpolate_f64(t, &tl.keyframes, |k| k.scale_x, scale);
                let sy = interpolate_f64(t, &tl.keyframes, |k| k.scale_y, scale);
                let rot = interpolate_f64(t, &tl.keyframes, |k| k.rotation, 0.0);
                (op, x, y, sx, sy, rot)
            } else {
                (1.0, 0.0, 0.0, 1.0, 1.0, 0.0)
            };

            let base_x = resolve_dimension(&position.x, total_w);
            let base_y = resolve_dimension(&position.y, total_h);
            let final_x = base_x + x_offset;
            let final_y = base_y + y_offset;

            let text_anchor = match anchor {
                Anchor::TopLeft | Anchor::CenterLeft | Anchor::BottomLeft => "start",
                Anchor::TopCenter | Anchor::Center | Anchor::BottomCenter => "middle",
                Anchor::TopRight | Anchor::CenterRight | Anchor::BottomRight => "end",
            };

            let escaped = content.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");

            Ok(format!(
                r#"<text x="0" y="0" fill="{}" font-family="{}" font-size="{}" font-weight="{}" text-anchor="{}" opacity="{}" transform="translate({}, {}) rotate({}) scale({}, {})">{}</text>"#,
                style.color, style.font_family, style.font_size, style.font_weight, text_anchor, opacity,
                final_x, final_y + style.font_size as f64,
                rotation,
                scale_x, scale_y,
                escaped
            ))
        }
        SceneElement::Image { src, position, size, timeline, .. } => {
            let (opacity, x_offset, y_offset, scale_x, scale_y, rotation) = if let Some(tl) = timeline {
                let op = interpolate_f64(t, &tl.keyframes, |k| k.opacity, 1.0);
                let x = interpolate_string_dimension(t, &tl.keyframes, |k| k.x.as_ref(), "0", total_w);
                let y = interpolate_string_dimension(t, &tl.keyframes, |k| k.y.as_ref(), "0", total_h);
                let scale = interpolate_f64(t, &tl.keyframes, |k| k.scale, 1.0);
                let sx = interpolate_f64(t, &tl.keyframes, |k| k.scale_x, scale);
                let sy = interpolate_f64(t, &tl.keyframes, |k| k.scale_y, scale);
                let rot = interpolate_f64(t, &tl.keyframes, |k| k.rotation, 0.0);
                (op, x, y, sx, sy, rot)
            } else {
                (1.0, 0.0, 0.0, 1.0, 1.0, 0.0)
            };

            let base_x = resolve_dimension(&position.x, total_w);
            let base_y = resolve_dimension(&position.y, total_h);
            let final_x = base_x + x_offset;
            let final_y = base_y + y_offset;

            let w = resolve_dimension(&size.width, total_w);
            let h = resolve_dimension(&size.height, total_h);

            Ok(format!(
                r#"<image href="{}" x="0" y="0" width="{}" height="{}" opacity="{}" transform="translate({}, {}) rotate({}) scale({}, {}) translate({}, {})"/>"#,
                src, w, h, opacity,
                final_x + w / 2.0, final_y + h / 2.0,
                rotation,
                scale_x, scale_y,
                -w / 2.0, -h / 2.0
            ))
        }
        SceneElement::Svg { content, position, size, timeline } => {
            let (opacity, x_offset, y_offset, scale_x, scale_y, rotation) = if let Some(tl) = timeline {
                let op = interpolate_f64(t, &tl.keyframes, |k| k.opacity, 1.0);
                let x = interpolate_string_dimension(t, &tl.keyframes, |k| k.x.as_ref(), "0", total_w);
                let y = interpolate_string_dimension(t, &tl.keyframes, |k| k.y.as_ref(), "0", total_h);
                let scale = interpolate_f64(t, &tl.keyframes, |k| k.scale, 1.0);
                let sx = interpolate_f64(t, &tl.keyframes, |k| k.scale_x, scale);
                let sy = interpolate_f64(t, &tl.keyframes, |k| k.scale_y, scale);
                let rot = interpolate_f64(t, &tl.keyframes, |k| k.rotation, 0.0);
                (op, x, y, sx, sy, rot)
            } else {
                (1.0, 0.0, 0.0, 1.0, 1.0, 0.0)
            };

            let base_x = resolve_dimension(&position.x, total_w);
            let base_y = resolve_dimension(&position.y, total_h);
            let final_x = base_x + x_offset;
            let final_y = base_y + y_offset;

            let w = size.as_ref().map(|s| resolve_dimension(&s.width, total_w)).unwrap_or(100.0);
            let h = size.as_ref().map(|s| resolve_dimension(&s.height, total_h)).unwrap_or(100.0);

            Ok(format!(
                r#"<g opacity="{}" transform="translate({}, {}) rotate({}) scale({}, {}) translate({}, {})">{}</g>"#,
                opacity,
                final_x + w / 2.0, final_y + h / 2.0,
                rotation,
                scale_x, scale_y,
                -w / 2.0, -h / 2.0,
                content
            ))
        }
        SceneElement::Chart { chart_type, data, position, size, theme: _, timeline } => {
            let (opacity, x_offset, y_offset, scale_x, scale_y, rotation) = if let Some(tl) = timeline {
                let op = interpolate_f64(t, &tl.keyframes, |k| k.opacity, 1.0);
                let x = interpolate_string_dimension(t, &tl.keyframes, |k| k.x.as_ref(), "0", total_w);
                let y = interpolate_string_dimension(t, &tl.keyframes, |k| k.y.as_ref(), "0", total_h);
                let scale = interpolate_f64(t, &tl.keyframes, |k| k.scale, 1.0);
                let sx = interpolate_f64(t, &tl.keyframes, |k| k.scale_x, scale);
                let sy = interpolate_f64(t, &tl.keyframes, |k| k.scale_y, scale);
                let rot = interpolate_f64(t, &tl.keyframes, |k| k.rotation, 0.0);
                (op, x, y, sx, sy, rot)
            } else {
                (1.0, 0.0, 0.0, 1.0, 1.0, 0.0)
            };

            let base_x = resolve_dimension(&position.x, total_w);
            let base_y = resolve_dimension(&position.y, total_h);
            let final_x = base_x + x_offset;
            let final_y = base_y + y_offset;

            let w = resolve_dimension(&size.width, total_w);
            let h = resolve_dimension(&size.height, total_h);

            let chart_theme = openmedia_svg::ChartTheme::dark();
            let chart_cfg = openmedia_svg::ChartConfig {
                chart_type: match chart_type.to_lowercase().as_str() {
                    "bar" => openmedia_svg::ChartType::Bar,
                    "line" => openmedia_svg::ChartType::Line,
                    "pie" => openmedia_svg::ChartType::Pie,
                    _ => openmedia_svg::ChartType::Bar,
                },
                data: data.clone(),
                title: None,
                subtitle: None,
                width: w as u32,
                height: h as u32,
                theme: chart_theme,
                legend: openmedia_svg::LegendConfig { show: false, position: openmedia_svg::LegendPosition::Bottom },
                grid: true,
                animate: false,
                padding: openmedia_svg::Padding { top: 10.0, right: 10.0, bottom: 10.0, left: 10.0 },
            };

            let chart_xml = openmedia_svg::generate_chart(&chart_cfg)
                .map_err(|e| OpenMediaError::Internal(e.to_string()))?;

            Ok(format!(
                r#"<g opacity="{}" transform="translate({}, {}) rotate({}) scale({}, {}) translate({}, {})">{}</g>"#,
                opacity,
                final_x + w / 2.0, final_y + h / 2.0,
                rotation,
                scale_x, scale_y,
                -w / 2.0, -h / 2.0,
                chart_xml
            ))
        }
        _ => Ok(String::new()),
    }
}

// === Browser Frame Renderer (CDP Headless Chrome) ===
pub struct BrowserFrameRenderer {
    browser: Browser,
    page: tokio::sync::Mutex<chromiumoxide::page::Page>,
}

impl BrowserFrameRenderer {
    pub async fn launch() -> Result<Self> {
        let profile_dir = std::env::temp_dir().join(format!("chrome-profile-{}", uuid::Uuid::new_v4()));
        let config = BrowserConfig::builder()
            .no_sandbox()
            .user_data_dir(profile_dir)
            .build()
            .map_err(|e| OpenMediaError::ConfigError(e.to_string()))?;
        let (browser, mut handler) = Browser::launch(config).await
            .map_err(|_| OpenMediaError::ChromeNotFound)?;

        tokio::spawn(async move {
            while let Some(h) = handler.next().await {
                if let Err(err) = h {
                    tracing::warn!("BrowserFrameRenderer loop error: {:?}", err);
                }
            }
        });

        let page = browser.new_page("about:blank").await
            .map_err(|e| OpenMediaError::Internal(e.to_string()))?;

        Ok(Self {
            browser,
            page: tokio::sync::Mutex::new(page),
        })
    }

    pub async fn close(mut self) {
        let _ = self.browser.close().await;
    }
}

#[async_trait::async_trait]
impl FrameRenderer for BrowserFrameRenderer {
    async fn render_frame(
        &self,
        scene: &VideoScene,
        time: f64,
        width: u32,
        height: u32,
    ) -> Result<image::RgbaImage> {
        let html_content = compile_scene_to_html(scene, time, width, height).await?;
        
        let page = self.page.lock().await;
            
        let params = chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams::builder()
            .width(width as i64)
            .height(height as i64)
            .device_scale_factor(1.0)
            .mobile(false)
            .build()
            .map_err(|e| OpenMediaError::Internal(e.to_string()))?;
        page.execute(params).await
            .map_err(|e| OpenMediaError::Internal(e.to_string()))?;

        page.set_content(html_content).await
            .map_err(|e| OpenMediaError::Internal(e.to_string()))?;

        // Give headless Chrome a short delay (50ms) to parse HTML, run scripts, load fonts,
        // and fully paint the frame before snapshotting it.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let params = chromiumoxide::page::ScreenshotParams::builder()
            .format(chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat::Png)
            .build();
            
        let screenshot_bytes = page.screenshot(params).await
            .map_err(|e| OpenMediaError::Internal(e.to_string()))?;
            
        let img = image::load_from_memory(&screenshot_bytes)
            .map_err(|e| OpenMediaError::ImageDecodeError(e.to_string()))?
            .to_rgba8();

        Ok(img)
    }

    fn name(&self) -> &str {
        "browser"
    }
}

pub async fn get_html_font_css(custom_fonts: &[CustomFontSpec]) -> String {
    static CSS_CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<Vec<CustomFontSpec>, String>>> = std::sync::OnceLock::new();
    let cache = CSS_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    
    if let Ok(guard) = cache.lock() {
        if let Some(cached_css) = guard.get(custom_fonts) {
            return cached_css.clone();
        }
    }

    let mut font_css = String::new();
    let resolved = resolve_custom_fonts(custom_fonts).await;
    for (family, bytes) in resolved {
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        font_css.push_str(&format!(
            r#"@font-face {{
                font-family: '{}';
                src: url('data:font/truetype;charset=utf-8;base64,{}') format('truetype');
            }}
            "#,
            family, b64
        ));
    }

    if let Ok(mut guard) = cache.lock() {
        guard.insert(custom_fonts.to_vec(), font_css.clone());
    }

    font_css
}

async fn compile_scene_to_html(scene: &VideoScene, time: f64, width: u32, height: u32) -> Result<String> {
    let mut active_scene = None;
    let mut active_scene_to = None;
    let mut transition_progress = 0.0;
    let mut active_transition = None;

    for s in &scene.scenes {
        if time >= s.start && time <= s.end {
            active_scene = Some(s);
            break;
        }
    }
    
    for trans in &scene.transitions {
        if let Some(from_s) = scene.scenes.iter().find(|s| s.id == trans.from) {
            let trans_start = from_s.end - trans.duration;
            if time >= trans_start && time <= from_s.end {
                active_scene = Some(from_s);
                active_scene_to = scene.scenes.iter().find(|s| s.id == trans.to);
                transition_progress = if trans.duration <= 0.0 {
                    1.0
                } else {
                    apply_transition_easing((time - trans_start) / trans.duration, trans.easing.as_deref())
                };
                active_transition = Some(trans);
                break;
            }
        }
    }

    let font_css = if let Some(ref fonts) = scene.custom_fonts {
        get_html_font_css(fonts).await
    } else {
        String::new()
    };

    let bg_color = &scene.background;
    let mut html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
<style>
  {}
  body {{
    margin: 0;
    padding: 0;
    width: {}px;
    height: {}px;
    background-color: {};
    overflow: hidden;
    position: relative;
    font-family: sans-serif;
  }}
  .element {{
    position: absolute;
    transform-origin: center;
    box-sizing: border-box;
  }}
</style>
</head>
<body>"#,
        font_css,
        width, height, bg_color
    );

    if let Some(trans) = active_transition {
        if let (Some(s_from), Some(s_to)) = (active_scene, active_scene_to) {
            let from_html = render_scene_elements_to_html(s_from, time - s_from.start, width, height)?;
            let to_html = render_scene_elements_to_html(s_to, time - s_to.start, width, height)?;
            
            let (from_style, to_style) = match trans.transition_type {
                TransitionType::Crossfade => (
                    format!("opacity: {};", 1.0 - transition_progress),
                    format!("opacity: {};", transition_progress),
                ),
                TransitionType::SlideLeft => (
                    format!("transform: translateX(-{}px);", transition_progress * width as f64),
                    format!("transform: translateX({}px);", (1.0 - transition_progress) * width as f64),
                ),
                TransitionType::SlideRight => (
                    format!("transform: translateX({}px);", transition_progress * width as f64),
                    format!("transform: translateX(-{}px);", (1.0 - transition_progress) * width as f64),
                ),
                TransitionType::SlideUp => (
                    format!("transform: translateY(-{}px);", transition_progress * height as f64),
                    format!("transform: translateY({}px);", (1.0 - transition_progress) * height as f64),
                ),
                TransitionType::SlideDown => (
                    format!("transform: translateY({}px);", transition_progress * height as f64),
                    format!("transform: translateY(-{}px);", (1.0 - transition_progress) * height as f64),
                ),
                _ => (
                    format!("opacity: {};", 1.0 - transition_progress),
                    format!("opacity: {};", transition_progress),
                ),
            };
            
            html.push_str(&format!(
                r#"<div data-scene-time="{}" style="position: absolute; width: 100%; height: 100%; {}">{}</div>"#,
                time - s_from.start, from_style, from_html
            ));
            html.push_str(&format!(
                r#"<div data-scene-time="{}" style="position: absolute; width: 100%; height: 100%; {}">{}</div>"#,
                time - s_to.start, to_style, to_html
            ));
        }
    } else if let Some(s) = active_scene {
        let content = render_scene_elements_to_html(s, time - s.start, width, height)?;
        html.push_str(&format!(
            r#"<div data-scene-time="{}" style="position: absolute; width: 100%; height: 100%;">{}</div>"#,
            time - s.start, content
        ));
    }

    // Inject animation controller script to sync CSS @keyframes animations
    let animation_js = format!(
        r#"<script>
(function() {{
  const els = document.querySelectorAll('*');
  for (const el of els) {{
    const style = window.getComputedStyle(el);
    const animationName = style.animationName;
    if (animationName && animationName !== 'none') {{
      const parentWithTime = el.closest('[data-scene-time]');
      const time = parentWithTime ? parseFloat(parentWithTime.getAttribute('data-scene-time')) : {};
      let origDelay = parseFloat(el.getAttribute('data-orig-delay'));
      if (isNaN(origDelay)) {{
        const delayStr = style.animationDelay;
        origDelay = 0.0;
        if (delayStr) {{
          if (delayStr.endsWith('ms')) {{
            origDelay = parseFloat(delayStr) / 1000.0;
          }} else if (delayStr.endsWith('s')) {{
            origDelay = parseFloat(delayStr);
          }}
        }}
        el.setAttribute('data-orig-delay', origDelay);
      }}
      const newDelay = origDelay - time;
      el.style.setProperty('animation-delay', newDelay + 's', 'important');
      el.style.setProperty('animation-play-state', 'paused', 'important');
    }}
  }}
}})();
</script>"#,
        time
    );
    html.push_str(&animation_js);

    html.push_str("</body></html>");
    Ok(html)
}

fn render_scene_elements_to_html(s: &Scene, t: f64, width: u32, height: u32) -> Result<String> {
    let mut elements_html = String::new();
    let total_w = width as f64;
    let total_h = height as f64;

    for el in &s.elements {
        match el {
            SceneElement::Text { content, style, position, anchor, timeline } => {
                let (opacity, x_offset, y_offset, scale_x, scale_y, rotation) = if let Some(tl) = timeline {
                    let op = interpolate_f64(t, &tl.keyframes, |k| k.opacity, 1.0);
                    let x = interpolate_string_dimension(t, &tl.keyframes, |k| k.x.as_ref(), "0", total_w);
                    let y = interpolate_string_dimension(t, &tl.keyframes, |k| k.y.as_ref(), "0", total_h);
                    let scale = interpolate_f64(t, &tl.keyframes, |k| k.scale, 1.0);
                    let sx = interpolate_f64(t, &tl.keyframes, |k| k.scale_x, scale);
                    let sy = interpolate_f64(t, &tl.keyframes, |k| k.scale_y, scale);
                    let rot = interpolate_f64(t, &tl.keyframes, |k| k.rotation, 0.0);
                    (op, x, y, sx, sy, rot)
                } else {
                    (1.0, 0.0, 0.0, 1.0, 1.0, 0.0)
                };

                let base_x = resolve_dimension(&position.x, total_w);
                let base_y = resolve_dimension(&position.y, total_h);
                let final_x = base_x + x_offset;
                let final_y = base_y + y_offset;

                let text_align = match anchor {
                    Anchor::TopLeft | Anchor::CenterLeft | Anchor::BottomLeft => "left",
                    Anchor::TopCenter | Anchor::Center | Anchor::BottomCenter => "center",
                    Anchor::TopRight | Anchor::CenterRight | Anchor::BottomRight => "right",
                };

                elements_html.push_str(&format!(
                    r#"<div class="element" style="left: {}px; top: {}px; opacity: {}; transform: rotate({}deg) scale({}, {}); font-family: {}; font-size: {}px; color: {}; font-weight: {}; text-align: {};">{}</div>"#,
                    final_x, final_y, opacity, rotation, scale_x, scale_y, style.font_family, style.font_size, style.color, style.font_weight, text_align, content
                ));
            }
            SceneElement::Image { src, position, size, timeline, .. } => {
                let (opacity, x_offset, y_offset, scale_x, scale_y, rotation) = if let Some(tl) = timeline {
                    let op = interpolate_f64(t, &tl.keyframes, |k| k.opacity, 1.0);
                    let x = interpolate_string_dimension(t, &tl.keyframes, |k| k.x.as_ref(), "0", total_w);
                    let y = interpolate_string_dimension(t, &tl.keyframes, |k| k.y.as_ref(), "0", total_h);
                    let scale = interpolate_f64(t, &tl.keyframes, |k| k.scale, 1.0);
                    let sx = interpolate_f64(t, &tl.keyframes, |k| k.scale_x, scale);
                    let sy = interpolate_f64(t, &tl.keyframes, |k| k.scale_y, scale);
                    let rot = interpolate_f64(t, &tl.keyframes, |k| k.rotation, 0.0);
                    (op, x, y, sx, sy, rot)
                } else {
                    (1.0, 0.0, 0.0, 1.0, 1.0, 0.0)
                };

                let base_x = resolve_dimension(&position.x, total_w);
                let base_y = resolve_dimension(&position.y, total_h);
                let final_x = base_x + x_offset;
                let final_y = base_y + y_offset;

                let w = resolve_dimension(&size.width, total_w);
                let h = resolve_dimension(&size.height, total_h);

                elements_html.push_str(&format!(
                    r#"<img class="element" src="{}" style="left: {}px; top: {}px; width: {}px; height: {}px; opacity: {}; transform: rotate({}deg) scale({}, {}); object-fit: cover;"/>"#,
                    src, final_x, final_y, w, h, opacity, rotation, scale_x, scale_y
                ));
            }
            SceneElement::Shape { shape, size, position, style, timeline } => {
                let (opacity, x_offset, y_offset, scale_x, scale_y, rotation) = if let Some(tl) = timeline {
                    let op = interpolate_f64(t, &tl.keyframes, |k| k.opacity, 1.0);
                    let x = interpolate_string_dimension(t, &tl.keyframes, |k| k.x.as_ref(), "0", total_w);
                    let y = interpolate_string_dimension(t, &tl.keyframes, |k| k.y.as_ref(), "0", total_h);
                    let scale = interpolate_f64(t, &tl.keyframes, |k| k.scale, 1.0);
                    let sx = interpolate_f64(t, &tl.keyframes, |k| k.scale_x, scale);
                    let sy = interpolate_f64(t, &tl.keyframes, |k| k.scale_y, scale);
                    let rot = interpolate_f64(t, &tl.keyframes, |k| k.rotation, 0.0);
                    (op, x, y, sx, sy, rot)
                } else {
                    (1.0, 0.0, 0.0, 1.0, 1.0, 0.0)
                };

                let base_x = resolve_dimension(&position.x, total_w);
                let base_y = resolve_dimension(&position.y, total_h);
                let final_x = base_x + x_offset;
                let final_y = base_y + y_offset;

                let w = resolve_dimension(&size.width, total_w);
                let h = resolve_dimension(&size.height, total_h);
                let fill_str = style.fill.as_deref().unwrap_or("transparent");
                let stroke_str = style.stroke.as_deref().unwrap_or("none");
                let stroke_w = style.stroke_width.unwrap_or(0.0);

                let radius_str = match shape {
                    ShapeType::Circle => "border-radius: 50%;".to_string(),
                    ShapeType::RoundedRect => format!("border-radius: {}px;", style.border_radius.unwrap_or(4.0)),
                    _ => String::new(),
                };

                elements_html.push_str(&format!(
                    r#"<div class="element" style="left: {}px; top: {}px; width: {}px; height: {}px; opacity: {}; transform: rotate({}deg) scale({}, {}); background-color: {}; border: {}px solid {}; {}"></div>"#,
                    final_x, final_y, w, h, opacity, rotation, scale_x, scale_y, fill_str, stroke_w, stroke_str, radius_str
                ));
            }
            SceneElement::Svg { content, position, size, timeline } => {
                let (opacity, x_offset, y_offset, scale_x, scale_y, rotation) = if let Some(tl) = timeline {
                    let op = interpolate_f64(t, &tl.keyframes, |k| k.opacity, 1.0);
                    let x = interpolate_string_dimension(t, &tl.keyframes, |k| k.x.as_ref(), "0", total_w);
                    let y = interpolate_string_dimension(t, &tl.keyframes, |k| k.y.as_ref(), "0", total_h);
                    let scale = interpolate_f64(t, &tl.keyframes, |k| k.scale, 1.0);
                    let sx = interpolate_f64(t, &tl.keyframes, |k| k.scale_x, scale);
                    let sy = interpolate_f64(t, &tl.keyframes, |k| k.scale_y, scale);
                    let rot = interpolate_f64(t, &tl.keyframes, |k| k.rotation, 0.0);
                    (op, x, y, sx, sy, rot)
                } else {
                    (1.0, 0.0, 0.0, 1.0, 1.0, 0.0)
                };

                let base_x = resolve_dimension(&position.x, total_w);
                let base_y = resolve_dimension(&position.y, total_h);
                let final_x = base_x + x_offset;
                let final_y = base_y + y_offset;

                let w_str = size.as_ref().map(|s| format!("width: {}px;", resolve_dimension(&s.width, total_w))).unwrap_or_default();
                let h_str = size.as_ref().map(|s| format!("height: {}px;", resolve_dimension(&s.height, total_h))).unwrap_or_default();

                elements_html.push_str(&format!(
                    r#"<div class="element" style="left: {}px; top: {}px; {} {} opacity: {}; transform: rotate({}deg) scale({}, {});">{}</div>"#,
                    final_x, final_y, w_str, h_str, opacity, rotation, scale_x, scale_y, content
                ));
            }
            SceneElement::Html { content, position, size, timeline } => {
                let (opacity, x_offset, y_offset, scale_x, scale_y, rotation) = if let Some(tl) = timeline {
                    let op = interpolate_f64(t, &tl.keyframes, |k| k.opacity, 1.0);
                    let x = interpolate_string_dimension(t, &tl.keyframes, |k| k.x.as_ref(), "0", total_w);
                    let y = interpolate_string_dimension(t, &tl.keyframes, |k| k.y.as_ref(), "0", total_h);
                    let scale = interpolate_f64(t, &tl.keyframes, |k| k.scale, 1.0);
                    let sx = interpolate_f64(t, &tl.keyframes, |k| k.scale_x, scale);
                    let sy = interpolate_f64(t, &tl.keyframes, |k| k.scale_y, scale);
                    let rot = interpolate_f64(t, &tl.keyframes, |k| k.rotation, 0.0);
                    (op, x, y, sx, sy, rot)
                } else {
                    (1.0, 0.0, 0.0, 1.0, 1.0, 0.0)
                };

                let base_x = resolve_dimension(&position.x, total_w);
                let base_y = resolve_dimension(&position.y, total_h);
                let final_x = base_x + x_offset;
                let final_y = base_y + y_offset;

                let w = resolve_dimension(&size.width, total_w);
                let h = resolve_dimension(&size.height, total_h);

                elements_html.push_str(&format!(
                    r#"<div class="element" style="left: {}px; top: {}px; width: {}px; height: {}px; opacity: {}; transform: rotate({}deg) scale({}, {});">{}</div>"#,
                    final_x, final_y, w, h, opacity, rotation, scale_x, scale_y, content
                ));
            }
            SceneElement::Code { content, language: _, theme: _, position, size, font_size, timeline } => {
                let (opacity, x_offset, y_offset, scale_x, scale_y, rotation) = if let Some(tl) = timeline {
                    let op = interpolate_f64(t, &tl.keyframes, |k| k.opacity, 1.0);
                    let x = interpolate_string_dimension(t, &tl.keyframes, |k| k.x.as_ref(), "0", total_w);
                    let y = interpolate_string_dimension(t, &tl.keyframes, |k| k.y.as_ref(), "0", total_h);
                    let scale = interpolate_f64(t, &tl.keyframes, |k| k.scale, 1.0);
                    let sx = interpolate_f64(t, &tl.keyframes, |k| k.scale_x, scale);
                    let sy = interpolate_f64(t, &tl.keyframes, |k| k.scale_y, scale);
                    let rot = interpolate_f64(t, &tl.keyframes, |k| k.rotation, 0.0);
                    (op, x, y, sx, sy, rot)
                } else {
                    (1.0, 0.0, 0.0, 1.0, 1.0, 0.0)
                };

                let base_x = resolve_dimension(&position.x, total_w);
                let base_y = resolve_dimension(&position.y, total_h);
                let final_x = base_x + x_offset;
                let final_y = base_y + y_offset;

                let w = resolve_dimension(&size.width, total_w);
                let h = resolve_dimension(&size.height, total_h);

                elements_html.push_str(&format!(
                    r#"<pre class="element" style="left: {}px; top: {}px; width: {}px; height: {}px; opacity: {}; transform: rotate({}deg) scale({}, {}); font-size: {}px; overflow: auto; background: #282c34; color: #abb2bf; padding: 10px; border-radius: 4px; margin: 0;"><code>{}</code></pre>"#,
                    final_x, final_y, w, h, opacity, rotation, scale_x, scale_y, font_size, content
                ));
            }
            SceneElement::Chart { chart_type, data, position, size, theme: _, timeline } => {
                let (opacity, x_offset, y_offset, scale_x, scale_y, rotation) = if let Some(tl) = timeline {
                    let op = interpolate_f64(t, &tl.keyframes, |k| k.opacity, 1.0);
                    let x = interpolate_string_dimension(t, &tl.keyframes, |k| k.x.as_ref(), "0", total_w);
                    let y = interpolate_string_dimension(t, &tl.keyframes, |k| k.y.as_ref(), "0", total_h);
                    let scale = interpolate_f64(t, &tl.keyframes, |k| k.scale, 1.0);
                    let sx = interpolate_f64(t, &tl.keyframes, |k| k.scale_x, scale);
                    let sy = interpolate_f64(t, &tl.keyframes, |k| k.scale_y, scale);
                    let rot = interpolate_f64(t, &tl.keyframes, |k| k.rotation, 0.0);
                    (op, x, y, sx, sy, rot)
                } else {
                    (1.0, 0.0, 0.0, 1.0, 1.0, 0.0)
                };

                let base_x = resolve_dimension(&position.x, total_w);
                let base_y = resolve_dimension(&position.y, total_h);
                let final_x = base_x + x_offset;
                let final_y = base_y + y_offset;

                let w = resolve_dimension(&size.width, total_w);
                let h = resolve_dimension(&size.height, total_h);

                let chart_theme = openmedia_svg::ChartTheme::dark();
                let chart_cfg = openmedia_svg::ChartConfig {
                    chart_type: match chart_type.to_lowercase().as_str() {
                        "bar" => openmedia_svg::ChartType::Bar,
                        "line" => openmedia_svg::ChartType::Line,
                        "pie" => openmedia_svg::ChartType::Pie,
                        _ => openmedia_svg::ChartType::Bar,
                    },
                    data: data.clone(),
                    title: None,
                    subtitle: None,
                    width: w as u32,
                    height: h as u32,
                    theme: chart_theme,
                    legend: openmedia_svg::LegendConfig { show: false, position: openmedia_svg::LegendPosition::Bottom },
                    grid: true,
                    animate: false,
                    padding: openmedia_svg::Padding { top: 10.0, right: 10.0, bottom: 10.0, left: 10.0 },
                };

                let chart_xml = openmedia_svg::generate_chart(&chart_cfg)
                    .map_err(|e| OpenMediaError::Internal(e.to_string()))?;

                elements_html.push_str(&format!(
                    r#"<div class="element" style="left: {}px; top: {}px; width: {}px; height: {}px; opacity: {}; transform: rotate({}deg) scale({}, {});">{}</div>"#,
                    final_x, final_y, w, h, opacity, rotation, scale_x, scale_y, chart_xml
                ));
            }
            _ => {}
        }
    }
    Ok(elements_html)
}

pub fn apply_transition_easing(progress: f64, easing: Option<&str>) -> f64 {
    let progress = progress.clamp(0.0, 1.0);
    if let Some(eas) = easing {
        let eas = eas.trim();
        if eas.eq_ignore_ascii_case("ease_in") || eas.eq_ignore_ascii_case("ease-in") {
            progress * progress
        } else if eas.eq_ignore_ascii_case("ease_out") || eas.eq_ignore_ascii_case("ease-out") {
            progress * (2.0 - progress)
        } else if eas.eq_ignore_ascii_case("ease_in_out") || eas.eq_ignore_ascii_case("ease-in-out") {
            if progress < 0.5 {
                2.0 * progress * progress
            } else {
                -1.0 + (4.0 - 2.0 * progress) * progress
            }
        } else {
            progress
        }
    } else {
        progress
    }
}

fn box_blur(img: &image::RgbaImage, radius: usize) -> image::RgbaImage {
    let w = img.width();
    let h = img.height();
    let mut temp = image::RgbaImage::new(w, h);
    let mut out = image::RgbaImage::new(w, h);
    
    // Pass 1: Horizontal
    for y in 0..h {
        for x in 0..w {
            let mut r_sum = 0u32;
            let mut g_sum = 0u32;
            let mut b_sum = 0u32;
            let mut a_sum = 0u32;
            let mut count = 0u32;
            
            let start = if x as usize >= radius { x as usize - radius } else { 0 };
            let end = std::cmp::min(x as usize + radius, w as usize - 1);
            
            for k in start..=end {
                let p = img.get_pixel(k as u32, y);
                r_sum += p[0] as u32;
                g_sum += p[1] as u32;
                b_sum += p[2] as u32;
                a_sum += p[3] as u32;
                count += 1;
            }
            temp.put_pixel(x, y, image::Rgba([
                (r_sum / count) as u8,
                (g_sum / count) as u8,
                (b_sum / count) as u8,
                (a_sum / count) as u8,
            ]));
        }
    }
    
    // Pass 2: Vertical
    for x in 0..w {
        for y in 0..h {
            let mut r_sum = 0u32;
            let mut g_sum = 0u32;
            let mut b_sum = 0u32;
            let mut a_sum = 0u32;
            let mut count = 0u32;
            
            let start = if y as usize >= radius { y as usize - radius } else { 0 };
            let end = std::cmp::min(y as usize + radius, h as usize - 1);
            
            for k in start..=end {
                let p = temp.get_pixel(x, k as u32);
                r_sum += p[0] as u32;
                g_sum += p[1] as u32;
                b_sum += p[2] as u32;
                a_sum += p[3] as u32;
                count += 1;
            }
            out.put_pixel(x, y, image::Rgba([
                (r_sum / count) as u8,
                (g_sum / count) as u8,
                (b_sum / count) as u8,
                (a_sum / count) as u8,
            ]));
        }
    }
    out
}

// === Unified Video compiler and renderer ===
pub fn blend_frames(
    from: &image::RgbaImage,
    to: &image::RgbaImage,
    progress: f64,
    trans_type: &TransitionType,
) -> image::RgbaImage {
    let w = from.width();
    let h = from.height();
    let mut out = image::RgbaImage::new(w, h);
    
    match trans_type {
        TransitionType::Crossfade => {
            for y in 0..h {
                for x in 0..w {
                    let p1 = from.get_pixel(x, y);
                    let p2 = to.get_pixel(x, y);
                    let r = (p1[0] as f64 * (1.0 - progress) + p2[0] as f64 * progress) as u8;
                    let g = (p1[1] as f64 * (1.0 - progress) + p2[1] as f64 * progress) as u8;
                    let b = (p1[2] as f64 * (1.0 - progress) + p2[2] as f64 * progress) as u8;
                    let a = (p1[3] as f64 * (1.0 - progress) + p2[3] as f64 * progress) as u8;
                    out.put_pixel(x, y, image::Rgba([r, g, b, a]));
                }
            }
        }
        TransitionType::SlideLeft => {
            let offset = (progress * w as f64) as i32;
            for y in 0..h {
                for x in 0..w {
                    let target_x = x as i32 + offset;
                    if target_x < w as i32 {
                        out.put_pixel(x, y, *from.get_pixel(target_x as u32, y));
                    } else {
                        let to_x = target_x - w as i32;
                        out.put_pixel(x, y, *to.get_pixel(to_x as u32, y));
                    }
                }
            }
        }
        TransitionType::SlideRight => {
            let offset = (progress * w as f64) as i32;
            for y in 0..h {
                for x in 0..w {
                    let target_x = x as i32 - offset;
                    if target_x >= 0 {
                        out.put_pixel(x, y, *from.get_pixel(target_x as u32, y));
                    } else {
                        let to_x = target_x + w as i32;
                        out.put_pixel(x, y, *to.get_pixel(to_x as u32, y));
                    }
                }
            }
        }
        TransitionType::SlideUp => {
            let offset = (progress * h as f64) as i32;
            for y in 0..h {
                for x in 0..w {
                    let target_y = y as i32 + offset;
                    if target_y < h as i32 {
                        out.put_pixel(x, y, *from.get_pixel(x, target_y as u32));
                    } else {
                        let to_y = target_y - h as i32;
                        out.put_pixel(x, y, *to.get_pixel(x, to_y as u32));
                    }
                }
            }
        }
        TransitionType::SlideDown => {
            let offset = (progress * h as f64) as i32;
            for y in 0..h {
                for x in 0..w {
                    let target_y = y as i32 - offset;
                    if target_y >= 0 {
                        out.put_pixel(x, y, *from.get_pixel(x, target_y as u32));
                    } else {
                        let to_y = target_y + h as i32;
                        out.put_pixel(x, y, *to.get_pixel(x, to_y as u32));
                    }
                }
            }
        }
        TransitionType::WipeLeft => {
            let boundary = ((1.0 - progress) * w as f64) as u32;
            for y in 0..h {
                for x in 0..w {
                    if x < boundary {
                        out.put_pixel(x, y, *from.get_pixel(x, y));
                    } else {
                        out.put_pixel(x, y, *to.get_pixel(x, y));
                    }
                }
            }
        }
        TransitionType::WipeRight => {
            let boundary = (progress * w as f64) as u32;
            for y in 0..h {
                for x in 0..w {
                    if x < boundary {
                        out.put_pixel(x, y, *to.get_pixel(x, y));
                    } else {
                        out.put_pixel(x, y, *from.get_pixel(x, y));
                    }
                }
            }
        }
        TransitionType::Blur => {
            let intensity = 1.0 - (progress - 0.5).abs() * 2.0;
            let radius = (intensity * 10.0).round() as usize;
            if radius > 0 {
                let blurred_from = box_blur(from, radius);
                let blurred_to = box_blur(to, radius);
                for y in 0..h {
                    for x in 0..w {
                        let p1 = blurred_from.get_pixel(x, y);
                        let p2 = blurred_to.get_pixel(x, y);
                        let r = (p1[0] as f64 * (1.0 - progress) + p2[0] as f64 * progress) as u8;
                        let g = (p1[1] as f64 * (1.0 - progress) + p2[1] as f64 * progress) as u8;
                        let b = (p1[2] as f64 * (1.0 - progress) + p2[2] as f64 * progress) as u8;
                        let a = (p1[3] as f64 * (1.0 - progress) + p2[3] as f64 * progress) as u8;
                        out.put_pixel(x, y, image::Rgba([r, g, b, a]));
                    }
                }
            } else {
                for y in 0..h {
                    for x in 0..w {
                        let p1 = from.get_pixel(x, y);
                        let p2 = to.get_pixel(x, y);
                        let r = (p1[0] as f64 * (1.0 - progress) + p2[0] as f64 * progress) as u8;
                        let g = (p1[1] as f64 * (1.0 - progress) + p2[1] as f64 * progress) as u8;
                        let b = (p1[2] as f64 * (1.0 - progress) + p2[2] as f64 * progress) as u8;
                        let a = (p1[3] as f64 * (1.0 - progress) + p2[3] as f64 * progress) as u8;
                        out.put_pixel(x, y, image::Rgba([r, g, b, a]));
                    }
                }
            }
        }
        TransitionType::Glitch => {
            let intensity = 1.0 - (progress - 0.5).abs() * 2.0;
            let disp_max = (intensity * 15.0) as i32;
            
            for y in 0..h {
                // scanline row displacement
                let mut seed = y as u32 + (progress * 1000.0) as u32;
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                
                let offset_x = if seed % 100 < (intensity * 40.0) as u32 {
                    ((seed % 31) as i32 - 15) * disp_max / 15
                } else {
                    0
                };

                for x in 0..w {
                    let get_chan = |img: &image::RgbaImage, channel: usize, dx: i32| -> u8 {
                        let target_x = std::cmp::max(0, std::cmp::min(w as i32 - 1, x as i32 + dx)) as u32;
                        img.get_pixel(target_x, y)[channel]
                    };

                    // Read split channels from source
                    let r1 = get_chan(from, 0, offset_x - 3);
                    let g1 = get_chan(from, 1, offset_x);
                    let b1 = get_chan(from, 2, offset_x + 3);
                    let a1 = get_chan(from, 3, offset_x);

                    // Read split channels from target
                    let r2 = get_chan(to, 0, offset_x - 3);
                    let g2 = get_chan(to, 1, offset_x);
                    let b2 = get_chan(to, 2, offset_x + 3);
                    let a2 = get_chan(to, 3, offset_x);

                    // Blend channels
                    let mut r = (r1 as f64 * (1.0 - progress) + r2 as f64 * progress) as i32;
                    let mut g = (g1 as f64 * (1.0 - progress) + g2 as f64 * progress) as i32;
                    let mut b = (b1 as f64 * (1.0 - progress) + b2 as f64 * progress) as i32;
                    let a = (a1 as f64 * (1.0 - progress) + a2 as f64 * progress) as u8;

                    // Add a touch of noise/static
                    if intensity > 0.1 && (seed % 97) == 0 {
                        let noise = ((seed % 51) as i32 - 25) * (intensity * 2.0) as i32;
                        r = std::cmp::max(0, std::cmp::min(255, r + noise));
                        g = std::cmp::max(0, std::cmp::min(255, g + noise));
                        b = std::cmp::max(0, std::cmp::min(255, b + noise));
                    }

                    out.put_pixel(x, y, image::Rgba([r as u8, g as u8, b as u8, a]));
                }
            }
        }
        TransitionType::RadialWipe => {
            let cx = w as f64 / 2.0;
            let cy = h as f64 / 2.0;
            for y in 0..h {
                for x in 0..w {
                    let dx = x as f64 - cx;
                    let dy = y as f64 - cy;
                    let angle = dy.atan2(dx) + std::f64::consts::PI; // [0, 2*PI]
                    let angle_ratio = angle / (2.0 * std::f64::consts::PI);
                    if angle_ratio < progress {
                        out.put_pixel(x, y, *to.get_pixel(x, y));
                    } else {
                        out.put_pixel(x, y, *from.get_pixel(x, y));
                    }
                }
            }
        }
        _ => {
            for y in 0..h {
                for x in 0..w {
                    let p1 = from.get_pixel(x, y);
                    let p2 = to.get_pixel(x, y);
                    let r = (p1[0] as f64 * (1.0 - progress) + p2[0] as f64 * progress) as u8;
                    let g = (p1[1] as f64 * (1.0 - progress) + p2[1] as f64 * progress) as u8;
                    let b = (p1[2] as f64 * (1.0 - progress) + p2[2] as f64 * progress) as u8;
                    let a = (p1[3] as f64 * (1.0 - progress) + p2[3] as f64 * progress) as u8;
                    out.put_pixel(x, y, image::Rgba([r, g, b, a]));
                }
            }
        }
    }
    out
}

#[derive(Clone)]
enum CacheEntry {
    Success(Vec<u8>),
    Failure(std::time::Instant),
}

pub async fn resolve_custom_fonts(
    custom_fonts: &[CustomFontSpec],
) -> std::collections::HashMap<String, Vec<u8>> {
    static MEMORY_CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, CacheEntry>>> = std::sync::OnceLock::new();
    let cache = MEMORY_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    
    static HTTP_CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    let client = HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_default()
    });
    
    let mut resolved = std::collections::HashMap::new();
    let cache_dir = std::env::temp_dir().join("openmedia_fonts_cache");
    let _ = tokio::fs::create_dir_all(&cache_dir).await;

    for font in custom_fonts {
        // Check memory cache first
        let mut cached_bytes = None;
        let mut is_cached_failure = false;
        
        if let Ok(guard) = cache.lock() {
            if let Some(entry) = guard.get(&font.src) {
                match entry {
                    CacheEntry::Success(bytes) => {
                        cached_bytes = Some(bytes.clone());
                    }
                    CacheEntry::Failure(instant) => {
                        if instant.elapsed() <= std::time::Duration::from_secs(60) {
                            is_cached_failure = true;
                        }
                    }
                }
            }
        }

        if let Some(bytes) = cached_bytes {
            resolved.insert(font.family.clone(), bytes);
            continue;
        }
        if is_cached_failure {
            continue;
        }

        // If not in cache, attempt resolution
        let font_bytes = if font.src.starts_with("http://") || font.src.starts_with("https://") {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            font.src.hash(&mut hasher);
            let hash_val = hasher.finish();
            let cached_path = cache_dir.join(format!("{}.ttf", hash_val));

            if tokio::fs::metadata(&cached_path).await.is_ok() {
                tokio::fs::read(&cached_path).await.ok()
            } else {
                if let Ok(resp) = client.get(&font.src).send().await {
                    if let Ok(bytes) = resp.bytes().await {
                        let bytes_vec = bytes.to_vec();
                        let temp_file_name = format!("{}.tmp", uuid::Uuid::new_v4());
                        let temp_path = cache_dir.join(temp_file_name);
                        if tokio::fs::write(&temp_path, &bytes_vec).await.is_ok() {
                            if tokio::fs::rename(&temp_path, &cached_path).await.is_err() {
                                let _ = tokio::fs::remove_file(&temp_path).await;
                            }
                        }
                        Some(bytes_vec)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        } else {
            tokio::fs::read(&font.src).await.ok()
        };

        // Cache the result (Some(bytes) if resolution succeeded, Failure if it failed)
        if let Ok(mut guard) = cache.lock() {
            let entry = match &font_bytes {
                Some(bytes) => CacheEntry::Success(bytes.clone()),
                None => CacheEntry::Failure(std::time::Instant::now()),
            };
            guard.insert(font.src.clone(), entry);
        }

        if let Some(bytes) = font_bytes {
            resolved.insert(font.family.clone(), bytes);
        }
    }
    resolved
}

pub async fn render_video_scene(
    scene: &VideoScene,
    output_path: &Path,
) -> Result<openmedia_core::VideoSpec> {
    let start_time = std::time::Instant::now();
    let width = scene.width;
    let height = scene.height;
    let fps = scene.fps;
    let duration = scene.duration;
    let total_frames = (duration * fps as f64).round() as u32;

    let use_browser = scene.scenes.iter().any(|s| {
        s.elements.iter().any(|el| {
            matches!(el, SceneElement::Html { .. } | SceneElement::Code { .. })
        })
    });

    let renderer_name = if use_browser { "browser" } else { "svg" };
    let temp_silent = output_path.with_extension("silent.mp4");

    // Spawn FFmpeg pipe
    let mut cmd = tokio::process::Command::new("ffmpeg");
    cmd.args([
        "-y",
        "-f", "image2pipe",
        "-vcodec", "mjpeg",
        "-r", &fps.to_string(),
        "-i", "-",
        "-c:v", "libx264",
        "-pix_fmt", "yuv420p",
        "-crf", "23",
        "-preset", "medium",
    ])
    .arg(&temp_silent);

    cmd.stdin(std::process::Stdio::piped())
       .stdout(std::process::Stdio::null())
       .stderr(std::process::Stdio::null());

    let mut child = cmd.spawn().map_err(OpenMediaError::IoError)?;
    let mut stdin = child.stdin.take().ok_or_else(|| OpenMediaError::Internal("Failed to open FFmpeg stdin".into()))?;

    if use_browser {
        let renderer = BrowserFrameRenderer::launch().await?;
        for f in 0..total_frames {
            let t = f as f64 / fps as f64;
            let frame = renderer.render_frame(scene, t, width, height).await?;
            let rgb_frame = image::DynamicImage::ImageRgba8(frame).into_rgb8();
            let mut bytes = Vec::new();
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 95);
            rgb_frame.write_with_encoder(encoder).map_err(|e| OpenMediaError::ImageEncodeError { format: "jpeg".to_string(), reason: e.to_string() })?;
            stdin.write_all(&bytes).await.map_err(OpenMediaError::IoError)?;
        }
        renderer.close().await;
    } else {
        let renderer = SvgFrameRenderer;
        for f in 0..total_frames {
            let t = f as f64 / fps as f64;
            let frame = renderer.render_frame(scene, t, width, height).await?;
            let rgb_frame = image::DynamicImage::ImageRgba8(frame).into_rgb8();
            let mut bytes = Vec::new();
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 95);
            rgb_frame.write_with_encoder(encoder).map_err(|e| OpenMediaError::ImageEncodeError { format: "jpeg".to_string(), reason: e.to_string() })?;
            stdin.write_all(&bytes).await.map_err(OpenMediaError::IoError)?;
        }
    }

    drop(stdin);
    let status = child.wait().await.map_err(OpenMediaError::IoError)?;
    if !status.success() {
        return Err(OpenMediaError::Internal(format!("FFmpeg frame rendering failed with exit status {:?}", status.code())));
    }

    // Mix audio if present
    if let Some(audio_cfg) = &scene.audio {
        let mut audio_cmd = tokio::process::Command::new("ffmpeg");
        audio_cmd.arg("-y").arg("-i").arg(&temp_silent);
        
        for track in &audio_cfg.tracks {
            audio_cmd.arg("-i").arg(&track.src);
        }

        // Generate filter_complex script to delay and volume blend
        let mut filter_complex = String::new();
        for (i, track) in audio_cfg.tracks.iter().enumerate() {
            let delay_ms = (track.start * 1000.0) as i32;
            let idx = i + 1;
            filter_complex.push_str(&format!(
                "[{}:a]adelay={}|{},volume={}[a{}];",
                idx, delay_ms, delay_ms, track.volume, idx
            ));
        }

        for i in 0..audio_cfg.tracks.len() {
            filter_complex.push_str(&format!("[a{}]", i + 1));
        }
        filter_complex.push_str(&format!("amix=inputs={}:duration=first[out_a]", audio_cfg.tracks.len()));

        audio_cmd.args([
            "-filter_complex", &filter_complex,
            "-map", "0:v",
            "-map", "[out_a]",
            "-c:v", "copy",
            "-c:a", "aac",
        ])
        .arg(output_path);

        audio_cmd.stdout(std::process::Stdio::piped())
                 .stderr(std::process::Stdio::piped());

        let mix_child = audio_cmd.spawn().map_err(OpenMediaError::IoError)?;
        let output = mix_child.wait_with_output().await.map_err(OpenMediaError::IoError)?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(OpenMediaError::EncodingError(format!(
                "FFmpeg audio mixing failed with status {:?}.\nStdout: {}\nStderr: {}",
                output.status.code(), stdout, stderr
            )));
        }
        
        let _ = std::fs::remove_file(temp_silent);
    } else {
        std::fs::rename(temp_silent, output_path).map_err(OpenMediaError::IoError)?;
    }

    let file_size = std::fs::metadata(output_path).map(|m| m.len()).unwrap_or(0);
    let generation_time = start_time.elapsed().as_secs_f64();

    Ok(openmedia_core::VideoSpec {
        path: output_path.to_path_buf(),
        width,
        height,
        duration,
        fps,
        codec: "h264".to_string(),
        file_size,
        generation_id: uuid::Uuid::now_v7().to_string(),
        renderer_used: renderer_name.to_string(),
        total_frames,
        generation_time,
    })
}

// === Legacy helper matching the old schema ===
pub async fn html_to_image(
    html_content: &str,
    width: Option<u32>,
    height: Option<u32>,
    device_scale_factor: Option<f64>,
    format: &str,
    output_path: &Path,
) -> Result<ImageOutput> {
    use std::time::Instant;
    use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;
    use chromiumoxide::page::ScreenshotParams;

    let start_time = Instant::now();

    let w = width.unwrap_or(1920);
    let h = height.unwrap_or(1080);
    let scale = device_scale_factor.unwrap_or(1.0);

    let profile_dir = std::env::temp_dir().join(format!("chrome-profile-{}", uuid::Uuid::new_v4()));
    let config = BrowserConfig::builder()
        .viewport(Viewport {
            width: w,
            height: h,
            device_scale_factor: Some(scale),
            emulating_mobile: false,
            is_landscape: w > h,
            has_touch: false,
        })
        .no_sandbox()
        .user_data_dir(profile_dir)
        .build()
        .map_err(|e| OpenMediaError::ConfigError(e.to_string()))?;

    let (mut browser, mut handler) = Browser::launch(config).await
        .map_err(|_| OpenMediaError::ChromeNotFound)?;

    tokio::spawn(async move {
        while let Some(h) = handler.next().await {
            if let Err(err) = h {
                tracing::error!("Legacy browser handler error: {:?}", err);
            }
        }
    });

    let page = browser.new_page("about:blank").await
        .map_err(|e| OpenMediaError::Internal(e.to_string()))?;

    let html_path = Path::new(html_content);
    if html_path.exists() && html_path.is_file() {
        let abs_path = html_path.canonicalize()
            .map_err(OpenMediaError::IoError)?;
        let url = format!("file://{}", abs_path.to_string_lossy());
        page.goto(&url).await
            .map_err(|e| OpenMediaError::Internal(e.to_string()))?;
    } else {
        page.set_content(html_content).await
            .map_err(|e| OpenMediaError::Internal(e.to_string()))?;
    }

    // Give headless Chrome a delay (200ms) to ensure resources and layouts are fully loaded/rendered
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let clean_format = format.to_lowercase();
    let cdp_format = match clean_format.as_str() {
        "png" => CaptureScreenshotFormat::Png,
        "jpeg" | "jpg" => CaptureScreenshotFormat::Jpeg,
        "webp" => CaptureScreenshotFormat::Webp,
        other => {
            return Err(OpenMediaError::InvalidParameter {
                param: "output_format".to_string(),
                reason: format!("Unsupported screenshot format: {}", other),
            });
        }
    };

    let params = ScreenshotParams::builder()
        .format(cdp_format)
        .build();

    page.save_screenshot(params, output_path).await
        .map_err(|e| OpenMediaError::ImageEncodeError {
            format: clean_format.clone(),
            reason: e.to_string(),
        })?;

    let _ = browser.close().await;

    let file_size = std::fs::metadata(output_path)?.len();
    let generation_time = start_time.elapsed().as_secs_f64();

    Ok(ImageOutput {
        path: output_path.to_path_buf(),
        width: w,
        height: h,
        seed: 0,
        format: clean_format,
        file_size,
        generation_id: uuid::Uuid::now_v7().to_string(),
        clip_score: None,
        aesthetic_score: None,
        model_used: "headless-chrome".to_string(),
        backend_used: "chromiumoxide".to_string(),
        generation_time,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transition_easing_calculations() {
        assert_eq!(apply_transition_easing(0.5, None), 0.5);
        assert_eq!(apply_transition_easing(0.5, Some("ease_in")), 0.25);
        assert_eq!(apply_transition_easing(0.5, Some("ease-out ")), 0.75);
        assert_eq!(apply_transition_easing(0.25, Some("EASE_IN_OUT")), 0.125);
        assert_eq!(apply_transition_easing(0.75, Some("ease_in_out")), 0.875);
    }

    #[tokio::test]
    async fn test_custom_font_resolution_local() {
        let temp_dir = std::env::temp_dir();
        let font_file = temp_dir.join("test_font.ttf");
        let expected_bytes = vec![1, 2, 3, 4];
        std::fs::write(&font_file, &expected_bytes).unwrap();

        let spec = CustomFontSpec {
            family: "TestFont".to_string(),
            src: font_file.to_string_lossy().to_string(),
        };

        let resolved = resolve_custom_fonts(&[spec]).await;
        assert_eq!(resolved.get("TestFont"), Some(&expected_bytes));

        let _ = std::fs::remove_file(font_file);
    }

    #[tokio::test]
    async fn test_custom_font_resolution_memory_cache() {
        let temp_dir = std::env::temp_dir();
        let font_file = temp_dir.join("test_font_cache.ttf");
        let expected_bytes = vec![5, 6, 7, 8];
        std::fs::write(&font_file, &expected_bytes).unwrap();

        let spec = CustomFontSpec {
            family: "CachedFont".to_string(),
            src: font_file.to_string_lossy().to_string(),
        };

        // First call loads from file and caches it
        let resolved = resolve_custom_fonts(&[spec.clone()]).await;
        assert_eq!(resolved.get("CachedFont"), Some(&expected_bytes));

        // Delete the file to ensure subsequent reads come from the memory cache
        let _ = std::fs::remove_file(&font_file);

        // Second call should retrieve from cache
        let resolved_again = resolve_custom_fonts(&[spec]).await;
        assert_eq!(resolved_again.get("CachedFont"), Some(&expected_bytes));
    }

    #[tokio::test]
    async fn test_custom_font_failed_caching() {
        let temp_dir = std::env::temp_dir();
        let font_file = temp_dir.join("failed_font.ttf");
        let _ = std::fs::remove_file(&font_file); // Ensure it doesn't exist

        let spec = CustomFontSpec {
            family: "FailedFont".to_string(),
            src: font_file.to_string_lossy().to_string(),
        };

        // First call fails to load (file doesn't exist), should not be in resolved map
        let resolved = resolve_custom_fonts(&[spec.clone()]).await;
        assert!(resolved.get("FailedFont").is_none());

        // Now create the file
        let expected_bytes = vec![9, 10, 11, 12];
        std::fs::write(&font_file, &expected_bytes).unwrap();

        // Second call should still fail/be skipped because it cached None
        let resolved_again = resolve_custom_fonts(&[spec]).await;
        assert!(resolved_again.get("FailedFont").is_none());

        let _ = std::fs::remove_file(&font_file);
    }

    #[tokio::test]
    async fn test_get_html_font_css_cache() {
        let temp_dir = std::env::temp_dir();
        let font_file = temp_dir.join("css_cache_font.ttf");
        let expected_bytes = vec![13, 14, 15, 16];
        std::fs::write(&font_file, &expected_bytes).unwrap();

        let spec = CustomFontSpec {
            family: "CssFont".to_string(),
            src: font_file.to_string_lossy().to_string(),
        };

        // Get CSS
        let css1 = get_html_font_css(&[spec.clone()]).await;
        assert!(css1.contains("font-family: 'CssFont';"));
        assert!(css1.contains("url('data:font/truetype;charset=utf-8;base64,"));

        // Delete file
        let _ = std::fs::remove_file(&font_file);

        // Get CSS again, should be a cache hit
        let css2 = get_html_font_css(&[spec]).await;
        assert_eq!(css1, css2);
    }

    #[test]
    fn test_advanced_transitions_blend() {
        let from = image::RgbaImage::from_pixel(100, 100, image::Rgba([255, 0, 0, 255]));
        let to = image::RgbaImage::from_pixel(100, 100, image::Rgba([0, 0, 255, 255]));

        let blended_blur = blend_frames(&from, &to, 0.5, &TransitionType::Blur);
        assert_eq!(blended_blur.width(), 100);

        let blended_glitch = blend_frames(&from, &to, 0.5, &TransitionType::Glitch);
        assert_eq!(blended_glitch.width(), 100);

        let blended_radial = blend_frames(&from, &to, 0.5, &TransitionType::RadialWipe);
        assert_eq!(blended_radial.width(), 100);
    }
}
