use anyhow::{anyhow, Result};
use openmedia_core::Config;
use openmedia_mcp::{McpObject, OpenMediaServer};
use rmcp::handler::server::wrapper::{Json, Parameters};
use serde_json::{json, Value};

pub async fn get_server() -> &'static OpenMediaServer {
    static SERVER: std::sync::OnceLock<OpenMediaServer> = std::sync::OnceLock::new();
    if let Some(server) = SERVER.get() {
        server
    } else {
        let config = Config::load().unwrap_or_default();
        let server = OpenMediaServer::new(config)
            .await
            .expect("Failed to initialize OpenMediaServer");
        let _ = SERVER.set(server);
        SERVER.get().unwrap()
    }
}

pub fn map_mcp_err(err: String) -> anyhow::Error {
    anyhow!("MCP Error: {}", err)
}

macro_rules! define_openmedia_tool {
    ($struct_name:ident, $tool_name:expr, $description:expr, $request_type:ty, $server_method:ident) => {
        pub struct $struct_name;

        #[async_trait::async_trait]
        impl crate::tools::Tool for $struct_name {
            fn name(&self) -> &str {
                $tool_name
            }

            fn description(&self) -> &str {
                $description
            }

            fn parameters(&self) -> Value {
                let schema = schemars::schema_for!($request_type);
                serde_json::to_value(schema).unwrap_or_else(|_| json!({}))
            }

            async fn call(&self, arguments: &Value) -> Result<Value> {
                let req: $request_type = serde_json::from_value(arguments.clone())?;
                let Json(McpObject(res)) = get_server()
                    .await
                    .$server_method(Parameters(req))
                    .await
                    .map_err(map_mcp_err)?;
                Ok(res)
            }
        }
    };
}

// ── 1. Model & SVG Tools ───────────────────────────────────────
define_openmedia_tool!(OpenMediaModelDownloadTool, "openmedia_model_download", "Download a specified model file (CLIP text/vision or Aesthetic predictor) from Hugging Face Hub with progress tracking.", openmedia_mcp::ModelDownloadRequest, model_download);
define_openmedia_tool!(
    OpenMediaRasterizeSvgTool,
    "openmedia_rasterize_svg",
    "Rasterize an SVG string or file path into a PNG, JPEG, or WebP image.",
    openmedia_mcp::RasterizeSvgRequest,
    rasterize_svg
);
define_openmedia_tool!(
    OpenMediaDiagramGenerateMermaidTool,
    "openmedia_diagram_generate_mermaid",
    "Compile a Mermaid diagram string into an SVG, PNG, JPEG, or WebP diagram.",
    openmedia_mcp::GenerateMermaidRequest,
    diagram_generate_mermaid
);
define_openmedia_tool!(
    OpenMediaHtmlToImageTool,
    "openmedia_html_to_image",
    "Render HTML and CSS templates/files into an image (PNG, JPEG, or WebP).",
    openmedia_mcp::HtmlToImageRequest,
    html_to_image
);
define_openmedia_tool!(
    OpenMediaCreateSvgTool,
    "openmedia_create_svg",
    "Generate custom SVG layouts from a list of shapes.",
    openmedia_mcp::CreateSvgRequest,
    create_svg
);
define_openmedia_tool!(
    OpenMediaCreateChartTool,
    "openmedia_create_chart",
    "Generate vertical bars, lines, area, scatter, radar, and pie charts from raw data.",
    openmedia_mcp::CreateChartRequest,
    create_chart
);
define_openmedia_tool!(
    OpenMediaCreateIconTool,
    "openmedia_create_icon",
    "Retrieve styled vector icons from the embedded Lucide library.",
    openmedia_mcp::CreateIconRequest,
    create_icon
);

// ── 2. SVG Animation Tools ─────────────────────────────────────
define_openmedia_tool!(
    OpenMediaAnimateSvgTool,
    "openmedia_animate_svg",
    "Apply keyframes/SMIL animation presets (fade_in, spin, bounce, etc.) to SVG elements.",
    openmedia_mcp::AnimateSvgRequest,
    animate_svg
);
define_openmedia_tool!(
    OpenMediaAnimateCreateTimelineTool,
    "openmedia_animate_create_timeline",
    "Coordinately sequence animations of multiple elements over a timeline.",
    openmedia_mcp::AnimateTimelineRequest,
    animate_create_timeline
);
define_openmedia_tool!(
    OpenMediaAnimateMorphPathsTool,
    "openmedia_animate_morph_paths",
    "Interpolate paths morphing between two vector strings.",
    openmedia_mcp::AnimateMorphRequest,
    animate_morph_paths
);
define_openmedia_tool!(
    OpenMediaAnimateGenerateSpinnerTool,
    "openmedia_animate_generate_spinner",
    "Create beautiful animated loading spinners in SVG.",
    openmedia_mcp::GenerateSpinnerRequest,
    animate_generate_spinner
);
define_openmedia_tool!(
    OpenMediaAnimateFromLottieTool,
    "openmedia_animate_from_lottie",
    "Convert a Lottie JSON animation into an animated SVG.",
    openmedia_mcp::LottieToSvgRequest,
    animate_from_lottie
);
define_openmedia_tool!(
    OpenMediaAnimateToLottieTool,
    "openmedia_animate_to_lottie",
    "Convert an animated SVG back into Lottie JSON.",
    openmedia_mcp::SvgToLottieRequest,
    animate_to_lottie
);

// ── 3. Image Filtering & Processing ────────────────────────────
define_openmedia_tool!(
    OpenMediaImageApplyFilterTool,
    "openmedia_image_apply_filter",
    "Apply filters (invert, grayscale, etc.) to an image.",
    openmedia_mcp::ImageApplyFilterRequest,
    image_apply_filter
);
define_openmedia_tool!(
    OpenMediaImageResizeTool,
    "openmedia_image_resize",
    "Resize an image with configurable width and height.",
    openmedia_mcp::ImageResizeRequest,
    image_resize
);
define_openmedia_tool!(
    OpenMediaImageCropTool,
    "openmedia_image_crop",
    "Crop an image using custom bounding box coordinates.",
    openmedia_mcp::ImageCropRequest,
    image_crop
);
define_openmedia_tool!(
    OpenMediaImageTransformTool,
    "openmedia_image_transform",
    "Transform an existing image guided by strength parameters.",
    openmedia_mcp::ImageTransformRequest,
    image_transform
);
define_openmedia_tool!(
    OpenMediaImageConvertTool,
    "openmedia_image_convert",
    "Convert image file format extension target.",
    openmedia_mcp::ImageConvertRequest,
    image_convert
);
define_openmedia_tool!(
    OpenMediaImageBatchProcessTool,
    "openmedia_image_batch_process",
    "Process image filters in batches.",
    openmedia_mcp::ImageBatchProcessRequest,
    image_batch_process
);

// ── 4. Video Compositing & Templates ────────────────────────────
define_openmedia_tool!(
    OpenMediaVideoCreateTool,
    "openmedia_video_create",
    "Compile frame-by-frame videos defined using a JSON Scene DSL.",
    openmedia_mcp::VideoCreateRequest,
    video_create
);
define_openmedia_tool!(
    OpenMediaVideoPreviewTool,
    "openmedia_video_preview",
    "Generate a video preview frame at a specific timestamp offset.",
    openmedia_mcp::VideoPreviewRequest,
    video_preview
);
define_openmedia_tool!(
    OpenMediaVideoCreateSlideshowTool,
    "openmedia_video_create_slideshow",
    "Compile an image sequence slideshow with audio overlays.",
    openmedia_mcp::VideoCreateSlideshowRequest,
    video_create_slideshow
);
define_openmedia_tool!(
    OpenMediaVideoAddTransitionTool,
    "openmedia_video_add_transition",
    "Apply scene transition blend clips.",
    openmedia_mcp::VideoAddTransitionRequest,
    video_add_transition
);
define_openmedia_tool!(
    OpenMediaVideoAddAudioTool,
    "openmedia_video_add_audio",
    "Add background narration/music tracks to a video.",
    openmedia_mcp::VideoAddAudioRequest,
    video_add_audio
);
define_openmedia_tool!(
    OpenMediaVideoFromTemplateTool,
    "openmedia_video_from_template",
    "Instantiate a video template replacing placeholder arguments.",
    openmedia_mcp::VideoFromTemplateRequest,
    video_from_template
);
define_openmedia_tool!(
    OpenMediaVideoExtractFramesTool,
    "openmedia_video_extract_frames",
    "Extract frames/images from a video at key timestamp offsets.",
    openmedia_mcp::VideoExtractFramesRequest,
    video_extract_frames
);
define_openmedia_tool!(
    OpenMediaVideoTrimTool,
    "openmedia_video_trim",
    "Trim a video file to a specific time range.",
    openmedia_mcp::VideoTrimRequest,
    video_trim
);

// ── 5. Templates CRUD ──────────────────────────────────────────
define_openmedia_tool!(
    OpenMediaTemplateCreateTool,
    "openmedia_template_create",
    "Create and save a custom video scene template.",
    openmedia_mcp::TemplateCreateRequest,
    template_create
);
define_openmedia_tool!(
    OpenMediaTemplateReadTool,
    "openmedia_template_read",
    "Read templates configurations details or list templates.",
    openmedia_mcp::TemplateReadRequest,
    template_read
);
define_openmedia_tool!(
    OpenMediaTemplateUpdateTool,
    "openmedia_template_update",
    "Update an existing template definition.",
    openmedia_mcp::TemplateUpdateRequest,
    template_update
);
define_openmedia_tool!(
    OpenMediaTemplateDeleteTool,
    "openmedia_template_delete",
    "Delete an existing template definition.",
    openmedia_mcp::TemplateDeleteRequest,
    template_delete
);

// ── 6. Self-Improvement & Quality scoring ──────────────────────
define_openmedia_tool!(
    OpenMediaImproveScoreImageTool,
    "openmedia_improve_score_image",
    "Score prompt alignment using CLIP and Aesthetic models.",
    openmedia_mcp::ImproveScoreImageRequest,
    improve_score_image
);
define_openmedia_tool!(
    OpenMediaImproveRefinePromptTool,
    "openmedia_improve_refine_prompt",
    "Get prompt refinement suffix recommendations based on score feedbacks.",
    openmedia_mcp::ImproveRefinePromptRequest,
    improve_refine_prompt
);
define_openmedia_tool!(
    OpenMediaImproveAutoRefineTool,
    "openmedia_improve_auto_refine",
    "Iteratively refine prompts to generate high aesthetic quality assets.",
    openmedia_mcp::ImproveAutoRefineRequest,
    improve_auto_refine
);
define_openmedia_tool!(
    OpenMediaImproveFeedbackTool,
    "openmedia_improve_feedback",
    "Log manual ratings score and description feedback on generations.",
    openmedia_mcp::ImproveFeedbackRequest,
    improve_feedback
);
define_openmedia_tool!(
    OpenMediaImproveQualityReportTool,
    "openmedia_improve_quality_report",
    "Fetch comprehensive statistics report of the generation history DB.",
    openmedia_mcp::ImproveQualityReportRequest,
    improve_quality_report
);

// ── 7. Ping (Special casing) ───────────────────────────────────
pub struct OpenMediaPingTool;

#[async_trait::async_trait]
impl crate::tools::Tool for OpenMediaPingTool {
    fn name(&self) -> &str {
        "openmedia_ping"
    }

    fn description(&self) -> &str {
        "Ping the media generation server to check status and health"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn call(&self, _arguments: &Value) -> Result<Value> {
        let res = get_server().await.ping().await;
        Ok(json!({ "status": res }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_openmedia_server_ping() {
        let res = get_server().await.ping().await;
        assert!(res.contains("pong"));
    }
}
