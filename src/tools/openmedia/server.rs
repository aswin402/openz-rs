use crate::tools::openmedia::core::{Config, HardwareInfo, ModelRegistry, Result as CoreResult};
use crate::tools::openmedia::image::DiffusionPipeline;
use crate::tools::openmedia::improve::*;
use crate::tools::openmedia::process::DummyGpuPipeline;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

pub use super::handlers::helpers::*;
pub use super::handlers::image_handlers::normalize_process_operation_value;

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
            ClipScorer::load(&config.paths.model_dir.join("clip"))
                .await
                .ok()
        } else {
            None
        };

        let aesthetic_scorer = if config.improve.enable_aesthetic_scoring {
            AestheticScorer::load(&config.paths.model_dir.join("clip/aesthetic-predictor.onnx"))
                .await
                .ok()
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
    #[serde(alias = "svg_path", alias = "svgPath", alias = "path", alias = "file_path", alias = "filePath", alias = "input", alias = "source", alias = "file")]
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
    #[serde(alias = "type", alias = "spinnerType", alias = "style", alias = "kind", alias = "spinner_style")]
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
    #[serde(alias = "path", alias = "file_path", alias = "filePath", alias = "file", alias = "image", alias = "src", alias = "input")]
    pub image_path: String,
    /// Type of filter to apply (grayscale, invert, brightness, contrast, saturation, hue_rotate, sepia, threshold, blur, sharpen, unsharp_mask)
    #[serde(alias = "filter", alias = "filterType", alias = "type", alias = "filter_name")]
    pub filter_type: String,
    /// Value parameter for the filter (e.g. radius for blur, intensity for sepia, value for brightness/contrast)
    #[serde(alias = "param", alias = "radius", alias = "value", alias = "intensity", alias = "amount")]
    pub parameter: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImageResizeRequest {
    /// Path to the input image file
    #[serde(alias = "path", alias = "file_path", alias = "filePath", alias = "file", alias = "image", alias = "src", alias = "input")]
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
    #[serde(alias = "path", alias = "file_path", alias = "filePath", alias = "file", alias = "image", alias = "src", alias = "input")]
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
    #[serde(alias = "path", alias = "file_path", alias = "filePath", alias = "file", alias = "image", alias = "src", alias = "input")]
    pub image_path: String,
    /// Transform type (rotate, flip_horizontal, flip_vertical)
    pub transform_type: String,
    /// Rotation angle in degrees (90, 180, 270)
    pub angle: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImageConvertRequest {
    /// Path to the input image file
    #[serde(alias = "path", alias = "file_path", alias = "filePath", alias = "file", alias = "image", alias = "src", alias = "input")]
    pub image_path: String,
    /// Target output format (png, jpeg, webp, avif)
    #[serde(alias = "target_format", alias = "to_format", alias = "targetFormat", alias = "toFormat", alias = "ext", alias = "type")]
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ModelDownloadRequest {
    /// Unique model identifier (e.g., "clip-vit-b32-text", "clip-vit-b32-vision", or "aesthetic-predictor")
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GenerateMermaidRequest {
    /// Raw Mermaid diagram markdown or text
    #[serde(alias = "diagram", alias = "mermaid", alias = "content", alias = "text", alias = "graph", alias = "chart")]
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
    #[serde(alias = "backgroundColor", alias = "bg", alias = "background")]
    pub background_color: Option<String>,
    /// Output format (svg, png, jpeg, webp). Default is svg.
    #[serde(alias = "format", alias = "outputFormat", alias = "type")]
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
    #[serde(alias = "name", alias = "category", alias = "key", alias = "x", alias = "title")]
    pub label: String,
    /// Y-axis numeric value
    #[serde(alias = "val", alias = "count", alias = "amount", alias = "y", alias = "number")]
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateChartRequest {
    /// Type of chart (bar, line, pie)
    #[serde(alias = "type", alias = "kind", alias = "chartType", alias = "chart")]
    pub chart_type: String,
    /// Optional main title text for the chart
    #[serde(alias = "name", alias = "heading")]
    pub title: Option<String>,
    /// Array of data points containing labels and values
    #[serde(alias = "data_points", alias = "dataPoints", alias = "points", alias = "rows")]
    pub data: Vec<ChartPointDto>,
    /// Target image width (default 800)
    pub width: Option<u32>,
    /// Target height (default 600)
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateIconRequest {
    /// Name of the icon to retrieve (e.g. home, user, settings)
    #[serde(alias = "icon", alias = "icon_name", alias = "iconName", alias = "id")]
    pub name: String,
    /// Custom size in pixels (default 24)
    pub size: Option<u32>,
    /// Custom stroke/fill color hex (default #ffffff)
    pub color: Option<String>,
    /// Custom stroke width (default 2.0)
    #[serde(alias = "strokeWidth", alias = "stroke", alias = "width")]
    pub stroke_width: Option<f32>,
}
