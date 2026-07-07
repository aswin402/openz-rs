use openmedia_process::{batch_process_files, FilterChain, ProcessOperation};

#[tokio::test]
async fn test_batch_processing() {
    let temp_dir = tempfile::tempdir().unwrap();
    let img_path = temp_dir.path().join("img1.png");

    let mut bytes = Vec::new();
    let img = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        10,
        10,
        image::Rgba([0, 0, 0, 255]),
    ));
    img.write_to(
        &mut std::io::Cursor::new(&mut bytes),
        image::ImageFormat::Png,
    )
    .unwrap();
    std::fs::write(&img_path, bytes).unwrap();

    let mut chain = FilterChain::new();
    chain.add(ProcessOperation::Invert);

    let output_dir = temp_dir.path().join("output");
    let pattern = format!("{}/*.png", temp_dir.path().to_str().unwrap());
    let processed = batch_process_files(&pattern, &chain, &output_dir)
        .await
        .unwrap();
    assert_eq!(processed.len(), 1);
    assert!(processed[0].exists());
}
