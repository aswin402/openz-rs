//! Shared request parsing and media-rendering helpers.
//!
//! Keeping these adapters outside the MCP server definition lets handler
//! families share compatibility behavior without growing the server module
//! into another monolith.

use openmedia_video::VideoScene;

pub(crate) fn parse_json_value_field(
    value: &serde_json::Value,
    field_name: &str,
) -> Result<serde_json::Value, String> {
    if let Some(raw) = value.as_str() {
        let trimmed = raw.trim();
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            return serde_json::from_str(trimmed)
                .map_err(|e| format!("failed to parse {field_name} JSON string: {e}"));
        }
    }
    Ok(value.clone())
}

pub(crate) fn parse_video_scene_value(value: &serde_json::Value) -> Result<VideoScene, String> {
    if let Some(raw) = value.as_str() {
        let trimmed = raw.trim();
        if trimmed.starts_with('{') {
            return serde_json::from_str(trimmed)
                .map_err(|e| format!("failed to parse scene JSON string: {e}"));
        }

        let path = std::path::Path::new(trimmed);
        if path.exists() && path.is_file() {
            let s = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            return serde_json::from_str(&s)
                .map_err(|e| format!("failed to parse scene file '{}': {e}", path.display()));
        }

        return Err(format!(
            "scene string must be a VideoScene JSON object string or an existing file path: {trimmed}"
        ));
    }

    serde_json::from_value(value.clone()).map_err(|e| e.to_string())
}

pub(crate) fn parse_easing(s: Option<&str>) -> openmedia_animate::Easing {
    let name = s.unwrap_or("linear");
    match name.to_lowercase().as_str() {
        "linear" => openmedia_animate::Easing::Linear,
        "ease_in" | "easein" | "ease-in" => openmedia_animate::Easing::EaseInQuad,
        "ease_out" | "easeout" | "ease-out" => openmedia_animate::Easing::EaseOutQuad,
        "ease_in_out" | "easeinout" | "ease-in-out" => openmedia_animate::Easing::EaseInOutQuad,
        "ease_in_cubic" | "ease-in-cubic" => openmedia_animate::Easing::EaseInCubic,
        "ease_out_cubic" | "ease-out-cubic" => openmedia_animate::Easing::EaseOutCubic,
        "ease_in_out_cubic" | "ease-in-out-cubic" => openmedia_animate::Easing::EaseInOutCubic,
        "ease_in_expo" | "ease-in-expo" => openmedia_animate::Easing::EaseInExpo,
        "ease_out_expo" | "ease-out-expo" => openmedia_animate::Easing::EaseOutExpo,
        "ease_in_out_expo" | "ease-in-out-expo" => openmedia_animate::Easing::EaseInOutExpo,
        "bounce" | "ease_out_bounce" | "ease-out-bounce" => {
            openmedia_animate::Easing::EaseOutBounce
        }
        "elastic" | "ease_out_elastic" | "ease-out-elastic" => {
            openmedia_animate::Easing::EaseOutElastic
        }
        "spring" => openmedia_animate::Easing::Spring {
            stiffness: 100.0,
            damping: 10.0,
            mass: 1.0,
        },
        _ => {
            if name.starts_with("cubic-bezier(") && name.ends_with(')') {
                let content = &name["cubic-bezier(".len()..name.len() - 1];
                let parts: Vec<&str> = content.split(',').map(|p| p.trim()).collect();
                if parts.len() == 4 {
                    let x1 = parts[0].parse::<f64>().unwrap_or(0.25);
                    let y1 = parts[1].parse::<f64>().unwrap_or(0.1);
                    let x2 = parts[2].parse::<f64>().unwrap_or(0.25);
                    let y2 = parts[3].parse::<f64>().unwrap_or(1.0);
                    return openmedia_animate::Easing::CubicBezier(x1, y1, x2, y2);
                }
            }
            openmedia_animate::Easing::Linear
        }
    }
}

pub(crate) fn parse_preset(s: &str) -> openmedia_animate::AnimationPreset {
    match s.to_lowercase().as_str() {
        "fade_in" | "fadein" | "fade-in" => openmedia_animate::AnimationPreset::FadeIn,
        "fade_out" | "fadeout" | "fade-out" => openmedia_animate::AnimationPreset::FadeOut,
        "slide_in_left" | "slide-in-left" => openmedia_animate::AnimationPreset::SlideInLeft,
        "slide_in_right" | "slide-in-right" => openmedia_animate::AnimationPreset::SlideInRight,
        "slide_in_up" | "slide-in-up" => openmedia_animate::AnimationPreset::SlideInUp,
        "slide_in_down" | "slide-in-down" => openmedia_animate::AnimationPreset::SlideInDown,
        "bounce" => openmedia_animate::AnimationPreset::Bounce,
        "pulse" => openmedia_animate::AnimationPreset::Pulse,
        "spin" => openmedia_animate::AnimationPreset::Spin,
        "shake" => openmedia_animate::AnimationPreset::Shake,
        "wobble" => openmedia_animate::AnimationPreset::Wobble,
        "typewriter" => openmedia_animate::AnimationPreset::Typewriter,
        "draw_path" | "draw-path" | "drawpath" => openmedia_animate::AnimationPreset::DrawPath,
        "morph" => openmedia_animate::AnimationPreset::Morph,
        "gradient_shift" | "gradient-shift" => openmedia_animate::AnimationPreset::GradientShift,
        "parallax_scroll" | "parallax-scroll" => {
            openmedia_animate::AnimationPreset::ParallaxScroll
        }
        "stagger" => openmedia_animate::AnimationPreset::Stagger,
        _ => openmedia_animate::AnimationPreset::FadeIn,
    }
}

pub(crate) fn inject_css_class(svg: &str, element_id: &str, class_name: &str) -> String {
    let clean_id = element_id.trim_start_matches('#');
    let patterns = [format!("id=\"{}\"", clean_id), format!("id='{}'", clean_id)];

    let mut found_pos = None;
    for pat in &patterns {
        if let Some(pos) = svg.find(pat) {
            found_pos = Some((pos, pat.len()));
            break;
        }
    }

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

    if let Some(c_pos) = tag_content.find(class_pat_double) {
        let insert_idx = c_pos + class_pat_double.len();
        tag_content.insert_str(insert_idx, &format!("{} ", class_name));
    } else if let Some(c_pos) = tag_content.find(class_pat_single) {
        let insert_idx = c_pos + class_pat_single.len();
        tag_content.insert_str(insert_idx, &format!("{} ", class_name));
    } else if let Some(space_pos) = tag_content.find(' ') {
        tag_content.insert_str(space_pos, &format!(" class=\"{}\"", class_name));
    } else {
        let insert_pos = if tag_content.ends_with("/>") {
            tag_content.len() - 2
        } else {
            tag_content.len() - 1
        };
        tag_content.insert_str(insert_pos, &format!(" class=\"{}\" ", class_name));
    }

    let mut result = svg.to_string();
    result.replace_range(start_tag_idx..=end_tag_idx, &tag_content);
    result
}

pub(crate) fn inject_style_or_xml(mut svg: String, content_to_inject: &str) -> String {
    let lower = svg.to_lowercase();
    if let Some(close_idx) = lower.rfind("</svg>") {
        svg.insert_str(close_idx, content_to_inject);
    } else {
        svg.push_str(content_to_inject);
    }
    svg
}

pub(crate) fn parse_svg_dimensions(svg: &str) -> (u32, u32) {
    let mut width = 800;
    let mut height = 600;

    if let Some(pos) = svg.find("width=\"") {
        let start = pos + "width=\"".len();
        if let Some(end) = svg[start..].find('"') {
            if let Ok(val) = svg[start..start + end].parse::<f64>() {
                width = val as u32;
            }
        }
    }

    if let Some(pos) = svg.find("height=\"") {
        let start = pos + "height=\"".len();
        if let Some(end) = svg[start..].find('"') {
            if let Ok(val) = svg[start..start + end].parse::<f64>() {
                height = val as u32;
            }
        }
    }

    (width, height)
}

pub(crate) fn override_theme_fields(
    theme: &mut mermaid_rs_renderer::Theme,
    overrides: &serde_json::Value,
) {
    if let Some(map) = overrides.as_object() {
        for (key, val) in map {
            if let Some(val_str) = val.as_str() {
                match key.as_str() {
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
                }
            } else if let Some(val_f64) = val.as_f64() {
                if key == "font_size" {
                    theme.font_size = val_f64 as f32;
                }
            }
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
    default_type: openmedia_video::TransitionType,
) -> (openmedia_video::TransitionType, f64, Option<String>) {
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
) -> Option<openmedia_video::AudioConfig> {
    if let Some(tracks_arr) = parameters.get("audio_tracks").and_then(|v| v.as_array()) {
        let mut tracks = Vec::new();
        for track_val in tracks_arr {
            if let Some(src) = track_val.get("src").and_then(|v| v.as_str()) {
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

                tracks.push(openmedia_video::AudioTrack {
                    src: src.to_string(),
                    start,
                    volume,
                    fade_in,
                    fade_out,
                });
            }
        }
        if !tracks.is_empty() {
            return Some(openmedia_video::AudioConfig { tracks });
        }
    } else if let Some(bg_music) = parameters.get("background_music").and_then(|v| v.as_str()) {
        return Some(openmedia_video::AudioConfig {
            tracks: vec![openmedia_video::AudioTrack {
                src: bg_music.to_string(),
                start: 0.0,
                volume: 0.5,
                fade_in: None,
                fade_out: None,
            }],
        });
    }
    None
}

pub(crate) fn get_templates_dir() -> std::path::PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("assets")
        .join("templates")
}

pub(crate) fn interpolate_template(
    template_json: &serde_json::Value,
    parameters: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let mut template_str = serde_json::to_string(template_json).map_err(|e| e.to_string())?;

    if let Some(obj) = parameters.as_object() {
        for (key, val) in obj {
            let placeholder = format!("{{{{{}}}}}", key);
            let replacement = match val {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            template_str = template_str.replace(&placeholder, &replacement);
        }
    }

    serde_json::from_str(&template_str)
        .map_err(|e| format!("Failed to parse interpolated template: {}", e))
}

pub(crate) fn parse_custom_fonts(
    parameters: &serde_json::Value,
) -> Option<Vec<openmedia_video::CustomFontSpec>> {
    if let Some(fonts_arr) = parameters.get("custom_fonts").and_then(|v| v.as_array()) {
        let mut specs = Vec::new();
        for font_val in fonts_arr {
            if let (Some(family), Some(src)) = (
                font_val.get("family").and_then(|v| v.as_str()),
                font_val.get("src").and_then(|v| v.as_str()),
            ) {
                specs.push(openmedia_video::CustomFontSpec {
                    family: family.to_string(),
                    src: src.to_string(),
                });
            }
        }
        if !specs.is_empty() {
            return Some(specs);
        }
    }
    None
}

pub fn parse_transition_type(s: &str) -> openmedia_video::TransitionType {
    parse_transition_type_with_fallback(s, openmedia_video::TransitionType::Crossfade)
}

pub fn parse_transition_type_with_fallback(
    s: &str,
    default_type: openmedia_video::TransitionType,
) -> openmedia_video::TransitionType {
    match s.to_lowercase().as_str() {
        "none" => openmedia_video::TransitionType::None,
        "crossfade" => openmedia_video::TransitionType::Crossfade,
        "slide_left" | "slideleft" | "slide-left" => openmedia_video::TransitionType::SlideLeft,
        "slide_right" | "slideright" | "slide-right" => {
            openmedia_video::TransitionType::SlideRight
        }
        "slide_up" | "slideup" | "slide-up" => openmedia_video::TransitionType::SlideUp,
        "slide_down" | "slidedown" | "slide-down" => openmedia_video::TransitionType::SlideDown,
        "zoom_in" | "zoomin" | "zoom-in" => openmedia_video::TransitionType::ZoomIn,
        "zoom_out" | "zoomout" | "zoom-out" => openmedia_video::TransitionType::ZoomOut,
        "wipe_left" | "wipeleft" | "wipe-left" => openmedia_video::TransitionType::WipeLeft,
        "wipe_right" | "wiperight" | "wipe-right" => openmedia_video::TransitionType::WipeRight,
        "blur" => openmedia_video::TransitionType::Blur,
        "glitch" => openmedia_video::TransitionType::Glitch,
        "radial_wipe" | "radialwipe" | "radial-wipe" => {
            openmedia_video::TransitionType::RadialWipe
        }
        _ => default_type,
    }
}
