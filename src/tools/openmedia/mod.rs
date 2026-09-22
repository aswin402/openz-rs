use anyhow::{anyhow, Result};
use crate::tools::openmedia::core::Config;
use crate::tools::openmedia::server::*;
use serde_json::{json, Value};

pub mod animate;
pub mod core;
pub mod handlers;
pub mod image;
pub mod improve;
pub mod process;
pub mod server;
pub mod svg;
pub mod video;

pub async fn get_server() -> Result<&'static OpenMediaServer> {
    static SERVER: std::sync::OnceLock<OpenMediaServer> = std::sync::OnceLock::new();
    if let Some(server) = SERVER.get() {
        return Ok(server);
    }

    let config = Config::load().unwrap_or_default();
    match OpenMediaServer::new(config).await {
        Ok(server) => {
            let _ = SERVER.set(server);
            SERVER
                .get()
                .ok_or_else(|| anyhow!("OpenMediaServer initialization did not complete"))
        }
        Err(e) => SERVER
            .get()
            .ok_or_else(|| anyhow!("Failed to initialize OpenMediaServer: {}", e)),
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
                let res = get_server()
                    .await?
                    .$server_method(req)
                    .await
                    .map_err(|e| anyhow!("{e}"))?;
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

fn coerce_number_value(v: &Value) -> Option<Value> {
    if let Some(s) = v.as_str() {
        let trimmed = s.trim();
        if let Ok(i) = trimmed.parse::<i64>() {
            return Some(json!(i));
        }
        if let Ok(f) = trimmed.parse::<f64>() {
            return Some(json!(f));
        }
    }
    None
}

fn coerce_numeric_fields(obj: &mut serde_json::Map<String, Value>, fields: &[&str]) {
    for field in fields {
        if let Some(val) = obj.get(*field) {
            if let Some(coerced) = coerce_number_value(val) {
                obj.insert((*field).to_string(), coerced);
            }
        }
    }
}

fn normalize_create_svg_arguments(arguments: &Value) -> Value {
    let Some(obj) = arguments.as_object() else {
        return arguments.clone();
    };

    let mut normalized = obj.clone();
    coerce_numeric_fields(&mut normalized, &["width", "height"]);

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
                coerce_numeric_fields(
                    map,
                    &[
                        "x", "y", "width", "height", "cx", "cy", "r", "rx", "ry", "x1", "y1",
                        "x2", "y2", "stroke_width", "font_size", "opacity",
                    ],
                );
                if map.get("type").and_then(|v| v.as_str()) == Some("text")
                    && !map.contains_key("content")
                {
                    if let Some(text) = map.remove("text") {
                        map.insert("content".to_string(), text);
                    }
                }
                if let Some(stroke_width) = map.remove("strokeWidth") {
                    let val = coerce_number_value(&stroke_width).unwrap_or(stroke_width);
                    map.insert("stroke_width".to_string(), val);
                }
                if let Some(text_anchor) = map.remove("textAnchor") {
                    map.insert("text_anchor".to_string(), text_anchor);
                }
                if let Some(dominant_baseline) = map.remove("dominantBaseline") {
                    map.insert("dominant_baseline".to_string(), dominant_baseline);
                }
                if let Some(alignment_baseline) = map.remove("alignmentBaseline") {
                    map.insert("dominant_baseline".to_string(), alignment_baseline);
                }
                if let Some(font_size) = map.remove("fontSize") {
                    let val = coerce_number_value(&font_size).unwrap_or(font_size);
                    map.insert("font_size".to_string(), val);
                }
                if let Some(font_family) = map.remove("fontFamily") {
                    map.insert("font_family".to_string(), font_family);
                }
                if let Some(font_weight) = map.remove("fontWeight") {
                    let val = coerce_number_value(&font_weight).unwrap_or(font_weight);
                    map.insert("font_weight".to_string(), val);
                }
                if let Some(stroke_linecap) = map.remove("strokeLinecap") {
                    map.insert("stroke_linecap".to_string(), stroke_linecap);
                }
                if map.get("type").and_then(|v| v.as_str()) == Some("text") {
                    if !map.contains_key("text_anchor") {
                        map.insert(
                            "text_anchor".to_string(),
                            serde_json::Value::String("middle".to_string()),
                        );
                    }
                    if !map.contains_key("dominant_baseline")
                        && map.get("text_anchor").and_then(|v| v.as_str()) == Some("middle")
                    {
                        map.insert(
                            "dominant_baseline".to_string(),
                            serde_json::Value::String("middle".to_string()),
                        );
                    }
                }
            }
        }
    }

    Value::Object(normalized)
}

fn normalize_animate_svg_arguments(arguments: &Value) -> Value {
    let Some(mut obj) = arguments.as_object().cloned() else {
        return arguments.clone();
    };
    coerce_numeric_fields(&mut obj, &["width", "height", "fps", "duration"]);
    Value::Object(obj)
}

fn normalize_video_create_slideshow_arguments(arguments: &Value) -> Value {
    let Some(mut obj) = arguments.as_object().cloned() else {
        return arguments.clone();
    };
    coerce_numeric_fields(
        &mut obj,
        &["width", "height", "fps", "duration_per_image", "transition_duration"],
    );
    Value::Object(obj)
}

fn normalize_video_trim_arguments(arguments: &Value) -> Value {
    let Some(mut obj) = arguments.as_object().cloned() else {
        return arguments.clone();
    };
    coerce_numeric_fields(&mut obj, &["start_time", "end_time"]);
    Value::Object(obj)
}

fn normalize_video_from_template_arguments(arguments: &Value) -> Value {
    let parsed = parse_json_string_fields(arguments, &["parameters"]);
    let Some(mut obj) = parsed.as_object().cloned() else {
        return parsed;
    };
    if let Some(params_obj) = obj.get_mut("parameters").and_then(|v| v.as_object_mut()) {
        coerce_numeric_fields(
            params_obj,
            &["width", "height", "fps", "duration_per_image"],
        );
    }
    Value::Object(obj)
}

fn normalize_improve_feedback_arguments(arguments: &Value) -> Value {
    let Some(mut obj) = arguments.as_object().cloned() else {
        return arguments.clone();
    };
    if let Some(r_val) = obj.get("rating") {
        let r = r_val
            .as_f64()
            .or_else(|| r_val.as_str().and_then(|s| s.parse::<f64>().ok()));
        if let Some(mut rating) = r {
            if rating > 1.0 {
                if rating <= 5.0 {
                    rating /= 5.0;
                } else if rating <= 10.0 {
                    rating /= 10.0;
                } else if rating <= 100.0 {
                    rating /= 100.0;
                }
            }
            obj.insert("rating".to_string(), json!(rating.clamp(0.0, 1.0)));
        }
    }
    Value::Object(obj)
}

fn normalize_image_batch_process_arguments(arguments: &Value) -> Value {
    let parsed = parse_json_string_fields(arguments, &["operations"]);
    let Some(mut obj) = parsed.as_object().cloned() else {
        return parsed;
    };
    if let Some(ops) = obj.get_mut("operations").and_then(|v| v.as_array_mut()) {
        for op in ops {
            *op = crate::tools::openmedia::server::normalize_process_operation_value(op);
        }
    }
    Value::Object(obj)
}

fn create_svg_parameter_schema() -> Value {
    let example = json!([
        {"type": "rect", "x": 0, "y": 0, "width": 800, "height": 600, "fill": "#07050a"},
        {"type": "circle", "cx": 400, "cy": 220, "r": 96, "fill": "#111827", "stroke": "#00e5ff", "stroke_width": 4, "opacity": 0.9},
        {"type": "line", "x1": 290, "y1": 170, "x2": 510, "y2": 270, "stroke": "#b366ff", "stroke_width": 18, "stroke_linecap": "round"},
        {"type": "text", "x": 400, "y": 410, "content": "OpenZ", "fill": "#ffffff", "font_size": 72, "font_family": "JetBrains Mono", "font_weight": 800, "text_anchor": "middle", "dominant_baseline": "middle"}
    ]);
    json!({
        "type": "object",
        "properties": {
            "width": { "type": "integer", "minimum": 1, "description": "SVG canvas width in pixels." },
            "height": { "type": "integer", "minimum": 1, "description": "SVG canvas height in pixels." },
            "elements": {
                "type": "array",
                "description": "SVG element list. Valid type values: rect, circle, line, text. Text uses content (or alias text), x, y, fill, font_size, font_family, font_weight, text_anchor, dominant_baseline. Use line for diagonals and separators. Use centered coordinates with text_anchor=middle and dominant_baseline=middle for aligned logos.",
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

fn normalize_create_chart_arguments(arguments: &Value) -> Value {
    let parsed = parse_json_string_fields(arguments, &["data", "data_points", "dataPoints", "points", "rows"]);
    let Some(mut obj) = parsed.as_object().cloned() else {
        return parsed;
    };
    coerce_numeric_fields(&mut obj, &["width", "height"]);

    let data_key = if obj.contains_key("data") {
        Some("data")
    } else if obj.contains_key("data_points") {
        Some("data_points")
    } else if obj.contains_key("dataPoints") {
        Some("dataPoints")
    } else if obj.contains_key("points") {
        Some("points")
    } else if obj.contains_key("rows") {
        Some("rows")
    } else {
        None
    };

    if let Some(key) = data_key {
        if let Some(points) = obj.get_mut(key).and_then(|v| v.as_array_mut()) {
            for pt in points {
                if let Some(pt_obj) = pt.as_object_mut() {
                    coerce_numeric_fields(pt_obj, &["value", "val", "count", "amount", "y", "number"]);
                }
            }
        }
    }
    Value::Object(obj)
}

fn normalize_create_icon_arguments(arguments: &Value) -> Value {
    let Some(mut obj) = arguments.as_object().cloned() else {
        return arguments.clone();
    };
    coerce_numeric_fields(&mut obj, &["size", "stroke_width", "strokeWidth", "stroke", "width"]);
    Value::Object(obj)
}

fn normalize_rasterize_svg_arguments(arguments: &Value) -> Value {
    let Some(mut obj) = arguments.as_object().cloned() else {
        return arguments.clone();
    };
    coerce_numeric_fields(&mut obj, &["width", "height"]);
    Value::Object(obj)
}

fn normalize_diagram_generate_mermaid_arguments(arguments: &Value) -> Value {
    let parsed = parse_json_string_fields(arguments, &["custom_theme"]);
    let Some(mut obj) = parsed.as_object().cloned() else {
        return parsed;
    };
    coerce_numeric_fields(
        &mut obj,
        &[
            "width",
            "height",
            "node_spacing",
            "rank_spacing",
            "preferred_aspect_ratio",
        ],
    );
    Value::Object(obj)
}

fn normalize_animate_generate_spinner_arguments(arguments: &Value) -> Value {
    let Some(mut obj) = arguments.as_object().cloned() else {
        return arguments.clone();
    };
    coerce_numeric_fields(&mut obj, &["size"]);
    Value::Object(obj)
}

fn normalize_image_apply_filter_arguments(arguments: &Value) -> Value {
    let Some(mut obj) = arguments.as_object().cloned() else {
        return arguments.clone();
    };
    coerce_numeric_fields(&mut obj, &["parameter", "param", "radius", "value", "intensity", "amount"]);
    Value::Object(obj)
}

fn normalize_image_resize_arguments(arguments: &Value) -> Value {
    let Some(mut obj) = arguments.as_object().cloned() else {
        return arguments.clone();
    };
    coerce_numeric_fields(&mut obj, &["width", "height"]);
    Value::Object(obj)
}

fn normalize_image_crop_arguments(arguments: &Value) -> Value {
    let Some(mut obj) = arguments.as_object().cloned() else {
        return arguments.clone();
    };
    coerce_numeric_fields(&mut obj, &["x", "y", "width", "height"]);
    Value::Object(obj)
}

fn normalize_image_transform_arguments(arguments: &Value) -> Value {
    let Some(mut obj) = arguments.as_object().cloned() else {
        return arguments.clone();
    };
    coerce_numeric_fields(&mut obj, &["angle"]);
    Value::Object(obj)
}

fn normalize_image_convert_arguments(arguments: &Value) -> Value {
    let Some(mut obj) = arguments.as_object().cloned() else {
        return arguments.clone();
    };
    coerce_numeric_fields(&mut obj, &["quality"]);
    Value::Object(obj)
}

fn normalize_openmedia_arguments(tool_name: &str, arguments: &Value) -> Value {
    match tool_name {
        "openmedia_create_chart" => normalize_create_chart_arguments(arguments),
        "openmedia_create_icon" => normalize_create_icon_arguments(arguments),
        "openmedia_create_svg" => normalize_create_svg_arguments(arguments),
        "openmedia_rasterize_svg" => normalize_rasterize_svg_arguments(arguments),
        "openmedia_diagram_generate_mermaid" => normalize_diagram_generate_mermaid_arguments(arguments),
        "openmedia_animate_svg" => normalize_animate_svg_arguments(arguments),
        "openmedia_animate_generate_spinner" => normalize_animate_generate_spinner_arguments(arguments),
        "openmedia_image_apply_filter" => normalize_image_apply_filter_arguments(arguments),
        "openmedia_image_resize" => normalize_image_resize_arguments(arguments),
        "openmedia_image_crop" => normalize_image_crop_arguments(arguments),
        "openmedia_image_transform" => normalize_image_transform_arguments(arguments),
        "openmedia_image_convert" => normalize_image_convert_arguments(arguments),
        "openmedia_image_batch_process" => normalize_image_batch_process_arguments(arguments),
        "openmedia_video_create_slideshow" => normalize_video_create_slideshow_arguments(arguments),
        "openmedia_video_trim" => normalize_video_trim_arguments(arguments),
        "openmedia_video_from_template" => normalize_video_from_template_arguments(arguments),
        "openmedia_improve_feedback" => normalize_improve_feedback_arguments(arguments),
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
define_openmedia_tool!(
    OpenMediaModelDownloadTool,
    "openmedia_model_download",
    "Download a specified model file (CLIP text/vision or Aesthetic predictor) from Hugging Face Hub with progress tracking.",
    crate::tools::openmedia::server::ModelDownloadRequest,
    model_download
);
define_openmedia_tool!(
    OpenMediaRasterizeSvgTool,
    "openmedia_rasterize_svg",
    "Rasterize an SVG string or file path into a PNG, JPEG, or WebP image.",
    crate::tools::openmedia::server::RasterizeSvgRequest,
    rasterize_svg
);
define_openmedia_tool!(
    OpenMediaDiagramGenerateMermaidTool,
    "openmedia_diagram_generate_mermaid",
    "Compile a Mermaid diagram string into an SVG, PNG, JPEG, or WebP diagram.",
    crate::tools::openmedia::server::GenerateMermaidRequest,
    diagram_generate_mermaid
);
define_openmedia_tool!(
    OpenMediaHtmlToImageTool,
    "openmedia_html_to_image",
    "Render HTML and CSS templates/files into an image (PNG, JPEG, or WebP).",
    crate::tools::openmedia::server::HtmlToImageRequest,
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

        let req: crate::tools::openmedia::server::CreateSvgRequest = serde_json::from_value(normalized)?;
        let mut res = get_server()
            .await?
            .create_svg(req)
            .await
            .map_err(|e| anyhow!("{e}"))?;

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
    crate::tools::openmedia::server::CreateChartRequest,
    create_chart
);
define_openmedia_tool!(
    OpenMediaCreateIconTool,
    "openmedia_create_icon",
    "Retrieve styled vector icons from the embedded Lucide library.",
    crate::tools::openmedia::server::CreateIconRequest,
    create_icon
);

// ── 2. SVG Animation Tools ─────────────────────────────────────
define_openmedia_tool!(
    OpenMediaAnimateSvgTool,
    "openmedia_animate_svg",
    "Apply keyframes/SMIL animation presets (fade_in, spin, bounce, etc.) to SVG elements.",
    crate::tools::openmedia::server::AnimateSvgRequest,
    animate_svg
);
define_openmedia_tool!(
    OpenMediaAnimateCreateTimelineTool,
    "openmedia_animate_create_timeline",
    "Coordinately sequence animations of multiple elements over a timeline.",
    crate::tools::openmedia::server::AnimateTimelineRequest,
    animate_create_timeline
);
define_openmedia_tool!(
    OpenMediaAnimateMorphPathsTool,
    "openmedia_animate_morph_paths",
    "Interpolate paths morphing between two vector strings.",
    crate::tools::openmedia::server::AnimateMorphRequest,
    animate_morph_paths
);
define_openmedia_tool!(
    OpenMediaAnimateGenerateSpinnerTool,
    "openmedia_animate_generate_spinner",
    "Create beautiful animated loading spinners in SVG.",
    crate::tools::openmedia::server::GenerateSpinnerRequest,
    animate_generate_spinner
);
define_openmedia_tool!(
    OpenMediaAnimateFromLottieTool,
    "openmedia_animate_from_lottie",
    "Convert a Lottie JSON animation into an animated SVG.",
    crate::tools::openmedia::server::LottieToSvgRequest,
    animate_from_lottie
);
define_openmedia_tool!(
    OpenMediaAnimateToLottieTool,
    "openmedia_animate_to_lottie",
    "Convert an animated SVG back into Lottie JSON.",
    crate::tools::openmedia::server::SvgToLottieRequest,
    animate_to_lottie
);

// ── 3. Image Filtering & Processing ────────────────────────────
define_openmedia_tool!(
    OpenMediaImageApplyFilterTool,
    "openmedia_image_apply_filter",
    "Apply filters (invert, grayscale, etc.) to an image.",
    crate::tools::openmedia::server::ImageApplyFilterRequest,
    image_apply_filter
);
define_openmedia_tool!(
    OpenMediaImageResizeTool,
    "openmedia_image_resize",
    "Resize an image with configurable width and height.",
    crate::tools::openmedia::server::ImageResizeRequest,
    image_resize
);
define_openmedia_tool!(
    OpenMediaImageCropTool,
    "openmedia_image_crop",
    "Crop an image using custom bounding box coordinates.",
    crate::tools::openmedia::server::ImageCropRequest,
    image_crop
);
define_openmedia_tool!(
    OpenMediaImageTransformTool,
    "openmedia_image_transform",
    "Transform an existing image guided by strength parameters.",
    crate::tools::openmedia::server::ImageTransformRequest,
    image_transform
);
define_openmedia_tool!(
    OpenMediaImageConvertTool,
    "openmedia_image_convert",
    "Convert image file format extension target.",
    crate::tools::openmedia::server::ImageConvertRequest,
    image_convert
);
define_openmedia_tool!(
    OpenMediaImageBatchProcessTool,
    "openmedia_image_batch_process",
    "Process image filters in batches.",
    crate::tools::openmedia::server::ImageBatchProcessRequest,
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
        let req: crate::tools::openmedia::server::VideoCreateRequest = serde_json::from_value(normalized)?;
        let res = get_server()
            .await?
            .video_create(req)
            .await
            .map_err(|e| anyhow!("{e}"))?;
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
        let req: crate::tools::openmedia::server::VideoPreviewRequest = serde_json::from_value(normalized)?;
        let res = get_server()
            .await?
            .video_preview(req)
            .await
            .map_err(|e| anyhow!("{e}"))?;
        Ok(res)
    }
}
define_openmedia_tool!(
    OpenMediaVideoCreateSlideshowTool,
    "openmedia_video_create_slideshow",
    "Compile an image sequence slideshow with audio overlays.",
    crate::tools::openmedia::server::VideoCreateSlideshowRequest,
    video_create_slideshow
);
define_openmedia_tool!(
    OpenMediaVideoAddTransitionTool,
    "openmedia_video_add_transition",
    "Apply scene transition blend clips.",
    crate::tools::openmedia::server::VideoAddTransitionRequest,
    video_add_transition
);
define_openmedia_tool!(
    OpenMediaVideoAddAudioTool,
    "openmedia_video_add_audio",
    "Add background narration/music tracks to a video.",
    crate::tools::openmedia::server::VideoAddAudioRequest,
    video_add_audio
);
define_openmedia_tool!(
    OpenMediaVideoFromTemplateTool,
    "openmedia_video_from_template",
    "Instantiate a video template replacing placeholder arguments.",
    crate::tools::openmedia::server::VideoFromTemplateRequest,
    video_from_template
);
define_openmedia_tool!(
    OpenMediaVideoExtractFramesTool,
    "openmedia_video_extract_frames",
    "Extract frames/images from a video at key timestamp offsets.",
    crate::tools::openmedia::server::VideoExtractFramesRequest,
    video_extract_frames
);
define_openmedia_tool!(
    OpenMediaVideoTrimTool,
    "openmedia_video_trim",
    "Trim a video file to a specific time range.",
    crate::tools::openmedia::server::VideoTrimRequest,
    video_trim
);

// ── 5. Templates CRUD ──────────────────────────────────────────
define_openmedia_tool!(
    OpenMediaTemplateCreateTool,
    "openmedia_template_create",
    "Create and save a custom video scene template.",
    crate::tools::openmedia::server::TemplateCreateRequest,
    template_create
);
define_openmedia_tool!(
    OpenMediaTemplateReadTool,
    "openmedia_template_read",
    "Read templates configurations details or list templates.",
    crate::tools::openmedia::server::TemplateReadRequest,
    template_read
);
define_openmedia_tool!(
    OpenMediaTemplateUpdateTool,
    "openmedia_template_update",
    "Update an existing template definition.",
    crate::tools::openmedia::server::TemplateUpdateRequest,
    template_update
);
define_openmedia_tool!(
    OpenMediaTemplateDeleteTool,
    "openmedia_template_delete",
    "Delete an existing template definition.",
    crate::tools::openmedia::server::TemplateDeleteRequest,
    template_delete
);

// ── 6. Self-Improvement & Quality scoring ──────────────────────
define_openmedia_tool!(
    OpenMediaImproveScoreImageTool,
    "openmedia_improve_score_image",
    "Score prompt alignment using CLIP and Aesthetic models.",
    crate::tools::openmedia::server::ImproveScoreImageRequest,
    improve_score_image
);
define_openmedia_tool!(
    OpenMediaImproveRefinePromptTool,
    "openmedia_improve_refine_prompt",
    "Get prompt refinement suffix recommendations based on score feedbacks.",
    crate::tools::openmedia::server::ImproveRefinePromptRequest,
    improve_refine_prompt
);
define_openmedia_tool!(
    OpenMediaImproveAutoRefineTool,
    "openmedia_improve_auto_refine",
    "Iteratively refine prompts to generate high aesthetic quality assets.",
    crate::tools::openmedia::server::ImproveAutoRefineRequest,
    improve_auto_refine
);
define_openmedia_tool!(
    OpenMediaImproveFeedbackTool,
    "openmedia_improve_feedback",
    "Log manual ratings score and description feedback on generations.",
    crate::tools::openmedia::server::ImproveFeedbackRequest,
    improve_feedback
);
define_openmedia_tool!(
    OpenMediaImproveQualityReportTool,
    "openmedia_improve_quality_report",
    "Fetch comprehensive statistics report of the generation history DB.",
    crate::tools::openmedia::server::ImproveQualityReportRequest,
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
        let res = get_server().await?.ping().await;
        Ok(json!({ "status": res }))
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

