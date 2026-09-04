use openmedia_core::{Config, HardwareInfo, ModelRegistry, Result as CoreResult};
use openmedia_image::DiffusionPipeline;
use openmedia_improve::*;
use openmedia_process::DummyGpuPipeline;
pub use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::{tool_handler, ServerHandler};
use rmcp::handler::server::tool::ToolRouter;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

mod render_handlers;
mod image_handlers;
mod animation_handlers;
mod improvement_handlers;
mod svg_handlers;
mod template_handlers;
mod video_handlers;
mod helpers;

pub(crate) use helpers::{
    get_templates_dir, inject_css_class, inject_style_or_xml, interpolate_template,
    override_theme_fields, parse_audio_config, parse_custom_fonts, parse_easing,
    parse_json_value_field, parse_preset, parse_svg_dimensions, parse_transition_params,
    parse_video_scene_value, resolve_theme_preset,
};
pub use helpers::{parse_transition_type, parse_transition_type_with_fallback};

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
            serde_json::json!({ "type": "object" }),
        )
        .unwrap()
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

impl OpenMediaServer {
    pub fn tool_router() -> ToolRouter<Self> {
        render_handlers::router()
            + image_handlers::router()
            + animation_handlers::router()
            + improvement_handlers::router()
            + svg_handlers::router()
            + template_handlers::router()
            + video_handlers::router()
    }
}

#[tool_handler]
impl ServerHandler for OpenMediaServer {}

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
            b'R', b'I', b'F', b'F', 0x64, 0x1f, 0, 0, b'W', b'A', b'V', b'E', b'f', b'm', b't',
            b' ', 16, 0, 0, 0, 1, 0, 1, 0, 0x40, 0x1f, 0, 0, // 8000
            0x40, 0x1f, 0, 0, // 8000
            1, 0, 8, 0, b'd', b'a', b't', b'a', 0x40, 0x1f, 0, 0, // 8000
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
        </svg>"#
            .to_string();

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
        let output_png: openmedia_core::ImageOutput =
            serde_json::from_value(val_png.into()).unwrap();
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
                let output: openmedia_core::ImageOutput =
                    serde_json::from_value(val.0.into()).unwrap();
                assert_eq!(output.width, 800);
                assert_eq!(output.height, 600);
                assert!(output.path.exists());
                let _ = std::fs::remove_file(output.path);
            }
            Err(e) => {
                assert!(
                    e.contains("ChromeNotFound")
                        || e.contains("Chrome not found")
                        || e.contains("headless-chrome")
                        || e.contains("oneshot canceled"),
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
        </svg>"#
            .to_string();

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
        </svg>"#
            .to_string();

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
        </svg>"#
            .to_string();

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
        }"#
        .to_string();

        let params_import = Parameters(LottieToSvgRequest { lottie_json });
        let res_import = server.animate_from_lottie(params_import).await;
        assert!(res_import.is_ok());
        let val_import = res_import.unwrap().0;
        let out_import: openmedia_core::AnimatedSvgOutput =
            serde_json::from_value(val_import.into()).unwrap();
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
            ChartPointDto {
                label: "A".to_string(),
                value: 10.0,
            },
            ChartPointDto {
                label: "B".to_string(),
                value: 20.0,
            },
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
        let templates = list_val["templates"]
            .as_array()
            .expect("Templates list not found");
        let found = templates
            .iter()
            .any(|t| t["name"] == "test_mcp_template_crud_test_tmpl" && t["type"] == "custom");
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
        assert!(
            video_res.is_ok(),
            "Failed to generate video from custom template: {:?}",
            video_res.err()
        );
        let video_val = video_res.unwrap().0;
        let video_output: openmedia_core::VideoSpec =
            serde_json::from_value(video_val.into()).unwrap();
        assert!(video_output.path.exists());
        let _ = std::fs::remove_file(&video_output.path);

        // 6. Delete template
        let delete_params = Parameters(TemplateDeleteRequest { name: name.clone() });
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

    fn minimal_video_scene_json() -> serde_json::Value {
        serde_json::json!({
            "width": 320,
            "height": 240,
            "fps": 5,
            "duration": 2.0,
            "background": "#000000",
            "scenes": [{
                "id": "scene_1",
                "start": 0.0,
                "end": 2.0,
                "elements": []
            }],
            "transitions": [],
            "audio": null
        })
    }

    #[test]
    fn test_parse_json_value_field_accepts_object_and_json_string() {
        let object = serde_json::json!({ "title": "Demo" });
        assert_eq!(
            parse_json_value_field(&object, "parameters").unwrap(),
            object
        );

        let parsed = parse_json_value_field(
            &serde_json::Value::String("{\"items\":[1,2]}".to_string()),
            "parameters",
        )
        .unwrap();
        assert_eq!(parsed["items"][0], 1);

        let parsed_array = parse_json_value_field(
            &serde_json::Value::String("[1,2,3]".to_string()),
            "elements",
        )
        .unwrap();
        assert_eq!(parsed_array.as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_parse_video_scene_value_accepts_object() {
        let scene = parse_video_scene_value(&minimal_video_scene_json()).unwrap();
        assert_eq!(scene.width, 320);
        assert_eq!(scene.scenes.len(), 1);
    }

    #[test]
    fn test_parse_video_scene_value_accepts_json_string() {
        let raw = minimal_video_scene_json().to_string();
        let scene = parse_video_scene_value(&serde_json::Value::String(raw)).unwrap();
        assert_eq!(scene.height, 240);
        assert_eq!(scene.fps, 5);
    }

    #[test]
    fn test_parse_video_scene_value_rejects_missing_path() {
        let err = parse_video_scene_value(&serde_json::Value::String(
            "/tmp/openz-missing-scene.json".to_string(),
        ))
        .unwrap_err();
        assert!(err.contains("scene string must be"));
    }

    #[test]
    fn test_mcp_transition_presets_parsing() {
        assert_eq!(
            parse_transition_type("blur"),
            openmedia_video::TransitionType::Blur
        );
        assert_eq!(
            parse_transition_type("GLITCH"),
            openmedia_video::TransitionType::Glitch
        );
        assert_eq!(
            parse_transition_type("radial_wipe"),
            openmedia_video::TransitionType::RadialWipe
        );
        assert_eq!(
            parse_transition_type("radialwipe"),
            openmedia_video::TransitionType::RadialWipe
        );
        assert_eq!(
            parse_transition_type("radial-wipe"),
            openmedia_video::TransitionType::RadialWipe
        );
    }
}
