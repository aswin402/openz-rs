use crate::ResizeMethod;
use image::DynamicImage;

pub fn resize_image(img: &DynamicImage, w: u32, h: u32, method: ResizeMethod) -> DynamicImage {
    let filter = match method {
        ResizeMethod::Nearest => image::imageops::FilterType::Nearest,
        ResizeMethod::Bilinear => image::imageops::FilterType::Triangle,
        ResizeMethod::Lanczos3 => image::imageops::FilterType::Lanczos3,
    };
    img.resize_exact(w, h, filter)
}

pub fn crop_image(img: &DynamicImage, x: u32, y: u32, w: u32, h: u32) -> DynamicImage {
    img.crop_imm(x, y, w, h)
}
