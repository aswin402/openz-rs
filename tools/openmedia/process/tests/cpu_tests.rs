use image::{DynamicImage, RgbaImage};
use openmedia_process::{apply_cpu_operation, ProcessOperation};

#[test]
fn test_grayscale_and_invert() {
    let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(
        10,
        10,
        image::Rgba([100, 150, 200, 255]),
    ));
    let gray = apply_cpu_operation(&img, &ProcessOperation::Grayscale).unwrap();
    assert_eq!(gray.width(), 10);

    let inverted = apply_cpu_operation(&img, &ProcessOperation::Invert).unwrap();
    let inverted_rgba = inverted.to_rgba8();
    assert_eq!(inverted_rgba.get_pixel(0, 0)[0], 155); // 255 - 100
}
