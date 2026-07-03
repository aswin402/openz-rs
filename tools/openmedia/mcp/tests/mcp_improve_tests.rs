use openmedia_mcp::{
    OpenMediaServer, Parameters, ImproveScoreImageRequest, ImproveRefinePromptRequest,
    ImproveAutoRefineRequest, ImproveFeedbackRequest, ImproveQualityReportRequest,
};
use openmedia_core::Config;

#[tokio::test]
async fn test_mcp_self_improvement_tools() {
    let mut config = Config::default();
    let temp_dir = tempfile::tempdir().unwrap();
    config.paths.model_dir = temp_dir.path().join("models");
    config.paths.output_dir = temp_dir.path().join("output");
    config.paths.history_db = temp_dir.path().join("history.db");

    let server = OpenMediaServer::new(config).await.unwrap();

    // Create a dummy image to score
    let image_path = temp_dir.path().join("test_image.png");
    let img = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(64, 64, image::Rgba([200, 200, 200, 255])));
    let mut bytes = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
    std::fs::write(&image_path, &bytes).unwrap();

    // Test 1: improve_score_image
    let score_params = Parameters(ImproveScoreImageRequest {
        image_path: image_path.to_str().unwrap().to_string(),
        prompt: Some("a beautiful red circle".to_string()),
    });
    let res = server.improve_score_image(score_params).await.unwrap();
    let val = res.0;
    assert!(val.get("clip_score").is_some());
    assert!(val.get("aesthetic_score").is_some());
    assert!(val.get("needs_refinement").is_some());

    // Test 2: improve_refine_prompt
    let refine_params = Parameters(ImproveRefinePromptRequest {
        prompt: "a cat".to_string(),
        negative_prompt: Some("blurry".to_string()),
        clip_score: Some(0.18),
        aesthetic_score: Some(3.5),
        round: Some(0),
    });
    let res = server.improve_refine_prompt(refine_params).await.unwrap();
    let val = res.0;
    assert!(val.get("prompt").unwrap().as_str().unwrap().contains("cat"));
    assert!(val.get("prompt").unwrap().as_str().unwrap().contains("detailed"));
    assert!(val.get("suggested_steps").is_some());

    // Test 3: improve_auto_refine
    let auto_refine_params = Parameters(ImproveAutoRefineRequest {
        prompt: "a sunset over mountains".to_string(),
        negative_prompt: None,
        width: Some(128),
        height: Some(128),
        max_iterations: Some(2),
    });
    let res = server.improve_auto_refine(auto_refine_params).await.unwrap();
    let val = res.0;
    let out: openmedia_improve::GenerationRecord = serde_json::from_value(val.into()).unwrap();
    assert_eq!(out.tool_name, "improve_auto_refine");
    assert_eq!(out.width, Some(128));
    assert_eq!(out.height, Some(128));
    assert!(std::path::Path::new(&out.output_path).exists());

    // Test 4: improve_feedback
    let feedback_params = Parameters(ImproveFeedbackRequest {
        generation_id: out.id.clone(),
        rating: 0.9,
        feedback: Some("Excellent auto refinement results!".to_string()),
        keep: Some(true),
    });
    let res = server.improve_feedback(feedback_params).await.unwrap();
    let val = res.0;
    assert_eq!(val.get("status").unwrap().as_str().unwrap(), "success");

    // Test 5: improve_quality_report
    let report_params = Parameters(ImproveQualityReportRequest {
        tool_name: Some("improve_auto_refine".to_string()),
    });
    let res = server.improve_quality_report(report_params).await.unwrap();
    let val = res.0;
    assert_eq!(val.get("total_generations").unwrap().as_u64().unwrap(), 2); // 2 iterations run in auto_refine
    assert!(val.get("avg_clip_score").is_some());
    assert!(val.get("avg_aesthetic_score").is_some());
    assert!(!val.get("recent_records").unwrap().as_array().unwrap().is_empty());
}
