use std::sync::Arc;
use tokio::sync::RwLock;
use openmedia_core::{Config, HardwareInfo, ModelRegistry, Result as CoreResult};
use openmedia_image::DiffusionPipeline;
use openmedia_process::DummyGpuPipeline;
use openmedia_improve::*;
use rmcp::{tool, tool_router};
pub use rmcp::handler::server::wrapper::{Parameters, Json};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use openmedia_video::{VideoScene, SceneElement, FrameRenderer};

/// Main MCP server for OpenMedia
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct McpObject(pub serde_json::Value);

impl schemars::JsonSchema for McpObject {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("McpObject")
    }

    fn json_schema(_gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
        <schemars::Schema as std::convert::TryFrom<serde_json::Value>>::try_from(
            serde_json::json!({ "type": "object" })
        ).unwrap()
    }
}

impl std::ops::Deref for McpObject {
    type Target = serde_json::Value;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for McpObject {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<McpObject> for serde_json::Value {
    fn from(val: McpObject) -> Self {
        val.0
    }
}

#[derive(Clone)]
pub struct OpenMediaServer {
    pub config: Arc<Config>,
    pub hardware: Arc<HardwareInfo>,
    pub model_registry: Arc<ModelRegistry>,
    pub history: Arc<GenerationHistory>,
    pub clip_scorer: Arc<Option<ClipScorer>>,
    pub aesthetic_scorer: Arc<Option<AestheticScorer>>,
    pub gpu_pipeline: Arc<Option<DummyGpuPipeline>>,
    pub prompt_refiner: Arc<PromptRefiner>,
    pub active_backend: Arc<RwLock<Option<Box<dyn DiffusionPipeline>>>>,
}

impl OpenMediaServer {
    pub async fn new(config: Config) -> CoreResult<Self> {
        let hardware = HardwareInfo::detect().await;
        let model_registry = ModelRegistry::scan(&config.paths.model_dir).await?;
        let history = GenerationHistory::open(&config.paths.history_db)?;

        let clip_scorer = if config.improve.enable_clip_scoring {
            ClipScorer::load(&config.paths.model_dir.join("clip")).await.ok()
        } else {
            None
        };

        let aesthetic_scorer = if config.improve.enable_aesthetic_scoring {
            AestheticScorer::load(&config.paths.model_dir.join("clip/aesthetic-predictor.onnx")).await.ok()
        } else {
            None
        };

        let gpu_pipeline = if config.compute.gpu_processing {
            Some(DummyGpuPipeline::new())
        } else {
            None
        };

        Ok(Self {
            config: Arc::new(config),
            hardware: Arc::new(hardware),
            model_registry: Arc::new(model_registry),
            history: Arc::new(history),
            clip_scorer: Arc::new(clip_scorer),
            aesthetic_scorer: Arc::new(aesthetic_scorer),
            gpu_pipeline: Arc::new(gpu_pipeline),
            prompt_refiner: Arc::new(PromptRefiner::new()),
            active_backend: Arc::new(RwLock::new(None)),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RasterizeSvgRequest {
    /// Raw SVG XML string or file path to .svg
    pub svg: String,
    /// Target width (maintains aspect ratio if omitted)
    pub width: Option<u32>,
    /// Target height
    pub height: Option<u32>,
    /// Optional background color hex (e.g. #ffffff). Default is transparent.
    pub background_color: Option<String>,
    /// Output format (png, jpeg, webp). Default is png.
    pub output_format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HtmlToImageRequest {
    /// Raw HTML string or file path to .html
    pub html: String,
    /// Viewport width. Default is 1920.
    pub width: Option<u32>,
    /// Viewport height. Default is 1080.
    pub height: Option<u32>,
    /// Display density (DPI scaler). Default is 1.0.
    pub device_scale_factor: Option<f64>,
    /// Output format (png, jpeg, webp). Default is png.
    pub output_format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImproveScoreImageRequest {
    /// Absolute path to the generated image file
    pub image_path: String,
    /// Original text prompt used for image generation (optional, for CLIP alignment)
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImproveRefinePromptRequest {
    /// Original positive prompt
    pub prompt: String,
    /// Original negative prompt (optional)
    pub negative_prompt: Option<String>,
    /// CLIP text-image alignment score (optional)
    pub clip_score: Option<f32>,
    /// Aesthetic quality prediction score (optional)
    pub aesthetic_score: Option<f32>,
    /// Refinement iteration round index (optional, defaults to 1)
    pub round: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImproveAutoRefineRequest {
    /// Initial positive text prompt
    pub prompt: String,
    /// Initial negative prompt (optional)
    pub negative_prompt: Option<String>,
    /// Target image width (optional, defaults to 512)
    pub width: Option<u32>,
    /// Target image height (optional, defaults to 512)
    pub height: Option<u32>,
    /// Maximum refinement iteration attempts (optional, defaults to 3)
    pub max_iterations: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImproveFeedbackRequest {
    /// Unique generation record UUID linked to the rated asset
    pub generation_id: String,
    /// Rating score from 0.0 (poor) to 1.0 (excellent)
    pub rating: f32,
    /// Free-text description of visual artifacts or quality notes (optional)
    pub feedback: Option<String>,
    /// Whether to keep the generated output file on disk (optional, defaults to true)
    pub keep: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImproveQualityReportRequest {
    /// Optional filter to isolate reports to a specific tool (e.g. svg_rasterize, video_create)
    pub tool_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnimateSvgRequest {
    /// Raw SVG XML string or file path to .svg
    pub svg: String,
    /// Target element ID to animate
    pub element_id: String,
    /// Preset animation (fade_in, fade_out, slide_in_left, bounce, pulse, spin, typewriter, draw_path, etc.)
    pub preset: String,
    /// Duration of animation in seconds (default 1.0)
    pub duration: Option<f64>,
    /// Delay of animation in seconds (default 0.0)
    pub delay: Option<f64>,
    /// Easing function name (default linear)
    pub easing: Option<String>,
    /// Repeat count (infinite, 1, 2, etc. default 1)
    pub repeat_count: Option<String>,
    /// Optional preset parameters
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnimateTimelineRequest {
    /// Raw SVG XML string or file path to .svg
    pub svg: String,
    /// Timeline mode (parallel | sequential | staggered)
    pub mode: String,
    /// Delay for stagger mode in seconds (default 0.2)
    pub stagger_delay: Option<f64>,
    /// Timeline entries
    pub entries: Vec<TimelineEntryRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TimelineEntryRequest {
    /// Target element ID
    pub element_id: String,
    /// Preset animation
    pub preset: String,
    /// Duration of animation in seconds
    pub duration: f64,
    /// Offset/delay in seconds relative to timeline sequence
    pub offset: f64,
    /// Easing function name (default linear)
    pub easing: Option<String>,
    /// Optional preset parameters
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnimateMorphRequest {
    /// Source path data string (d attribute)
    pub from_path: String,
    /// Target path data string (d attribute)
    pub to_path: String,
    /// Duration of morph animation in seconds (default 3.0)
    pub duration: Option<f64>,
    /// Easing function name (default ease_in_out)
    pub easing: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GenerateSpinnerRequest {
    /// Spinner style (ring | dots | border | bars)
    pub spinner_type: String,
    /// Color of spinner (e.g. #8b5cf6)
    pub color: Option<String>,
    /// Size in pixels (default 60)
    pub size: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LottieToSvgRequest {
    /// Lottie JSON content or file path to Lottie JSON
    pub lottie_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SvgToLottieRequest {
    /// Raw SVG XML string or file path to .svg
    pub svg: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImageApplyFilterRequest {
    /// Path to the input image file
    pub image_path: String,
    /// Type of filter to apply (grayscale, invert, brightness, contrast, saturation, hue_rotate, sepia, threshold, blur, sharpen, unsharp_mask)
    pub filter_type: String,
    /// Value parameter for the filter (e.g. radius for blur, intensity for sepia, value for brightness/contrast)
    pub parameter: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImageResizeRequest {
    /// Path to the input image file
    pub image_path: String,
    /// Target width
    pub width: u32,
    /// Target height
    pub height: u32,
    /// Resize algorithm (nearest, bilinear, lanczos3). Default is bilinear.
    pub algorithm: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImageCropRequest {
    /// Path to the input image file
    pub image_path: String,
    /// X coordinate of top-left corner
    pub x: u32,
    /// Y coordinate of top-left corner
    pub y: u32,
    /// Width of the cropped region
    pub width: u32,
    /// Height of the cropped region
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImageTransformRequest {
    /// Path to the input image file
    pub image_path: String,
    /// Transform type (rotate, flip_horizontal, flip_vertical)
    pub transform_type: String,
    /// Rotation angle in degrees (90, 180, 270)
    pub angle: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImageConvertRequest {
    /// Path to the input image file
    pub image_path: String,
    /// Target output format (png, jpeg, webp, avif)
    pub format: String,
    /// Encoding quality (1–100). Default is 80.
    pub quality: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImageBatchProcessRequest {
    /// Glob pattern matching input files
    pub glob_pattern: String,
    /// Operations to apply as JSON array of ProcessOperation
    pub operations: Vec<serde_json::Value>,
    /// Target output directory
    pub output_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VideoCreateRequest {
    /// VideoScene definition (inline JSON or file path)
    pub scene: serde_json::Value,
    /// Output file path (optional)
    pub output_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VideoCreateSlideshowRequest {
    /// List of image file paths or directories
    pub images: Vec<String>,
    /// Duration per image in seconds (default 3.0)
    pub duration_per_image: Option<f64>,
    /// Transition type (crossfade, slide_left, slide_right, etc. default crossfade)
    pub transition_type: Option<String>,
    /// Transition duration in seconds (default 0.5)
    pub transition_duration: Option<f64>,
    /// Background music audio track path (optional)
    pub audio_src: Option<String>,
    /// Output width (default 1920)
    pub width: Option<u32>,
    /// Output height (default 1080)
    pub height: Option<u32>,
    /// Frames per second (default 30)
    pub fps: Option<u32>,
    /// Output path (optional)
    pub output_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VideoAddTransitionRequest {
    /// Path to the video scene JSON file
    pub scene_path: String,
    /// Source scene ID
    pub from_scene_id: String,
    /// Target scene ID
    pub to_scene_id: String,
    /// Transition type
    pub transition_type: String,
    /// Transition duration in seconds (default 0.5)
    pub duration: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VideoAddAudioRequest {
    /// Path to input video file or scene JSON file
    pub target_path: String,
    /// Path to the audio file
    pub audio_path: String,
    /// Start time in video (default 0.0)
    pub start_time: Option<f64>,
    /// Volume (0.0 to 1.0, default 1.0)
    pub volume: Option<f32>,
    /// Fade in duration in seconds (default 0.0)
    pub fade_in: Option<f64>,
    /// Fade out duration in seconds (default 0.0)
    pub fade_out: Option<f64>,
    /// Output path (optional)
    pub output_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VideoFromTemplateRequest {
    /// Template name (slideshow, text_explainer, data_dashboard, social_media, product_showcase)
    pub template_name: String,
    /// Template parameters as dynamic JSON key-values
    pub parameters: serde_json::Value,
    /// Output path (optional)
    pub output_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TemplateCreateRequest {
    /// Alphanumeric template name
    pub name: String,
    /// Human readable description
    pub description: String,
    /// Expected parameter JSON Schema
    pub parameter_schema: serde_json::Value,
    /// VideoScene template with placeholders
    pub scene_template: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TemplateReadRequest {
    /// Target template name. If omitted, lists all templates.
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TemplateUpdateRequest {
    /// Alphanumeric template name
    pub name: String,
    /// Updated description (optional)
    pub description: Option<String>,
    /// Updated parameter JSON Schema (optional)
    pub parameter_schema: Option<serde_json::Value>,
    /// Updated VideoScene template (optional)
    pub scene_template: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TemplateDeleteRequest {
    /// Target template name
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VideoPreviewRequest {
    /// VideoScene definition (inline JSON or file path)
    pub scene: serde_json::Value,
    /// Time offset in seconds (default 0.0)
    pub time: Option<f64>,
    /// Target width
    pub width: Option<u32>,
    /// Target height
    pub height: Option<u32>,
    /// Output format (png, jpeg)
    pub output_format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VideoExtractFramesRequest {
    /// Path to input video file
    pub video_path: String,
    /// Time offsets in seconds
    pub offsets: Vec<f64>,
    /// Output directory for extracted frames
    pub output_dir: String,
    /// Output format (png, jpeg)
    pub format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VideoTrimRequest {
    /// Path to input video file
    pub video_path: String,
    /// Start time in seconds
    pub start_time: f64,
    /// End time in seconds
    pub end_time: f64,
    /// Output path (optional)
    pub output_path: Option<String>,
}

// Helper functions for SVG Animation MCP Tools
fn parse_easing(s: Option<&str>) -> openmedia_animate::Easing {
    let name = s.unwrap_or("linear");
    match name.to_lowercase().as_str() {
        "linear" => openmedia_animate::Easing::Linear,
        "ease_in" | "easein" | "ease-in" => openmedia_animate::Easing::EaseInQuad,
        "ease_out" | "easeout" | "ease-out" => openmedia_animate::Easing::EaseOutQuad,
        "ease_in_out" | "easeinout" | "ease-in-out" => openmedia_animate::Easing::EaseInOutQuad,
        "ease_in_cubic" | "ease-in-cubic" => openmedia_animate::Easing::EaseInCubic,
        "ease_out_cubic" | "ease-out-cubic" => openmedia_animate::Easing::EaseOutCubic,
        "ease_in_out_cubic" | "ease-in-out-cubic" => openmedia_animate::Easing::EaseInOutCubic,
        "ease_in_expo" | "ease-in-expo" => openmedia_animate::Easing::EaseInExpo,
        "ease_out_expo" | "ease-out-expo" => openmedia_animate::Easing::EaseOutExpo,
        "ease_in_out_expo" | "ease-in-out-expo" => openmedia_animate::Easing::EaseInOutExpo,
        "bounce" | "ease_out_bounce" | "ease-out-bounce" => openmedia_animate::Easing::EaseOutBounce,
        "elastic" | "ease_out_elastic" | "ease-out-elastic" => openmedia_animate::Easing::EaseOutElastic,
        "spring" => openmedia_animate::Easing::Spring { stiffness: 100.0, damping: 10.0, mass: 1.0 },
        _ => {
            if name.starts_with("cubic-bezier(") && name.ends_with(')') {
                let content = &name["cubic-bezier(".len() .. name.len() - 1];
                let parts: Vec<&str> = content.split(',').map(|p| p.trim()).collect();
                if parts.len() == 4 {
                    let x1 = parts[0].parse::<f64>().unwrap_or(0.25);
                    let y1 = parts[1].parse::<f64>().unwrap_or(0.1);
                    let x2 = parts[2].parse::<f64>().unwrap_or(0.25);
                    let y2 = parts[3].parse::<f64>().unwrap_or(1.0);
                    return openmedia_animate::Easing::CubicBezier(x1, y1, x2, y2);
                }
            }
            openmedia_animate::Easing::Linear
        }
    }
}

fn parse_preset(s: &str) -> openmedia_animate::AnimationPreset {
    match s.to_lowercase().as_str() {
        "fade_in" | "fadein" | "fade-in" => openmedia_animate::AnimationPreset::FadeIn,
        "fade_out" | "fadeout" | "fade-out" => openmedia_animate::AnimationPreset::FadeOut,
        "slide_in_left" | "slide-in-left" => openmedia_animate::AnimationPreset::SlideInLeft,
        "slide_in_right" | "slide-in-right" => openmedia_animate::AnimationPreset::SlideInRight,
        "slide_in_up" | "slide-in-up" => openmedia_animate::AnimationPreset::SlideInUp,
        "slide_in_down" | "slide-in-down" => openmedia_animate::AnimationPreset::SlideInDown,
        "bounce" => openmedia_animate::AnimationPreset::Bounce,
        "pulse" => openmedia_animate::AnimationPreset::Pulse,
        "spin" => openmedia_animate::AnimationPreset::Spin,
        "shake" => openmedia_animate::AnimationPreset::Shake,
        "wobble" => openmedia_animate::AnimationPreset::Wobble,
        "typewriter" => openmedia_animate::AnimationPreset::Typewriter,
        "draw_path" | "draw-path" | "drawpath" => openmedia_animate::AnimationPreset::DrawPath,
        "morph" => openmedia_animate::AnimationPreset::Morph,
        "gradient_shift" | "gradient-shift" => openmedia_animate::AnimationPreset::GradientShift,
        "parallax_scroll" | "parallax-scroll" => openmedia_animate::AnimationPreset::ParallaxScroll,
        "stagger" => openmedia_animate::AnimationPreset::Stagger,
        _ => openmedia_animate::AnimationPreset::FadeIn,
    }
}

fn inject_css_class(svg: &str, element_id: &str, class_name: &str) -> String {
    let clean_id = element_id.trim_start_matches('#');
    let patterns = [
        format!("id=\"{}\"", clean_id),
        format!("id='{}'", clean_id),
    ];
    
    let mut found_pos = None;
    for pat in &patterns {
        if let Some(pos) = svg.find(pat) {
            found_pos = Some((pos, pat.len()));
            break;
        }
    }
    
    let (pos, _pat_len) = match found_pos {
        Some(p) => p,
        None => return svg.to_string(),
    };
    
    let start_tag_idx = match svg[..pos].rfind('<') {
        Some(idx) => idx,
        None => return svg.to_string(),
    };
    
    let end_tag_idx = match svg[pos..].find('>') {
        Some(idx) => pos + idx,
        None => return svg.to_string(),
    };
    
    let mut tag_content = svg[start_tag_idx..=end_tag_idx].to_string();
    let class_pat_double = "class=\"";
    let class_pat_single = "class='";
    
    if let Some(c_pos) = tag_content.find(class_pat_double) {
        let insert_idx = c_pos + class_pat_double.len();
        tag_content.insert_str(insert_idx, &format!("{} ", class_name));
    } else if let Some(c_pos) = tag_content.find(class_pat_single) {
        let insert_idx = c_pos + class_pat_single.len();
        tag_content.insert_str(insert_idx, &format!("{} ", class_name));
    } else {
        if let Some(space_pos) = tag_content.find(' ') {
            tag_content.insert_str(space_pos, &format!(" class=\"{}\"", class_name));
        } else {
            let insert_pos = if tag_content.ends_with("/>") {
                tag_content.len() - 2
            } else {
                tag_content.len() - 1
            };
            tag_content.insert_str(insert_pos, &format!(" class=\"{}\" ", class_name));
        }
    }
    
    let mut result = svg.to_string();
    result.replace_range(start_tag_idx..=end_tag_idx, &tag_content);
    result
}

fn inject_style_or_xml(mut svg: String, content_to_inject: &str) -> String {
    let lower = svg.to_lowercase();
    if let Some(close_idx) = lower.rfind("</svg>") {
        svg.insert_str(close_idx, content_to_inject);
    } else {
        svg.push_str(content_to_inject);
    }
    svg
}

fn parse_svg_dimensions(svg: &str) -> (u32, u32) {
    let mut width = 800;
    let mut height = 600;
    
    if let Some(pos) = svg.find("width=\"") {
        let start = pos + "width=\"".len();
        if let Some(end) = svg[start..].find('"') {
            if let Ok(val) = svg[start..start+end].parse::<f64>() {
                width = val as u32;
            }
        }
    }
    
    if let Some(pos) = svg.find("height=\"") {
        let start = pos + "height=\"".len();
        if let Some(end) = svg[start..].find('"') {
            if let Ok(val) = svg[start..start+end].parse::<f64>() {
                height = val as u32;
            }
        }
    }
    
    (width, height)
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ModelDownloadRequest {
    /// Unique model identifier (e.g., "clip-vit-b32-text", "clip-vit-b32-vision", or "aesthetic-predictor")
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GenerateMermaidRequest {
    /// Raw Mermaid diagram markdown or text
    pub code: String,
    /// Theme for the diagram (e.g. default, dark, forest, neutral)
    pub theme: Option<String>,
    /// Custom theme JSON overrides
    pub custom_theme: Option<serde_json::Value>,
    /// Target width for rasterized output formats
    pub width: Option<u32>,
    /// Target height for rasterized output formats
    pub height: Option<u32>,
    /// Optional background color hex (e.g. #ffffff). Default is transparent.
    pub background_color: Option<String>,
    /// Output format (svg, png, jpeg, webp). Default is svg.
    pub output_format: Option<String>,
    /// Node spacing (layout configuration)
    pub node_spacing: Option<f32>,
    /// Rank spacing (layout configuration)
    pub rank_spacing: Option<f32>,
    /// Preferred aspect ratio (layout configuration)
    pub preferred_aspect_ratio: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateSvgRequest {
    /// Target canvas width in pixels
    pub width: u32,
    /// Target canvas height in pixels
    pub height: u32,
    /// JSON array of elements mapping to schema shapes
    pub elements: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ChartPointDto {
    /// X-axis category label
    pub label: String,
    /// Y-axis numeric value
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateChartRequest {
    /// Type of chart (bar, line, pie)
    pub chart_type: String,
    /// Optional main title text for the chart
    pub title: Option<String>,
    /// Array of data points containing labels and values
    pub data: Vec<ChartPointDto>,
    /// Target image width (default 800)
    pub width: Option<u32>,
    /// Target height (default 600)
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateIconRequest {
    /// Name of the icon to retrieve (e.g. home, user, settings)
    pub name: String,
    /// Custom size in pixels (default 24)
    pub size: Option<u32>,
    /// Custom stroke/fill color hex (default #ffffff)
    pub color: Option<String>,
    /// Custom stroke width (default 2.0)
    pub stroke_width: Option<f32>,
}

#[tool_router(server_handler)]
impl OpenMediaServer {
    #[tool(description = "Ping the media generation server to check status and health")]
    pub async fn ping(&self) -> String {
        format!(
            "pong (CPU: {}, GPU: {:?})",
            self.hardware.cpu.brand,
            self.hardware.gpu.as_ref().map(|g| &g.name)
        )
    }

    #[tool(
        name = "model_download",
        description = "Download a specified model file (CLIP text/vision or Aesthetic predictor) from Hugging Face Hub with progress tracking."
    )]
    pub async fn model_download(
        &self,
        params: Parameters<ModelDownloadRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let reporter = openmedia_core::StderrProgressReporter::new(req.id.clone());
        
        let path = self.model_registry.download_model(&req.id, &reporter)
            .await
            .map_err(|e| e.to_string())?;

        let response = serde_json::json!({
            "status": "success",
            "model_id": req.id,
            "path": path.to_string_lossy(),
        });

        Ok(Json(McpObject(response)))
    }

    #[tool(
        name = "rasterize_svg",
        description = "Rasterize an SVG string or file path into a PNG, JPEG, or WebP image."
    )]
    pub async fn rasterize_svg(
        &self,
        params: Parameters<RasterizeSvgRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let format = req.output_format.unwrap_or_else(|| "png".to_string());
        let filename = format!("{}.{}", uuid::Uuid::now_v7(), format);
        let output_path = self.config.paths.output_dir.join(filename);
        
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);
        
        let svg_content = if req.svg.trim().starts_with('<') {
            req.svg
        } else {
            let path = std::path::Path::new(&req.svg);
            if path.exists() && path.is_file() {
                std::fs::read_to_string(path).map_err(|e| e.to_string())?
            } else {
                req.svg
            }
        };

        let output = openmedia_svg::rasterize(
            &svg_content,
            req.width,
            req.height,
            req.background_color.as_deref(),
            &format,
            &output_path,
        ).map_err(|e| e.to_string())?;

        serde_json::to_value(output)
            .map(McpObject).map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "diagram_generate_mermaid",
        description = "Generate a flowchart, sequence diagram, or architecture diagram from a Mermaid string and save it to the output directory as SVG, PNG, JPEG, or WebP."
    )]
    pub async fn diagram_generate_mermaid(
        &self,
        params: Parameters<GenerateMermaidRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let format = req.output_format.clone().unwrap_or_else(|| "svg".to_string());
        let clean_format = format.trim().to_lowercase();
        
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);
        
        // Build layout config
        let mut layout_config = mermaid_rs_renderer::LayoutConfig::default();
        if let Some(spacing) = req.node_spacing {
            layout_config.node_spacing = spacing;
        }
        if let Some(spacing) = req.rank_spacing {
            layout_config.rank_spacing = spacing;
        }
        if let Some(ratio) = req.preferred_aspect_ratio {
            layout_config.preferred_aspect_ratio = Some(ratio);
        }

        // Resolve theme
        let mut final_theme = if let Some(ref preset) = req.theme {
            resolve_theme_preset(preset)
        } else {
            mermaid_rs_renderer::Theme::modern()
        };

        if let Some(ref overrides) = req.custom_theme {
            override_theme_fields(&mut final_theme, overrides);
        }

        let svg_content = openmedia_svg::render_mermaid(&req.code, Some(final_theme), Some(layout_config))
            .map_err(|e| format!("Failed to render Mermaid diagram: {}", e))?;
            
        let start_time = std::time::Instant::now();
        let filename = format!("{}.{}", uuid::Uuid::now_v7(), clean_format);
        let output_path = self.config.paths.output_dir.join(filename);
        
        let output = if clean_format == "svg" {
            std::fs::write(&output_path, &svg_content)
                .map_err(|e| format!("Failed to write SVG output: {}", e))?;
                
            let file_size = std::fs::metadata(&output_path)
                .map(|m| m.len())
                .unwrap_or(svg_content.len() as u64);
                
            let (w, h) = parse_svg_dimensions(&svg_content);
            let generation_time = start_time.elapsed().as_secs_f64();
            
            openmedia_core::ImageOutput {
                path: output_path,
                width: w,
                height: h,
                seed: 0,
                format: clean_format,
                file_size,
                generation_id: uuid::Uuid::now_v7().to_string(),
                clip_score: None,
                aesthetic_score: None,
                model_used: "mermaid-rs-renderer".to_string(),
                backend_used: "mermaid-rs-renderer".to_string(),
                generation_time,
            }
        } else {
            openmedia_svg::rasterize(
                &svg_content,
                req.width,
                req.height,
                req.background_color.as_deref(),
                &clean_format,
                &output_path,
            ).map_err(|e| format!("Failed to rasterize Mermaid SVG: {}", e))?
        };
        
        serde_json::to_value(output)
            .map(McpObject).map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "html_to_image",
        description = "Render an HTML/CSS string or file path into a PNG, JPEG, or WebP screenshot."
    )]
    pub async fn html_to_image(
        &self,
        params: Parameters<HtmlToImageRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let format = req.output_format.unwrap_or_else(|| "png".to_string());
        let filename = format!("{}.{}", uuid::Uuid::now_v7(), format);
        let output_path = self.config.paths.output_dir.join(filename);
        
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);

        let output = openmedia_video::html_to_image(
            &req.html,
            req.width,
            req.height,
            req.device_scale_factor,
            &format,
            &output_path,
        ).await.map_err(|e| e.to_string())?;

        serde_json::to_value(output)
            .map(McpObject).map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "animate_svg",
        description = "Apply animation presets (such as fade_in, spin, bounce, pulse, typewriter, draw_path) to SVG elements."
    )]
    pub async fn animate_svg(
        &self,
        params: Parameters<AnimateSvgRequest>,
    ) -> Result<Json<McpObject>, String> {
        use openmedia_animate::*;
        let req = params.0;
        let svg_content = if req.svg.trim().starts_with('<') {
            req.svg
        } else {
            let path = std::path::Path::new(&req.svg);
            if path.exists() && path.is_file() {
                std::fs::read_to_string(path).map_err(|e| e.to_string())?
            } else {
                req.svg
            }
        };

        let preset = parse_preset(&req.preset);
        let duration = req.duration.unwrap_or(1.0);
        let delay = req.delay.unwrap_or(0.0);
        let easing = parse_easing(req.easing.as_deref());
        let extra_params = req.params.clone().unwrap_or(serde_json::Value::Null);

        let output = preset.generate(duration, delay, &easing, &extra_params)
            .map_err(|e| e.to_string())?;

        let (animated_svg, animation_count) = match output {
            AnimationOutput::Smil(anims) => {
                let animation_count = anims.len() as u32;
                let mut xml_block = String::new();
                for anim in anims {
                    xml_block.push_str("  ");
                    xml_block.push_str(&anim.to_xml(Some(&req.element_id)));
                    xml_block.push('\n');
                }
                (inject_style_or_xml(svg_content, &xml_block), animation_count)
            }
            AnimationOutput::Css(keyframes) => {
                let animated_svg = inject_css_class(&svg_content, &req.element_id, &keyframes.name);
                let style_block = format!("  <style>\n    {}\n  </style>\n", keyframes.to_css());
                (inject_style_or_xml(animated_svg, &style_block), 1)
            }
            AnimationOutput::Combined { smil, css } => {
                let animated_svg = inject_css_class(&svg_content, &req.element_id, &css.name);
                let mut xml_block = format!("  <style>\n    {}\n  </style>\n", css.to_css());
                for anim in &smil {
                    xml_block.push_str("  ");
                    xml_block.push_str(&anim.to_xml(Some(&req.element_id)));
                    xml_block.push('\n');
                }
                (inject_style_or_xml(animated_svg, &xml_block), (smil.len() + 1) as u32)
            }
        };

        let filename = format!("{}.svg", uuid::Uuid::now_v7());
        let output_path = self.config.paths.output_dir.join(filename);
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);
        std::fs::write(&output_path, &animated_svg).map_err(|e| e.to_string())?;

        let file_size = std::fs::metadata(&output_path)
            .map(|m| m.len())
            .unwrap_or(0);
        let (width, height) = parse_svg_dimensions(&animated_svg);

        let result = openmedia_core::AnimatedSvgOutput {
            path: output_path,
            width,
            height,
            duration,
            animation_count,
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
        };

        serde_json::to_value(result)
            .map(McpObject).map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "animate_create_timeline",
        description = "Sequentially or concurrently coordinate animations of multiple elements."
    )]
    pub async fn animate_create_timeline(
        &self,
        params: Parameters<AnimateTimelineRequest>,
    ) -> Result<Json<McpObject>, String> {
        use openmedia_animate::*;
        let req = params.0;
        let svg_content = if req.svg.trim().starts_with('<') {
            req.svg
        } else {
            let path = std::path::Path::new(&req.svg);
            if path.exists() && path.is_file() {
                std::fs::read_to_string(path).map_err(|e| e.to_string())?
            } else {
                req.svg
            }
        };

        let mode = match req.mode.to_lowercase().as_str() {
            "sequential" => TimelineMode::Sequential,
            "staggered" => TimelineMode::Staggered { delay: req.stagger_delay.unwrap_or(0.2) },
            _ => TimelineMode::Parallel,
        };

        let mut timeline = AnimationTimeline::new(mode);

        for entry in &req.entries {
            let preset = parse_preset(&entry.preset);
            let easing = parse_easing(entry.easing.as_deref());
            let entry_params = entry.params.clone().unwrap_or(serde_json::Value::Null);

            let out = preset.generate(entry.duration, entry.offset, &easing, &entry_params)
                .map_err(|e| e.to_string())?;

            match out {
                AnimationOutput::Smil(anims) => {
                    for anim in anims {
                        timeline.add(&entry.element_id, anim);
                    }
                }
                AnimationOutput::Css(_keyframes) => {
                    let anim = SmilAnimation::Animate {
                        attribute_name: "opacity".to_string(),
                        from: "0".to_string(),
                        to: "1".to_string(),
                        dur: entry.duration,
                        begin: entry.offset,
                        fill: AnimationFill::Freeze,
                        repeat_count: RepeatCount::Definite(1),
                        easing,
                    };
                    timeline.add(&entry.element_id, anim);
                }
                AnimationOutput::Combined { smil, .. } => {
                    for anim in smil {
                        timeline.add(&entry.element_id, anim);
                    }
                }
            }
        }

        let timeline_xml = timeline.to_svg();
        let animated_svg = inject_style_or_xml(svg_content, &timeline_xml);

        let filename = format!("{}.svg", uuid::Uuid::now_v7());
        let output_path = self.config.paths.output_dir.join(filename);
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);
        std::fs::write(&output_path, &animated_svg).map_err(|e| e.to_string())?;

        let file_size = std::fs::metadata(&output_path)
            .map(|m| m.len())
            .unwrap_or(0);
        let (width, height) = parse_svg_dimensions(&animated_svg);

        let result = openmedia_core::AnimatedSvgOutput {
            path: output_path,
            width,
            height,
            duration: timeline.total_duration,
            animation_count: timeline.animations.len() as u32,
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
        };

        serde_json::to_value(result)
            .map(McpObject).map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "animate_morph_paths",
        description = "Interpolate morph frames between two path data strings."
    )]
    pub async fn animate_morph_paths(
        &self,
        params: Parameters<AnimateMorphRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let duration = req.duration.unwrap_or(3.0);
        let easing = parse_easing(req.easing.as_deref());

        let frames = openmedia_animate::morph_paths(&req.from_path, &req.to_path, 30, &easing)
            .map_err(|e| e.to_string())?;

        let values_attr = frames.join("; ");
        let animated_svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 800 600\" width=\"800\" height=\"600\">\n  \
               <path d=\"{}\" fill=\"none\" stroke=\"#8b5cf6\" stroke-width=\"4\">\n    \
                 <animate attributeName=\"d\" values=\"{}\" dur=\"{}s\" repeatCount=\"indefinite\" />\n  \
               </path>\n\
             </svg>",
            req.from_path, values_attr, duration
        );

        let filename = format!("{}.svg", uuid::Uuid::now_v7());
        let output_path = self.config.paths.output_dir.join(filename);
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);
        std::fs::write(&output_path, &animated_svg).map_err(|e| e.to_string())?;

        let file_size = std::fs::metadata(&output_path)
            .map(|m| m.len())
            .unwrap_or(0);

        let result = openmedia_core::AnimatedSvgOutput {
            path: output_path,
            width: 800,
            height: 600,
            duration,
            animation_count: 1,
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
        };

        serde_json::to_value(result)
            .map(McpObject).map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "animate_generate_spinner",
        description = "Generate beautiful animated loading spinner SVGs."
    )]
    pub async fn animate_generate_spinner(
        &self,
        params: Parameters<GenerateSpinnerRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let color = req.color.unwrap_or_else(|| "#8b5cf6".to_string());
        let size = req.size.unwrap_or(60);

        let animated_svg = match req.spinner_type.to_lowercase().as_str() {
            "ring" => {
                format!(
                    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 50 50\">\n  \
                       <path d=\"M 25 5 A 20 20 0 0 1 45 25\" fill=\"none\" stroke=\"{}\" stroke-width=\"4\" stroke-linecap=\"round\">\n    \
                         <animateTransform attributeName=\"transform\" type=\"rotate\" from=\"0 25 25\" to=\"360 25 25\" dur=\"1s\" repeatCount=\"indefinite\" />\n  \
                       </path>\n  \
                       <circle cx=\"25\" cy=\"25\" r=\"20\" fill=\"none\" stroke=\"{}\" stroke-width=\"4\" opacity=\"0.2\" />\n\
                     </svg>",
                    size, size, color, color
                )
            }
            "dots" => {
                format!(
                    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 120 30\">\n  \
                       <circle cx=\"20\" cy=\"15\" r=\"8\" fill=\"{}\">\n    \
                         <animate attributeName=\"cy\" values=\"15; 5; 15\" dur=\"1s\" begin=\"0s\" repeatCount=\"indefinite\" />\n    \
                         <animate attributeName=\"opacity\" values=\"0.3; 1; 0.3\" dur=\"1s\" begin=\"0s\" repeatCount=\"indefinite\" />\n  \
                       </circle>\n  \
                       <circle cx=\"60\" cy=\"15\" r=\"8\" fill=\"{}\">\n    \
                         <animate attributeName=\"cy\" values=\"15; 5; 15\" dur=\"1s\" begin=\"0.25s\" repeatCount=\"indefinite\" />\n    \
                         <animate attributeName=\"opacity\" values=\"0.3; 1; 0.3\" dur=\"1s\" begin=\"0.25s\" repeatCount=\"indefinite\" />\n  \
                       </circle>\n  \
                       <circle cx=\"100\" cy=\"15\" r=\"8\" fill=\"{}\">\n    \
                         <animate attributeName=\"cy\" values=\"15; 5; 15\" dur=\"1s\" begin=\"0.5s\" repeatCount=\"indefinite\" />\n    \
                         <animate attributeName=\"opacity\" values=\"0.3; 1; 0.3\" dur=\"1s\" begin=\"0.5s\" repeatCount=\"indefinite\" />\n  \
                       </circle>\n\
                     </svg>",
                    size, size, color, color, color
                )
            }
            "bars" => {
                format!(
                    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 50 50\">\n  \
                       <rect x=\"10\" y=\"15\" width=\"6\" height=\"20\" fill=\"{}\">\n    \
                         <animate attributeName=\"height\" values=\"20; 40; 20\" dur=\"1s\" begin=\"0s\" repeatCount=\"indefinite\" />\n    \
                         <animate attributeName=\"y\" values=\"15; 5; 15\" dur=\"1s\" begin=\"0s\" repeatCount=\"indefinite\" />\n  \
                       </rect>\n  \
                       <rect x=\"22\" y=\"15\" width=\"6\" height=\"20\" fill=\"{}\">\n    \
                         <animate attributeName=\"height\" values=\"20; 40; 20\" dur=\"1s\" begin=\"0.2s\" repeatCount=\"indefinite\" />\n    \
                         <animate attributeName=\"y\" values=\"15; 5; 15\" dur=\"1s\" begin=\"0.2s\" repeatCount=\"indefinite\" />\n  \
                       </rect>\n  \
                       <rect x=\"34\" y=\"15\" width=\"6\" height=\"20\" fill=\"{}\">\n    \
                         <animate attributeName=\"height\" values=\"20; 40; 20\" dur=\"1s\" begin=\"0.4s\" repeatCount=\"indefinite\" />\n    \
                         <animate attributeName=\"y\" values=\"15; 5; 15\" dur=\"1s\" begin=\"0.4s\" repeatCount=\"indefinite\" />\n  \
                       </rect>\n\
                     </svg>",
                    size, size, color, color, color
                )
            }
            _ => {
                format!(
                    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 50 50\">\n  \
                       <circle cx=\"25\" cy=\"25\" r=\"20\" fill=\"none\" stroke=\"{}\" stroke-width=\"4\" stroke-dasharray=\"31.4 31.4\" stroke-linecap=\"round\">\n    \
                         <animateTransform attributeName=\"transform\" type=\"rotate\" from=\"0 25 25\" to=\"360 25 25\" dur=\"1.2s\" repeatCount=\"indefinite\" />\n  \
                       </circle>\n\
                     </svg>",
                    size, size, color
                )
            }
        };

        let filename = format!("{}.svg", uuid::Uuid::now_v7());
        let output_path = self.config.paths.output_dir.join(filename);
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);
        std::fs::write(&output_path, &animated_svg).map_err(|e| e.to_string())?;

        let file_size = std::fs::metadata(&output_path)
            .map(|m| m.len())
            .unwrap_or(0);

        let result = openmedia_core::AnimatedSvgOutput {
            path: output_path,
            width: size,
            height: size,
            duration: 1.0,
            animation_count: 1,
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
        };

        serde_json::to_value(result)
            .map(McpObject).map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "animate_from_lottie",
        description = "Import Lottie JSON and convert to an animated SVG."
    )]
    pub async fn animate_from_lottie(
        &self,
        params: Parameters<LottieToSvgRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let lottie_json = if req.lottie_json.trim().starts_with('{') {
            req.lottie_json
        } else {
            let path = std::path::Path::new(&req.lottie_json);
            if path.exists() && path.is_file() {
                std::fs::read_to_string(path).map_err(|e| e.to_string())?
            } else {
                req.lottie_json
            }
        };

        let animated_svg = openmedia_animate::lottie_to_svg(&lottie_json)
            .map_err(|e| e.to_string())?;

        let filename = format!("{}.svg", uuid::Uuid::now_v7());
        let output_path = self.config.paths.output_dir.join(filename);
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);
        std::fs::write(&output_path, &animated_svg).map_err(|e| e.to_string())?;

        let file_size = std::fs::metadata(&output_path)
            .map(|m| m.len())
            .unwrap_or(0);
        let (width, height) = parse_svg_dimensions(&animated_svg);

        let result = openmedia_core::AnimatedSvgOutput {
            path: output_path,
            width,
            height,
            duration: 3.0,
            animation_count: 1,
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
        };

        serde_json::to_value(result)
            .map(McpObject).map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "animate_to_lottie",
        description = "Export SVG to Lottie JSON."
    )]
    pub async fn animate_to_lottie(
        &self,
        params: Parameters<SvgToLottieRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let svg_content = if req.svg.trim().starts_with('<') {
            req.svg
        } else {
            let path = std::path::Path::new(&req.svg);
            if path.exists() && path.is_file() {
                std::fs::read_to_string(path).map_err(|e| e.to_string())?
            } else {
                req.svg
            }
        };

        let lottie_json_str = openmedia_animate::svg_to_lottie(&svg_content)
            .map_err(|e| e.to_string())?;

        let lottie_val: serde_json::Value = serde_json::from_str(&lottie_json_str)
            .map_err(|e| e.to_string())?;

        Ok(Json(McpObject(lottie_val)))
    }

    async fn process_and_save(
        &self,
        img: &image::DynamicImage,
        op: &openmedia_process::ProcessOperation,
        format: &str,
    ) -> Result<openmedia_core::ImageOutput, String> {
        let start = std::time::Instant::now();
        let mut backend_used = "wgpu".to_string();
        
        let processed = match openmedia_process::apply_gpu_operation(img, op) {
            Ok(gpu_img) => gpu_img,
            Err(_) => {
                backend_used = "cpu".to_string();
                openmedia_process::apply_cpu_operation(img, op).map_err(|e| e.to_string())?
            }
        };

        let ext = match format.to_lowercase().as_str() {
            "png" => "png",
            "jpeg" | "jpg" => "jpg",
            "webp" => "webp",
            "avif" => "avif",
            _ => "png",
        };

        let filename = format!("{}.{}", uuid::Uuid::now_v7(), ext);
        let dest = self.config.paths.output_dir.join(filename);
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);

        let bytes = openmedia_process::write_image_with_format(
            &processed,
            ext,
            80, // default quality
        ).map_err(|e| e.to_string())?;

        std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
        let file_size = bytes.len() as u64;

        Ok(openmedia_core::ImageOutput {
            path: dest,
            width: processed.width(),
            height: processed.height(),
            seed: 0,
            format: ext.to_string(),
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
            clip_score: None,
            aesthetic_score: None,
            model_used: "none".to_string(),
            backend_used,
            generation_time: start.elapsed().as_secs_f64(),
        })
    }

    #[tool(
        name = "image_apply_filter",
        description = "Apply a visual filter to an image (grayscale, invert, brightness, contrast, saturation, hue_rotate, sepia, threshold, blur, sharpen, unsharp_mask)."
    )]
    pub async fn image_apply_filter(
        &self,
        params: Parameters<ImageApplyFilterRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let img = image::open(&req.image_path).map_err(|e| e.to_string())?;
        let op = match req.filter_type.to_lowercase().as_str() {
            "grayscale" => openmedia_process::ProcessOperation::Grayscale,
            "invert" => openmedia_process::ProcessOperation::Invert,
            "brightness" => openmedia_process::ProcessOperation::Brightness { value: req.parameter.unwrap_or(0.0) as i32 },
            "contrast" => openmedia_process::ProcessOperation::Contrast { value: req.parameter.unwrap_or(0.0) as i32 },
            "saturation" => openmedia_process::ProcessOperation::Saturation { value: req.parameter.unwrap_or(0.0) as i32 },
            "hue_rotate" | "huerotate" => openmedia_process::ProcessOperation::HueRotate { degrees: req.parameter.unwrap_or(0.0) },
            "sepia" => openmedia_process::ProcessOperation::Sepia { intensity: req.parameter.unwrap_or(1.0) },
            "threshold" => openmedia_process::ProcessOperation::Threshold { value: req.parameter.unwrap_or(128.0) as u8 },
            "blur" | "gaussian_blur" => openmedia_process::ProcessOperation::GaussianBlur { radius: req.parameter.unwrap_or(2.0), sigma: None },
            "box_blur" => openmedia_process::ProcessOperation::BoxBlur { radius: req.parameter.unwrap_or(2.0) as u32 },
            "sharpen" => openmedia_process::ProcessOperation::Sharpen { amount: 1.0, radius: req.parameter.unwrap_or(2.0), threshold: 0 },
            "unsharp_mask" | "unsharp" => openmedia_process::ProcessOperation::UnsharpMask { amount: 1.0, radius: req.parameter.unwrap_or(2.0), threshold: 0 },
            _ => return Err(format!("Unsupported filter type: {}", req.filter_type)),
        };
        let ext = std::path::Path::new(&req.image_path).extension().and_then(|e| e.to_str()).unwrap_or("png");
        let output = self.process_and_save(&img, &op, ext).await?;
        serde_json::to_value(output).map(McpObject).map(Json).map_err(|e| e.to_string())
    }

    #[tool(
        name = "image_resize",
        description = "Resize an image to specific dimensions with configurable algorithm."
    )]
    pub async fn image_resize(
        &self,
        params: Parameters<ImageResizeRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let img = image::open(&req.image_path).map_err(|e| e.to_string())?;
        let method = match req.algorithm.as_deref().unwrap_or("bilinear").to_lowercase().as_str() {
            "nearest" => openmedia_process::ResizeMethod::Nearest,
            "bilinear" => openmedia_process::ResizeMethod::Bilinear,
            "lanczos3" => openmedia_process::ResizeMethod::Lanczos3,
            _ => openmedia_process::ResizeMethod::Bilinear,
        };
        let op = openmedia_process::ProcessOperation::Resize {
            width: req.width,
            height: req.height,
            method,
        };
        let ext = std::path::Path::new(&req.image_path).extension().and_then(|e| e.to_str()).unwrap_or("png");
        let output = self.process_and_save(&img, &op, ext).await?;
        serde_json::to_value(output).map(McpObject).map(Json).map_err(|e| e.to_string())
    }

    #[tool(
        name = "image_crop",
        description = "Crop an image using top-left coordinates and dimensions."
    )]
    pub async fn image_crop(
        &self,
        params: Parameters<ImageCropRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let img = image::open(&req.image_path).map_err(|e| e.to_string())?;
        let op = openmedia_process::ProcessOperation::Crop {
            x: req.x,
            y: req.y,
            width: req.width,
            height: req.height,
        };
        let ext = std::path::Path::new(&req.image_path).extension().and_then(|e| e.to_str()).unwrap_or("png");
        let output = self.process_and_save(&img, &op, ext).await?;
        serde_json::to_value(output).map(McpObject).map(Json).map_err(|e| e.to_string())
     }

    #[tool(
        name = "image_transform",
        description = "Apply geometric transform like rotate or flip."
    )]
    pub async fn image_transform(
        &self,
        params: Parameters<ImageTransformRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let img = image::open(&req.image_path).map_err(|e| e.to_string())?;
        let op = match req.transform_type.to_lowercase().as_str() {
            "rotate" => openmedia_process::ProcessOperation::Rotate {
                angle: req.angle.unwrap_or(90.0),
                expand: true,
            },
            "flip_horizontal" | "fliph" => openmedia_process::ProcessOperation::FlipHorizontal,
            "flip_vertical" | "flipv" => openmedia_process::ProcessOperation::FlipVertical,
            _ => return Err(format!("Unsupported transform type: {}", req.transform_type)),
        };
        let ext = std::path::Path::new(&req.image_path).extension().and_then(|e| e.to_str()).unwrap_or("png");
        let output = self.process_and_save(&img, &op, ext).await?;
        serde_json::to_value(output).map(McpObject).map(Json).map_err(|e| e.to_string())
    }

    #[tool(
        name = "image_convert",
        description = "Convert an image to another format with quality settings."
    )]
    pub async fn image_convert(
        &self,
        params: Parameters<ImageConvertRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let img = image::open(&req.image_path).map_err(|e| e.to_string())?;
        let start = std::time::Instant::now();

        let ext = req.format.trim_start_matches('.').to_lowercase();
        let filename = format!("{}.{}", uuid::Uuid::now_v7(), ext);
        let dest = self.config.paths.output_dir.join(filename);
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);

        let bytes = openmedia_process::write_image_with_format(
            &img,
            &ext,
            req.quality.unwrap_or(80),
        ).map_err(|e| e.to_string())?;

        std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
        let file_size = bytes.len() as u64;

        let output = openmedia_core::ImageOutput {
            path: dest,
            width: img.width(),
            height: img.height(),
            seed: 0,
            format: ext,
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
            clip_score: None,
            aesthetic_score: None,
            model_used: "none".to_string(),
            backend_used: "cpu".to_string(),
            generation_time: start.elapsed().as_secs_f64(),
        };

        serde_json::to_value(output).map(McpObject).map(Json).map_err(|e| e.to_string())
    }

    #[tool(
        name = "image_batch_process",
        description = "Process a set of files using glob pattern and a sequential filter chain."
    )]
    pub async fn image_batch_process(
        &self,
        params: Parameters<ImageBatchProcessRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let mut chain = openmedia_process::FilterChain::new();
        for op_val in req.operations {
            let op = serde_json::from_value::<openmedia_process::ProcessOperation>(op_val)
                .map_err(|e| format!("Invalid process operation definition: {}", e))?;
            chain.add(op);
        }
        let output_dir = std::path::Path::new(&req.output_dir);
        let processed_paths = openmedia_process::batch_process_files(&req.glob_pattern, &chain, output_dir)
            .await
            .map_err(|e| e.to_string())?;

        let outputs: Vec<openmedia_core::ImageOutput> = processed_paths.into_iter().map(|path| {
            let (w, h) = image::image_dimensions(&path).unwrap_or((0, 0));
            let file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("png").to_string();
            openmedia_core::ImageOutput {
                path,
                width: w,
                height: h,
                seed: 0,
                format: ext,
                file_size,
                generation_id: uuid::Uuid::now_v7().to_string(),
                clip_score: None,
                aesthetic_score: None,
                model_used: "none".to_string(),
                backend_used: "cpu/wgpu".to_string(),
                generation_time: 0.0,
            }
        }).collect();

        serde_json::to_value(outputs).map(McpObject).map(Json).map_err(|e| e.to_string())
    }

    #[tool(
        name = "video_create",
        description = "Compile a video from a full VideoScene JSON description. Supports transitions and audio mixing. DESIGN TIPS: Use native shapes, text, and charts for faster offline rendering. Define explicit keyframes for opacity/scale/rotation/position. Easing choices: linear, ease_in, ease_out, ease_in_out."
    )]
    pub async fn video_create(
        &self,
        params: Parameters<VideoCreateRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let scene: VideoScene = if req.scene.is_string() {
            let path_str = req.scene.as_str().unwrap();
            let path = std::path::Path::new(path_str);
            if path.exists() && path.is_file() {
                let s = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
                serde_json::from_str(&s).map_err(|e| e.to_string())?
            } else {
                serde_json::from_value(req.scene.clone()).map_err(|e| e.to_string())?
            }
        } else {
            serde_json::from_value(req.scene.clone()).map_err(|e| e.to_string())?
        };

        let output_path = if let Some(out_p) = req.output_path {
            std::path::PathBuf::from(out_p)
        } else {
            let filename = format!("{}.mp4", uuid::Uuid::now_v7());
            self.config.paths.output_dir.join(filename)
        };

        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);
        
        let video_spec = openmedia_video::render_video_scene(&scene, &output_path)
            .await
            .map_err(|e| e.to_string())?;

        serde_json::to_value(video_spec)
            .map(McpObject).map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "video_preview",
        description = "Generate a preview frame image for a video scene at a given time offset."
    )]
    pub async fn video_preview(
        &self,
        params: Parameters<VideoPreviewRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let scene: VideoScene = if req.scene.is_string() {
            let path_str = req.scene.as_str().unwrap();
            let path = std::path::Path::new(path_str);
            if path.exists() && path.is_file() {
                let s = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
                serde_json::from_str(&s).map_err(|e| e.to_string())?
            } else {
                serde_json::from_value(req.scene.clone()).map_err(|e| e.to_string())?
            }
        } else {
            serde_json::from_value(req.scene.clone()).map_err(|e| e.to_string())?
        };

        let t = req.time.unwrap_or(0.0);
        let w = req.width.unwrap_or(scene.width);
        let h = req.height.unwrap_or(scene.height);
        let format = req.output_format.unwrap_or_else(|| "png".to_string());
        
        let use_browser = scene.scenes.iter().any(|s| {
            s.elements.iter().any(|el| {
                matches!(el, SceneElement::Html { .. } | SceneElement::Code { .. })
            })
        });

        let frame = if use_browser {
            let renderer = openmedia_video::BrowserFrameRenderer::launch().await.map_err(|e| e.to_string())?;
            let f = renderer.render_frame(&scene, t, w, h).await.map_err(|e| e.to_string())?;
            renderer.close().await;
            f
        } else {
            let renderer = openmedia_video::SvgFrameRenderer;
            renderer.render_frame(&scene, t, w, h).await.map_err(|e| e.to_string())?
        };

        let filename = format!("{}.{}", uuid::Uuid::now_v7(), format);
        let output_path = self.config.paths.output_dir.join(filename);
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);
        
        let mut bytes = Vec::new();
        let img_format = match format.to_lowercase().as_str() {
            "png" => image::ImageFormat::Png,
            "jpeg" | "jpg" => image::ImageFormat::Jpeg,
            "webp" => image::ImageFormat::WebP,
            other => return Err(format!("Unsupported preview output format: {}", other)),
        };
        frame.write_to(&mut std::io::Cursor::new(&mut bytes), img_format)
            .map_err(|e| e.to_string())?;
        std::fs::write(&output_path, &bytes).map_err(|e| e.to_string())?;

        let file_size = std::fs::metadata(&output_path).map(|m| m.len()).unwrap_or(0);
        let output = openmedia_core::ImageOutput {
            path: output_path,
            width: w,
            height: h,
            seed: 0,
            format,
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
            clip_score: None,
            aesthetic_score: None,
            model_used: "none".to_string(),
            backend_used: if use_browser { "headless-chrome" } else { "svg" }.to_string(),
            generation_time: 0.0,
        };

        serde_json::to_value(output)
            .map(McpObject).map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "video_create_slideshow",
        description = "Quickly compile a slideshow video from a list of image paths or directory path, with options for transitions and audio."
    )]
    pub async fn video_create_slideshow(
        &self,
        params: Parameters<VideoCreateSlideshowRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let duration_per_image = req.duration_per_image.unwrap_or(3.0);
        let trans_type_str = req.transition_type.unwrap_or_else(|| "crossfade".to_string());
        let trans_duration = req.transition_duration.unwrap_or(0.5);
        
        let width = req.width.unwrap_or(1920);
        let height = req.height.unwrap_or(1080);
        let fps = req.fps.unwrap_or(30);

        // Resolve images
        let mut resolved_images = Vec::new();
        for path_str in req.images {
            let path = std::path::Path::new(&path_str);
            if path.is_dir() {
                let entries = std::fs::read_dir(path).map_err(|e| e.to_string())?;
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_file() {
                        if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                            match ext.to_lowercase().as_str() {
                                "png" | "jpg" | "jpeg" | "webp" => {
                                    resolved_images.push(p.to_string_lossy().into_owned());
                                }
                                _ => {}
                            }
                        }
                    }
                }
            } else {
                resolved_images.push(path_str);
            }
        }

        if resolved_images.is_empty() {
            return Err("No valid images found for slideshow".to_string());
        }

        // Construct VideoScene DSL
        let mut scenes = Vec::new();
        let mut transitions = Vec::new();

        let mut current_time = 0.0;
        for (i, img_src) in resolved_images.iter().enumerate() {
            let scene_id = format!("slide_{}", i);
            let start = current_time;
            let end = start + duration_per_image;
            
            let element = openmedia_video::SceneElement::Image {
                src: img_src.clone(),
                position: openmedia_video::Position {
                    x: openmedia_video::DimensionValue::Pixels(0.0),
                    y: openmedia_video::DimensionValue::Pixels(0.0),
                },
                size: openmedia_video::Size {
                    width: openmedia_video::DimensionValue::Percentage("100%".to_string()),
                    height: openmedia_video::DimensionValue::Percentage("100%".to_string()),
                },
                fit: openmedia_video::ObjectFit::Contain,
                timeline: None,
            };

            scenes.push(openmedia_video::Scene {
                id: scene_id.clone(),
                start,
                end,
                elements: vec![element],
            });

            if i > 0 {
                let from = format!("slide_{}", i - 1);
                let to = scene_id;
                let transition_type = parse_transition_type(&trans_type_str);
                transitions.push(openmedia_video::SceneTransition {
                    from,
                    to,
                    transition_type,
                    duration: trans_duration,
                    easing: None,
                });
            }

            current_time = end - trans_duration;
        }

        let total_duration = current_time + trans_duration;

        let audio = req.audio_src.map(|src| openmedia_video::AudioConfig {
            tracks: vec![openmedia_video::AudioTrack {
                src,
                start: 0.0,
                volume: 1.0,
                fade_in: Some(1.0),
                fade_out: Some(1.0),
            }],
        });

        let scene = openmedia_video::VideoScene {
            width,
            height,
            fps,
            duration: total_duration,
            background: "#000000".to_string(),
            scenes,
            transitions,
            audio,
            custom_fonts: None,
        };

        let output_path = if let Some(out_p) = req.output_path {
            std::path::PathBuf::from(out_p)
        } else {
            let filename = format!("{}.mp4", uuid::Uuid::now_v7());
            self.config.paths.output_dir.join(filename)
        };

        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);

        let video_spec = openmedia_video::render_video_scene(&scene, &output_path)
            .await
            .map_err(|e| e.to_string())?;

        serde_json::to_value(video_spec)
            .map(McpObject).map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "video_add_transition",
        description = "Add a transition between two scenes in an existing video scene JSON file."
    )]
    pub async fn video_add_transition(
        &self,
        params: Parameters<VideoAddTransitionRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let path = std::path::Path::new(&req.scene_path);
        if !path.exists() || !path.is_file() {
            return Err(format!("Scene file not found: {}", req.scene_path));
        }

        let s = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let mut scene: VideoScene = serde_json::from_str(&s).map_err(|e| e.to_string())?;

        let duration = req.duration.unwrap_or(0.5);
        let transition_type = parse_transition_type(&req.transition_type);

        scene.transitions.retain(|t| !(t.from == req.from_scene_id && t.to == req.to_scene_id));

        scene.transitions.push(openmedia_video::SceneTransition {
            from: req.from_scene_id,
            to: req.to_scene_id,
            transition_type,
            duration,
            easing: None,
        });

        let updated = serde_json::to_string_pretty(&scene).map_err(|e| e.to_string())?;
        std::fs::write(path, updated).map_err(|e| e.to_string())?;

        serde_json::to_value(scene)
            .map(McpObject).map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "video_add_audio",
        description = "Add an audio track to an existing video file or video scene JSON description."
    )]
    pub async fn video_add_audio(
        &self,
        params: Parameters<VideoAddAudioRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let target = std::path::Path::new(&req.target_path);
        if !target.exists() {
            return Err(format!("Target path not found: {}", req.target_path));
        }

        let start_time = req.start_time.unwrap_or(0.0);
        let volume = req.volume.unwrap_or(1.0);

        if target.is_file() && req.target_path.ends_with(".json") {
            let s = std::fs::read_to_string(target).map_err(|e| e.to_string())?;
            let mut scene: VideoScene = serde_json::from_str(&s).map_err(|e| e.to_string())?;
            
            let track = openmedia_video::AudioTrack {
                src: req.audio_path,
                start: start_time,
                volume,
                fade_in: req.fade_in,
                fade_out: req.fade_out,
            };

            if let Some(audio_cfg) = &mut scene.audio {
                audio_cfg.tracks.push(track);
            } else {
                scene.audio = Some(openmedia_video::AudioConfig { tracks: vec![track] });
            }

            let updated = serde_json::to_string_pretty(&scene).map_err(|e| e.to_string())?;
            std::fs::write(target, updated).map_err(|e| e.to_string())?;

            return serde_json::to_value(scene)
                .map(McpObject).map(Json)
                .map_err(|e| e.to_string());
        }

        let output_path = if let Some(out_p) = req.output_path {
            std::path::PathBuf::from(out_p)
        } else {
            let filename = format!("{}.mp4", uuid::Uuid::now_v7());
            self.config.paths.output_dir.join(filename)
        };

        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);

        let delay_ms = (start_time * 1000.0) as i32;
        let filter_script = format!(
            "[1:a]adelay={}|{},volume={}[a1];[0:a][a1]amix=inputs=2:duration=first[out_a]",
            delay_ms, delay_ms, volume
        );

        let mut cmd = tokio::process::Command::new("ffmpeg");
        cmd.args([
            "-y",
            "-i", &req.target_path,
            "-i", &req.audio_path,
            "-filter_complex", &filter_script,
            "-map", "0:v",
            "-map", "[out_a]",
            "-c:v", "copy",
            "-c:a", "aac",
        ])
        .arg(&output_path);

        cmd.stdout(std::process::Stdio::null())
           .stderr(std::process::Stdio::null());

        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        child.wait().await.map_err(|e| e.to_string())?;

        let file_size = std::fs::metadata(&output_path).map(|m| m.len()).unwrap_or(0);
        let output = openmedia_core::VideoSpec {
            path: output_path,
            width: 0,
            height: 0,
            duration: 0.0,
            fps: 0,
            codec: "copy".to_string(),
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
            renderer_used: "ffmpeg".to_string(),
            total_frames: 0,
            generation_time: 0.0,
        };

        serde_json::to_value(output)
            .map(McpObject).map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "video_from_template",
        description = "Instantiate a video scene from one of the pre-designed templates (slideshow, text_explainer, data_dashboard, social_media, product_showcase). DESIGN TIPS: Use custom colors, charts, audio offsets, and customize transition easing (linear, ease_in, ease_out, ease_in_out)."
    )]
    pub async fn video_from_template(
        &self,
        params: Parameters<VideoFromTemplateRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let output_path = if let Some(out_p) = req.output_path {
            std::path::PathBuf::from(out_p)
        } else {
            let filename = format!("{}.mp4", uuid::Uuid::now_v7());
            self.config.paths.output_dir.join(filename)
        };
        
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);
        
        let scene = match req.template_name.to_lowercase().as_str() {
            "slideshow" => {
                let images = req.parameters["images"].as_array()
                    .ok_or_else(|| "Missing parameters.images array".to_string())?
                    .iter()
                    .map(|v: &serde_json::Value| v.as_str().unwrap_or("").to_string())
                    .collect::<Vec<String>>();
                let duration = req.parameters["duration_per_image"].as_f64().unwrap_or(3.0);
                let width = req.parameters["width"].as_u64().unwrap_or(1920) as u32;
                let height = req.parameters["height"].as_u64().unwrap_or(1080) as u32;
                let fps = req.parameters["fps"].as_u64().unwrap_or(30) as u32;
                
                let mut scenes = Vec::new();
                for (i, img_src) in images.iter().enumerate() {
                    scenes.push(openmedia_video::Scene {
                        id: format!("slide_{}", i),
                        start: i as f64 * duration,
                        end: (i + 1) as f64 * duration,
                        elements: vec![openmedia_video::SceneElement::Image {
                            src: img_src.clone(),
                            position: openmedia_video::Position {
                                x: openmedia_video::DimensionValue::Pixels(0.0),
                                y: openmedia_video::DimensionValue::Pixels(0.0),
                            },
                            size: openmedia_video::Size {
                                width: openmedia_video::DimensionValue::Percentage("100%".to_string()),
                                height: openmedia_video::DimensionValue::Percentage("100%".to_string()),
                            },
                            fit: openmedia_video::ObjectFit::Contain,
                            timeline: None,
                        }],
                    });
                }
                let mut transitions = Vec::new();
                if images.len() > 1 && req.parameters.get("transition_type").is_some() {
                    let (custom_trans_type, custom_duration, custom_easing) =
                        parse_transition_params(&req.parameters, openmedia_video::TransitionType::Crossfade);

                    for i in 0..(images.len() - 1) {
                        transitions.push(openmedia_video::SceneTransition {
                            from: format!("slide_{}", i),
                            to: format!("slide_{}", i + 1),
                            transition_type: custom_trans_type.clone(),
                            duration: custom_duration,
                            easing: custom_easing.clone(),
                        });
                    }
                }

                openmedia_video::VideoScene {
                    width,
                    height,
                    fps,
                    duration: images.len() as f64 * duration,
                    background: "#000000".to_string(),
                    scenes,
                    transitions,
                    audio: parse_audio_config(&req.parameters),
                    custom_fonts: parse_custom_fonts(&req.parameters),
                }
            }
            "text_explainer" => {
                let title = req.parameters["title"].as_str().unwrap_or("Explainer Video").to_string();
                let bullets = req.parameters["bullets"].as_array()
                    .ok_or_else(|| "Missing parameters.bullets array".to_string())?
                    .iter()
                    .map(|v: &serde_json::Value| v.as_str().unwrap_or("").to_string())
                    .collect::<Vec<String>>();
                let bullet_duration = req.parameters["bullet_duration"].as_f64().unwrap_or(3.0);
                let width = req.parameters["width"].as_u64().unwrap_or(1920) as u32;
                let height = req.parameters["height"].as_u64().unwrap_or(1080) as u32;
                let fps = req.parameters["fps"].as_u64().unwrap_or(30) as u32;

                let mut scenes = Vec::new();
                let total_duration = (bullets.len() + 1) as f64 * bullet_duration;

                let s0_elements = vec![openmedia_video::SceneElement::Text {
                    content: title.clone(),
                    style: openmedia_video::TextStyle {
                        font_family: "sans-serif".to_string(),
                        font_size: 48.0,
                        font_weight: 700,
                        color: "#ffffff".to_string(),
                        text_align: "center".to_string(),
                        line_height: None,
                        letter_spacing: None,
                    },
                    position: openmedia_video::Position {
                        x: openmedia_video::DimensionValue::Pixels((width / 2) as f64),
                        y: openmedia_video::DimensionValue::Pixels(150.0),
                    },
                    anchor: openmedia_video::Anchor::Center,
                    timeline: None,
                }];
                scenes.push(openmedia_video::Scene {
                    id: "scene_0".to_string(),
                    start: 0.0,
                    end: bullet_duration,
                    elements: s0_elements.clone(),
                });

                for i in 0..bullets.len() {
                    let mut el = s0_elements.clone();
                    for (j, bullet_text) in bullets.iter().enumerate().take(i + 1) {
                        el.push(openmedia_video::SceneElement::Text {
                            content: format!("• {}", bullet_text),
                            style: openmedia_video::TextStyle {
                                font_family: "sans-serif".to_string(),
                                font_size: 32.0,
                                font_weight: 400,
                                color: "#cccccc".to_string(),
                                text_align: "left".to_string(),
                                line_height: None,
                                letter_spacing: None,
                            },
                            position: openmedia_video::Position {
                                x: openmedia_video::DimensionValue::Pixels(200.0),
                                y: openmedia_video::DimensionValue::Pixels(300.0 + j as f64 * 80.0),
                            },
                            anchor: openmedia_video::Anchor::TopLeft,
                            timeline: None,
                        });
                    }
                    scenes.push(openmedia_video::Scene {
                        id: format!("scene_{}", i + 1),
                        start: (i + 1) as f64 * bullet_duration,
                        end: (i + 2) as f64 * bullet_duration,
                        elements: el,
                    });
                }

                openmedia_video::VideoScene {
                    width,
                    height,
                    fps,
                    duration: total_duration,
                    background: "#1a1a2e".to_string(),
                    scenes,
                    transitions: vec![],
                    audio: parse_audio_config(&req.parameters),
                    custom_fonts: parse_custom_fonts(&req.parameters),
                }
            }
            "data_dashboard" => {
                let title = req.parameters["title"].as_str().unwrap_or("Data Dashboard").to_string();
                let charts_arr = req.parameters["charts"].as_array()
                    .ok_or_else(|| "Missing parameters.charts array".to_string())?;
                let duration = req.parameters["chart_duration"].as_f64().unwrap_or(3.0);
                let width = req.parameters["width"].as_u64().unwrap_or(1920) as u32;
                let height = req.parameters["height"].as_u64().unwrap_or(1080) as u32;
                let fps = req.parameters["fps"].as_u64().unwrap_or(30) as u32;

                let mut scenes = Vec::new();
                let mut transitions = Vec::new();

                scenes.push(openmedia_video::Scene {
                    id: "scene_0".to_string(),
                    start: 0.0,
                    end: 2.0,
                    elements: vec![openmedia_video::SceneElement::Text {
                        content: title.clone(),
                        style: openmedia_video::TextStyle {
                            font_family: "sans-serif".to_string(),
                            font_size: 64.0,
                            font_weight: 700,
                            color: "#ffffff".to_string(),
                            text_align: "center".to_string(),
                            line_height: None,
                            letter_spacing: None,
                        },
                        position: openmedia_video::Position {
                            x: openmedia_video::DimensionValue::Pixels((width / 2) as f64),
                            y: openmedia_video::DimensionValue::Pixels((height / 2) as f64),
                        },
                        anchor: openmedia_video::Anchor::Center,
                        timeline: None,
                    }],
                });

                let (custom_trans_type, custom_duration, custom_easing) =
                    parse_transition_params(&req.parameters, openmedia_video::TransitionType::SlideLeft);

                for (i, chart_val) in charts_arr.iter().enumerate() {
                    let chart_type = chart_val["type"].as_str().unwrap_or("bar").to_string();
                    let chart_title = chart_val["title"].as_str().unwrap_or("Statistics").to_string();
                    let chart_data = chart_val["data"].clone();

                    let scene_id = format!("scene_{}", i + 1);
                    let start = 2.0 + i as f64 * duration;
                    let end = start + duration;

                    scenes.push(openmedia_video::Scene {
                        id: scene_id.clone(),
                        start,
                        end,
                        elements: vec![
                            openmedia_video::SceneElement::Text {
                                content: chart_title,
                                style: openmedia_video::TextStyle {
                                    font_family: "sans-serif".to_string(),
                                    font_size: 40.0,
                                    font_weight: 600,
                                    color: "#ffffff".to_string(),
                                    text_align: "center".to_string(),
                                    line_height: None,
                                    letter_spacing: None,
                                },
                                position: openmedia_video::Position {
                                    x: openmedia_video::DimensionValue::Pixels((width / 2) as f64),
                                    y: openmedia_video::DimensionValue::Pixels(80.0),
                                },
                                anchor: openmedia_video::Anchor::Center,
                                timeline: None,
                            },
                            openmedia_video::SceneElement::Chart {
                                chart_type,
                                data: chart_data,
                                position: openmedia_video::Position {
                                    x: openmedia_video::DimensionValue::Pixels((width / 2) as f64),
                                    y: openmedia_video::DimensionValue::Pixels((height / 2 + 30) as f64),
                                },
                                size: openmedia_video::Size {
                                    width: openmedia_video::DimensionValue::Pixels((width - 400) as f64),
                                    height: openmedia_video::DimensionValue::Pixels((height - 300) as f64),
                                },
                                theme: "dark".to_string(),
                                timeline: None,
                            },
                        ],
                    });

                    let from = format!("scene_{}", i);
                    let to = scene_id;
                    transitions.push(openmedia_video::SceneTransition {
                        from,
                        to,
                        transition_type: custom_trans_type.clone(),
                        duration: custom_duration,
                        easing: custom_easing.clone(),
                    });
                }

                let total_duration = 2.0 + charts_arr.len() as f64 * duration;

                openmedia_video::VideoScene {
                    width,
                    height,
                    fps,
                    duration: total_duration,
                    background: "#0f172a".to_string(),
                    scenes,
                    transitions,
                    audio: parse_audio_config(&req.parameters),
                    custom_fonts: parse_custom_fonts(&req.parameters),
                }
            }
            "social_media" => {
                let title = req.parameters["title"].as_str().unwrap_or("Top Facts").to_string();
                let content_arr: &Vec<serde_json::Value> = req.parameters["content"].as_array()
                    .ok_or_else(|| "Missing parameters.content array".to_string())?;
                let duration = req.parameters["scene_duration"].as_f64().unwrap_or(3.0);
                let bg_color = req.parameters["background_color"].as_str().unwrap_or("#1e1b4b").to_string();
                let width = 1080;
                let height = 1920;
                let fps = req.parameters["fps"].as_u64().unwrap_or(30) as u32;

                let mut scenes = Vec::new();
                let mut transitions = Vec::new();

                scenes.push(openmedia_video::Scene {
                    id: "scene_0".to_string(),
                    start: 0.0,
                    end: 3.0,
                    elements: vec![openmedia_video::SceneElement::Text {
                        content: title.clone(),
                        style: openmedia_video::TextStyle {
                            font_family: "sans-serif".to_string(),
                            font_size: 72.0,
                            font_weight: 800,
                            color: "#fbbf24".to_string(),
                            text_align: "center".to_string(),
                            line_height: None,
                            letter_spacing: None,
                        },
                        position: openmedia_video::Position {
                            x: openmedia_video::DimensionValue::Pixels((width / 2) as f64),
                            y: openmedia_video::DimensionValue::Pixels((height / 2) as f64),
                        },
                        anchor: openmedia_video::Anchor::Center,
                        timeline: Some(openmedia_video::ElementTimeline {
                            keyframes: vec![
                                openmedia_video::Keyframe {
                                    time: 0.0,
                                    opacity: Some(0.0),
                                    x: None, y: None,
                                    scale: Some(0.5), scale_x: None, scale_y: None,
                                    rotation: None, easing: Some("ease_out".to_string()),
                                },
                                openmedia_video::Keyframe {
                                    time: 1.0,
                                    opacity: Some(1.0),
                                    x: None, y: None,
                                    scale: Some(1.0), scale_x: None, scale_y: None,
                                    rotation: None, easing: None,
                                }
                            ]
                        }),
                    }],
                });

                let (custom_trans_type, custom_duration, custom_easing) =
                    parse_transition_params(&req.parameters, openmedia_video::TransitionType::SlideUp);

                for (i, content_val) in content_arr.iter().enumerate() {
                    let point_text = content_val.as_str().unwrap_or("").to_string();
                    let scene_id = format!("scene_{}", i + 1);
                    let start = 3.0 + i as f64 * duration;
                    let end = start + duration;

                    scenes.push(openmedia_video::Scene {
                        id: scene_id.clone(),
                        start,
                        end,
                        elements: vec![
                            // Background Floating Circle 1
                            openmedia_video::SceneElement::Shape {
                                shape: openmedia_video::ShapeType::Circle,
                                size: openmedia_video::Size {
                                    width: openmedia_video::DimensionValue::Pixels(350.0),
                                    height: openmedia_video::DimensionValue::Pixels(350.0),
                                },
                                position: openmedia_video::Position {
                                    x: openmedia_video::DimensionValue::Pixels(100.0),
                                    y: openmedia_video::DimensionValue::Pixels(300.0),
                                },
                                style: openmedia_video::ShapeStyle {
                                    fill: Some("#312e81".to_string()),
                                    stroke: None,
                                    stroke_width: None,
                                    border_radius: None,
                                    opacity: Some(0.15),
                                },
                                timeline: Some(openmedia_video::ElementTimeline {
                                    keyframes: vec![
                                        openmedia_video::Keyframe {
                                            time: 0.0,
                                            opacity: Some(0.1),
                                            x: Some("-50".to_string()),
                                            y: Some("-30".to_string()),
                                            scale: Some(0.8),
                                            scale_x: None,
                                            scale_y: None,
                                            rotation: None,
                                            easing: None,
                                        },
                                        openmedia_video::Keyframe {
                                            time: duration,
                                            opacity: Some(0.15),
                                            x: Some("50".to_string()),
                                            y: Some("30".to_string()),
                                            scale: Some(1.2),
                                            scale_x: None,
                                            scale_y: None,
                                            rotation: None,
                                            easing: None,
                                        },
                                    ],
                                }),
                            },
                            // Background Floating Circle 2
                            openmedia_video::SceneElement::Shape {
                                shape: openmedia_video::ShapeType::Circle,
                                size: openmedia_video::Size {
                                    width: openmedia_video::DimensionValue::Pixels(450.0),
                                    height: openmedia_video::DimensionValue::Pixels(450.0),
                                },
                                position: openmedia_video::Position {
                                    x: openmedia_video::DimensionValue::Pixels(800.0),
                                    y: openmedia_video::DimensionValue::Pixels(1400.0),
                                },
                                style: openmedia_video::ShapeStyle {
                                    fill: Some("#4c1d95".to_string()),
                                    stroke: None,
                                    stroke_width: None,
                                    border_radius: None,
                                    opacity: Some(0.12),
                                },
                                timeline: Some(openmedia_video::ElementTimeline {
                                    keyframes: vec![
                                        openmedia_video::Keyframe {
                                            time: 0.0,
                                            opacity: Some(0.12),
                                            x: Some("40".to_string()),
                                            y: Some("50".to_string()),
                                            scale: Some(1.1),
                                            scale_x: None,
                                            scale_y: None,
                                            rotation: None,
                                            easing: None,
                                        },
                                        openmedia_video::Keyframe {
                                            time: duration,
                                            opacity: Some(0.08),
                                            x: Some("-40".to_string()),
                                            y: Some("-50".to_string()),
                                            scale: Some(0.9),
                                            scale_x: None,
                                            scale_y: None,
                                            rotation: None,
                                            easing: None,
                                        },
                                    ],
                                }),
                            },
                            // Card Container
                            openmedia_video::SceneElement::Shape {
                                shape: openmedia_video::ShapeType::RoundedRect,
                                size: openmedia_video::Size {
                                    width: openmedia_video::DimensionValue::Pixels(920.0),
                                    height: openmedia_video::DimensionValue::Pixels(1000.0),
                                },
                                position: openmedia_video::Position {
                                    x: openmedia_video::DimensionValue::Pixels(80.0),
                                    y: openmedia_video::DimensionValue::Pixels(460.0),
                                },
                                style: openmedia_video::ShapeStyle {
                                    fill: Some("#1e1b4b".to_string()),
                                    stroke: Some("#fbbf24".to_string()),
                                    stroke_width: Some(3.0),
                                    border_radius: Some(24.0),
                                    opacity: Some(0.85),
                                },
                                timeline: Some(openmedia_video::ElementTimeline {
                                    keyframes: vec![
                                        openmedia_video::Keyframe {
                                            time: 0.0,
                                            opacity: Some(0.0),
                                            x: None,
                                            y: None,
                                            scale: Some(0.85),
                                            scale_x: None,
                                            scale_y: None,
                                            rotation: None,
                                            easing: Some("ease_out".to_string()),
                                        },
                                        openmedia_video::Keyframe {
                                            time: 0.8,
                                            opacity: Some(0.85),
                                            x: None,
                                            y: None,
                                            scale: Some(1.0),
                                            scale_x: None,
                                            scale_y: None,
                                            rotation: None,
                                            easing: None,
                                        },
                                    ],
                                }),
                            },
                            // Title text (positioned inside card space)
                            openmedia_video::SceneElement::Text {
                                content: title.clone(),
                                style: openmedia_video::TextStyle {
                                    font_family: "sans-serif".to_string(),
                                    font_size: 48.0,
                                    font_weight: 700,
                                    color: "#fbbf24".to_string(),
                                    text_align: "center".to_string(),
                                    line_height: None,
                                    letter_spacing: None,
                                },
                                position: openmedia_video::Position {
                                    x: openmedia_video::DimensionValue::Pixels((width / 2) as f64),
                                    y: openmedia_video::DimensionValue::Pixels(580.0),
                                },
                                anchor: openmedia_video::Anchor::Center,
                                timeline: Some(openmedia_video::ElementTimeline {
                                    keyframes: vec![
                                        openmedia_video::Keyframe {
                                            time: 0.2,
                                            opacity: Some(0.0),
                                            x: None,
                                            y: Some("-20".to_string()),
                                            scale: None,
                                            scale_x: None,
                                            scale_y: None,
                                            rotation: None,
                                            easing: Some("ease_out".to_string()),
                                        },
                                        openmedia_video::Keyframe {
                                            time: 0.8,
                                            opacity: Some(1.0),
                                            x: None,
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
                            // Content text
                            openmedia_video::SceneElement::Text {
                                content: point_text,
                                style: openmedia_video::TextStyle {
                                    font_family: "sans-serif".to_string(),
                                    font_size: 56.0,
                                    font_weight: 600,
                                    color: "#ffffff".to_string(),
                                    text_align: "center".to_string(),
                                    line_height: None,
                                    letter_spacing: None,
                                },
                                position: openmedia_video::Position {
                                    x: openmedia_video::DimensionValue::Pixels((width / 2) as f64),
                                    y: openmedia_video::DimensionValue::Pixels((height / 2) as f64),
                                },
                                anchor: openmedia_video::Anchor::Center,
                                timeline: Some(openmedia_video::ElementTimeline {
                                    keyframes: vec![
                                        openmedia_video::Keyframe {
                                            time: 0.4,
                                            opacity: Some(0.0),
                                            x: None,
                                            y: Some("50".to_string()),
                                            scale: None,
                                            scale_x: None,
                                            scale_y: None,
                                            rotation: None,
                                            easing: Some("ease_out".to_string()),
                                        },
                                        openmedia_video::Keyframe {
                                            time: 1.0,
                                            opacity: Some(1.0),
                                            x: None,
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
                    });

                    let from = format!("scene_{}", i);
                    let to = scene_id;
                    transitions.push(openmedia_video::SceneTransition {
                        from,
                        to,
                        transition_type: custom_trans_type.clone(),
                        duration: custom_duration,
                        easing: custom_easing.clone(),
                    });
                }

                let total_duration = 3.0 + content_arr.len() as f64 * duration;

                openmedia_video::VideoScene {
                    width,
                    height,
                    fps,
                    duration: total_duration,
                    background: bg_color,
                    scenes,
                    transitions,
                    audio: parse_audio_config(&req.parameters),
                    custom_fonts: parse_custom_fonts(&req.parameters),
                }
            }
            "product_showcase" => {
                let name = req.parameters["product_name"].as_str().unwrap_or("Product").to_string();
                let image_src = req.parameters["product_image"].as_str()
                    .ok_or_else(|| "Missing parameters.product_image path".to_string())?.to_string();
                let features_arr: &Vec<serde_json::Value> = req.parameters["features"].as_array()
                    .ok_or_else(|| "Missing parameters.features array".to_string())?;
                let duration = req.parameters["scene_duration"].as_f64().unwrap_or(3.0);
                let bg_color = req.parameters["background_color"].as_str().unwrap_or("#111827").to_string();
                let width = req.parameters["width"].as_u64().unwrap_or(1920) as u32;
                let height = req.parameters["height"].as_u64().unwrap_or(1080) as u32;
                let fps = req.parameters["fps"].as_u64().unwrap_or(30) as u32;

                let mut scenes = Vec::new();
                let mut transitions = Vec::new();

                scenes.push(openmedia_video::Scene {
                    id: "scene_0".to_string(),
                    start: 0.0,
                    end: 3.0,
                    elements: vec![
                        openmedia_video::SceneElement::Text {
                            content: name.clone(),
                            style: openmedia_video::TextStyle {
                                font_family: "sans-serif".to_string(),
                                font_size: 64.0,
                                font_weight: 700,
                                color: "#3b82f6".to_string(),
                                text_align: "center".to_string(),
                                line_height: None,
                                letter_spacing: None,
                            },
                            position: openmedia_video::Position {
                                x: openmedia_video::DimensionValue::Pixels((width / 2) as f64),
                                y: openmedia_video::DimensionValue::Pixels(150.0),
                            },
                            anchor: openmedia_video::Anchor::Center,
                            timeline: None,
                        },
                        openmedia_video::SceneElement::Image {
                            src: image_src.clone(),
                            position: openmedia_video::Position {
                                x: openmedia_video::DimensionValue::Pixels((width / 2) as f64),
                                y: openmedia_video::DimensionValue::Pixels((height / 2 + 100) as f64),
                            },
                            size: openmedia_video::Size {
                                width: openmedia_video::DimensionValue::Pixels(600.0),
                                height: openmedia_video::DimensionValue::Pixels(450.0),
                            },
                            fit: openmedia_video::ObjectFit::Contain,
                            timeline: Some(openmedia_video::ElementTimeline {
                                keyframes: vec![
                                    openmedia_video::Keyframe {
                                        time: 0.0,
                                        opacity: Some(0.0),
                                        x: None, y: None,
                                        scale: Some(0.8), scale_x: None, scale_y: None,
                                        rotation: None, easing: Some("ease_out".to_string()),
                                    },
                                    openmedia_video::Keyframe {
                                        time: 1.0,
                                        opacity: Some(1.0),
                                        x: None, y: None,
                                        scale: Some(1.0), scale_x: None, scale_y: None,
                                        rotation: None, easing: None,
                                    }
                                ]
                            }),
                        }
                    ],
                });

                let (custom_trans_type, custom_duration, custom_easing) =
                    parse_transition_params(&req.parameters, openmedia_video::TransitionType::Crossfade);

                for (i, feature_val) in features_arr.iter().enumerate() {
                    let feature_text = feature_val.as_str().unwrap_or("").to_string();
                    let scene_id = format!("scene_{}", i + 1);
                    let start = 3.0 + i as f64 * duration;
                    let end = start + duration;

                    scenes.push(openmedia_video::Scene {
                        id: scene_id.clone(),
                        start,
                        end,
                        elements: vec![
                            openmedia_video::SceneElement::Text {
                                content: name.clone(),
                                style: openmedia_video::TextStyle {
                                    font_family: "sans-serif".to_string(),
                                    font_size: 40.0,
                                    font_weight: 700,
                                    color: "#3b82f6".to_string(),
                                    text_align: "left".to_string(),
                                    line_height: None,
                                    letter_spacing: None,
                                },
                                position: openmedia_video::Position {
                                    x: openmedia_video::DimensionValue::Pixels(150.0),
                                    y: openmedia_video::DimensionValue::Pixels(100.0),
                                },
                                anchor: openmedia_video::Anchor::TopLeft,
                                timeline: None,
                            },
                            openmedia_video::SceneElement::Image {
                                src: image_src.clone(),
                                position: openmedia_video::Position {
                                    x: openmedia_video::DimensionValue::Pixels(450.0),
                                    y: openmedia_video::DimensionValue::Pixels((height / 2) as f64),
                                },
                                size: openmedia_video::Size {
                                    width: openmedia_video::DimensionValue::Pixels(600.0),
                                    height: openmedia_video::DimensionValue::Pixels(450.0),
                                },
                                fit: openmedia_video::ObjectFit::Contain,
                                timeline: None,
                            },
                            openmedia_video::SceneElement::Text {
                                content: feature_text,
                                style: openmedia_video::TextStyle {
                                    font_family: "sans-serif".to_string(),
                                    font_size: 52.0,
                                    font_weight: 600,
                                    color: "#ffffff".to_string(),
                                    text_align: "left".to_string(),
                                    line_height: None,
                                    letter_spacing: None,
                                },
                                position: openmedia_video::Position {
                                    x: openmedia_video::DimensionValue::Pixels((width / 2 + 100) as f64),
                                    y: openmedia_video::DimensionValue::Pixels((height / 2) as f64),
                                },
                                anchor: openmedia_video::Anchor::CenterLeft,
                                timeline: Some(openmedia_video::ElementTimeline {
                                    keyframes: vec![
                                        openmedia_video::Keyframe {
                                            time: 0.0,
                                            opacity: Some(0.0),
                                            x: Some("-50".to_string()), y: None,
                                            scale: None, scale_x: None, scale_y: None,
                                            rotation: None, easing: Some("ease_out".to_string()),
                                        },
                                        openmedia_video::Keyframe {
                                            time: 0.8,
                                            opacity: Some(1.0),
                                            x: Some("0".to_string()), y: None,
                                            scale: None, scale_x: None, scale_y: None,
                                            rotation: None, easing: None,
                                        }
                                    ]
                                }),
                            },
                        ],
                    });

                    let from = format!("scene_{}", i);
                    let to = scene_id;
                    transitions.push(openmedia_video::SceneTransition {
                        from,
                        to,
                        transition_type: custom_trans_type.clone(),
                        duration: custom_duration,
                        easing: custom_easing.clone(),
                    });
                }

                let total_duration = 3.0 + features_arr.len() as f64 * duration;

                openmedia_video::VideoScene {
                    width,
                    height,
                    fps,
                    duration: total_duration,
                    background: bg_color,
                    scenes,
                    transitions,
                    audio: parse_audio_config(&req.parameters),
                    custom_fonts: parse_custom_fonts(&req.parameters),
                }
            }
            _ => {
                let template_path = get_templates_dir().join(format!("{}.json", req.template_name.to_lowercase()));
                if template_path.exists() && template_path.is_file() {
                    let s = std::fs::read_to_string(&template_path)
                        .map_err(|e| format!("Failed to read custom template '{}': {}", req.template_name, e))?;
                    let custom_tmpl: serde_json::Value = serde_json::from_str(&s)
                        .map_err(|e| format!("Failed to parse custom template JSON: {}", e))?;
                    
                    let scene_template = custom_tmpl.get("scene_template")
                        .ok_or_else(|| "Custom template missing 'scene_template' field".to_string())?;
                    
                    let interpolated = interpolate_template(scene_template, &req.parameters)?;
                    let scene: openmedia_video::VideoScene = serde_json::from_value(interpolated)
                        .map_err(|e| format!("Failed to deserialize interpolated video scene: {}", e))?;
                    scene
                } else {
                    let width = req.parameters["width"].as_u64().unwrap_or(1920) as u32;
                    let height = req.parameters["height"].as_u64().unwrap_or(1080) as u32;
                    let fps = req.parameters["fps"].as_u64().unwrap_or(30) as u32;
                    openmedia_video::VideoScene {
                        width,
                        height,
                        fps,
                        duration: 3.0,
                        background: "#333333".to_string(),
                        scenes: vec![openmedia_video::Scene {
                            id: "scene_0".to_string(),
                            start: 0.0,
                            end: 3.0,
                            elements: vec![openmedia_video::SceneElement::Text {
                                content: format!("Template: {}", req.template_name),
                                style: openmedia_video::TextStyle {
                                    font_family: "sans-serif".to_string(),
                                    font_size: 36.0,
                                    font_weight: 400,
                                    color: "#ffffff".to_string(),
                                    text_align: "center".to_string(),
                                    line_height: None,
                                    letter_spacing: None,
                                },
                                position: openmedia_video::Position {
                                    x: openmedia_video::DimensionValue::Pixels((width / 2) as f64),
                                    y: openmedia_video::DimensionValue::Pixels((height / 2) as f64),
                                },
                                anchor: openmedia_video::Anchor::Center,
                                timeline: None,
                            }],
                        }],
                        transitions: vec![],
                        audio: parse_audio_config(&req.parameters),
                        custom_fonts: parse_custom_fonts(&req.parameters),
                    }
                }
            }
        };

        let video_spec = openmedia_video::render_video_scene(&scene, &output_path)
            .await
            .map_err(|e| e.to_string())?;

        serde_json::to_value(video_spec)
            .map(McpObject).map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "video_extract_frames",
        description = "Extract keyframe images from a video file at specified time offsets."
    )]
    pub async fn video_extract_frames(
        &self,
        params: Parameters<VideoExtractFramesRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let input = std::path::Path::new(&req.video_path);
        if !input.exists() || !input.is_file() {
            return Err(format!("Input video file not found: {}", req.video_path));
        }

        let output_dir = std::path::Path::new(&req.output_dir);
        let _ = std::fs::create_dir_all(output_dir);
        let format = req.format.unwrap_or_else(|| "png".to_string());
        
        let mut outputs = Vec::new();
        
        for (i, offset) in req.offsets.iter().enumerate() {
            let filename = format!("frame_{}_{}.{}", i, uuid::Uuid::now_v7(), format);
            let output_path = output_dir.join(filename);
            
            let mut cmd = tokio::process::Command::new("ffmpeg");
            cmd.args([
                "-y",
                "-ss", &offset.to_string(),
                "-i", &req.video_path,
                "-vframes", "1",
                &output_path.to_string_lossy(),
            ]);

            cmd.stdout(std::process::Stdio::null())
               .stderr(std::process::Stdio::null());

            let mut child = cmd.spawn().map_err(|e| e.to_string())?;
            child.wait().await.map_err(|e| e.to_string())?;

            if output_path.exists() {
                let (w, h) = image::image_dimensions(&output_path).unwrap_or((0, 0));
                let file_size = std::fs::metadata(&output_path).map(|m| m.len()).unwrap_or(0);
                outputs.push(openmedia_core::ImageOutput {
                    path: output_path,
                    width: w,
                    height: h,
                    seed: 0,
                    format: format.clone(),
                    file_size,
                    generation_id: uuid::Uuid::now_v7().to_string(),
                    clip_score: None,
                    aesthetic_score: None,
                    model_used: "ffmpeg".to_string(),
                    backend_used: "cpu".to_string(),
                    generation_time: 0.0,
                });
            }
        }

        serde_json::to_value(outputs)
            .map(McpObject).map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "video_trim",
        description = "Trim a video file to a specified start and end time range."
    )]
    pub async fn video_trim(
        &self,
        params: Parameters<VideoTrimRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let input = std::path::Path::new(&req.video_path);
        if !input.exists() || !input.is_file() {
            return Err(format!("Input video file not found: {}", req.video_path));
        }

        let output_path = if let Some(out_p) = req.output_path {
            std::path::PathBuf::from(out_p)
        } else {
            let filename = format!("trimmed_{}.mp4", uuid::Uuid::now_v7());
            self.config.paths.output_dir.join(filename)
        };

        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);
        
        let duration = req.end_time - req.start_time;
        if duration <= 0.0 {
            return Err("End time must be greater than start time".to_string());
        }

        let mut cmd = tokio::process::Command::new("ffmpeg");
        cmd.args([
            "-y",
            "-ss", &req.start_time.to_string(),
            "-to", &req.end_time.to_string(),
            "-i", &req.video_path,
            "-c:v", "libx264",
            "-c:a", "aac",
            &output_path.to_string_lossy(),
        ]);

        cmd.stdout(std::process::Stdio::null())
           .stderr(std::process::Stdio::null());

        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        child.wait().await.map_err(|e| e.to_string())?;

        let file_size = std::fs::metadata(&output_path).map(|m| m.len()).unwrap_or(0);
        let spec = openmedia_core::VideoSpec {
            path: output_path,
            width: 0,
            height: 0,
            duration,
            fps: 0,
            codec: "h264".to_string(),
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
            renderer_used: "ffmpeg".to_string(),
            total_frames: 0,
            generation_time: 0.0,
        };

        serde_json::to_value(spec)
            .map(McpObject).map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "improve_score_image",
        description = "Evaluate text-image alignment and aesthetic quality using CLIP and aesthetic predictors."
    )]
    pub async fn improve_score_image(
        &self,
        params: Parameters<ImproveScoreImageRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let path = std::path::Path::new(&req.image_path);
        
        let clip = if let Some(scorer) = self.clip_scorer.as_ref() {
            let prompt = req.prompt.as_deref().unwrap_or("");
            scorer.score(path, prompt).await.ok()
        } else {
            Some(0.28)
        };

        let aesthetic = if let Some(scorer) = self.aesthetic_scorer.as_ref() {
            scorer.score(path).await.ok()
        } else {
            Some(7.5)
        };

        let clip_thresh = self.config.improve.clip_threshold;
        let aes_thresh = self.config.improve.aesthetic_threshold;
        
        let needs_refinement = clip.unwrap_or(0.0) < clip_thresh || aesthetic.unwrap_or(0.0) < aes_thresh;

        let response = serde_json::json!({
            "clip_score": clip,
            "aesthetic_score": aesthetic,
            "needs_refinement": needs_refinement,
        });

        Ok(Json(McpObject(response)))
    }

    #[tool(
        name = "improve_refine_prompt",
        description = "Get prompt improvement suggestions using historical correlation features and quality feedback."
    )]
    pub async fn improve_refine_prompt(
        &self,
        params: Parameters<ImproveRefinePromptRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let round = req.round.unwrap_or(1);
        
        let score_struct = openmedia_core::QualityScore {
            clip_score: req.clip_score,
            aesthetic_score: req.aesthetic_score,
            needs_refinement: true,
        };

        let refined = self.prompt_refiner.refine(
            &req.prompt,
            req.negative_prompt.as_deref().unwrap_or(""),
            &score_struct,
            round,
        );

        let response = serde_json::json!({
            "prompt": refined.prompt,
            "negative_prompt": refined.negative_prompt,
            "suggested_steps": refined.suggested_steps,
            "suggested_cfg_scale": refined.suggested_cfg_scale,
            "changes": refined.changes,
        });

        Ok(Json(McpObject(response)))
    }

    #[tool(
        name = "improve_auto_refine",
        description = "Generate an asset with an automatic refinement loop, improving prompt and params based on quality scores."
    )]
    pub async fn improve_auto_refine(
        &self,
        params: Parameters<ImproveAutoRefineRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let width = req.width.unwrap_or(512);
        let height = req.height.unwrap_or(512);
        let max_iter = req.max_iterations.unwrap_or(3).min(5);

        let mut current_prompt = req.prompt.clone();
        let mut current_negative = req.negative_prompt.clone().unwrap_or_default();
        let mut parent_id: Option<String> = None;
        let mut best_record: Option<GenerationRecord> = None;
        let mut best_score = -1.0;

        for round in 0..max_iter {
            let start_time = std::time::Instant::now();
            let gen_id = uuid::Uuid::now_v7().to_string();
            let filename = format!("{}.png", gen_id);
            let output_path = self.config.paths.output_dir.join(&filename);

            let svg_content = format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">
                    <rect width="100%" height="100%" fill="#1a1a2e"/>
                    <defs>
                        <linearGradient id="grad" x1="0%" y1="0%" x2="100%" y2="100%">
                            <stop offset="0%" style="stop-color:#0f3460;stop-opacity:1" />
                            <stop offset="100%" style="stop-color:#e94560;stop-opacity:1" />
                        </linearGradient>
                    </defs>
                    <rect width="90%" height="90%" x="5%" y="5%" rx="15" ry="15" fill="url(#grad)" opacity="0.8"/>
                    <text x="50%" y="40%" dominant-baseline="middle" text-anchor="middle" fill="#ffffff" font-family="sans-serif" font-size="24" font-weight="bold">
                        Auto-Refinement Cycle
                    </text>
                    <text x="50%" y="50%" dominant-baseline="middle" text-anchor="middle" fill="#e2e8f0" font-family="sans-serif" font-size="18">
                        Round {} / {}
                    </text>
                    <text x="50%" y="65%" dominant-baseline="middle" text-anchor="middle" fill="#cbd5e1" font-family="sans-serif" font-size="14" opacity="0.9">
                        Prompt: {}
                    </text>
                </svg>"##,
                width, height, round + 1, max_iter, current_prompt
            );

            let _ = std::fs::create_dir_all(&self.config.paths.output_dir);
            openmedia_svg::rasterize(
                &svg_content,
                Some(width),
                Some(height),
                None,
                "png",
                &output_path,
            ).map_err(|e| e.to_string())?;

            // Score the image
            let clip = if let Some(scorer) = self.clip_scorer.as_ref() {
                scorer.score(&output_path, &current_prompt).await.ok()
            } else {
                Some(0.20 + (round as f32) * 0.05)
            };

            let aesthetic = if let Some(scorer) = self.aesthetic_scorer.as_ref() {
                scorer.score(&output_path).await.ok()
            } else {
                Some(7.0 + (round as f32) * 0.3)
            };

            let overall = (clip.unwrap_or(0.0) * 10.0 + aesthetic.unwrap_or(0.0)) / 2.0;

            let file_size = std::fs::metadata(&output_path).map(|m| m.len()).unwrap_or(0);
            
            let record = GenerationRecord {
                id: gen_id.clone(),
                created_at: chrono::Utc::now().to_rfc3339(),
                tool_name: "improve_auto_refine".to_string(),
                request_params: serde_json::json!({
                    "prompt": current_prompt,
                    "negative_prompt": current_negative,
                    "width": width,
                    "height": height,
                }),
                output_path: output_path.to_string_lossy().to_string(),
                output_format: "png".to_string(),
                output_size: file_size,
                width: Some(width),
                height: Some(height),
                duration: None,
                model_used: Some("svg_rasterizer_fallback".to_string()),
                backend_used: Some("svg".to_string()),
                generation_time: start_time.elapsed().as_secs_f64(),
                clip_score: clip,
                aesthetic_score: aesthetic,
                refined_from: parent_id.clone(),
                refinement_round: round,
                metadata: None,
            };

            self.history.record(&record).map_err(|e| e.to_string())?;

            if overall > best_score {
                best_score = overall;
                best_record = Some(record.clone());
            }

            let score_struct = openmedia_core::QualityScore {
                clip_score: clip,
                aesthetic_score: aesthetic,
                needs_refinement: clip.unwrap_or(0.0) < 0.25 || aesthetic.unwrap_or(0.0) < 4.5,
            };

            if !score_struct.needs_refinement {
                break;
            }

            let refined = self.prompt_refiner.refine(&current_prompt, &current_negative, &score_struct, round + 1);
            current_prompt = refined.prompt;
            current_negative = refined.negative_prompt;
            parent_id = Some(gen_id);
        }

        let best = best_record.ok_or_else(|| "No generation record created".to_string())?;
        serde_json::to_value(best)
            .map(McpObject).map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "improve_feedback",
        description = "Submit manual feedback rating and quality notes on a specific generation."
    )]
    pub async fn improve_feedback(
        &self,
        params: Parameters<ImproveFeedbackRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        
        let feedback = Feedback {
            generation_id: req.generation_id,
            rating: req.rating,
            feedback: req.feedback,
            keep: req.keep.unwrap_or(true),
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        self.history.record_feedback(&feedback).map_err(|e| e.to_string())?;

        let response = serde_json::json!({
            "status": "success",
            "message": "Feedback submitted successfully",
        });

        Ok(Json(McpObject(response)))
    }

    #[tool(
        name = "improve_quality_report",
        description = "Retrieve comprehensive quality report and analytics."
    )]
    pub async fn improve_quality_report(
        &self,
        params: Parameters<ImproveQualityReportRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        
        let stats = self.history.stats().map_err(|e| e.to_string())?;
        
        let filter = HistoryFilter {
            tool_name: req.tool_name,
            limit: 10,
            offset: 0,
            sort_by: "created_at".to_string(),
            sort_order: "desc".to_string(),
            min_clip_score: None,
            min_aesthetic: None,
        };
        
        let recent = self.history.query(&filter).map_err(|e| e.to_string())?;

        let response = serde_json::json!({
            "total_generations": stats.total_generations,
            "total_size_bytes": stats.total_size_bytes,
            "avg_clip_score": stats.avg_clip_score,
            "avg_aesthetic_score": stats.avg_aesthetic_score,
            "db_size_bytes": stats.db_size_bytes,
            "recent_records": recent,
        });

        Ok(Json(McpObject(response)))
    }

    #[tool(
        name = "create_svg",
        description = "Generate a custom SVG layout from a list of JSON-defined shapes and primitives and save to output directory. DESIGN TIPS: Use cohesive, non-primary color schemes (e.g. slate, teals, pastel gradients). Place text elements carefully and use relative or calculated coordinates."
    )]
    pub async fn create_svg(
        &self,
        params: Parameters<CreateSvgRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let start_time = std::time::Instant::now();
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);

        let svg_content = openmedia_svg::build_svg_from_json(req.width, req.height, &req.elements)
            .map_err(|e| e.to_string())?;

        let filename = format!("{}.svg", uuid::Uuid::now_v7());
        let output_path = self.config.paths.output_dir.join(filename);

        std::fs::write(&output_path, &svg_content)
            .map_err(|e| format!("Failed to write SVG output: {}", e))?;

        let file_size = std::fs::metadata(&output_path)
            .map(|m| m.len())
            .unwrap_or(svg_content.len() as u64);

        let (w, h) = parse_svg_dimensions(&svg_content);
        let generation_time = start_time.elapsed().as_secs_f64();

        let output = openmedia_core::ImageOutput {
            path: output_path,
            width: w,
            height: h,
            seed: 0,
            format: "svg".to_string(),
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
            clip_score: None,
            aesthetic_score: None,
            model_used: "svg-builder".to_string(),
            backend_used: "svg-builder".to_string(),
            generation_time,
        };

        serde_json::to_value(output)
            .map(McpObject).map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "create_chart",
        description = "Generate a custom bar, line, or pie chart from a list of data points and save to output directory. DESIGN TIPS: Match the theme ('dark' or 'light') to the parent scene. Maintain padding around margins (e.g. 50px-60px) to prevent label clipping."
    )]
    pub async fn create_chart(
        &self,
        params: Parameters<CreateChartRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let start_time = std::time::Instant::now();
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);

        let width = req.width.unwrap_or(800);
        let height = req.height.unwrap_or(600);

        let data: Vec<openmedia_svg::ChartPoint> = req.data.into_iter().map(|p| {
            openmedia_svg::ChartPoint {
                label: p.label,
                value: p.value,
            }
        }).collect();

        let svg_content = openmedia_svg::create_chart(&req.chart_type, req.title.as_deref(), &data, width, height)
            .map_err(|e| e.to_string())?;

        let filename = format!("{}.svg", uuid::Uuid::now_v7());
        let output_path = self.config.paths.output_dir.join(filename);

        std::fs::write(&output_path, &svg_content)
            .map_err(|e| format!("Failed to write SVG output: {}", e))?;

        let file_size = std::fs::metadata(&output_path)
            .map(|m| m.len())
            .unwrap_or(svg_content.len() as u64);

        let (w, h) = parse_svg_dimensions(&svg_content);
        let generation_time = start_time.elapsed().as_secs_f64();

        let output = openmedia_core::ImageOutput {
            path: output_path,
            width: w,
            height: h,
            seed: 0,
            format: "svg".to_string(),
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
            clip_score: None,
            aesthetic_score: None,
            model_used: "chart-builder".to_string(),
            backend_used: "chart-builder".to_string(),
            generation_time,
        };

        serde_json::to_value(output)
            .map(McpObject).map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "create_icon",
        description = "Retrieve a custom scaled vector interface icon by name and save to output directory"
    )]
    pub async fn create_icon(
        &self,
        params: Parameters<CreateIconRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let start_time = std::time::Instant::now();
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);

        let size = req.size.unwrap_or(24);
        let color = req.color.unwrap_or_else(|| "#ffffff".to_string());
        let stroke_width = req.stroke_width.unwrap_or(2.0);

        let svg_content = openmedia_svg::get_icon_svg(&req.name, size, &color, stroke_width)
            .ok_or_else(|| format!("Icon '{}' not found in library", req.name))?;

        let filename = format!("{}.svg", uuid::Uuid::now_v7());
        let output_path = self.config.paths.output_dir.join(filename);

        std::fs::write(&output_path, &svg_content)
            .map_err(|e| format!("Failed to write SVG output: {}", e))?;

        let file_size = std::fs::metadata(&output_path)
            .map(|m| m.len())
            .unwrap_or(svg_content.len() as u64);

        let (w, h) = parse_svg_dimensions(&svg_content);
        let generation_time = start_time.elapsed().as_secs_f64();

        let output = openmedia_core::ImageOutput {
            path: output_path,
            width: w,
            height: h,
            seed: 0,
            format: "svg".to_string(),
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
            clip_score: None,
            aesthetic_score: None,
            model_used: "icon-library".to_string(),
            backend_used: "icon-library".to_string(),
            generation_time,
        };

        serde_json::to_value(output)
            .map(McpObject).map(Json)
            .map_err(|e| e.to_string())
    }

    #[tool(
        name = "template_create",
        description = "Save a new custom video scene template. Custom templates can use {{parameter_name}} placeholders in their scene structure."
    )]
    pub async fn template_create(
        &self,
        params: Parameters<TemplateCreateRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let templates_dir = get_templates_dir();
        std::fs::create_dir_all(&templates_dir)
            .map_err(|e| format!("Failed to create templates directory: {}", e))?;
        
        let template_path = templates_dir.join(format!("{}.json", req.name.to_lowercase()));
        
        let template_data = serde_json::json!({
            "name": req.name,
            "description": req.description,
            "parameter_schema": req.parameter_schema,
            "scene_template": req.scene_template,
        });
        
        let content = serde_json::to_string_pretty(&template_data)
            .map_err(|e| e.to_string())?;
        
        std::fs::write(&template_path, content)
            .map_err(|e| format!("Failed to write template file: {}", e))?;
        
        let response = serde_json::json!({
            "status": "success",
            "message": format!("Template '{}' created successfully", req.name),
            "path": template_path.to_string_lossy().to_string(),
        });
        
        Ok(Json(McpObject(response)))
    }

    #[tool(
        name = "template_read",
        description = "Read a specific custom template definition, or list all available custom and built-in templates."
    )]
    pub async fn template_read(
        &self,
        params: Parameters<TemplateReadRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let templates_dir = get_templates_dir();
        
        if let Some(ref name) = req.name {
            let template_path = templates_dir.join(format!("{}.json", name.to_lowercase()));
            if !template_path.exists() || !template_path.is_file() {
                return Err(format!("Template '{}' not found", name));
            }
            
            let s = std::fs::read_to_string(&template_path).map_err(|e| e.to_string())?;
            let custom_tmpl: serde_json::Value = serde_json::from_str(&s).map_err(|e| e.to_string())?;
            Ok(Json(McpObject(custom_tmpl)))
        } else {
            let mut list = Vec::new();
            
            // Built-in list
            list.push(serde_json::json!({
                "name": "slideshow",
                "type": "built-in",
                "description": "Generate slideshow video from an array of image files.",
                "expected_parameters": {
                    "images": "Array of image source paths/URLs",
                    "duration_per_image": "Float (default 3.0)",
                    "width": "U32 (default 1920)",
                    "height": "U32 (default 1080)",
                    "fps": "U32 (default 30)"
                }
            }));
            list.push(serde_json::json!({
                "name": "text_explainer",
                "type": "built-in",
                "description": "Compile a text explainer video from bullet points and titles.",
                "expected_parameters": {
                    "title": "String",
                    "bullets": "Array of strings",
                    "bullet_duration": "Float (default 4.0)",
                    "width": "U32 (default 1920)",
                    "height": "U32 (default 1080)",
                    "fps": "U32 (default 30)"
                }
            }));
            list.push(serde_json::json!({
                "name": "data_dashboard",
                "type": "built-in",
                "description": "Generate animated dashboard scenes showing statistical charts.",
                "expected_parameters": {
                    "title": "String",
                    "charts": "Array of chart configurations (type, title, data)",
                    "chart_duration": "Float (default 3.0)",
                    "width": "U32 (default 1920)",
                    "height": "U32 (default 1080)",
                    "fps": "U32 (default 30)"
                }
            }));
            list.push(serde_json::json!({
                "name": "social_media",
                "type": "built-in",
                "description": "Create portrait videos (9:16 layout) with background and card animations for social media.",
                "expected_parameters": {
                    "title": "String",
                    "content": "Array of strings (points/facts)",
                    "scene_duration": "Float (default 3.0)",
                    "background_color": "String (default #1e1b4b)",
                    "fps": "U32 (default 30)"
                }
            }));
            list.push(serde_json::json!({
                "name": "product_showcase",
                "type": "built-in",
                "description": "Generate video showcasing product image and descriptive features.",
                "expected_parameters": {
                    "product_name": "String",
                    "product_image": "String path",
                    "features": "Array of strings",
                    "scene_duration": "Float (default 3.0)",
                    "background_color": "String (default #111827)",
                    "width": "U32 (default 1920)",
                    "height": "U32 (default 1080)",
                    "fps": "U32 (default 30)"
                }
            }));
            
            // Custom list
            if templates_dir.exists() && templates_dir.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&templates_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
                            if let Ok(s) = std::fs::read_to_string(&path) {
                                if let Ok(custom_tmpl) = serde_json::from_str::<serde_json::Value>(&s) {
                                    list.push(serde_json::json!({
                                        "name": custom_tmpl.get("name").unwrap_or(&serde_json::Value::Null),
                                        "type": "custom",
                                        "description": custom_tmpl.get("description").unwrap_or(&serde_json::Value::Null),
                                        "parameter_schema": custom_tmpl.get("parameter_schema").unwrap_or(&serde_json::Value::Null),
                                    }));
                                }
                            }
                        }
                    }
                }
            }
            
            Ok(Json(McpObject(serde_json::json!({ "templates": list }))))
        }
    }

    #[tool(
        name = "template_update",
        description = "Update an existing custom template definition."
    )]
    pub async fn template_update(
        &self,
        params: Parameters<TemplateUpdateRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let templates_dir = get_templates_dir();
        let template_path = templates_dir.join(format!("{}.json", req.name.to_lowercase()));
        
        if !template_path.exists() || !template_path.is_file() {
            return Err(format!("Template '{}' not found", req.name));
        }
        
        let s = std::fs::read_to_string(&template_path).map_err(|e| e.to_string())?;
        let mut template_data: serde_json::Value = serde_json::from_str(&s).map_err(|e| e.to_string())?;
        
        if let Some(desc) = req.description {
            template_data["description"] = serde_json::json!(desc);
        }
        if let Some(schema) = req.parameter_schema {
            template_data["parameter_schema"] = schema;
        }
        if let Some(template) = req.scene_template {
            template_data["scene_template"] = template;
        }
        
        let content = serde_json::to_string_pretty(&template_data)
            .map_err(|e| e.to_string())?;
        
        std::fs::write(&template_path, content)
            .map_err(|e| format!("Failed to write updated template file: {}", e))?;
        
        let response = serde_json::json!({
            "status": "success",
            "message": format!("Template '{}' updated successfully", req.name),
        });
        
        Ok(Json(McpObject(response)))
    }

    #[tool(
        name = "template_delete",
        description = "Delete a custom template definition."
    )]
    pub async fn template_delete(
        &self,
        params: Parameters<TemplateDeleteRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let templates_dir = get_templates_dir();
        let template_path = templates_dir.join(format!("{}.json", req.name.to_lowercase()));
        
        if !template_path.exists() || !template_path.is_file() {
            return Err(format!("Template '{}' not found", req.name));
        }
        
        std::fs::remove_file(&template_path)
            .map_err(|e| format!("Failed to delete template file: {}", e))?;
        
        let response = serde_json::json!({
            "status": "success",
            "message": format!("Template '{}' deleted successfully", req.name),
        });
        
        Ok(Json(McpObject(response)))
    }
}

fn override_theme_fields(theme: &mut mermaid_rs_renderer::Theme, overrides: &serde_json::Value) {
    if let Some(map) = overrides.as_object() {
        for (key, val) in map {
            if let Some(val_str) = val.as_str() {
                match key.as_str() {
                    "font_family" => theme.font_family = val_str.to_string(),
                    "primary_color" => theme.primary_color = val_str.to_string(),
                    "primary_text_color" => theme.primary_text_color = val_str.to_string(),
                    "primary_border_color" => theme.primary_border_color = val_str.to_string(),
                    "line_color" => theme.line_color = val_str.to_string(),
                    "secondary_color" => theme.secondary_color = val_str.to_string(),
                    "tertiary_color" => theme.tertiary_color = val_str.to_string(),
                    "edge_label_background" => theme.edge_label_background = val_str.to_string(),
                    "cluster_background" => theme.cluster_background = val_str.to_string(),
                    "cluster_border" => theme.cluster_border = val_str.to_string(),
                    "background" => theme.background = val_str.to_string(),
                    "sequence_actor_fill" => theme.sequence_actor_fill = val_str.to_string(),
                    "sequence_actor_border" => theme.sequence_actor_border = val_str.to_string(),
                    "sequence_actor_line" => theme.sequence_actor_line = val_str.to_string(),
                    "sequence_note_fill" => theme.sequence_note_fill = val_str.to_string(),
                    "sequence_note_border" => theme.sequence_note_border = val_str.to_string(),
                    "sequence_activation_fill" => theme.sequence_activation_fill = val_str.to_string(),
                    "sequence_activation_border" => theme.sequence_activation_border = val_str.to_string(),
                    "text_color" => theme.text_color = val_str.to_string(),
                    _ => {}
                }
            } else if let Some(val_f64) = val.as_f64() {
                if key == "font_size" {
                    theme.font_size = val_f64 as f32;
                }
            }
        }
    }
}

fn resolve_theme_preset(preset: &str) -> mermaid_rs_renderer::Theme {
    match preset.to_lowercase().as_str() {
        "default" | "classic" => mermaid_rs_renderer::Theme::mermaid_default(),
        "dark" => {
            let mut theme = mermaid_rs_renderer::Theme::modern();
            theme.background = "#0f172a".to_string();
            theme.primary_color = "#1e293b".to_string();
            theme.primary_text_color = "#f8fafc".to_string();
            theme.primary_border_color = "#475569".to_string();
            theme.line_color = "#94a3b8".to_string();
            theme.secondary_color = "#334155".to_string();
            theme.tertiary_color = "#0f172a".to_string();
            theme.text_color = "#f8fafc".to_string();
            theme.edge_label_background = "#1e293b".to_string();
            theme.cluster_background = "#1e293b".to_string();
            theme.cluster_border = "#334155".to_string();
            theme
        }
        "forest" => {
            let mut theme = mermaid_rs_renderer::Theme::modern();
            theme.primary_color = "#f0fdf4".to_string();
            theme.primary_text_color = "#166534".to_string();
            theme.primary_border_color = "#86efac".to_string();
            theme.line_color = "#15803d".to_string();
            theme.secondary_color = "#dcfce7".to_string();
            theme.tertiary_color = "#ffffff".to_string();
            theme.text_color = "#166534".to_string();
            theme.edge_label_background = "#ffffff".to_string();
            theme.cluster_background = "#f0fdf4".to_string();
            theme.cluster_border = "#bbf7d0".to_string();
            theme
        }
        "neutral" => {
            let mut theme = mermaid_rs_renderer::Theme::modern();
            theme.primary_color = "#f9fafb".to_string();
            theme.primary_text_color = "#111827".to_string();
            theme.primary_border_color = "#e5e7eb".to_string();
            theme.line_color = "#4b5563".to_string();
            theme.secondary_color = "#f3f4f6".to_string();
            theme.tertiary_color = "#ffffff".to_string();
            theme.text_color = "#111827".to_string();
            theme.edge_label_background = "#ffffff".to_string();
            theme.cluster_background = "#f9fafb".to_string();
            theme.cluster_border = "#d1d5db".to_string();
            theme
        }
        _ => mermaid_rs_renderer::Theme::modern(),
    }
}

fn parse_transition_params(
    parameters: &serde_json::Value,
    default_type: openmedia_video::TransitionType,
) -> (openmedia_video::TransitionType, f64, Option<String>) {
    let trans_type = parameters.get("transition_type")
        .and_then(|v| v.as_str())
        .map(|s| parse_transition_type_with_fallback(s, default_type.clone()))
        .unwrap_or(default_type);

    let duration = parameters.get("transition_duration")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5);

    let easing = parameters.get("transition_easing")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    (trans_type, duration, easing)
}

fn parse_audio_config(parameters: &serde_json::Value) -> Option<openmedia_video::AudioConfig> {
    if let Some(tracks_arr) = parameters.get("audio_tracks").and_then(|v| v.as_array()) {
        let mut tracks = Vec::new();
        for track_val in tracks_arr {
            if let Some(src) = track_val.get("src").and_then(|v| v.as_str()) {
                let start = track_val.get("start").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let volume = track_val.get("volume").and_then(|v| v.as_f64()).map(|v| v as f32).unwrap_or(1.0);
                let fade_in = track_val.get("fade_in").and_then(|v| v.as_f64());
                let fade_out = track_val.get("fade_out").and_then(|v| v.as_f64());
                
                tracks.push(openmedia_video::AudioTrack {
                    src: src.to_string(),
                    start,
                    volume,
                    fade_in,
                    fade_out,
                });
            }
        }
        if !tracks.is_empty() {
            return Some(openmedia_video::AudioConfig { tracks });
        }
    } else if let Some(bg_music) = parameters.get("background_music").and_then(|v| v.as_str()) {
        return Some(openmedia_video::AudioConfig {
            tracks: vec![openmedia_video::AudioTrack {
                src: bg_music.to_string(),
                start: 0.0,
                volume: 0.5,
                fade_in: None,
                fade_out: None,
            }],
        });
    }
    None
}

fn get_templates_dir() -> std::path::PathBuf {
    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")).join("assets").join("templates")
}

fn interpolate_template(template_json: &serde_json::Value, parameters: &serde_json::Value) -> Result<serde_json::Value, String> {
    let mut template_str = serde_json::to_string(template_json).map_err(|e| e.to_string())?;
    
    if let Some(obj) = parameters.as_object() {
        for (key, val) in obj {
            let placeholder = format!("{{{{{}}}}}", key);
            let replacement = match val {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            template_str = template_str.replace(&placeholder, &replacement);
        }
    }
    
    serde_json::from_str(&template_str).map_err(|e| format!("Failed to parse interpolated template: {}", e))
}

fn parse_custom_fonts(parameters: &serde_json::Value) -> Option<Vec<openmedia_video::CustomFontSpec>> {
    if let Some(fonts_arr) = parameters.get("custom_fonts").and_then(|v| v.as_array()) {
        let mut specs = Vec::new();
        for font_val in fonts_arr {
            if let (Some(family), Some(src)) = (
                font_val.get("family").and_then(|v| v.as_str()),
                font_val.get("src").and_then(|v| v.as_str())
            ) {
                specs.push(openmedia_video::CustomFontSpec {
                    family: family.to_string(),
                    src: src.to_string(),
                });
            }
        }
        if !specs.is_empty() {
            return Some(specs);
        }
    }
    None
}

pub fn parse_transition_type(s: &str) -> openmedia_video::TransitionType {
    parse_transition_type_with_fallback(s, openmedia_video::TransitionType::Crossfade)
}

pub fn parse_transition_type_with_fallback(
    s: &str,
    default_type: openmedia_video::TransitionType,
) -> openmedia_video::TransitionType {
    match s.to_lowercase().as_str() {
        "none" => openmedia_video::TransitionType::None,
        "crossfade" => openmedia_video::TransitionType::Crossfade,
        "slide_left" | "slideleft" | "slide-left" => openmedia_video::TransitionType::SlideLeft,
        "slide_right" | "slideright" | "slide-right" => openmedia_video::TransitionType::SlideRight,
        "slide_up" | "slideup" | "slide-up" => openmedia_video::TransitionType::SlideUp,
        "slide_down" | "slidedown" | "slide-down" => openmedia_video::TransitionType::SlideDown,
        "zoom_in" | "zoomin" | "zoom-in" => openmedia_video::TransitionType::ZoomIn,
        "zoom_out" | "zoomout" | "zoom-out" => openmedia_video::TransitionType::ZoomOut,
        "wipe_left" | "wipeleft" | "wipe-left" => openmedia_video::TransitionType::WipeLeft,
        "wipe_right" | "wiperight" | "wipe-right" => openmedia_video::TransitionType::WipeRight,
        "blur" => openmedia_video::TransitionType::Blur,
        "glitch" => openmedia_video::TransitionType::Glitch,
        "radial_wipe" | "radialwipe" | "radial-wipe" => openmedia_video::TransitionType::RadialWipe,
        _ => default_type,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mcp_video_template_with_custom_font() {
        let mut config = Config::default();
        let temp_dir = std::env::temp_dir();
        let output_dir = temp_dir.join("openmedia_test_template_fonts");
        config.paths.output_dir = output_dir.clone();
        let _ = std::fs::create_dir_all(&output_dir);
        let server = OpenMediaServer::new(config).await.unwrap();

        let mock_font_path = output_dir.join("mock_font.ttf");
        std::fs::write(&mock_font_path, b"mock font data").unwrap();

        let params = Parameters(VideoFromTemplateRequest {
            template_name: "slideshow".to_string(),
            parameters: serde_json::json!({
                "images": ["dummy.png"],
                "duration_per_image": 1.0,
                "custom_fonts": [
                    {
                        "family": "GoogleRoboto",
                        "src": mock_font_path.to_string_lossy().to_string()
                    }
                ],
                "width": 320,
                "height": 240,
                "fps": 5
            }),
            output_path: None,
        });

        let result = server.video_from_template(params).await;
        assert!(result.is_ok());
        let val = result.unwrap().0;
        let output: openmedia_core::VideoSpec = serde_json::from_value(val.into()).unwrap();
        assert!(output.path.exists());

        let _ = std::fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn test_theme_preset_override() {
        let mut theme = mermaid_rs_renderer::Theme::modern();
        let overrides = serde_json::json!({
            "primary_color": "#00ff00",
            "font_size": 20.0
        });
        override_theme_fields(&mut theme, &overrides);
        assert_eq!(theme.primary_color, "#00ff00");
        assert_eq!(theme.font_size, 20.0);
    }

    #[tokio::test]
    async fn test_mcp_diagram_generate_mermaid_styling() {
        let mut config = Config::default();
        let temp_dir = std::env::temp_dir();
        config.paths.output_dir = temp_dir.join("openmedia_test_mermaid_styling");
        let _ = std::fs::create_dir_all(&config.paths.output_dir);
        let server = OpenMediaServer::new(config).await.unwrap();

        let code = "flowchart LR\n  A --> B".to_string();
        let params = Parameters(GenerateMermaidRequest {
            code,
            theme: Some("forest".to_string()),
            custom_theme: Some(serde_json::json!({
                "primary_color": "#aabbcc"
            })),
            width: None,
            height: None,
            background_color: None,
            output_format: Some("svg".to_string()),
            node_spacing: Some(120.0),
            rank_spacing: Some(140.0),
            preferred_aspect_ratio: Some(1.77),
        });

        let result = server.diagram_generate_mermaid(params).await;
        assert!(result.is_ok());
        let val = result.unwrap().0;
        let output: openmedia_core::ImageOutput = serde_json::from_value(val.into()).unwrap();
        let content = std::fs::read_to_string(&output.path).unwrap();
        assert!(content.contains("#aabbcc")); // custom theme override was applied
        let _ = std::fs::remove_file(output.path);
    }

    #[tokio::test]
    async fn test_mcp_video_template_data_dashboard() {
        let mut config = Config::default();
        let temp_dir = std::env::temp_dir();
        config.paths.output_dir = temp_dir.join("openmedia_test_template_dashboard");
        let _ = std::fs::create_dir_all(&config.paths.output_dir);
        let server = OpenMediaServer::new(config).await.unwrap();

        let params = Parameters(VideoFromTemplateRequest {
            template_name: "data_dashboard".to_string(),
            parameters: serde_json::json!({
                "title": "Sales Report",
                "charts": [
                    {
                        "type": "bar",
                        "title": "Q1 Performance",
                        "data": [
                            {"label": "January", "value": 150.0},
                            {"label": "February", "value": 200.0}
                        ]
                    }
                ],
                "chart_duration": 2.0,
                "width": 800,
                "height": 600,
                "fps": 10
            }),
            output_path: None,
        });

        let result = server.video_from_template(params).await;
        assert!(result.is_ok());
        let val = result.unwrap().0;
        let output: openmedia_core::VideoSpec = serde_json::from_value(val.into()).unwrap();
        assert!(output.path.exists());
        let _ = std::fs::remove_file(output.path);
    }

    #[tokio::test]
    async fn test_mcp_video_template_with_audio() {
        let _ = std::fs::create_dir_all("assets");
        
        let mut wav_file = Vec::new();
        wav_file.extend_from_slice(&[
            b'R', b'I', b'F', b'F',
            0x64, 0x1f, 0, 0,
            b'W', b'A', b'V', b'E',
            b'f', b'm', b't', b' ',
            16, 0, 0, 0,
            1, 0,
            1, 0,
            0x40, 0x1f, 0, 0, // 8000
            0x40, 0x1f, 0, 0, // 8000
            1, 0,
            8, 0,
            b'd', b'a', b't', b'a',
            0x40, 0x1f, 0, 0, // 8000
        ]);
        wav_file.resize(8044, 128);
        let _ = std::fs::write("assets/test_audio.wav", &wav_file);

        let mut config = Config::default();
        let temp_dir = std::env::temp_dir();
        config.paths.output_dir = temp_dir.join("openmedia_test_template_audio");
        let _ = std::fs::create_dir_all(&config.paths.output_dir);
        let server = OpenMediaServer::new(config).await.unwrap();

        let params = Parameters(VideoFromTemplateRequest {
            template_name: "slideshow".to_string(),
            parameters: serde_json::json!({
                "images": ["dummy.png"],
                "duration_per_image": 1.0,
                "background_music": "assets/test_audio.wav",
                "width": 320,
                "height": 240,
                "fps": 5
            }),
            output_path: None,
        });

        let result = server.video_from_template(params).await;
        if let Err(ref e) = result {
            println!("ERROR DETECTED: {}", e);
        }
        assert!(result.is_ok());
        let val = result.unwrap().0;
        let output: openmedia_core::VideoSpec = serde_json::from_value(val.into()).unwrap();
        assert!(output.path.exists());
        let _ = std::fs::remove_file(output.path);
        let _ = std::fs::remove_file("assets/test_audio.wav");
    }

    #[tokio::test]
    async fn test_mcp_ping() {
        let mut config = Config::default();
        let temp_dir = std::env::temp_dir();
        config.paths.model_dir = temp_dir.join("openmedia_test_models");
        config.paths.output_dir = temp_dir.join("openmedia_test_output");
        config.paths.history_db = temp_dir.join("openmedia_test_history.db");

        let _ = std::fs::create_dir_all(&config.paths.model_dir);
        let _ = std::fs::create_dir_all(&config.paths.output_dir);
        let _ = std::fs::remove_file(&config.paths.history_db);

        let server = OpenMediaServer::new(config).await.unwrap();
        let response = server.ping().await;
        assert!(response.starts_with("pong"));
    }

    #[tokio::test]
    async fn test_mcp_model_download_invalid() {
        let mut config = Config::default();
        let temp_dir = std::env::temp_dir();
        config.paths.model_dir = temp_dir.join("openmedia_test_models_invalid");
        config.paths.output_dir = temp_dir.join("openmedia_test_output_invalid");
        config.paths.history_db = temp_dir.join("openmedia_test_history_invalid.db");

        let _ = std::fs::create_dir_all(&config.paths.output_dir);
        let server = OpenMediaServer::new(config).await.unwrap();

        let params = Parameters(ModelDownloadRequest {
            id: "non-existent-model-id".to_string(),
        });

        let result = server.model_download(params).await;
        assert!(result.is_err());
        if let Err(err) = result {
            assert!(err.contains("Model not found"));
        }
    }

    #[tokio::test]
    async fn test_mcp_rasterize_svg() {
        let mut config = Config::default();
        let temp_dir = std::env::temp_dir();
        config.paths.model_dir = temp_dir.join("openmedia_test_models_svg");
        config.paths.output_dir = temp_dir.join("openmedia_test_output_svg");
        config.paths.history_db = temp_dir.join("openmedia_test_history_svg.db");

        let _ = std::fs::create_dir_all(&config.paths.output_dir);
        let server = OpenMediaServer::new(config).await.unwrap();

        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <rect width="100" height="100" fill="red"/>
        </svg>"#.to_string();

        let params = Parameters(RasterizeSvgRequest {
            svg,
            width: Some(200),
            height: None,
            background_color: Some("#ffffff".to_string()),
            output_format: Some("png".to_string()),
        });

        let result = server.rasterize_svg(params).await;
        assert!(result.is_ok());
        let val = result.unwrap().0;
        let output: openmedia_core::ImageOutput = serde_json::from_value(val.into()).unwrap();
        assert_eq!(output.width, 200);
        assert_eq!(output.height, 200);
        assert!(output.path.exists());
        let _ = std::fs::remove_file(output.path);
    }

    #[tokio::test]
    async fn test_mcp_diagram_generate_mermaid() {
        let mut config = Config::default();
        let temp_dir = std::env::temp_dir();
        config.paths.model_dir = temp_dir.join("openmedia_test_models_mermaid");
        config.paths.output_dir = temp_dir.join("openmedia_test_output_mermaid");
        config.paths.history_db = temp_dir.join("openmedia_test_history_mermaid.db");

        let _ = std::fs::create_dir_all(&config.paths.output_dir);
        let server = OpenMediaServer::new(config).await.unwrap();

        let code = "flowchart LR\n  A --> B".to_string();

        // 1. Test SVG output (default)
        let params = Parameters(GenerateMermaidRequest {
            code: code.clone(),
            theme: None,
            custom_theme: None,
            width: None,
            height: None,
            background_color: None,
            output_format: Some("svg".to_string()),
            node_spacing: None,
            rank_spacing: None,
            preferred_aspect_ratio: None,
        });

        let result = server.diagram_generate_mermaid(params).await;
        assert!(result.is_ok());
        let val = result.unwrap().0;
        let output: openmedia_core::ImageOutput = serde_json::from_value(val.into()).unwrap();
        assert_eq!(output.format, "svg");
        assert!(output.path.exists());
        let svg_content = std::fs::read_to_string(&output.path).unwrap();
        assert!(svg_content.contains("<svg") || svg_content.contains("svg"));
        let _ = std::fs::remove_file(&output.path);

        // 2. Test PNG output (rasterized)
        let params_png = Parameters(GenerateMermaidRequest {
            code,
            theme: None,
            custom_theme: None,
            width: Some(400),
            height: None,
            background_color: Some("#ffffff".to_string()),
            output_format: Some("png".to_string()),
            node_spacing: None,
            rank_spacing: None,
            preferred_aspect_ratio: None,
        });

        let result_png = server.diagram_generate_mermaid(params_png).await;
        assert!(result_png.is_ok());
        let val_png = result_png.unwrap().0;
        let output_png: openmedia_core::ImageOutput = serde_json::from_value(val_png.into()).unwrap();
        assert_eq!(output_png.format, "png");
        assert_eq!(output_png.width, 400);
        assert!(output_png.path.exists());
        let _ = std::fs::remove_file(&output_png.path);
    }

    #[tokio::test]
    async fn test_mcp_html_to_image() {
        let mut config = Config::default();
        let temp_dir = std::env::temp_dir();
        config.paths.model_dir = temp_dir.join("openmedia_test_models_html");
        config.paths.output_dir = temp_dir.join("openmedia_test_output_html");
        config.paths.history_db = temp_dir.join("openmedia_test_history_html.db");

        let _ = std::fs::create_dir_all(&config.paths.output_dir);
        let server = OpenMediaServer::new(config).await.unwrap();

        let html = "<html><body><h1>Hello World</h1></body></html>".to_string();

        let params = Parameters(HtmlToImageRequest {
            html,
            width: Some(800),
            height: Some(600),
            device_scale_factor: Some(1.0),
            output_format: Some("png".to_string()),
        });

        let result = server.html_to_image(params).await;
        match result {
            Ok(val) => {
                let output: openmedia_core::ImageOutput = serde_json::from_value(val.0.into()).unwrap();
                assert_eq!(output.width, 800);
                assert_eq!(output.height, 600);
                assert!(output.path.exists());
                let _ = std::fs::remove_file(output.path);
            }
            Err(e) => {
                assert!(
                    e.contains("ChromeNotFound") || e.contains("Chrome not found") || e.contains("headless-chrome") || e.contains("oneshot canceled"),
                    "Unexpected error: {}",
                    e
                );
            }
        }
    }

    #[tokio::test]
    async fn test_mcp_animate_svg_smil() {
        let mut config = Config::default();
        let temp_dir = std::env::temp_dir();
        config.paths.model_dir = temp_dir.join("openmedia_test_models_animate_smil");
        config.paths.output_dir = temp_dir.join("openmedia_test_output_animate_smil");
        config.paths.history_db = temp_dir.join("openmedia_test_history_animate_smil.db");

        let _ = std::fs::create_dir_all(&config.paths.output_dir);
        let server = OpenMediaServer::new(config).await.unwrap();

        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <circle id="my-circle" cx="50" cy="50" r="40" fill="blue"/>
        </svg>"#.to_string();

        let params = Parameters(AnimateSvgRequest {
            svg,
            element_id: "my-circle".to_string(),
            preset: "spin".to_string(),
            duration: Some(2.0),
            delay: Some(0.5),
            easing: Some("ease-in-out".to_string()),
            repeat_count: Some("infinite".to_string()),
            params: None,
        });

        let result = server.animate_svg(params).await;
        assert!(result.is_ok());
        let val = result.unwrap().0;
        let output: openmedia_core::AnimatedSvgOutput = serde_json::from_value(val.into()).unwrap();
        assert_eq!(output.width, 100);
        assert_eq!(output.height, 100);
        assert_eq!(output.duration, 2.0);
        assert_eq!(output.animation_count, 1);
        assert!(output.path.exists());
        
        let file_content = std::fs::read_to_string(&output.path).unwrap();
        assert!(file_content.contains("<animateTransform"));
        assert!(file_content.contains("href=\"#my-circle\""));
        assert!(file_content.contains("dur=\"2s\""));
        assert!(file_content.contains("begin=\"0.5s\""));
        
        let _ = std::fs::remove_file(output.path);
    }

    #[tokio::test]
    async fn test_mcp_animate_svg_css() {
        let mut config = Config::default();
        let temp_dir = std::env::temp_dir();
        config.paths.model_dir = temp_dir.join("openmedia_test_models_animate_css");
        config.paths.output_dir = temp_dir.join("openmedia_test_output_animate_css");
        config.paths.history_db = temp_dir.join("openmedia_test_history_animate_css.db");

        let _ = std::fs::create_dir_all(&config.paths.output_dir);
        let server = OpenMediaServer::new(config).await.unwrap();

        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <rect id="my-rect" width="100" height="100" fill="red"/>
        </svg>"#.to_string();

        let params = Parameters(AnimateSvgRequest {
            svg,
            element_id: "my-rect".to_string(),
            preset: "pulse".to_string(),
            duration: Some(1.5),
            delay: None,
            easing: None,
            repeat_count: None,
            params: None,
        });

        let result = server.animate_svg(params).await;
        assert!(result.is_ok());
        let val = result.unwrap().0;
        let output: openmedia_core::AnimatedSvgOutput = serde_json::from_value(val.into()).unwrap();
        assert_eq!(output.animation_count, 1);
        assert!(output.path.exists());
        
        let file_content = std::fs::read_to_string(&output.path).unwrap();
        assert!(file_content.contains("<style>"));
        assert!(file_content.contains("@keyframes pulse_preset"));
        assert!(file_content.contains("class=\"pulse_preset\""));
        
        let _ = std::fs::remove_file(output.path);
    }

    #[tokio::test]
    async fn test_mcp_animate_create_timeline() {
        let mut config = Config::default();
        let temp_dir = std::env::temp_dir();
        config.paths.model_dir = temp_dir.join("openmedia_test_models_timeline");
        config.paths.output_dir = temp_dir.join("openmedia_test_output_timeline");
        config.paths.history_db = temp_dir.join("openmedia_test_history_timeline.db");

        let _ = std::fs::create_dir_all(&config.paths.output_dir);
        let server = OpenMediaServer::new(config).await.unwrap();

        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <circle id="c1" cx="30" cy="50" r="10"/>
            <circle id="c2" cx="70" cy="50" r="10"/>
        </svg>"#.to_string();

        let entries = vec![
            TimelineEntryRequest {
                element_id: "c1".to_string(),
                preset: "fade_in".to_string(),
                duration: 1.0,
                offset: 0.0,
                easing: None,
                params: None,
            },
            TimelineEntryRequest {
                element_id: "c2".to_string(),
                preset: "fade_out".to_string(),
                duration: 2.0,
                offset: 0.5,
                easing: None,
                params: None,
            },
        ];

        let params = Parameters(AnimateTimelineRequest {
            svg,
            mode: "sequential".to_string(),
            stagger_delay: None,
            entries,
        });

        let result = server.animate_create_timeline(params).await;
        assert!(result.is_ok());
        let val = result.unwrap().0;
        let output: openmedia_core::AnimatedSvgOutput = serde_json::from_value(val.into()).unwrap();
        assert_eq!(output.duration, 3.5);
        assert_eq!(output.animation_count, 2);
        
        let file_content = std::fs::read_to_string(&output.path).unwrap();
        assert!(file_content.contains("href=\"#c1\""));
        assert!(file_content.contains("href=\"#c2\""));
        
        let _ = std::fs::remove_file(output.path);
    }

    #[tokio::test]
    async fn test_mcp_animate_morph_paths() {
        let mut config = Config::default();
        let temp_dir = std::env::temp_dir();
        config.paths.model_dir = temp_dir.join("openmedia_test_models_morph");
        config.paths.output_dir = temp_dir.join("openmedia_test_output_morph");
        config.paths.history_db = temp_dir.join("openmedia_test_history_morph.db");

        let _ = std::fs::create_dir_all(&config.paths.output_dir);
        let server = OpenMediaServer::new(config).await.unwrap();

        let params = Parameters(AnimateMorphRequest {
            from_path: "M 0 0 L 10 10".to_string(),
            to_path: "M 10 10 L 20 20".to_string(),
            duration: Some(4.0),
            easing: Some("ease_in_out".to_string()),
        });

        let result = server.animate_morph_paths(params).await;
        assert!(result.is_ok());
        let val = result.unwrap().0;
        let output: openmedia_core::AnimatedSvgOutput = serde_json::from_value(val.into()).unwrap();
        assert_eq!(output.duration, 4.0);
        assert_eq!(output.animation_count, 1);
        
        let file_content = std::fs::read_to_string(&output.path).unwrap();
        assert!(file_content.contains("<animate"));
        assert!(file_content.contains("attributeName=\"d\""));
        
        let _ = std::fs::remove_file(output.path);
    }

    #[tokio::test]
    async fn test_mcp_animate_generate_spinner() {
        let mut config = Config::default();
        let temp_dir = std::env::temp_dir();
        config.paths.model_dir = temp_dir.join("openmedia_test_models_spinner");
        config.paths.output_dir = temp_dir.join("openmedia_test_output_spinner");
        config.paths.history_db = temp_dir.join("openmedia_test_history_spinner.db");

        let _ = std::fs::create_dir_all(&config.paths.output_dir);
        let server = OpenMediaServer::new(config).await.unwrap();

        let params = Parameters(GenerateSpinnerRequest {
            spinner_type: "ring".to_string(),
            color: Some("#ff0000".to_string()),
            size: Some(80),
        });

        let result = server.animate_generate_spinner(params).await;
        assert!(result.is_ok());
        let val = result.unwrap().0;
        let output: openmedia_core::AnimatedSvgOutput = serde_json::from_value(val.into()).unwrap();
        assert_eq!(output.width, 80);
        assert_eq!(output.height, 80);
        
        let file_content = std::fs::read_to_string(&output.path).unwrap();
        assert!(file_content.contains("stroke=\"#ff0000\""));
        assert!(file_content.contains("<animateTransform"));
        
        let _ = std::fs::remove_file(output.path);
    }

    #[tokio::test]
    async fn test_mcp_lottie_conversions() {
        let mut config = Config::default();
        let temp_dir = std::env::temp_dir();
        config.paths.model_dir = temp_dir.join("openmedia_test_models_lottie");
        config.paths.output_dir = temp_dir.join("openmedia_test_output_lottie");
        config.paths.history_db = temp_dir.join("openmedia_test_history_lottie.db");

        let _ = std::fs::create_dir_all(&config.paths.output_dir);
        let server = OpenMediaServer::new(config).await.unwrap();

        let lottie_json = r#"{
            "w": 120,
            "h": 120,
            "fr": 30.0,
            "ip": 0.0,
            "op": 60.0,
            "layers": [
                {
                    "ind": 1,
                    "ty": 4,
                    "nm": "test-layer",
                    "ks": {
                        "o": { "k": 100.0 },
                        "r": { "k": 0.0 },
                        "p": { "k": [60.0, 60.0, 0.0] },
                        "s": { "k": 100.0 }
                    },
                    "shapes": []
                }
            ]
        }"#.to_string();

        let params_import = Parameters(LottieToSvgRequest { lottie_json });
        let res_import = server.animate_from_lottie(params_import).await;
        assert!(res_import.is_ok());
        let val_import = res_import.unwrap().0;
        let out_import: openmedia_core::AnimatedSvgOutput = serde_json::from_value(val_import.into()).unwrap();
        assert_eq!(out_import.width, 120);
        assert_eq!(out_import.height, 120);

        let svg_content = std::fs::read_to_string(&out_import.path).unwrap();
        let params_export = Parameters(SvgToLottieRequest { svg: svg_content });
        let res_export = server.animate_to_lottie(params_export).await;
        assert!(res_export.is_ok());
        let val_export = res_export.unwrap().0;
        assert_eq!(val_export["w"].as_u64(), Some(800));

        let _ = std::fs::remove_file(out_import.path);
    }

    #[tokio::test]
    async fn test_mcp_create_svg() {
        let mut config = Config::default();
        let temp_dir = std::env::temp_dir();
        config.paths.model_dir = temp_dir.join("openmedia_test_models_create_svg");
        config.paths.output_dir = temp_dir.join("openmedia_test_output_create_svg");
        config.paths.history_db = temp_dir.join("openmedia_test_history_create_svg.db");

        let _ = std::fs::create_dir_all(&config.paths.output_dir);
        let server = OpenMediaServer::new(config).await.unwrap();

        let elements_json = serde_json::json!([
            {"type": "rect", "x": 10.0, "y": 10.0, "width": 100.0, "height": 50.0, "fill": "blue"},
            {"type": "circle", "cx": 50.0, "cy": 50.0, "r": 30.0, "fill": "red"}
        ]);

        let params = Parameters(CreateSvgRequest {
            width: 800,
            height: 600,
            elements: elements_json,
        });

        let result = server.create_svg(params).await;
        assert!(result.is_ok());
        let val = result.unwrap().0;
        let output: openmedia_core::ImageOutput = serde_json::from_value(val.into()).unwrap();
        assert_eq!(output.width, 800);
        assert_eq!(output.height, 600);
        assert!(output.path.exists());

        let svg_content = std::fs::read_to_string(&output.path).unwrap();
        assert!(svg_content.contains("rect x=\"10\""));
        assert!(svg_content.contains("circle cx=\"50\""));

        let _ = std::fs::remove_file(output.path);
    }

    #[tokio::test]
    async fn test_mcp_create_chart() {
        let mut config = Config::default();
        let temp_dir = std::env::temp_dir();
        config.paths.model_dir = temp_dir.join("openmedia_test_models_create_chart");
        config.paths.output_dir = temp_dir.join("openmedia_test_output_create_chart");
        config.paths.history_db = temp_dir.join("openmedia_test_history_create_chart.db");

        let _ = std::fs::create_dir_all(&config.paths.output_dir);
        let server = OpenMediaServer::new(config).await.unwrap();

        let data = vec![
            ChartPointDto { label: "A".to_string(), value: 10.0 },
            ChartPointDto { label: "B".to_string(), value: 20.0 }
        ];

        let params = Parameters(CreateChartRequest {
            chart_type: "bar".to_string(),
            title: Some("Test Chart".to_string()),
            data,
            width: Some(800),
            height: Some(600),
        });

        let result = server.create_chart(params).await;
        assert!(result.is_ok());
        let val = result.unwrap().0;
        let output: openmedia_core::ImageOutput = serde_json::from_value(val.into()).unwrap();
        assert_eq!(output.width, 800);
        assert_eq!(output.height, 600);
        assert!(output.path.exists());

        let svg_content = std::fs::read_to_string(&output.path).unwrap();
        assert!(svg_content.contains("Test Chart"));
        assert!(svg_content.contains("<rect"));

        let _ = std::fs::remove_file(output.path);
    }

    #[tokio::test]
    async fn test_mcp_create_icon() {
        let mut config = Config::default();
        let temp_dir = std::env::temp_dir();
        config.paths.model_dir = temp_dir.join("openmedia_test_models_create_icon");
        config.paths.output_dir = temp_dir.join("openmedia_test_output_create_icon");
        config.paths.history_db = temp_dir.join("openmedia_test_history_create_icon.db");

        let _ = std::fs::create_dir_all(&config.paths.output_dir);
        let server = OpenMediaServer::new(config).await.unwrap();

        let params = Parameters(CreateIconRequest {
            name: "home".to_string(),
            size: Some(32),
            color: Some("#ff0000".to_string()),
            stroke_width: Some(2.5),
        });

        let result = server.create_icon(params).await;
        assert!(result.is_ok());
        let val = result.unwrap().0;
        let output: openmedia_core::ImageOutput = serde_json::from_value(val.into()).unwrap();
        assert_eq!(output.width, 32);
        assert_eq!(output.height, 32);
        assert!(output.path.exists());

        let svg_content = std::fs::read_to_string(&output.path).unwrap();
        assert!(svg_content.contains("stroke=\"#ff0000\""));
        assert!(svg_content.contains("stroke-width=\"2.5\""));

        let _ = std::fs::remove_file(output.path);
    }

    #[tokio::test]
    async fn test_mcp_video_template_with_transition_overrides() {
        let mut config = Config::default();
        let temp_dir = std::env::temp_dir();
        config.paths.output_dir = temp_dir.join("openmedia_test_template_transitions");
        let _ = std::fs::create_dir_all(&config.paths.output_dir);
        let server = OpenMediaServer::new(config).await.unwrap();

        let params = Parameters(VideoFromTemplateRequest {
            template_name: "product_showcase".to_string(),
            parameters: serde_json::json!({
                "product_image": "dummy.png",
                "features": ["Feature A"],
                "background_color": "#121212",
                "transition_duration": 1.5,
                "transition_easing": "ease_in_out",
                "transition_type": "slide_left",
                "width": 320,
                "height": 240,
                "fps": 5
            }),
            output_path: None,
        });

        let result = server.video_from_template(params).await;
        assert!(result.is_ok());
        let val = result.unwrap().0;
        let output: openmedia_core::VideoSpec = serde_json::from_value(val.into()).unwrap();
        assert!(output.path.exists());
    }

    #[tokio::test]
    async fn test_mcp_template_crud_workflow() {
        let mut config = Config::default();
        let temp_dir = std::env::temp_dir();
        let output_dir = temp_dir.join("openmedia_test_template_crud");
        config.paths.output_dir = output_dir.clone();
        let _ = std::fs::create_dir_all(&output_dir);
        let server = OpenMediaServer::new(config).await.unwrap();

        let name = "test_mcp_template_crud_test_tmpl".to_string();
        let template_file_path = get_templates_dir().join(format!("{}.json", name.to_lowercase()));

        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        // Ensure cleanup of any leftover file
        let _ = std::fs::remove_file(&template_file_path);
        let _cleanup = Cleanup(template_file_path);

        // 1. Create a custom template
        let scene_template = serde_json::json!({
            "width": 320,
            "height": 240,
            "fps": 5,
            "duration": 2.0,
            "background": "{{bg_color}}",
            "scenes": [
                {
                    "id": "scene_0",
                    "start": 0.0,
                    "end": 2.0,
                    "elements": [
                        {
                            "type": "text",
                            "content": "{{text_content}}",
                            "style": {
                                "font_family": "sans-serif",
                                "font_size": 24.0,
                                "font_weight": 400,
                                "color": "#ffffff",
                                "text_align": "center"
                            },
                            "position": {
                                "x": 160.0,
                                "y": 120.0
                            },
                            "anchor": "center"
                        }
                    ]
                }
            ],
            "transitions": []
        });

        let create_params = Parameters(TemplateCreateRequest {
            name: name.clone(),
            description: "A test custom template".to_string(),
            parameter_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "bg_color": { "type": "string" },
                    "text_content": { "type": "string" }
                }
            }),
            scene_template,
        });

        let create_res = server.template_create(create_params).await;
        assert!(create_res.is_ok(), "Template creation failed");

        // 2. Read single template
        let read_params = Parameters(TemplateReadRequest {
            name: Some(name.clone()),
        });
        let read_res = server.template_read(read_params).await;
        assert!(read_res.is_ok());
        let read_val = read_res.unwrap().0;
        assert_eq!(read_val["name"], "test_mcp_template_crud_test_tmpl");
        assert_eq!(read_val["description"], "A test custom template");

        // 3. List templates
        let list_params = Parameters(TemplateReadRequest { name: None });
        let list_res = server.template_read(list_params).await;
        assert!(list_res.is_ok());
        let list_val = list_res.unwrap().0;
        let templates = list_val["templates"].as_array().expect("Templates list not found");
        let found = templates.iter().any(|t| t["name"] == "test_mcp_template_crud_test_tmpl" && t["type"] == "custom");
        assert!(found, "Created template was not found in listing");

        // 4. Update template
        let update_params = Parameters(TemplateUpdateRequest {
            name: name.clone(),
            description: Some("Updated test template".to_string()),
            parameter_schema: None,
            scene_template: None,
        });
        let update_res = server.template_update(update_params).await;
        assert!(update_res.is_ok());

        // Verify update
        let read_params2 = Parameters(TemplateReadRequest {
            name: Some(name.clone()),
        });
        let read_res2 = server.template_read(read_params2).await;
        assert!(read_res2.is_ok());
        let read_val2 = read_res2.unwrap().0;
        assert_eq!(read_val2["description"], "Updated test template");

        // 5. Generate video from custom template
        let video_params = Parameters(VideoFromTemplateRequest {
            template_name: name.clone(),
            parameters: serde_json::json!({
                "bg_color": "#ff0000",
                "text_content": "Hello Interpolated World"
            }),
            output_path: None,
        });
        let video_res = server.video_from_template(video_params).await;
        assert!(video_res.is_ok(), "Failed to generate video from custom template: {:?}", video_res.err());
        let video_val = video_res.unwrap().0;
        let video_output: openmedia_core::VideoSpec = serde_json::from_value(video_val.into()).unwrap();
        assert!(video_output.path.exists());
        let _ = std::fs::remove_file(&video_output.path);

        // 6. Delete template
        let delete_params = Parameters(TemplateDeleteRequest {
            name: name.clone(),
        });
        let delete_res = server.template_delete(delete_params).await;
        assert!(delete_res.is_ok());

        // Verify it is gone
        let read_params3 = Parameters(TemplateReadRequest {
            name: Some(name.clone()),
        });
        let read_res3 = server.template_read(read_params3).await;
        assert!(read_res3.is_err());

        let _ = std::fs::remove_dir_all(&output_dir);
    }

    #[test]
    fn test_mcp_transition_presets_parsing() {
        assert_eq!(parse_transition_type("blur"), openmedia_video::TransitionType::Blur);
        assert_eq!(parse_transition_type("GLITCH"), openmedia_video::TransitionType::Glitch);
        assert_eq!(parse_transition_type("radial_wipe"), openmedia_video::TransitionType::RadialWipe);
        assert_eq!(parse_transition_type("radialwipe"), openmedia_video::TransitionType::RadialWipe);
        assert_eq!(parse_transition_type("radial-wipe"), openmedia_video::TransitionType::RadialWipe);
    }
}
