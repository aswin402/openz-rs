use openmedia_process::{resize_image, ResizeMethod, write_image_with_format};
use image::{DynamicImage, RgbaImage};

#[test]
fn test_resize_and_avif_encoding() {
    let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(100, 100, image::Rgba([128, 128, 128, 255])));
    let resized = resize_image(&img, 50, 50, ResizeMethod::Bilinear);
    assert_eq!(resized.width(), 50);
    
    let bytes = write_image_with_format(&resized, "avif", 80).unwrap();
    assert!(!bytes.is_empty());

    let png_bytes = write_image_with_format(&resized, "png", 100).unwrap();
    assert!(!png_bytes.is_empty());

    let jpeg_bytes = write_image_with_format(&resized, "jpeg", 80).unwrap();
    assert!(!jpeg_bytes.is_empty());

    let webp_bytes = write_image_with_format(&resized, "webp", 80).unwrap();
    assert!(!webp_bytes.is_empty());
}
