use super::*;

#[tokio::test]
async fn test_create_animated_svg_basic() -> Result<()> {
    let tool = SvgAnimatorTool;
    let temp_dir =
        std::env::temp_dir().join(format!("openz_svg_test_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir)?;
    let output_file = temp_dir.join("test_anim.svg");

    let args = json!({
        "width": 300,
        "height": 300,
        "background": "#1a1a2e",
        "title": "Test Animated SVG",
        "output_path": output_file.to_str().unwrap(),
        "defs": [
            {
                "type": "linearGradient",
                "id": "grad1",
                "x1": "0%", "y1": "0%", "x2": "100%", "y2": "0%",
                "stops": [
                    { "offset": "0%", "color": "#4A90E2", "opacity": "1" },
                    { "offset": "100%", "color": "#E74C3C", "opacity": "1" }
                ]
            }
        ],
        "elements": [
            {
                "shape": "circle",
                "cx": "150",
                "cy": "150",
                "r": "60",
                "fill": "url(#grad1)",
                "animations": [
                    { "type": "pulse", "dur": "1.5s", "repeat": "indefinite", "from_scale": "0.9", "to_scale": "1.1" }
                ]
            },
            {
                "shape": "text",
                "x": "150",
                "y": "250",
                "content": "Hello SVG!",
                "font_size": "20",
                "fill": "#ffffff",
                "text_anchor": "middle",
                "animations": [
                    { "type": "fade_in", "dur": "2s", "repeat": "1" }
                ]
            }
        ]
    });

    let res = tool.call(&args).await?;
    assert_eq!(res["status"], "success");
    assert!(output_file.exists());

    let content = fs::read_to_string(&output_file)?;
    assert!(content.contains("<svg"));
    assert!(content.contains("<circle"));
    assert!(content.contains("animateTransform"));
    assert!(content.contains("animate"));
    assert!(content.contains("dominant-baseline=\"middle\""));

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[tokio::test]
async fn test_create_svg_with_path_draw_on() -> Result<()> {
    let tool = SvgAnimatorTool;
    let temp_dir =
        std::env::temp_dir().join(format!("openz_svg_draw_test_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir)?;
    let output_file = temp_dir.join("draw_on.svg");

    let args = json!({
        "width": 400,
        "height": 200,
        "background": "#f0f0f0",
        "output_path": output_file.to_str().unwrap(),
        "elements": [
            {
                "shape": "path",
                "d": "M10,100 Q100,20 200,100 T390,100",
                "stroke": "#E74C3C",
                "stroke_width": "3",
                "fill": "none",
                "stroke_dasharray": "500",
                "stroke_dashoffset": "500",
                "animations": [
                    { "type": "stroke_dash", "dur": "3s", "repeat": "indefinite", "length": "500" }
                ]
            }
        ]
    });

    let res = tool.call(&args).await?;
    assert_eq!(res["status"], "success");
    assert!(output_file.exists());

    let content = fs::read_to_string(&output_file)?;
    assert!(content.contains("stroke-dashoffset"));

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[tokio::test]
async fn test_create_svg_raw_string() -> Result<()> {
    let tool = SvgAnimatorTool;
    let temp_dir =
        std::env::temp_dir().join(format!("openz_svg_raw_test_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir)?;
    let output_file = temp_dir.join("raw.svg");

    let args = json!({
        "width": 200,
        "height": 200,
        "background": "#ff00ff",
        "output_path": output_file.to_str().unwrap(),
        "raw_svg": "<circle cx=\"100\" cy=\"100\" r=\"50\" fill=\"white\" />"
    });

    let res = tool.call(&args).await?;
    assert_eq!(res["status"], "success");
    assert!(output_file.exists());

    let content = fs::read_to_string(&output_file)?;
    assert!(content.contains("<svg"));
    assert!(content.contains("fill=\"#ff00ff\""));
    assert!(content.contains("<circle cx=\"100\""));

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}
