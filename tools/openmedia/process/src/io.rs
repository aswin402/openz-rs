use image::DynamicImage;
use openmedia_core::{Result, OpenMediaError};

pub fn write_image_with_format(img: &DynamicImage, format: &str, quality: u8) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    match format.to_lowercase().as_str() {
        "png" => {
            let encoder = image::codecs::png::PngEncoder::new(&mut bytes);
            img.write_with_encoder(encoder)
                .map_err(|e| OpenMediaError::ImageEncodeError {
                    format: "png".to_string(),
                    reason: e.to_string(),
                })?;
        }
        "jpeg" | "jpg" => {
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, quality);
            img.write_with_encoder(encoder)
                .map_err(|e| OpenMediaError::ImageEncodeError {
                    format: "jpeg".to_string(),
                    reason: e.to_string(),
                })?;
        }
        "webp" => {
            let encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut bytes);
            img.write_with_encoder(encoder)
                .map_err(|e| OpenMediaError::ImageEncodeError {
                    format: "webp".to_string(),
                    reason: e.to_string(),
                })?;
        }
        "avif" => {
            img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Avif)
                .map_err(|e| OpenMediaError::ImageEncodeError {
                    format: "avif".to_string(),
                    reason: e.to_string(),
                })?;
        }
        _ => return Err(OpenMediaError::ImageEncodeError {
            format: format.to_string(),
            reason: "Unsupported format".to_string(),
        }),
    }
    Ok(bytes)
}
