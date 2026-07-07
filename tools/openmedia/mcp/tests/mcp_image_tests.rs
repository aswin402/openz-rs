use openmedia_core::Config;
use openmedia_mcp::{
    ImageApplyFilterRequest, ImageBatchProcessRequest, ImageConvertRequest, ImageCropRequest,
    ImageResizeRequest, ImageTransformRequest, OpenMediaServer, Parameters,
};

#[tokio::test]
async fn test_mcp_image_processing_tools() {
    let mut config = Config::default();
    let temp_dir = tempfile::tempdir().unwrap();
    config.paths.model_dir = temp_dir.path().join("models");
    config.paths.output_dir = temp_dir.path().join("output");
    config.paths.history_db = temp_dir.path().join("history.db");

    let server = OpenMediaServer::new(config).await.unwrap();

    // Create a dummy input image
    let input_path = temp_dir.path().join("input.png");
    let mut bytes = Vec::new();
    let img = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        32,
        32,
        image::Rgba([100, 150, 200, 255]),
    ));
    img.write_to(
        &mut std::io::Cursor::new(&mut bytes),
        image::ImageFormat::Png,
    )
    .unwrap();
    std::fs::write(&input_path, &bytes).unwrap();

    // Test 1: image_apply_filter (grayscale)
    let filter_params = Parameters(ImageApplyFilterRequest {
        image_path: input_path.to_str().unwrap().to_string(),
        filter_type: "grayscale".to_string(),
        parameter: None,
    });
    let res = server.image_apply_filter(filter_params).await.unwrap();
    let out: openmedia_core::ImageOutput = serde_json::from_value(res.0.into()).unwrap();
    assert_eq!(out.width, 32);
    assert_eq!(out.height, 32);
    assert!(out.path.exists());

    // Test 2: image_resize (to 16x16)
    let resize_params = Parameters(ImageResizeRequest {
        image_path: input_path.to_str().unwrap().to_string(),
        width: 16,
        height: 16,
        algorithm: Some("nearest".to_string()),
    });
    let res = server.image_resize(resize_params).await.unwrap();
    let out: openmedia_core::ImageOutput = serde_json::from_value(res.0.into()).unwrap();
    assert_eq!(out.width, 16);
    assert_eq!(out.height, 16);
    assert!(out.path.exists());

    // Test 3: image_crop (10x10 at 5,5)
    let crop_params = Parameters(ImageCropRequest {
        image_path: input_path.to_str().unwrap().to_string(),
        x: 5,
        y: 5,
        width: 10,
        height: 10,
    });
    let res = server.image_crop(crop_params).await.unwrap();
    let out: openmedia_core::ImageOutput = serde_json::from_value(res.0.into()).unwrap();
    assert_eq!(out.width, 10);
    assert_eq!(out.height, 10);
    assert!(out.path.exists());

    // Test 4: image_transform (rotate 90)
    let transform_params = Parameters(ImageTransformRequest {
        image_path: input_path.to_str().unwrap().to_string(),
        transform_type: "rotate".to_string(),
        angle: Some(90.0),
    });
    let res = server.image_transform(transform_params).await.unwrap();
    let out: openmedia_core::ImageOutput = serde_json::from_value(res.0.into()).unwrap();
    // Rotating 32x32 yields 32x32
    assert_eq!(out.width, 32);
    assert_eq!(out.height, 32);
    assert!(out.path.exists());

    // Test 5: image_convert (to jpeg)
    let convert_params = Parameters(ImageConvertRequest {
        image_path: input_path.to_str().unwrap().to_string(),
        format: "jpeg".to_string(),
        quality: Some(85),
    });
    let res = server.image_convert(convert_params).await.unwrap();
    let out: openmedia_core::ImageOutput = serde_json::from_value(res.0.into()).unwrap();
    assert_eq!(out.format, "jpeg");
    assert!(out.path.exists());

    // Test 6: image_batch_process (invert filter)
    let batch_output_dir = temp_dir.path().join("batch_output");
    let batch_params = Parameters(ImageBatchProcessRequest {
        glob_pattern: format!("{}/*.png", temp_dir.path().to_str().unwrap()),
        operations: vec![
            serde_json::to_value(openmedia_process::ProcessOperation::Invert).unwrap(),
        ],
        output_dir: batch_output_dir.to_str().unwrap().to_string(),
    });
    let res = server.image_batch_process(batch_params).await.unwrap();
    let outs: Vec<openmedia_core::ImageOutput> = serde_json::from_value(res.0.into()).unwrap();
    assert!(!outs.is_empty());
    assert!(outs[0].path.exists());
}
