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
                let normalized = normalize_openmedia_arguments($tool_name, arguments);
                let req: $request_type = serde_json::from_value(normalized)?;
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

fn parse_embedded_json_string(value: &Value) -> Option<Value> {
    let raw = value.as_str()?.trim();
    if !(raw.starts_with('{') || raw.starts_with('[')) {
        return None;
    }
    serde_json::from_str(raw).ok()
}

fn parse_json_string_fields(arguments: &Value, fields: &[&str]) -> Value {
    let Some(obj) = arguments.as_object() else {
        return arguments.clone();
    };

    let mut normalized = obj.clone();
    let mut changed = false;
    for field in fields {
        if let Some(parsed) = normalized.get(*field).and_then(parse_embedded_json_string) {
            normalized.insert((*field).to_string(), parsed);
            changed = true;
        }
    }

    if changed {
        Value::Object(normalized)
    } else {
        arguments.clone()
    }
}

fn normalize_create_svg_arguments(arguments: &Value) -> Value {
    let Some(obj) = arguments.as_object() else {
        return arguments.clone();
    };

    let mut normalized = obj.clone();
    if !normalized.contains_key("elements") {
        if let Some(shapes) = normalized.remove("shapes") {
            normalized.insert("elements".to_string(), shapes);
        }
    } else {
        normalized.remove("shapes");
    }

    if let Some(parsed) = normalized
        .get("elements")
        .and_then(parse_embedded_json_string)
    {
        normalized.insert("elements".to_string(), parsed);
    }

    if let Some(elements) = normalized
        .get_mut("elements")
        .and_then(|v| v.as_array_mut())
    {
        for element in elements {
            if let Some(map) = element.as_object_mut() {
                if map.get("type").and_then(|v| v.as_str()) == Some("text")
                    && !map.contains_key("content")
                {
                    if let Some(text) = map.remove("text") {
                        map.insert("content".to_string(), text);
                    }
                }
                if let Some(stroke_width) = map.remove("strokeWidth") {
                    map.insert("stroke_width".to_string(), stroke_width);
                }
                if let Some(text_anchor) = map.remove("textAnchor") {
                    map.insert("text_anchor".to_string(), text_anchor);
                }
                if let Some(font_size) = map.remove("fontSize") {
                    map.insert("font_size".to_string(), font_size);
                }
                if let Some(font_family) = map.remove("fontFamily") {
                    map.insert("font_family".to_string(), font_family);
                }
                if let Some(font_weight) = map.remove("fontWeight") {
                    map.insert("font_weight".to_string(), font_weight);
                }
                if let Some(stroke_linecap) = map.remove("strokeLinecap") {
                    map.insert("stroke_linecap".to_string(), stroke_linecap);
                }
            }
        }
    }

    Value::Object(normalized)
}

fn create_svg_parameter_schema() -> Value {
    let example = json!([
        {"type": "rect", "x": 0, "y": 0, "width": 800, "height": 600, "fill": "#07050a"},
        {"type": "circle", "cx": 400, "cy": 220, "r": 96, "fill": "#111827", "stroke": "#00e5ff", "stroke_width": 4, "opacity": 0.9},
        {"type": "line", "x1": 290, "y1": 170, "x2": 510, "y2": 270, "stroke": "#b366ff", "stroke_width": 18, "stroke_linecap": "round"},
        {"type": "text", "x": 400, "y": 410, "content": "OpenZ", "fill": "#ffffff", "font_size": 72, "font_family": "JetBrains Mono", "font_weight": 800, "text_anchor": "middle"}
    ]);
    json!({
        "type": "object",
        "properties": {
            "width": { "type": "integer", "minimum": 1, "description": "SVG canvas width in pixels." },
            "height": { "type": "integer", "minimum": 1, "description": "SVG canvas height in pixels." },
            "elements": {
                "type": "array",
                "description": "SVG element list. Valid type values: rect, circle, line, text. Text uses content (or alias text), x, y, fill, font_size, font_family, font_weight, text_anchor. Use line for diagonals and separators. Use centered coordinates and text_anchor=middle for aligned logos.",
                "examples": [example]
            },
            "shapes": { "type": "array", "description": "Alias for elements; normalized before execution." },
            "output_path": { "type": "string", "description": "Optional path where OpenZ should copy the generated SVG after OpenMedia creates it." }
        },
        "required": ["width", "height"],
        "anyOf": [
            { "required": ["elements"] },
            { "required": ["shapes"] }
        ]
    })
}

fn normalize_openmedia_arguments(tool_name: &str, arguments: &Value) -> Value {
    match tool_name {
        "openmedia_diagram_generate_mermaid" => {
            parse_json_string_fields(arguments, &["custom_theme"])
        }
        "openmedia_create_svg" => normalize_create_svg_arguments(arguments),
        "openmedia_image_batch_process" => parse_json_string_fields(arguments, &["operations"]),
        "openmedia_video_from_template" => parse_json_string_fields(arguments, &["parameters"]),
        "openmedia_template_create" => {
            parse_json_string_fields(arguments, &["parameter_schema", "scene_template"])
        }
        "openmedia_template_update" => {
            parse_json_string_fields(arguments, &["parameter_schema", "scene_template"])
        }
        _ => arguments.clone(),
    }
}

fn is_video_scene_object(value: &Value) -> bool {
    value
        .as_object()
        .map(|obj| {
            obj.contains_key("width")
                && obj.contains_key("height")
                && obj.contains_key("fps")
                && obj.contains_key("duration")
                && obj.contains_key("background")
                && obj.contains_key("scenes")
        })
        .unwrap_or(false)
}

fn normalize_video_scene_arguments(arguments: &Value) -> Value {
    let Some(obj) = arguments.as_object() else {
        return arguments.clone();
    };

    if obj.contains_key("scene") {
        return arguments.clone();
    }

    if let Some(scene_path) = obj.get("scene_path") {
        let mut normalized = obj.clone();
        normalized.insert("scene".to_string(), scene_path.clone());
        normalized.remove("scene_path");
        return Value::Object(normalized);
    }

    if is_video_scene_object(arguments) {
        let mut scene = obj.clone();
        let output_path = scene.remove("output_path");
        let mut normalized = serde_json::Map::new();
        normalized.insert("scene".to_string(), Value::Object(scene));
        if let Some(output_path) = output_path {
            normalized.insert("output_path".to_string(), output_path);
        }
        return Value::Object(normalized);
    }

    arguments.clone()
}

fn minimal_video_scene_example() -> Value {
    json!({
        "width": 1280,
        "height": 720,
        "fps": 24,
        "duration": 2.0,
        "background": "#1e293b",
        "scenes": [{
            "id": "scene_1",
            "start": 0.0,
            "end": 2.0,
            "elements": [{
                "type": "text",
                "content": "OpenZ",
                "style": {
                    "font_family": "sans-serif",
                    "font_size": 72.0,
                    "font_weight": 800,
                    "color": "#ffffff",
                    "text_align": "center"
                },
                "position": { "x": 640.0, "y": 360.0 },
                "anchor": "center",
                "timeline": null
            }]
        }],
        "transitions": [],
        "audio": null
    })
}

fn video_scene_parameter_schema(include_preview_fields: bool) -> Value {
    let mut properties = serde_json::Map::new();
    properties.insert(
        "scene".to_string(),
        json!({
            "description": "VideoScene object, JSON string, or path to a .json file. Prefer passing a structured object, not an escaped JSON string.",
            "anyOf": [
                {
                    "type": "object",
                    "required": ["width", "height", "fps", "duration", "background", "scenes"],
                    "properties": {
                        "width": { "type": "integer", "minimum": 1 },
                        "height": { "type": "integer", "minimum": 1 },
                        "fps": { "type": "integer", "minimum": 1 },
                        "duration": { "type": "number", "exclusiveMinimum": 0 },
                        "background": { "type": "string", "description": "Canvas background color. Prefer visible colors such as #1e293b over near-black unless the design intentionally needs it." },
                        "scenes": {
                            "type": "array",
                            "description": "Timeline scenes. Each scene requires id, start, end, and elements.",
                            "items": {
                                "type": "object",
                                "required": ["id", "start", "end", "elements"],
                                "properties": {
                                    "id": { "type": "string" },
                                    "start": { "type": "number", "description": "Start time in seconds." },
                                    "end": { "type": "number", "description": "End time in seconds." },
                                    "elements": {
                                        "type": "array",
                                        "description": "Scene elements. Valid type values: text, image, shape, svg, group, html, code, chart. Text elements require content, style.font_family, style.font_size, style.font_weight as a number, style.color, style.text_align, position, and anchor. Do not use rect/circle as element types; use type=shape with a shape field."
                                    },
                                    "animations": { "type": "array" }
                                }
                            }
                        },
                        "transitions": {
                            "type": "array",
                            "description": "Optional scene transitions. Valid type values include none, crossfade, slide_left, slide_right, slide_up, slide_down, zoom_in, zoom_out, wipe_left, wipe_right, wipe_up, wipe_down, dissolve, iris_in, iris_out, blur, glitch, radial_wipe. Do not use fade_in/fade_out as transition types."
                        },
                        "audio": { "type": ["object", "null"] },
                        "custom_fonts": { "type": ["array", "null"] }
                    }
                },
                { "type": "string" }
            ],
            "examples": [minimal_video_scene_example()]
        }),
    );
    properties.insert(
        "scene_path".to_string(),
        json!({
            "type": "string",
            "description": "Alias for scene when using a scene JSON file path."
        }),
    );

    if include_preview_fields {
        properties.insert(
            "time".to_string(),
            json!({ "type": "number", "description": "Time offset in seconds. Default 0.0." }),
        );
        properties.insert("width".to_string(), json!({ "type": "integer" }));
        properties.insert("height".to_string(), json!({ "type": "integer" }));
        properties.insert(
            "output_format".to_string(),
            json!({ "type": "string", "enum": ["png", "jpeg", "jpg"] }),
        );
    } else {
        properties.insert(
            "output_path".to_string(),
            json!({ "type": "string", "description": "Optional .mp4 output path." }),
        );
    }

    json!({
        "type": "object",
        "properties": properties,
        "anyOf": [
            { "required": ["scene"] },
            { "required": ["scene_path"] },
            { "required": ["width", "height", "fps", "duration", "background", "scenes"] }
        ]
    })
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
pub struct OpenMediaCreateSvgTool;

#[async_trait::async_trait]
impl crate::tools::Tool for OpenMediaCreateSvgTool {
    fn name(&self) -> &str {
        "openmedia_create_svg"
    }

    fn description(&self) -> &str {
        "Generate custom SVG layouts from JSON elements. Supports rect, circle, line, and text; includes alias normalization for shapes and text content."
    }

    fn parameters(&self) -> Value {
        create_svg_parameter_schema()
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let mut normalized = normalize_create_svg_arguments(arguments);
        let output_path = normalized
            .as_object_mut()
            .and_then(|obj| obj.remove("output_path"))
            .and_then(|v| v.as_str().map(|s| s.to_string()));

        let req: openmedia_mcp::CreateSvgRequest = serde_json::from_value(normalized)?;
        let Json(McpObject(mut res)) = get_server()
            .await
            .create_svg(Parameters(req))
            .await
            .map_err(map_mcp_err)?;

        if let Some(output_path) = output_path {
            if let Some(src_path) = res
                .get("path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
            {
                let target = crate::config::resolve_path(&output_path);
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(&src_path, &target)?;
                if let Some(obj) = res.as_object_mut() {
                    obj.insert(
                        "path".to_string(),
                        serde_json::Value::String(target.to_string_lossy().to_string()),
                    );
                    obj.insert(
                        "copied_from".to_string(),
                        serde_json::Value::String(src_path.to_string()),
                    );
                }
            }
        }

        Ok(res)
    }
}
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
pub struct OpenMediaVideoCreateTool;

#[async_trait::async_trait]
impl crate::tools::Tool for OpenMediaVideoCreateTool {
    fn name(&self) -> &str {
        "openmedia_video_create"
    }

    fn description(&self) -> &str {
        "Compile frame-by-frame videos from a VideoScene DSL. Pass scene as a structured object when possible; JSON strings and scene_path are also accepted."
    }

    fn parameters(&self) -> Value {
        video_scene_parameter_schema(false)
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let normalized = normalize_video_scene_arguments(arguments);
        let normalized = normalize_openmedia_arguments(self.name(), &normalized);
        let req: openmedia_mcp::VideoCreateRequest = serde_json::from_value(normalized)?;
        let Json(McpObject(res)) = get_server()
            .await
            .video_create(Parameters(req))
            .await
            .map_err(map_mcp_err)?;
        Ok(res)
    }
}

pub struct OpenMediaVideoPreviewTool;

#[async_trait::async_trait]
impl crate::tools::Tool for OpenMediaVideoPreviewTool {
    fn name(&self) -> &str {
        "openmedia_video_preview"
    }

    fn description(&self) -> &str {
        "Generate a preview frame for a VideoScene DSL at a timestamp. Pass scene as an object, JSON string, or scene_path."
    }

    fn parameters(&self) -> Value {
        video_scene_parameter_schema(true)
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let normalized = normalize_video_scene_arguments(arguments);
        let normalized = normalize_openmedia_arguments(self.name(), &normalized);
        let req: openmedia_mcp::VideoPreviewRequest = serde_json::from_value(normalized)?;
        let Json(McpObject(res)) = get_server()
            .await
            .video_preview(Parameters(req))
            .await
            .map_err(map_mcp_err)?;
        Ok(res)
    }
}
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
    use crate::tools::Tool;

    fn minimal_video_scene() -> Value {
        json!({
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
    fn test_openmedia_args_parse_json_string_fields() {
        let normalized = normalize_openmedia_arguments(
            "openmedia_video_from_template",
            &json!({
                "template_name": "social_media",
                "parameters": "{\"title\":\"Facts\",\"content\":[\"One\"]}"
            }),
        );
        assert_eq!(normalized["parameters"]["title"], "Facts");
        assert_eq!(normalized["parameters"]["content"][0], "One");

        let svg = normalize_openmedia_arguments(
            "openmedia_create_svg",
            &json!({ "width": 100, "height": 100, "elements": "[]" }),
        );
        assert!(svg["elements"].as_array().is_some());
    }

    #[test]
    fn test_openmedia_create_svg_args_accept_aliases_and_text_content() {
        let normalized = normalize_openmedia_arguments(
            "openmedia_create_svg",
            &json!({
                "width": 800,
                "height": 600,
                "shapes": [
                    {"type": "line", "x1": 10, "y1": 20, "x2": 200, "y2": 20, "stroke": "#00e5ff"},
                    {"type": "text", "x": 400, "y": 320, "text": "OpenZ", "fill": "#ffffff"}
                ]
            }),
        );

        assert!(normalized.get("shapes").is_none());
        assert_eq!(normalized["elements"][0]["type"], "line");
        assert_eq!(normalized["elements"][1]["content"], "OpenZ");
        assert!(normalized["elements"][1].get("text").is_none());
    }

    #[test]
    fn test_openmedia_create_svg_schema_includes_examples_and_output_path() {
        let tool = OpenMediaCreateSvgTool;
        let schema = tool.parameters();
        assert!(schema["properties"]["elements"]["examples"]
            .as_array()
            .is_some());
        assert_eq!(
            schema["properties"]["elements"]["examples"][0][0]["type"],
            "rect"
        );
        assert_eq!(
            schema["properties"]["elements"]["examples"][0][2]["type"],
            "line"
        );
        assert!(schema["properties"]["output_path"].is_object());
    }

    #[test]
    fn test_openmedia_video_schema_includes_valid_scene_example_and_element_contract() {
        let schema = video_scene_parameter_schema(false);
        let scene_param_schema = &schema["properties"]["scene"];
        assert!(scene_param_schema["examples"].as_array().is_some());
        let example = &scene_param_schema["examples"][0];
        let scene_schema = &scene_param_schema["anyOf"][0];
        assert_eq!(example["scenes"][0]["id"], "scene_1");
        assert_eq!(example["scenes"][0]["start"], 0.0);
        assert_eq!(example["scenes"][0]["end"], 2.0);
        assert_eq!(example["scenes"][0]["elements"][0]["type"], "text");
        assert_eq!(example["scenes"][0]["elements"][0]["content"], "OpenZ");
        assert_eq!(
            example["scenes"][0]["elements"][0]["style"]["font_weight"],
            800
        );
        assert!(
            scene_schema["properties"]["scenes"]["items"]["properties"]["elements"].is_object()
        );
    }

    #[test]
    fn test_openmedia_video_args_wrap_raw_scene() {
        let mut raw = minimal_video_scene();
        raw["output_path"] = json!("/tmp/out.mp4");

        let normalized = normalize_video_scene_arguments(&raw);

        assert_eq!(normalized["scene"]["width"], 320);
        assert_eq!(normalized["output_path"], "/tmp/out.mp4");
        assert!(normalized["scene"].get("output_path").is_none());
    }

    #[test]
    fn test_openmedia_video_args_accept_scene_path_alias() {
        let normalized = normalize_video_scene_arguments(&json!({
            "scene_path": "/tmp/scene.json",
            "output_path": "/tmp/out.mp4"
        }));

        assert_eq!(normalized["scene"], "/tmp/scene.json");
        assert!(normalized.get("scene_path").is_none());
        assert_eq!(normalized["output_path"], "/tmp/out.mp4");
    }

    #[tokio::test]
    async fn test_openmedia_server_ping() {
        let res = get_server().await.ping().await;
        assert!(res.contains("pong"));
    }
}
