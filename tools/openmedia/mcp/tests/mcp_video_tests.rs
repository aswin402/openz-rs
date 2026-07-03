use openmedia_mcp::{
    OpenMediaServer, Parameters, VideoCreateRequest, VideoCreateSlideshowRequest,
    VideoAddTransitionRequest, VideoAddAudioRequest, VideoFromTemplateRequest,
    VideoPreviewRequest, VideoExtractFramesRequest, VideoTrimRequest,
};
use openmedia_core::Config;

#[tokio::test]
async fn test_mcp_video_generation_tools() {
    let mut config = Config::default();
    let temp_dir = tempfile::tempdir().unwrap();
    config.paths.model_dir = temp_dir.path().join("models");
    config.paths.output_dir = temp_dir.path().join("output");
    config.paths.history_db = temp_dir.path().join("history.db");

    let server = OpenMediaServer::new(config).await.unwrap();

    // Create a dummy video scene JSON
    let scene_json = serde_json::json!({
        "width": 320,
        "height": 240,
        "fps": 5,
        "duration": 2.0,
        "background": "#000000",
        "scenes": [
            {
                "id": "scene_1",
                "start": 0.0,
                "end": 2.0,
                "elements": [
                    {
                        "type": "text",
                        "content": "Hello OpenMedia",
                        "style": {
                            "font_family": "sans-serif",
                            "font_size": 24.0,
                            "font_weight": 400,
                            "color": "#ffffff",
                            "text_align": "center"
                        },
                        "position": { "x": 160.0, "y": 120.0 },
                        "anchor": "center",
                        "timeline": null
                    }
                ]
            }
        ],
        "transitions": [],
        "audio": null
    });

    // Test 1: video_create
    let create_params = Parameters(VideoCreateRequest {
        scene: scene_json.clone(),
        output_path: None,
    });
    let res = server.video_create(create_params).await.unwrap();
    let out: openmedia_core::VideoSpec = serde_json::from_value(res.0.into()).unwrap();
    assert_eq!(out.width, 320);
    assert_eq!(out.height, 240);
    assert!(out.path.exists());

    // Test 2: video_preview
    let preview_params = Parameters(VideoPreviewRequest {
        scene: scene_json.clone(),
        time: Some(0.5),
        width: Some(160),
        height: Some(120),
        output_format: Some("png".to_string()),
    });
    let res = server.video_preview(preview_params).await.unwrap();
    let out_img: openmedia_core::ImageOutput = serde_json::from_value(res.0.into()).unwrap();
    assert_eq!(out_img.width, 160);
    assert_eq!(out_img.height, 120);
    assert!(out_img.path.exists());

    // Create a dummy image for slideshow test
    let slideshow_img_path = temp_dir.path().join("slide1.png");
    let mut img_bytes = Vec::new();
    let img = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(320, 240, image::Rgba([50, 100, 150, 255])));
    img.write_to(&mut std::io::Cursor::new(&mut img_bytes), image::ImageFormat::Png).unwrap();
    std::fs::write(&slideshow_img_path, &img_bytes).unwrap();

    // Test 3: video_create_slideshow
    let slideshow_params = Parameters(VideoCreateSlideshowRequest {
        images: vec![slideshow_img_path.to_str().unwrap().to_string()],
        duration_per_image: Some(1.0),
        transition_type: Some("crossfade".to_string()),
        transition_duration: Some(0.2),
        audio_src: None,
        width: Some(320),
        height: Some(240),
        fps: Some(5),
        output_path: None,
    });
    let res = server.video_create_slideshow(slideshow_params).await.unwrap();
    let out: openmedia_core::VideoSpec = serde_json::from_value(res.0.into()).unwrap();
    assert_eq!(out.width, 320);
    assert_eq!(out.height, 240);
    assert!(out.path.exists());

    // Test 4: video_from_template (slideshow template)
    let template_params = Parameters(VideoFromTemplateRequest {
        template_name: "slideshow".to_string(),
        parameters: serde_json::json!({
            "images": [slideshow_img_path.to_str().unwrap().to_string()],
            "duration_per_image": 1.0,
            "width": 320,
            "height": 240,
            "fps": 5
        }),
        output_path: None,
    });
    let res = server.video_from_template(template_params).await.unwrap();
    let out: openmedia_core::VideoSpec = serde_json::from_value(res.0.into()).unwrap();
    assert!(out.path.exists());

    // Test 5: video_extract_frames
    let extracted_dir = temp_dir.path().join("extracted");
    std::fs::create_dir_all(&extracted_dir).unwrap();
    let extract_params = Parameters(VideoExtractFramesRequest {
        video_path: out.path.to_str().unwrap().to_string(),
        offsets: vec![0.1, 0.5],
        output_dir: extracted_dir.to_str().unwrap().to_string(),
        format: Some("png".to_string()),
    });
    let res = server.video_extract_frames(extract_params).await.unwrap();
    let val = res.0;
    assert!(val.as_array().unwrap().len() >= 2);

    // Test 6: video_trim
    let trimmed_path = temp_dir.path().join("trimmed.mp4");
    let trim_params = Parameters(VideoTrimRequest {
        video_path: out.path.to_str().unwrap().to_string(),
        start_time: 0.1,
        end_time: 0.8,
        output_path: Some(trimmed_path.to_str().unwrap().to_string()),
    });
    let res = server.video_trim(trim_params).await.unwrap();
    let out_trimmed: openmedia_core::VideoSpec = serde_json::from_value(res.0.into()).unwrap();
    assert!(out_trimmed.path.exists());

    // Test 7: video_add_transition and video_add_audio on scene JSON file
    let scene_file_path = temp_dir.path().join("scene.json");
    std::fs::write(&scene_file_path, serde_json::to_string_pretty(&scene_json).unwrap()).unwrap();

    let add_transition_params = Parameters(VideoAddTransitionRequest {
        scene_path: scene_file_path.to_str().unwrap().to_string(),
        from_scene_id: "scene_1".to_string(),
        to_scene_id: "scene_2".to_string(),
        transition_type: "crossfade".to_string(),
        duration: Some(0.5),
    });
    let res = server.video_add_transition(add_transition_params).await.unwrap();
    let updated_scene = res.0;
    assert_eq!(updated_scene["transitions"].as_array().unwrap().len(), 1);

    let add_audio_params = Parameters(VideoAddAudioRequest {
        target_path: scene_file_path.to_str().unwrap().to_string(),
        audio_path: "dummy_audio.mp3".to_string(),
        start_time: Some(1.0),
        volume: Some(0.8),
        fade_in: None,
        fade_out: None,
        output_path: None,
    });
    let res = server.video_add_audio(add_audio_params).await.unwrap();
    let updated_scene_audio = res.0;
    assert!(updated_scene_audio["audio"].is_object());
}
