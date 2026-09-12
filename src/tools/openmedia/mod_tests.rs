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
