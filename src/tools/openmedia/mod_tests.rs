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
    assert_eq!(normalized["elements"][1]["text_anchor"], "middle");
    assert_eq!(normalized["elements"][1]["dominant_baseline"], "middle");
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
    let description = schema["properties"]["elements"]["description"]
        .as_str()
        .unwrap();
    assert!(description.contains("dominant_baseline"));
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
    let res = get_server().await.unwrap().ping().await;
    assert!(res.contains("pong"));
}

#[test]
fn test_openmedia_create_svg_coerces_string_numbers() {
    let normalized = normalize_openmedia_arguments(
        "openmedia_create_svg",
        &json!({
            "width": "400",
            "height": "300",
            "elements": [
                {
                    "type": "rect",
                    "x": "10",
                    "y": "20",
                    "width": "100",
                    "height": "50",
                    "fill": "#ff0000",
                    "stroke_width": "2"
                },
                {
                    "type": "circle",
                    "cx": "200",
                    "cy": "150",
                    "r": "40",
                    "fill": "#00ff00"
                }
            ]
        }),
    );

    assert_eq!(normalized["width"], 400);
    assert_eq!(normalized["height"], 300);
    assert_eq!(normalized["elements"][0]["x"], 10);
    assert_eq!(normalized["elements"][0]["y"], 20);
    assert_eq!(normalized["elements"][0]["width"], 100);
    assert_eq!(normalized["elements"][0]["height"], 50);
    assert_eq!(normalized["elements"][0]["stroke_width"], 2);
    assert_eq!(normalized["elements"][1]["cx"], 200);
    assert_eq!(normalized["elements"][1]["cy"], 150);
    assert_eq!(normalized["elements"][1]["r"], 40);
}

#[test]
fn test_openmedia_animate_and_video_coerce_string_numbers() {
    let anim = normalize_openmedia_arguments(
        "openmedia_animate_svg",
        &json!({
            "width": "500",
            "height": "500",
            "fps": "24",
            "duration": "3.5"
        }),
    );
    assert_eq!(anim["width"], 500);
    assert_eq!(anim["height"], 500);
    assert_eq!(anim["fps"], 24);
    assert_eq!(anim["duration"], 3.5);

    let slideshow = normalize_openmedia_arguments(
        "openmedia_video_create_slideshow",
        &json!({
            "images": ["/tmp/a.png"],
            "width": "640",
            "height": "360",
            "fps": "15",
            "duration_per_image": "2.0"
        }),
    );
    assert_eq!(slideshow["width"], 640);
    assert_eq!(slideshow["height"], 360);
    assert_eq!(slideshow["fps"], 15);
    assert_eq!(slideshow["duration_per_image"], 2.0);

    let trim = normalize_openmedia_arguments(
        "openmedia_video_trim",
        &json!({
            "video_path": "/tmp/test.mp4",
            "start_time": "1.5",
            "end_time": "5.0"
        }),
    );
    assert_eq!(trim["start_time"], 1.5);
    assert_eq!(trim["end_time"], 5.0);
}

#[test]
fn test_openmedia_improve_feedback_normalizes_rating() {
    let f5 = normalize_openmedia_arguments(
        "openmedia_improve_feedback",
        &json!({
            "generation_id": "gen-1",
            "rating": 5
        }),
    );
    assert_eq!(f5["rating"], 1.0);

    let f4 = normalize_openmedia_arguments(
        "openmedia_improve_feedback",
        &json!({
            "generation_id": "gen-1",
            "rating": "4"
        }),
    );
    assert_eq!(f4["rating"], 0.8);

    let f09 = normalize_openmedia_arguments(
        "openmedia_improve_feedback",
        &json!({
            "generation_id": "gen-1",
            "rating": 0.9
        }),
    );
    assert_eq!(f09["rating"], 0.9);
}

#[test]
fn test_openmedia_image_batch_process_normalizes_operations() {
    let batch = normalize_openmedia_arguments(
        "openmedia_image_batch_process",
        &json!({
            "glob_pattern": "*.png",
            "output_dir": "/tmp/out",
            "operations": [
                {
                    "operation": "resize",
                    "width": 120,
                    "height": 120
                },
                {
                    "operation": "invert"
                }
            ]
        }),
    );

    let ops = batch["operations"].as_array().unwrap();
    assert_eq!(ops[0]["Resize"]["width"], 120);
    assert_eq!(ops[0]["Resize"]["height"], 120);
    assert_eq!(ops[0]["Resize"]["method"], "lanczos3");
    assert_eq!(ops[1], "Invert");
}

#[test]
fn test_openmedia_chart_and_icon_normalizations() {
    let normalized = normalize_openmedia_arguments(
        "openmedia_create_chart",
        &json!({
            "type": "bar",
            "title": "Throughput",
            "width": "600",
            "height": "400",
            "points": [
                {"name": "Before", "val": "120.5"},
                {"name": "After", "val": "25.0"}
            ]
        }),
    );

    assert_eq!(normalized["width"], 600);
    assert_eq!(normalized["height"], 400);
    assert_eq!(normalized["points"][0]["val"], 120.5);

    let req: CreateChartRequest = serde_json::from_value(normalized).unwrap();
    assert_eq!(req.chart_type, "bar");
    assert_eq!(req.title.as_deref(), Some("Throughput"));
    assert_eq!(req.data.len(), 2);
    assert_eq!(req.data[0].label, "Before");
    assert_eq!(req.data[0].value, 120.5);

    let icon_norm = normalize_openmedia_arguments(
        "openmedia_create_icon",
        &json!({
            "icon": "settings",
            "size": "48",
            "strokeWidth": "2.5"
        }),
    );
    assert_eq!(icon_norm["size"], 48);
    let icon_req: CreateIconRequest = serde_json::from_value(icon_norm).unwrap();
    assert_eq!(icon_req.name, "settings");
    assert_eq!(icon_req.size, Some(48));
    assert_eq!(icon_req.stroke_width, Some(2.5));
}

#[test]
fn test_openmedia_mermaid_and_spinner_normalizations() {
    let mermaid_norm = normalize_openmedia_arguments(
        "openmedia_diagram_generate_mermaid",
        &json!({
            "diagram": "graph LR; A --> B;",
            "format": "png",
            "width": "1024",
            "height": "768",
            "backgroundColor": "#1e1e2e"
        }),
    );
    assert_eq!(mermaid_norm["width"], 1024);
    assert_eq!(mermaid_norm["height"], 768);
    let mermaid_req: GenerateMermaidRequest = serde_json::from_value(mermaid_norm).unwrap();
    assert_eq!(mermaid_req.code, "graph LR; A --> B;");
    assert_eq!(mermaid_req.output_format.as_deref(), Some("png"));
    assert_eq!(mermaid_req.background_color.as_deref(), Some("#1e1e2e"));

    let spinner_norm = normalize_openmedia_arguments(
        "openmedia_animate_generate_spinner",
        &json!({
            "style": "dots",
            "size": "64",
            "color": "#10b981"
        }),
    );
    assert_eq!(spinner_norm["size"], 64);
    let spinner_req: GenerateSpinnerRequest = serde_json::from_value(spinner_norm).unwrap();
    assert_eq!(spinner_req.spinner_type, "dots");
    assert_eq!(spinner_req.size, Some(64));
}

#[test]
fn test_openmedia_image_operations_normalizations() {
    let filter_norm = normalize_openmedia_arguments(
        "openmedia_image_apply_filter",
        &json!({
            "path": "/tmp/photo.jpg",
            "filter": "blur",
            "radius": "4.5"
        }),
    );
    let filter_req: ImageApplyFilterRequest = serde_json::from_value(filter_norm).unwrap();
    assert_eq!(filter_req.image_path, "/tmp/photo.jpg");
    assert_eq!(filter_req.filter_type, "blur");
    assert_eq!(filter_req.parameter, Some(4.5));

    let resize_norm = normalize_openmedia_arguments(
        "openmedia_image_resize",
        &json!({
            "filePath": "/tmp/photo.jpg",
            "width": "1280",
            "height": "720"
        }),
    );
    let resize_req: ImageResizeRequest = serde_json::from_value(resize_norm).unwrap();
    assert_eq!(resize_req.image_path, "/tmp/photo.jpg");
    assert_eq!(resize_req.width, 1280);
    assert_eq!(resize_req.height, 720);
}

#[test]
fn test_openmedia_expanded_icons_available() {
    use crate::tools::openmedia::svg::icons::{get_icon_inner, get_icon_svg};

    let icons_to_test = [
        "cpu", "terminal", "database", "server", "code", "git",
        "shield", "folder", "file", "download", "upload", "refresh",
        "lock", "unlock", "eye", "copy", "check-circle", "alert-triangle", "zap", "activity",
    ];

    for name in icons_to_test {
        assert!(get_icon_inner(name).is_some(), "icon '{}' should exist", name);
        let svg = get_icon_svg(name, 24, "#000", 2.0);
        assert!(svg.is_some(), "icon svg for '{}' should generate", name);
        assert!(svg.unwrap().contains("<svg"));
    }
}

#[tokio::test]
async fn test_openmedia_hardware_detection_real() {
    let hw = crate::tools::openmedia::core::HardwareInfo::detect().await;
    assert!(hw.cpu.logical_cores > 0);
    assert!(hw.ram.total > 0);
    assert!(!hw.cpu.brand.is_empty());
}

