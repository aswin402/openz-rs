#[test]
fn test_json_svg_generation() {
    let elements_json = serde_json::json!([
        {"type": "rect", "x": 10.0, "y": 10.0, "width": 100.0, "height": 50.0, "fill": "blue"},
        {"type": "circle", "cx": 50.0, "cy": 50.0, "r": 30.0, "fill": "red"}
    ]);
    let svg = openmedia_svg::build_svg_from_json(800, 600, &elements_json).unwrap();
    assert!(svg.contains("rect x=\"10\""));
    assert!(svg.contains("fill=\"blue\""));
    assert!(svg.contains("circle cx=\"50\""));
}
