use crate::tools::Tool;
use anyhow::{Result, anyhow};
use serde_json::Value;
use std::fs;
use std::path::Path;
use image::{ImageBuffer, Rgb};
use imageproc::rect::Rect;
use imageproc::drawing::{
    draw_filled_rect_mut, draw_hollow_rect_mut,
    draw_filled_circle_mut, draw_hollow_circle_mut,
    draw_line_segment_mut, draw_text_mut
};
use ab_glyph::{FontArc, PxScale};

pub struct GenerateImageTool;

#[async_trait::async_trait]
impl Tool for GenerateImageTool {
    fn name(&self) -> &str {
        "generate_image"
    }

    fn description(&self) -> &str {
        "Generates a simple custom PNG image using geometric shapes (rectangles, circles, lines) and text. Runs completely locally."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "width": {
                    "type": "integer",
                    "description": "Width of the image in pixels (default: 400)",
                    "default": 400
                },
                "height": {
                    "type": "integer",
                    "description": "Height of the image in pixels (default: 400)",
                    "default": 400
                },
                "background_color": {
                    "type": "string",
                    "description": "Hex color code for the background (e.g. '#ffffff' or '#000000', default: '#ffffff')",
                    "default": "#ffffff"
                },
                "output_path": {
                    "type": "string",
                    "description": "Path where the generated PNG will be saved (default: 'output.png')",
                    "default": "output.png"
                },
                "shapes": {
                    "type": "array",
                    "description": "List of shapes and text to draw sequentially on the canvas",
                    "items": {
                        "type": "object",
                        "properties": {
                            "type": {
                                "type": "string",
                                "description": "The type of drawing operation: 'rect', 'circle', 'line', or 'text'"
                            },
                            "color": {
                                "type": "string",
                                "description": "Hex color code for this operation (e.g. '#ff0000')"
                            },
                            "fill": {
                                "type": "boolean",
                                "description": "Whether to fill the shape (only applicable for 'rect' and 'circle', default: true)"
                            },
                            "x": { "type": "integer", "description": "X coordinate for rectangle or text" },
                            "y": { "type": "integer", "description": "Y coordinate for rectangle or text" },
                            "w": { "type": "integer", "description": "Width for rectangle" },
                            "h": { "type": "integer", "description": "Height for rectangle" },
                            "cx": { "type": "integer", "description": "Center X coordinate for circle" },
                            "cy": { "type": "integer", "description": "Center Y coordinate for circle" },
                            "r": { "type": "integer", "description": "Radius for circle" },
                            "x1": { "type": "integer", "description": "Start X for line" },
                            "y1": { "type": "integer", "description": "Start Y for line" },
                            "x2": { "type": "integer", "description": "End X for line" },
                            "y2": { "type": "integer", "description": "End Y for line" },
                            "text": { "type": "string", "description": "Text content to draw" },
                            "size": { "type": "number", "description": "Font size for text (default: 16.0)" }
                        },
                        "required": ["type"]
                    }
                }
            },
            "required": ["output_path"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        let width = arguments.get("width").and_then(|v| v.as_u64()).unwrap_or(400) as u32;
        let height = arguments.get("height").and_then(|v| v.as_u64()).unwrap_or(400) as u32;
        let bg_color_str = arguments.get("background_color").and_then(|v| v.as_str()).unwrap_or("#ffffff");
        let output_path = arguments.get("output_path").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("Missing output_path"))?;

        let bg_color = parse_hex_color(bg_color_str);

        // Initialize ImageBuffer
        let mut img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(width, height);
        for pixel in img.pixels_mut() {
            *pixel = bg_color;
        }

        // Try to load system font
        let font_path = Path::new("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf");
        let font = if font_path.exists() {
            if let Ok(font_data) = fs::read(font_path) {
                FontArc::try_from_vec(font_data).ok()
            } else {
                None
            }
        } else {
            None
        };

        if let Some(shapes_val) = arguments.get("shapes").and_then(|v| v.as_array()) {
            for shape in shapes_val {
                let shape_type = shape.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let color_str = shape.get("color").and_then(|v| v.as_str()).unwrap_or("#000000");
                let color = parse_hex_color(color_str);
                let fill = shape.get("fill").and_then(|v| v.as_bool()).unwrap_or(true);

                match shape_type {
                    "rect" => {
                        let x = shape.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                        let y = shape.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                        let w = shape.get("w").and_then(|v| v.as_u64()).unwrap_or(10) as u32;
                        let h = shape.get("h").and_then(|v| v.as_u64()).unwrap_or(10) as u32;

                        if fill {
                            draw_filled_rect_mut(&mut img, Rect::at(x, y).of_size(w, h), color);
                        } else {
                            draw_hollow_rect_mut(&mut img, Rect::at(x, y).of_size(w, h), color);
                        }
                    }
                    "circle" => {
                        let cx = shape.get("cx").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                        let cy = shape.get("cy").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                        let r = shape.get("r").and_then(|v| v.as_i64()).unwrap_or(10) as i32;

                        if fill {
                            draw_filled_circle_mut(&mut img, (cx, cy), r, color);
                        } else {
                            draw_hollow_circle_mut(&mut img, (cx, cy), r, color);
                        }
                    }
                    "line" => {
                        let x1 = shape.get("x1").and_then(|v| v.as_i64()).unwrap_or(0) as f32;
                        let y1 = shape.get("y1").and_then(|v| v.as_i64()).unwrap_or(0) as f32;
                        let x2 = shape.get("x2").and_then(|v| v.as_i64()).unwrap_or(0) as f32;
                        let y2 = shape.get("y2").and_then(|v| v.as_i64()).unwrap_or(0) as f32;

                        draw_line_segment_mut(&mut img, (x1, y1), (x2, y2), color);
                    }
                    "text" => {
                        let x = shape.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                        let y = shape.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                        let text_val = shape.get("text").and_then(|v| v.as_str()).unwrap_or("");
                        let size = shape.get("size").and_then(|v| v.as_f64()).unwrap_or(16.0) as f32;

                        if let Some(ref f) = font {
                            let scale = PxScale::from(size);
                            draw_text_mut(&mut img, color, x, y, scale, f, text_val);
                        }
                    }
                    _ => {}
                }
            }
        }

        // Save image to output_path
        img.save(output_path)?;

        Ok(serde_json::json!({
            "status": "success",
            "message": format!("Image successfully generated and saved to {}", output_path),
            "output_path": output_path
        }))
    }
}

fn parse_hex_color(hex: &str) -> Rgb<u8> {
    let hex = hex.trim().trim_start_matches('#');
    if hex.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..2], 16),
            u8::from_str_radix(&hex[2..4], 16),
            u8::from_str_radix(&hex[4..6], 16),
        ) {
            return Rgb([r, g, b]);
        }
    } else if hex.len() == 3 {
        let r_str = format!("{}{}", &hex[0..1], &hex[0..1]);
        let g_str = format!("{}{}", &hex[1..2], &hex[1..2]);
        let b_str = format!("{}{}", &hex[2..3], &hex[2..3]);
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&r_str, 16),
            u8::from_str_radix(&g_str, 16),
            u8::from_str_radix(&b_str, 16),
        ) {
            return Rgb([r, g, b]);
        }
    }
    Rgb([0, 0, 0])
}
