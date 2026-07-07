use crate::{ProcessOperation, ResizeMethod};
use image::{DynamicImage, ImageBuffer, Rgba};
use openmedia_core::{OpenMediaError, Result};
use rayon::prelude::*;

pub fn apply_cpu_operation(img: &DynamicImage, op: &ProcessOperation) -> Result<DynamicImage> {
    match op {
        ProcessOperation::Grayscale => Ok(img.grayscale()),
        ProcessOperation::Invert => {
            let mut rgba = img.to_rgba8();
            rgba.par_chunks_mut(4).for_each(|pixel| {
                pixel[0] = 255 - pixel[0];
                pixel[1] = 255 - pixel[1];
                pixel[2] = 255 - pixel[2];
            });
            Ok(DynamicImage::ImageRgba8(rgba))
        }
        ProcessOperation::GaussianBlur { radius, .. } => {
            let rgba = img.to_rgba8();
            let blurred = imageproc::filter::gaussian_blur_f32(&rgba, *radius);
            Ok(DynamicImage::ImageRgba8(blurred))
        }
        ProcessOperation::BoxBlur { radius } => {
            let rgba = img.to_rgba8();
            let width = rgba.width();
            let height = rgba.height();
            let mut r_buf = ImageBuffer::new(width, height);
            let mut g_buf = ImageBuffer::new(width, height);
            let mut b_buf = ImageBuffer::new(width, height);
            let mut a_buf = ImageBuffer::new(width, height);

            for (x, y, pixel) in rgba.enumerate_pixels() {
                r_buf.put_pixel(x, y, image::Luma([pixel[0]]));
                g_buf.put_pixel(x, y, image::Luma([pixel[1]]));
                b_buf.put_pixel(x, y, image::Luma([pixel[2]]));
                a_buf.put_pixel(x, y, image::Luma([pixel[3]]));
            }

            let r_blur = imageproc::filter::box_filter(&r_buf, *radius, *radius);
            let g_blur = imageproc::filter::box_filter(&g_buf, *radius, *radius);
            let b_blur = imageproc::filter::box_filter(&b_buf, *radius, *radius);
            let a_blur = imageproc::filter::box_filter(&a_buf, *radius, *radius);

            let mut blurred = ImageBuffer::new(width, height);
            for y in 0..height {
                for x in 0..width {
                    let r = r_blur.get_pixel(x, y)[0];
                    let g = g_blur.get_pixel(x, y)[0];
                    let b = b_blur.get_pixel(x, y)[0];
                    let a = a_blur.get_pixel(x, y)[0];
                    blurred.put_pixel(x, y, image::Rgba([r, g, b, a]));
                }
            }
            Ok(DynamicImage::ImageRgba8(blurred))
        }
        ProcessOperation::Sharpen {
            radius, threshold, ..
        } => Ok(img.unsharpen(*radius, *threshold as i32)),
        ProcessOperation::UnsharpMask {
            radius, threshold, ..
        } => Ok(img.unsharpen(*radius, *threshold as i32)),
        ProcessOperation::Brightness { value } => {
            let mut rgba = img.to_rgba8();
            rgba.par_chunks_mut(4).for_each(|pixel| {
                pixel[0] = (pixel[0] as i32 + value).clamp(0, 255) as u8;
                pixel[1] = (pixel[1] as i32 + value).clamp(0, 255) as u8;
                pixel[2] = (pixel[2] as i32 + value).clamp(0, 255) as u8;
            });
            Ok(DynamicImage::ImageRgba8(rgba))
        }
        ProcessOperation::Contrast { value } => {
            let mut rgba = img.to_rgba8();
            let factor = (259.0 * (*value as f32 + 255.0)) / (255.0 * (259.0 - *value as f32));
            rgba.par_chunks_mut(4).for_each(|pixel| {
                pixel[0] = (((pixel[0] as f32 - 128.0) * factor) + 128.0).clamp(0.0, 255.0) as u8;
                pixel[1] = (((pixel[1] as f32 - 128.0) * factor) + 128.0).clamp(0.0, 255.0) as u8;
                pixel[2] = (((pixel[2] as f32 - 128.0) * factor) + 128.0).clamp(0.0, 255.0) as u8;
            });
            Ok(DynamicImage::ImageRgba8(rgba))
        }
        ProcessOperation::Saturation { value } => {
            let mut rgba = img.to_rgba8();
            rgba.par_chunks_mut(4).for_each(|pixel| {
                let r = pixel[0] as f32;
                let g = pixel[1] as f32;
                let b = pixel[2] as f32;
                let gray = r * 0.299 + g * 0.587 + b * 0.114;
                let sat_factor = 1.0 + (*value as f32 / 100.0);
                pixel[0] = (gray + (r - gray) * sat_factor).clamp(0.0, 255.0) as u8;
                pixel[1] = (gray + (g - gray) * sat_factor).clamp(0.0, 255.0) as u8;
                pixel[2] = (gray + (b - gray) * sat_factor).clamp(0.0, 255.0) as u8;
            });
            Ok(DynamicImage::ImageRgba8(rgba))
        }
        ProcessOperation::HueRotate { degrees } => Ok(img.huerotate(*degrees as i32)),
        ProcessOperation::Sepia { intensity } => {
            let mut rgba = img.to_rgba8();
            rgba.par_chunks_mut(4).for_each(|pixel| {
                let r = pixel[0] as f32;
                let g = pixel[1] as f32;
                let b = pixel[2] as f32;
                let sr = (r * 0.393 + g * 0.769 + b * 0.189).min(255.0);
                let sg = (r * 0.349 + g * 0.686 + b * 0.168).min(255.0);
                let sb = (r * 0.272 + g * 0.534 + b * 0.131).min(255.0);
                pixel[0] = (r + (sr - r) * intensity).clamp(0.0, 255.0) as u8;
                pixel[1] = (g + (sg - g) * intensity).clamp(0.0, 255.0) as u8;
                pixel[2] = (b + (sb - b) * intensity).clamp(0.0, 255.0) as u8;
            });
            Ok(DynamicImage::ImageRgba8(rgba))
        }
        ProcessOperation::Threshold { value } => {
            let mut rgba = img.to_rgba8();
            rgba.par_chunks_mut(4).for_each(|pixel| {
                let luma = (pixel[0] as f32 * 0.299
                    + pixel[1] as f32 * 0.587
                    + pixel[2] as f32 * 0.114) as u8;
                let val = if luma >= *value { 255 } else { 0 };
                pixel[0] = val;
                pixel[1] = val;
                pixel[2] = val;
            });
            Ok(DynamicImage::ImageRgba8(rgba))
        }
        ProcessOperation::ColorMatrix { matrix } => {
            let mut rgba = img.to_rgba8();
            rgba.par_chunks_mut(4).for_each(|pixel| {
                let r = pixel[0] as f32;
                let g = pixel[1] as f32;
                let b = pixel[2] as f32;
                let a = pixel[3] as f32;

                let nr = r * matrix[0][0]
                    + g * matrix[0][1]
                    + b * matrix[0][2]
                    + a * matrix[0][3]
                    + matrix[0][4] * 255.0;
                let ng = r * matrix[1][0]
                    + g * matrix[1][1]
                    + b * matrix[1][2]
                    + a * matrix[1][3]
                    + matrix[1][4] * 255.0;
                let nb = r * matrix[2][0]
                    + g * matrix[2][1]
                    + b * matrix[2][2]
                    + a * matrix[2][3]
                    + matrix[2][4] * 255.0;
                let na = r * matrix[3][0]
                    + g * matrix[3][1]
                    + b * matrix[3][2]
                    + a * matrix[3][3]
                    + matrix[3][4] * 255.0;

                pixel[0] = nr.clamp(0.0, 255.0) as u8;
                pixel[1] = ng.clamp(0.0, 255.0) as u8;
                pixel[2] = nb.clamp(0.0, 255.0) as u8;
                pixel[3] = na.clamp(0.0, 255.0) as u8;
            });
            Ok(DynamicImage::ImageRgba8(rgba))
        }
        ProcessOperation::Resize {
            width,
            height,
            method,
        } => {
            let filter = match method {
                ResizeMethod::Nearest => image::imageops::FilterType::Nearest,
                ResizeMethod::Bilinear => image::imageops::FilterType::Triangle,
                ResizeMethod::Lanczos3 => image::imageops::FilterType::Lanczos3,
            };
            Ok(img.resize_exact(*width, *height, filter))
        }
        ProcessOperation::Crop {
            x,
            y,
            width,
            height,
        } => {
            let rgba = img.to_rgba8();
            let cropped = image::imageops::crop_imm(&rgba, *x, *y, *width, *height).to_image();
            Ok(DynamicImage::ImageRgba8(cropped))
        }
        ProcessOperation::Rotate { angle, .. } => {
            let rotated = match *angle as i64 {
                90 => img.rotate90(),
                180 => img.rotate180(),
                270 => img.rotate270(),
                _ => img.rotate90(), // Simplistic arbitrary angle fallback for core operations
            };
            Ok(rotated)
        }
        ProcessOperation::FlipHorizontal => Ok(img.fliph()),
        ProcessOperation::FlipVertical => Ok(img.flipv()),
        ProcessOperation::Pad {
            top,
            right,
            bottom,
            left,
            color,
        } => {
            let w = img.width() + left + right;
            let h = img.height() + top + bottom;
            let mut padded = ImageBuffer::from_pixel(w, h, Rgba(*color));
            image::imageops::overlay(&mut padded, img, *left as i64, *top as i64);
            Ok(DynamicImage::ImageRgba8(padded))
        }
        ProcessOperation::Composite { overlay, x, y, .. } => {
            let overlay_img = image::open(overlay).map_err(|e| {
                OpenMediaError::IoError(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    e.to_string(),
                ))
            })?;
            let mut base = img.to_rgba8();
            image::imageops::overlay(&mut base, &overlay_img, *x as i64, *y as i64);
            Ok(DynamicImage::ImageRgba8(base))
        }
    }
}
