//! Shared request parsing and media-rendering helpers.
//!
//! Keeping these adapters outside the MCP server definition lets handler
//! families share compatibility behavior without growing the server module
//! into another monolith.

use crate::tools::openmedia::video::VideoScene;

pub(crate) fn parse_json_value_field(
    value: &serde_json::Value,
    field_name: &str,
) -> Result<serde_json::Value, String> {
    match value.as_str().map(str::trim) {
        Some(trimmed) => match trimmed.starts_with('{') || trimmed.starts_with('[') {
            true => serde_json::from_str(trimmed)
                .map_err(|e| format!("failed to parse {field_name} JSON string: {e}")),
            false => Ok(value.clone()),
        },
        None => Ok(value.clone()),
    }
}

pub(crate) fn parse_video_scene_value(value: &serde_json::Value) -> Result<VideoScene, String> {
    match value.as_str().map(str::trim) {
        Some(trimmed) => match trimmed.starts_with('{') {
            true => serde_json::from_str(trimmed)
                .map_err(|e| format!("failed to parse scene JSON string: {e}")),
            false => {
                let path = std::path::Path::new(trimmed);
                match path.exists() && path.is_file() {
                    true => {
                        let s = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
                        serde_json::from_str(&s).map_err(|e| {
                            format!("failed to parse scene file '{}': {e}", path.display())
                        })
                    }
                    false => Err(format!(
                        "scene string must be a VideoScene JSON object string or an existing file path: {trimmed}"
                    )),
                }
            }
        },
        None => serde_json::from_value(value.clone()).map_err(|e| e.to_string()),
    }
}

pub(crate) fn parse_easing(s: Option<&str>) -> crate::tools::openmedia::animate::Easing {
    let name = s.unwrap_or("linear");
    match name.to_lowercase().as_str() {
        "linear" => crate::tools::openmedia::animate::Easing::Linear,
        "ease_in" | "easein" | "ease-in" => crate::tools::openmedia::animate::Easing::EaseInQuad,
        "ease_out" | "easeout" | "ease-out" => crate::tools::openmedia::animate::Easing::EaseOutQuad,
        "ease_in_out" | "easeinout" | "ease-in-out" => crate::tools::openmedia::animate::Easing::EaseInOutQuad,
        "ease_in_cubic" | "ease-in-cubic" => crate::tools::openmedia::animate::Easing::EaseInCubic,
        "ease_out_cubic" | "ease-out-cubic" => crate::tools::openmedia::animate::Easing::EaseOutCubic,
        "ease_in_out_cubic" | "ease-in-out-cubic" => crate::tools::openmedia::animate::Easing::EaseInOutCubic,
        "ease_in_expo" | "ease-in-expo" => crate::tools::openmedia::animate::Easing::EaseInExpo,
        "ease_out_expo" | "ease-out-expo" => crate::tools::openmedia::animate::Easing::EaseOutExpo,
        "ease_in_out_expo" | "ease-in-out-expo" => crate::tools::openmedia::animate::Easing::EaseInOutExpo,
        "bounce" | "ease_out_bounce" | "ease-out-bounce" => {
            crate::tools::openmedia::animate::Easing::EaseOutBounce
        }
        "elastic" | "ease_out_elastic" | "ease-out-elastic" => {
            crate::tools::openmedia::animate::Easing::EaseOutElastic
        }
        "spring" => crate::tools::openmedia::animate::Easing::Spring {
            stiffness: 100.0,
            damping: 10.0,
            mass: 1.0,
        },
        _ => name
            .strip_prefix("cubic-bezier(")
            .and_then(|rest| rest.strip_suffix(')'))
            .and_then(|content| {
                let parts: Vec<&str> = content.split(',').map(|p| p.trim()).collect();
                (parts.len() == 4).then(|| {
                    let x1 = parts[0].parse::<f64>().unwrap_or(0.25);
                    let y1 = parts[1].parse::<f64>().unwrap_or(0.1);
                    let x2 = parts[2].parse::<f64>().unwrap_or(0.25);
                    let y2 = parts[3].parse::<f64>().unwrap_or(1.0);
                    crate::tools::openmedia::animate::Easing::CubicBezier(x1, y1, x2, y2)
                })
            })
            .unwrap_or(crate::tools::openmedia::animate::Easing::Linear),
    }
}

pub(crate) fn parse_preset(s: &str) -> crate::tools::openmedia::animate::AnimationPreset {
    match s.to_lowercase().as_str() {
        "fade_in" | "fadein" | "fade-in" => crate::tools::openmedia::animate::AnimationPreset::FadeIn,
        "fade_out" | "fadeout" | "fade-out" => crate::tools::openmedia::animate::AnimationPreset::FadeOut,
        "slide_in_left" | "slide-in-left" => crate::tools::openmedia::animate::AnimationPreset::SlideInLeft,
        "slide_in_right" | "slide-in-right" => crate::tools::openmedia::animate::AnimationPreset::SlideInRight,
        "slide_in_up" | "slide-in-up" => crate::tools::openmedia::animate::AnimationPreset::SlideInUp,
        "slide_in_down" | "slide-in-down" => crate::tools::openmedia::animate::AnimationPreset::SlideInDown,
        "bounce" => crate::tools::openmedia::animate::AnimationPreset::Bounce,
        "pulse" => crate::tools::openmedia::animate::AnimationPreset::Pulse,
        "spin" => crate::tools::openmedia::animate::AnimationPreset::Spin,
        "shake" => crate::tools::openmedia::animate::AnimationPreset::Shake,
        "wobble" => crate::tools::openmedia::animate::AnimationPreset::Wobble,
        "typewriter" => crate::tools::openmedia::animate::AnimationPreset::Typewriter,
        "draw_path" | "draw-path" | "drawpath" => crate::tools::openmedia::animate::AnimationPreset::DrawPath,
        "morph" => crate::tools::openmedia::animate::AnimationPreset::Morph,
        "gradient_shift" | "gradient-shift" => crate::tools::openmedia::animate::AnimationPreset::GradientShift,
        "parallax_scroll" | "parallax-scroll" => {
            crate::tools::openmedia::animate::AnimationPreset::ParallaxScroll
        }
        "stagger" => crate::tools::openmedia::animate::AnimationPreset::Stagger,
        _ => crate::tools::openmedia::animate::AnimationPreset::FadeIn,
    }
}

pub(crate) fn inject_css_class(svg: &str, element_id: &str, class_name: &str) -> String {
    let clean_id = element_id.trim_start_matches('#');
    let patterns = [format!("id=\"{}\"", clean_id), format!("id='{}'", clean_id)];

    let found_pos = patterns
        .iter()
        .find_map(|pat| svg.find(pat.as_str()).map(|pos| (pos, pat.len())));

    let (pos, _pat_len) = match found_pos {
        Some(p) => p,
        None => return svg.to_string(),
    };

    let start_tag_idx = match svg[..pos].rfind('<') {
        Some(idx) => idx,
        None => return svg.to_string(),
    };

    let end_tag_idx = match svg[pos..].find('>') {
        Some(idx) => pos + idx,
        None => return svg.to_string(),
    };

    let mut tag_content = svg[start_tag_idx..=end_tag_idx].to_string();
    let class_pat_double = "class=\"";
    let class_pat_single = "class='";

    match (
        tag_content.find(class_pat_double),
        tag_content.find(class_pat_single),
        tag_content.find(' '),
    ) {
        (Some(c_pos), _, _) => {
            let insert_idx = c_pos + class_pat_double.len();
            tag_content.insert_str(insert_idx, &format!("{} ", class_name));
        }
        (None, Some(c_pos), _) => {
            let insert_idx = c_pos + class_pat_single.len();
            tag_content.insert_str(insert_idx, &format!("{} ", class_name));
        }
        (None, None, Some(space_pos)) => {
            tag_content.insert_str(space_pos, &format!(" class=\"{}\"", class_name));
        }
        (None, None, None) => {
            let insert_pos = match tag_content.ends_with("/>") {
                true => tag_content.len() - 2,
                false => tag_content.len() - 1,
            };
            tag_content.insert_str(insert_pos, &format!(" class=\"{}\" ", class_name));
        }
    }

    let mut result = svg.to_string();
    result.replace_range(start_tag_idx..=end_tag_idx, &tag_content);
    result
}

pub(crate) fn inject_style_or_xml(mut svg: String, content_to_inject: &str) -> String {
    let lower = svg.to_lowercase();
    match lower.rfind("</svg>") {
        Some(close_idx) => svg.insert_str(close_idx, content_to_inject),
        None => svg.push_str(content_to_inject),
    }
    svg
}

fn parse_svg_attr_u32(svg: &str, attr_prefix: &str, default_val: u32) -> u32 {
    svg.find(attr_prefix)
        .and_then(|pos| {
            let start = pos + attr_prefix.len();
            let end = svg[start..].find('"')?;
            svg[start..start + end].parse::<f64>().ok().map(|v| v as u32)
        })
        .unwrap_or(default_val)
}

pub(crate) fn parse_svg_dimensions(svg: &str) -> (u32, u32) {
    (
        parse_svg_attr_u32(svg, "width=\"", 800),
        parse_svg_attr_u32(svg, "height=\"", 600),
    )
}

pub(crate) fn override_theme_fields(
    theme: &mut mermaid_rs_renderer::Theme,
    overrides: &serde_json::Value,
) {
    let map = match overrides.as_object() {
        Some(m) => m,
        None => return,
    };
    for (key, val) in map {
        match (val.as_str(), val.as_f64()) {
            (Some(val_str), _) => match key.as_str() {
                "font_family" => theme.font_family = val_str.to_string(),
                "primary_color" => theme.primary_color = val_str.to_string(),
                "primary_text_color" => theme.primary_text_color = val_str.to_string(),
                "primary_border_color" => theme.primary_border_color = val_str.to_string(),
                "line_color" => theme.line_color = val_str.to_string(),
                "secondary_color" => theme.secondary_color = val_str.to_string(),
                "tertiary_color" => theme.tertiary_color = val_str.to_string(),
                "edge_label_background" => theme.edge_label_background = val_str.to_string(),
                "cluster_background" => theme.cluster_background = val_str.to_string(),
                "cluster_border" => theme.cluster_border = val_str.to_string(),
                "background" => theme.background = val_str.to_string(),
                "sequence_actor_fill" => theme.sequence_actor_fill = val_str.to_string(),
                "sequence_actor_border" => theme.sequence_actor_border = val_str.to_string(),
                "sequence_actor_line" => theme.sequence_actor_line = val_str.to_string(),
                "sequence_note_fill" => theme.sequence_note_fill = val_str.to_string(),
                "sequence_note_border" => theme.sequence_note_border = val_str.to_string(),
                "sequence_activation_fill" => {
                    theme.sequence_activation_fill = val_str.to_string()
                }
                "sequence_activation_border" => {
                    theme.sequence_activation_border = val_str.to_string()
                }
                "text_color" => theme.text_color = val_str.to_string(),
                _ => {}
            },
            (None, Some(val_f64)) => match key.as_str() {
                "font_size" => theme.font_size = val_f64 as f32,
                _ => {}
            },
            (None, None) => {}
        }
    }
}

pub(crate) fn resolve_theme_preset(preset: &str) -> mermaid_rs_renderer::Theme {
    match preset.to_lowercase().as_str() {
        "default" | "classic" => mermaid_rs_renderer::Theme::mermaid_default(),
        "dark" => {
            let mut theme = mermaid_rs_renderer::Theme::modern();
            theme.background = "#0f172a".to_string();
            theme.primary_color = "#1e293b".to_string();
            theme.primary_text_color = "#f8fafc".to_string();
            theme.primary_border_color = "#475569".to_string();
            theme.line_color = "#94a3b8".to_string();
            theme.secondary_color = "#334155".to_string();
            theme.tertiary_color = "#0f172a".to_string();
            theme.text_color = "#f8fafc".to_string();
            theme.edge_label_background = "#1e293b".to_string();
            theme.cluster_background = "#1e293b".to_string();
            theme.cluster_border = "#334155".to_string();
            theme
        }
        "forest" => {
            let mut theme = mermaid_rs_renderer::Theme::modern();
            theme.primary_color = "#f0fdf4".to_string();
            theme.primary_text_color = "#166534".to_string();
            theme.primary_border_color = "#86efac".to_string();
            theme.line_color = "#15803d".to_string();
            theme.secondary_color = "#dcfce7".to_string();
            theme.tertiary_color = "#ffffff".to_string();
            theme.text_color = "#166534".to_string();
            theme.edge_label_background = "#ffffff".to_string();
            theme.cluster_background = "#f0fdf4".to_string();
            theme.cluster_border = "#bbf7d0".to_string();
            theme
        }
        "neutral" => {
            let mut theme = mermaid_rs_renderer::Theme::modern();
            theme.primary_color = "#f9fafb".to_string();
            theme.primary_text_color = "#111827".to_string();
            theme.primary_border_color = "#e5e7eb".to_string();
            theme.line_color = "#4b5563".to_string();
            theme.secondary_color = "#f3f4f6".to_string();
            theme.tertiary_color = "#ffffff".to_string();
            theme.text_color = "#111827".to_string();
            theme.edge_label_background = "#ffffff".to_string();
            theme.cluster_background = "#f9fafb".to_string();
            theme.cluster_border = "#d1d5db".to_string();
            theme
        }
        _ => mermaid_rs_renderer::Theme::modern(),
    }
}

pub(crate) fn parse_transition_params(
    parameters: &serde_json::Value,
    default_type: crate::tools::openmedia::video::TransitionType,
) -> (crate::tools::openmedia::video::TransitionType, f64, Option<String>) {
    let trans_type = parameters
        .get("transition_type")
        .and_then(|v| v.as_str())
        .map(|s| parse_transition_type_with_fallback(s, default_type.clone()))
        .unwrap_or(default_type);

    let duration = parameters
        .get("transition_duration")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5);

    let easing = parameters
        .get("transition_easing")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    (trans_type, duration, easing)
}

pub(crate) fn parse_audio_config(
    parameters: &serde_json::Value,
) -> Option<crate::tools::openmedia::video::AudioConfig> {
    match parameters.get("audio_tracks").and_then(|v| v.as_array()) {
        Some(tracks_arr) => {
            let tracks: Vec<crate::tools::openmedia::video::AudioTrack> = tracks_arr
                .iter()
                .filter_map(|track_val| {
                    let src = track_val.get("src").and_then(|v| v.as_str())?;
                    let start = track_val
                        .get("start")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    let volume = track_val
                        .get("volume")
                        .and_then(|v| v.as_f64())
                        .map(|v| v as f32)
                        .unwrap_or(1.0);
                    let fade_in = track_val.get("fade_in").and_then(|v| v.as_f64());
                    let fade_out = track_val.get("fade_out").and_then(|v| v.as_f64());
                    Some(crate::tools::openmedia::video::AudioTrack {
                        src: src.to_string(),
                        start,
                        volume,
                        fade_in,
                        fade_out,
                    })
                })
                .collect();
            (!tracks.is_empty()).then_some(crate::tools::openmedia::video::AudioConfig { tracks })
        }
        None => parameters
            .get("background_music")
            .and_then(|v| v.as_str())
            .map(|bg_music| crate::tools::openmedia::video::AudioConfig {
                tracks: vec![crate::tools::openmedia::video::AudioTrack {
                    src: bg_music.to_string(),
                    start: 0.0,
                    volume: 0.5,
                    fade_in: None,
                    fade_out: None,
                }],
            }),
    }
}

pub(crate) fn get_templates_dir() -> std::path::PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let openz_dir = std::env::var("OPENZ_CONFIG_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| home.join(".openz"));
    openz_dir.join("media").join("templates")
}

pub(crate) fn interpolate_template(
    template_json: &serde_json::Value,
    parameters: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let mut template_str = serde_json::to_string(template_json).map_err(|e| e.to_string())?;

    match parameters.as_object() {
        Some(obj) => {
            for (key, val) in obj {
                let placeholder = format!("{{{{{}}}}}", key);
                let replacement = match val {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                template_str = template_str.replace(&placeholder, &replacement);
            }
        }
        None => {}
    }

    serde_json::from_str(&template_str)
        .map_err(|e| format!("Failed to parse interpolated template: {}", e))
}

pub(crate) fn parse_custom_fonts(
    parameters: &serde_json::Value,
) -> Option<Vec<crate::tools::openmedia::video::CustomFontSpec>> {
    let fonts_arr = parameters.get("custom_fonts").and_then(|v| v.as_array())?;
    let specs: Vec<crate::tools::openmedia::video::CustomFontSpec> = fonts_arr
        .iter()
        .filter_map(|font_val| {
            let family = font_val.get("family").and_then(|v| v.as_str())?;
            let src = font_val.get("src").and_then(|v| v.as_str())?;
            Some(crate::tools::openmedia::video::CustomFontSpec {
                family: family.to_string(),
                src: src.to_string(),
            })
        })
        .collect();
    (!specs.is_empty()).then_some(specs)
}

pub fn parse_transition_type(s: &str) -> crate::tools::openmedia::video::TransitionType {
    parse_transition_type_with_fallback(s, crate::tools::openmedia::video::TransitionType::Crossfade)
}

pub fn parse_transition_type_with_fallback(
    s: &str,
    default_type: crate::tools::openmedia::video::TransitionType,
) -> crate::tools::openmedia::video::TransitionType {
    match s.to_lowercase().as_str() {
        "none" => crate::tools::openmedia::video::TransitionType::None,
        "crossfade" => crate::tools::openmedia::video::TransitionType::Crossfade,
        "slide_left" | "slideleft" | "slide-left" => crate::tools::openmedia::video::TransitionType::SlideLeft,
        "slide_right" | "slideright" | "slide-right" => {
            crate::tools::openmedia::video::TransitionType::SlideRight
        }
        "slide_up" | "slideup" | "slide-up" => crate::tools::openmedia::video::TransitionType::SlideUp,
        "slide_down" | "slidedown" | "slide-down" => crate::tools::openmedia::video::TransitionType::SlideDown,
        "zoom_in" | "zoomin" | "zoom-in" => crate::tools::openmedia::video::TransitionType::ZoomIn,
        "zoom_out" | "zoomout" | "zoom-out" => crate::tools::openmedia::video::TransitionType::ZoomOut,
        "wipe_left" | "wipeleft" | "wipe-left" => crate::tools::openmedia::video::TransitionType::WipeLeft,
        "wipe_right" | "wiperight" | "wipe-right" => crate::tools::openmedia::video::TransitionType::WipeRight,
        "blur" => crate::tools::openmedia::video::TransitionType::Blur,
        "glitch" => crate::tools::openmedia::video::TransitionType::Glitch,
        "radial_wipe" | "radialwipe" | "radial-wipe" => {
            crate::tools::openmedia::video::TransitionType::RadialWipe
        }
        _ => default_type,
    }
}

pub(crate) fn resolve_content_or_file(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    match trimmed.starts_with('<') || trimmed.starts_with('{') {
        true => Ok(input.to_string()),
        false => {
            let path = std::path::Path::new(trimmed);
            match path.exists() && path.is_file() {
                true => std::fs::read_to_string(path)
                    .map_err(|e| format!("Failed to read file '{}': {}", path.display(), e)),
                false => Ok(input.to_string()),
            }
        }
    }
}

pub(crate) fn save_svg_to_output(
    output_dir: &std::path::Path,
    svg_content: &str,
    model_used: &str,
    backend_used: &str,
    start_time: std::time::Instant,
) -> Result<crate::tools::openmedia::core::ImageOutput, String> {
    let _ = std::fs::create_dir_all(output_dir);
    let filename = format!("{}.svg", uuid::Uuid::now_v7());
    let output_path = output_dir.join(filename);

    std::fs::write(&output_path, svg_content)
        .map_err(|e| format!("Failed to write SVG output: {}", e))?;

    let file_size = std::fs::metadata(&output_path)
        .map(|m| m.len())
        .unwrap_or(svg_content.len() as u64);

    let (width, height) = parse_svg_dimensions(svg_content);
    let generation_time = start_time.elapsed().as_secs_f64();

    Ok(crate::tools::openmedia::core::ImageOutput {
        path: output_path,
        width,
        height,
        seed: 0,
        format: "svg".to_string(),
        file_size,
        generation_id: uuid::Uuid::now_v7().to_string(),
        clip_score: None,
        aesthetic_score: None,
        model_used: model_used.to_string(),
        backend_used: backend_used.to_string(),
        generation_time,
    })
}

pub(crate) fn save_animated_svg_to_output(
    output_dir: &std::path::Path,
    animated_svg: &str,
    duration: f64,
    animation_count: u32,
) -> Result<crate::tools::openmedia::core::AnimatedSvgOutput, String> {
    let _ = std::fs::create_dir_all(output_dir);
    let filename = format!("{}.svg", uuid::Uuid::now_v7());
    let output_path = output_dir.join(filename);

    std::fs::write(&output_path, animated_svg)
        .map_err(|e| format!("Failed to write animated SVG: {}", e))?;

    let file_size = std::fs::metadata(&output_path)
        .map(|m| m.len())
        .unwrap_or(animated_svg.len() as u64);
    let (width, height) = parse_svg_dimensions(animated_svg);

    Ok(crate::tools::openmedia::core::AnimatedSvgOutput {
        path: output_path,
        width,
        height,
        duration,
        animation_count,
        file_size,
        generation_id: uuid::Uuid::now_v7().to_string(),
    })
}

