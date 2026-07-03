use openmedia_process::{apply_gpu_operation, ProcessOperation};
use image::{DynamicImage, RgbaImage};

#[test]
fn test_gpu_invert() {
    let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(16, 16, image::Rgba([100, 100, 100, 255])));
    match apply_gpu_operation(&img, &ProcessOperation::Invert) {
        Ok(inverted) => {
            let inverted_rgba = inverted.to_rgba8();
            assert_eq!(inverted_rgba.get_pixel(0, 0)[0], 155);
        }
        Err(openmedia_core::OpenMediaError::GpuError(msg)) => {
            eprintln!("GPU not available for test: {}. Skipping test verification.", msg);
        }
        Err(e) => {
            panic!("Unexpected error in GPU test: {:?}", e);
        }
    }
}
