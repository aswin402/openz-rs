//! Video creation, preview, editing, and template rendering handlers.

use super::{
    get_templates_dir, interpolate_template, parse_audio_config, parse_custom_fonts,
    parse_json_value_field, parse_transition_params, parse_transition_type,
    parse_video_scene_value, Json, McpObject, OpenMediaServer, Parameters, VideoCreateRequest,
    VideoCreateSlideshowRequest, VideoAddTransitionRequest, VideoAddAudioRequest,
    VideoFromTemplateRequest, VideoPreviewRequest, VideoExtractFramesRequest, VideoTrimRequest,
};
use openmedia_video::{FrameRenderer, SceneElement, VideoScene};
use rmcp::handler::server::tool::ToolRouter;

pub(crate) fn router() -> ToolRouter<OpenMediaServer> {
    OpenMediaServer::video_router()
}

#[rmcp::tool_router(router = video_router)]
impl OpenMediaServer {
    #[rmcp::tool(
        name = "video_create",
        description = "Compile a video from a full VideoScene JSON description. Supports transitions and audio mixing. DESIGN TIPS: Use native shapes, text, and charts for faster offline rendering. Define explicit keyframes for opacity/scale/rotation/position. Easing choices: linear, ease_in, ease_out, ease_in_out."
    )]
    pub async fn video_create(
        &self,
        params: Parameters<VideoCreateRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let scene = parse_video_scene_value(&req.scene)?;

        let output_path = if let Some(out_p) = req.output_path {
            std::path::PathBuf::from(out_p)
        } else {
            let filename = format!("{}.mp4", uuid::Uuid::now_v7());
            self.config.paths.output_dir.join(filename)
        };

        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);

        let video_spec = openmedia_video::render_video_scene(&scene, &output_path)
            .await
            .map_err(|e| e.to_string())?;

        serde_json::to_value(video_spec)
            .map(McpObject)
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[rmcp::tool(
        name = "video_preview",
        description = "Generate a preview frame image for a video scene at a given time offset."
    )]
    pub async fn video_preview(
        &self,
        params: Parameters<VideoPreviewRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let scene = parse_video_scene_value(&req.scene)?;

        let t = req.time.unwrap_or(0.0);
        let w = req.width.unwrap_or(scene.width);
        let h = req.height.unwrap_or(scene.height);
        let format = req.output_format.unwrap_or_else(|| "png".to_string());

        let use_browser = scene.scenes.iter().any(|s| {
            s.elements
                .iter()
                .any(|el| matches!(el, SceneElement::Html { .. } | SceneElement::Code { .. }))
        });

        let frame = if use_browser {
            let renderer = openmedia_video::BrowserFrameRenderer::launch()
                .await
                .map_err(|e| e.to_string())?;
            let f = renderer
                .render_frame(&scene, t, w, h)
                .await
                .map_err(|e| e.to_string())?;
            renderer.close().await;
            f
        } else {
            let renderer = openmedia_video::SvgFrameRenderer;
            renderer
                .render_frame(&scene, t, w, h)
                .await
                .map_err(|e| e.to_string())?
        };

        let filename = format!("{}.{}", uuid::Uuid::now_v7(), format);
        let output_path = self.config.paths.output_dir.join(filename);
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);

        let mut bytes = Vec::new();
        let img_format = match format.to_lowercase().as_str() {
            "png" => image::ImageFormat::Png,
            "jpeg" | "jpg" => image::ImageFormat::Jpeg,
            "webp" => image::ImageFormat::WebP,
            other => return Err(format!("Unsupported preview output format: {}", other)),
        };
        frame
            .write_to(&mut std::io::Cursor::new(&mut bytes), img_format)
            .map_err(|e| e.to_string())?;
        std::fs::write(&output_path, &bytes).map_err(|e| e.to_string())?;

        let file_size = std::fs::metadata(&output_path)
            .map(|m| m.len())
            .unwrap_or(0);
        let output = openmedia_core::ImageOutput {
            path: output_path,
            width: w,
            height: h,
            seed: 0,
            format,
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
            clip_score: None,
            aesthetic_score: None,
            model_used: "none".to_string(),
            backend_used: if use_browser {
                "headless-chrome"
            } else {
                "svg"
            }
            .to_string(),
            generation_time: 0.0,
        };

        serde_json::to_value(output)
            .map(McpObject)
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[rmcp::tool(
        name = "video_create_slideshow",
        description = "Quickly compile a slideshow video from a list of image paths or directory path, with options for transitions and audio."
    )]
    pub async fn video_create_slideshow(
        &self,
        params: Parameters<VideoCreateSlideshowRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let duration_per_image = req.duration_per_image.unwrap_or(3.0);
        let trans_type_str = req
            .transition_type
            .unwrap_or_else(|| "crossfade".to_string());
        let trans_duration = req.transition_duration.unwrap_or(0.5);

        let width = req.width.unwrap_or(1920);
        let height = req.height.unwrap_or(1080);
        let fps = req.fps.unwrap_or(30);

        // Resolve images
        let mut resolved_images = Vec::new();
        for path_str in req.images {
            let path = std::path::Path::new(&path_str);
            if path.is_dir() {
                let entries = std::fs::read_dir(path).map_err(|e| e.to_string())?;
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_file() {
                        if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                            match ext.to_lowercase().as_str() {
                                "png" | "jpg" | "jpeg" | "webp" => {
                                    resolved_images.push(p.to_string_lossy().into_owned());
                                }
                                _ => {}
                            }
                        }
                    }
                }
            } else {
                resolved_images.push(path_str);
            }
        }

        if resolved_images.is_empty() {
            return Err("No valid images found for slideshow".to_string());
        }

        // Construct VideoScene DSL
        let mut scenes = Vec::new();
        let mut transitions = Vec::new();

        let mut current_time = 0.0;
        for (i, img_src) in resolved_images.iter().enumerate() {
            let scene_id = format!("slide_{}", i);
            let start = current_time;
            let end = start + duration_per_image;

            let element = openmedia_video::SceneElement::Image {
                src: img_src.clone(),
                position: openmedia_video::Position {
                    x: openmedia_video::DimensionValue::Pixels(0.0),
                    y: openmedia_video::DimensionValue::Pixels(0.0),
                },
                size: openmedia_video::Size {
                    width: openmedia_video::DimensionValue::Percentage("100%".to_string()),
                    height: openmedia_video::DimensionValue::Percentage("100%".to_string()),
                },
                fit: openmedia_video::ObjectFit::Contain,
                timeline: None,
            };

            scenes.push(openmedia_video::Scene {
                id: scene_id.clone(),
                start,
                end,
                elements: vec![element],
            });

            if i > 0 {
                let from = format!("slide_{}", i - 1);
                let to = scene_id;
                let transition_type = parse_transition_type(&trans_type_str);
                transitions.push(openmedia_video::SceneTransition {
                    from,
                    to,
                    transition_type,
                    duration: trans_duration,
                    easing: None,
                });
            }

            current_time = end - trans_duration;
        }

        let total_duration = current_time + trans_duration;

        let audio = req.audio_src.map(|src| openmedia_video::AudioConfig {
            tracks: vec![openmedia_video::AudioTrack {
                src,
                start: 0.0,
                volume: 1.0,
                fade_in: Some(1.0),
                fade_out: Some(1.0),
            }],
        });

        let scene = openmedia_video::VideoScene {
            width,
            height,
            fps,
            duration: total_duration,
            background: "#000000".to_string(),
            scenes,
            transitions,
            audio,
            custom_fonts: None,
        };

        let output_path = if let Some(out_p) = req.output_path {
            std::path::PathBuf::from(out_p)
        } else {
            let filename = format!("{}.mp4", uuid::Uuid::now_v7());
            self.config.paths.output_dir.join(filename)
        };

        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);

        let video_spec = openmedia_video::render_video_scene(&scene, &output_path)
            .await
            .map_err(|e| e.to_string())?;

        serde_json::to_value(video_spec)
            .map(McpObject)
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[rmcp::tool(
        name = "video_add_transition",
        description = "Add a transition between two scenes in an existing video scene JSON file."
    )]
    pub async fn video_add_transition(
        &self,
        params: Parameters<VideoAddTransitionRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let path = std::path::Path::new(&req.scene_path);
        if !path.exists() || !path.is_file() {
            return Err(format!("Scene file not found: {}", req.scene_path));
        }

        let s = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let mut scene: VideoScene = serde_json::from_str(&s).map_err(|e| e.to_string())?;

        let duration = req.duration.unwrap_or(0.5);
        let transition_type = parse_transition_type(&req.transition_type);

        scene
            .transitions
            .retain(|t| !(t.from == req.from_scene_id && t.to == req.to_scene_id));

        scene.transitions.push(openmedia_video::SceneTransition {
            from: req.from_scene_id,
            to: req.to_scene_id,
            transition_type,
            duration,
            easing: None,
        });

        let updated = serde_json::to_string_pretty(&scene).map_err(|e| e.to_string())?;
        std::fs::write(path, updated).map_err(|e| e.to_string())?;

        serde_json::to_value(scene)
            .map(McpObject)
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[rmcp::tool(
        name = "video_add_audio",
        description = "Add an audio track to an existing video file or video scene JSON description."
    )]
    pub async fn video_add_audio(
        &self,
        params: Parameters<VideoAddAudioRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let target = std::path::Path::new(&req.target_path);
        if !target.exists() {
            return Err(format!("Target path not found: {}", req.target_path));
        }

        let start_time = req.start_time.unwrap_or(0.0);
        let volume = req.volume.unwrap_or(1.0);

        if target.is_file() && req.target_path.ends_with(".json") {
            let s = std::fs::read_to_string(target).map_err(|e| e.to_string())?;
            let mut scene: VideoScene = serde_json::from_str(&s).map_err(|e| e.to_string())?;

            let track = openmedia_video::AudioTrack {
                src: req.audio_path,
                start: start_time,
                volume,
                fade_in: req.fade_in,
                fade_out: req.fade_out,
            };

            if let Some(audio_cfg) = &mut scene.audio {
                audio_cfg.tracks.push(track);
            } else {
                scene.audio = Some(openmedia_video::AudioConfig {
                    tracks: vec![track],
                });
            }

            let updated = serde_json::to_string_pretty(&scene).map_err(|e| e.to_string())?;
            std::fs::write(target, updated).map_err(|e| e.to_string())?;

            return serde_json::to_value(scene)
                .map(McpObject)
                .map(Json)
                .map_err(|e| e.to_string());
        }

        let output_path = if let Some(out_p) = req.output_path {
            std::path::PathBuf::from(out_p)
        } else {
            let filename = format!("{}.mp4", uuid::Uuid::now_v7());
            self.config.paths.output_dir.join(filename)
        };

        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);

        let delay_ms = (start_time * 1000.0) as i32;
        let filter_script = format!(
            "[1:a]adelay={}|{},volume={}[a1];[0:a][a1]amix=inputs=2:duration=first[out_a]",
            delay_ms, delay_ms, volume
        );

        let mut cmd = tokio::process::Command::new("ffmpeg");
        cmd.args([
            "-y",
            "-i",
            &req.target_path,
            "-i",
            &req.audio_path,
            "-filter_complex",
            &filter_script,
            "-map",
            "0:v",
            "-map",
            "[out_a]",
            "-c:v",
            "copy",
            "-c:a",
            "aac",
        ])
        .arg(&output_path);

        cmd.stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());

        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        child.wait().await.map_err(|e| e.to_string())?;

        let file_size = std::fs::metadata(&output_path)
            .map(|m| m.len())
            .unwrap_or(0);
        let output = openmedia_core::VideoSpec {
            path: output_path,
            width: 0,
            height: 0,
            duration: 0.0,
            fps: 0,
            codec: "copy".to_string(),
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
            renderer_used: "ffmpeg".to_string(),
            total_frames: 0,
            generation_time: 0.0,
        };

        serde_json::to_value(output)
            .map(McpObject)
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[rmcp::tool(
        name = "video_from_template",
        description = "Instantiate a video scene from one of the pre-designed templates (slideshow, text_explainer, data_dashboard, social_media, product_showcase). DESIGN TIPS: Use custom colors, charts, audio offsets, and customize transition easing (linear, ease_in, ease_out, ease_in_out)."
    )]
    pub async fn video_from_template(
        &self,
        params: Parameters<VideoFromTemplateRequest>,
    ) -> Result<Json<McpObject>, String> {
        let mut req = params.0;
        req.parameters = parse_json_value_field(&req.parameters, "parameters")?;
        let output_path = if let Some(out_p) = req.output_path {
            std::path::PathBuf::from(out_p)
        } else {
            let filename = format!("{}.mp4", uuid::Uuid::now_v7());
            self.config.paths.output_dir.join(filename)
        };

        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);

        let scene = match req.template_name.to_lowercase().as_str() {
            "slideshow" => {
                let images = req.parameters["images"]
                    .as_array()
                    .ok_or_else(|| "Missing parameters.images array".to_string())?
                    .iter()
                    .map(|v: &serde_json::Value| v.as_str().unwrap_or("").to_string())
                    .collect::<Vec<String>>();
                let duration = req.parameters["duration_per_image"].as_f64().unwrap_or(3.0);
                let width = req.parameters["width"].as_u64().unwrap_or(1920) as u32;
                let height = req.parameters["height"].as_u64().unwrap_or(1080) as u32;
                let fps = req.parameters["fps"].as_u64().unwrap_or(30) as u32;

                let mut scenes = Vec::new();
                for (i, img_src) in images.iter().enumerate() {
                    scenes.push(openmedia_video::Scene {
                        id: format!("slide_{}", i),
                        start: i as f64 * duration,
                        end: (i + 1) as f64 * duration,
                        elements: vec![openmedia_video::SceneElement::Image {
                            src: img_src.clone(),
                            position: openmedia_video::Position {
                                x: openmedia_video::DimensionValue::Pixels(0.0),
                                y: openmedia_video::DimensionValue::Pixels(0.0),
                            },
                            size: openmedia_video::Size {
                                width: openmedia_video::DimensionValue::Percentage(
                                    "100%".to_string(),
                                ),
                                height: openmedia_video::DimensionValue::Percentage(
                                    "100%".to_string(),
                                ),
                            },
                            fit: openmedia_video::ObjectFit::Contain,
                            timeline: None,
                        }],
                    });
                }
                let mut transitions = Vec::new();
                if images.len() > 1 && req.parameters.get("transition_type").is_some() {
                    let (custom_trans_type, custom_duration, custom_easing) =
                        parse_transition_params(
                            &req.parameters,
                            openmedia_video::TransitionType::Crossfade,
                        );

                    for i in 0..(images.len() - 1) {
                        transitions.push(openmedia_video::SceneTransition {
                            from: format!("slide_{}", i),
                            to: format!("slide_{}", i + 1),
                            transition_type: custom_trans_type.clone(),
                            duration: custom_duration,
                            easing: custom_easing.clone(),
                        });
                    }
                }

                openmedia_video::VideoScene {
                    width,
                    height,
                    fps,
                    duration: images.len() as f64 * duration,
                    background: "#000000".to_string(),
                    scenes,
                    transitions,
                    audio: parse_audio_config(&req.parameters),
                    custom_fonts: parse_custom_fonts(&req.parameters),
                }
            }
            "text_explainer" => {
                let title = req.parameters["title"]
                    .as_str()
                    .unwrap_or("Explainer Video")
                    .to_string();
                let bullets = req.parameters["bullets"]
                    .as_array()
                    .ok_or_else(|| "Missing parameters.bullets array".to_string())?
                    .iter()
                    .map(|v: &serde_json::Value| v.as_str().unwrap_or("").to_string())
                    .collect::<Vec<String>>();
                let bullet_duration = req.parameters["bullet_duration"].as_f64().unwrap_or(3.0);
                let width = req.parameters["width"].as_u64().unwrap_or(1920) as u32;
                let height = req.parameters["height"].as_u64().unwrap_or(1080) as u32;
                let fps = req.parameters["fps"].as_u64().unwrap_or(30) as u32;

                let mut scenes = Vec::new();
                let total_duration = (bullets.len() + 1) as f64 * bullet_duration;

                let s0_elements = vec![openmedia_video::SceneElement::Text {
                    content: title.clone(),
                    style: openmedia_video::TextStyle {
                        font_family: "sans-serif".to_string(),
                        font_size: 48.0,
                        font_weight: 700,
                        color: "#ffffff".to_string(),
                        text_align: "center".to_string(),
                        line_height: None,
                        letter_spacing: None,
                    },
                    position: openmedia_video::Position {
                        x: openmedia_video::DimensionValue::Pixels((width / 2) as f64),
                        y: openmedia_video::DimensionValue::Pixels(150.0),
                    },
                    anchor: openmedia_video::Anchor::Center,
                    timeline: None,
                }];
                scenes.push(openmedia_video::Scene {
                    id: "scene_0".to_string(),
                    start: 0.0,
                    end: bullet_duration,
                    elements: s0_elements.clone(),
                });

                for i in 0..bullets.len() {
                    let mut el = s0_elements.clone();
                    for (j, bullet_text) in bullets.iter().enumerate().take(i + 1) {
                        el.push(openmedia_video::SceneElement::Text {
                            content: format!("• {}", bullet_text),
                            style: openmedia_video::TextStyle {
                                font_family: "sans-serif".to_string(),
                                font_size: 32.0,
                                font_weight: 400,
                                color: "#cccccc".to_string(),
                                text_align: "left".to_string(),
                                line_height: None,
                                letter_spacing: None,
                            },
                            position: openmedia_video::Position {
                                x: openmedia_video::DimensionValue::Pixels(200.0),
                                y: openmedia_video::DimensionValue::Pixels(300.0 + j as f64 * 80.0),
                            },
                            anchor: openmedia_video::Anchor::TopLeft,
                            timeline: None,
                        });
                    }
                    scenes.push(openmedia_video::Scene {
                        id: format!("scene_{}", i + 1),
                        start: (i + 1) as f64 * bullet_duration,
                        end: (i + 2) as f64 * bullet_duration,
                        elements: el,
                    });
                }

                openmedia_video::VideoScene {
                    width,
                    height,
                    fps,
                    duration: total_duration,
                    background: "#1a1a2e".to_string(),
                    scenes,
                    transitions: vec![],
                    audio: parse_audio_config(&req.parameters),
                    custom_fonts: parse_custom_fonts(&req.parameters),
                }
            }
            "data_dashboard" => {
                let title = req.parameters["title"]
                    .as_str()
                    .unwrap_or("Data Dashboard")
                    .to_string();
                let charts_arr = req.parameters["charts"]
                    .as_array()
                    .ok_or_else(|| "Missing parameters.charts array".to_string())?;
                let duration = req.parameters["chart_duration"].as_f64().unwrap_or(3.0);
                let width = req.parameters["width"].as_u64().unwrap_or(1920) as u32;
                let height = req.parameters["height"].as_u64().unwrap_or(1080) as u32;
                let fps = req.parameters["fps"].as_u64().unwrap_or(30) as u32;

                let mut scenes = Vec::new();
                let mut transitions = Vec::new();

                scenes.push(openmedia_video::Scene {
                    id: "scene_0".to_string(),
                    start: 0.0,
                    end: 2.0,
                    elements: vec![openmedia_video::SceneElement::Text {
                        content: title.clone(),
                        style: openmedia_video::TextStyle {
                            font_family: "sans-serif".to_string(),
                            font_size: 64.0,
                            font_weight: 700,
                            color: "#ffffff".to_string(),
                            text_align: "center".to_string(),
                            line_height: None,
                            letter_spacing: None,
                        },
                        position: openmedia_video::Position {
                            x: openmedia_video::DimensionValue::Pixels((width / 2) as f64),
                            y: openmedia_video::DimensionValue::Pixels((height / 2) as f64),
                        },
                        anchor: openmedia_video::Anchor::Center,
                        timeline: None,
                    }],
                });

                let (custom_trans_type, custom_duration, custom_easing) = parse_transition_params(
                    &req.parameters,
                    openmedia_video::TransitionType::SlideLeft,
                );

                for (i, chart_val) in charts_arr.iter().enumerate() {
                    let chart_type = chart_val["type"].as_str().unwrap_or("bar").to_string();
                    let chart_title = chart_val["title"]
                        .as_str()
                        .unwrap_or("Statistics")
                        .to_string();
                    let chart_data = chart_val["data"].clone();

                    let scene_id = format!("scene_{}", i + 1);
                    let start = 2.0 + i as f64 * duration;
                    let end = start + duration;

                    scenes.push(openmedia_video::Scene {
                        id: scene_id.clone(),
                        start,
                        end,
                        elements: vec![
                            openmedia_video::SceneElement::Text {
                                content: chart_title,
                                style: openmedia_video::TextStyle {
                                    font_family: "sans-serif".to_string(),
                                    font_size: 40.0,
                                    font_weight: 600,
                                    color: "#ffffff".to_string(),
                                    text_align: "center".to_string(),
                                    line_height: None,
                                    letter_spacing: None,
                                },
                                position: openmedia_video::Position {
                                    x: openmedia_video::DimensionValue::Pixels((width / 2) as f64),
                                    y: openmedia_video::DimensionValue::Pixels(80.0),
                                },
                                anchor: openmedia_video::Anchor::Center,
                                timeline: None,
                            },
                            openmedia_video::SceneElement::Chart {
                                chart_type,
                                data: chart_data,
                                position: openmedia_video::Position {
                                    x: openmedia_video::DimensionValue::Pixels((width / 2) as f64),
                                    y: openmedia_video::DimensionValue::Pixels(
                                        (height / 2 + 30) as f64,
                                    ),
                                },
                                size: openmedia_video::Size {
                                    width: openmedia_video::DimensionValue::Pixels(
                                        (width - 400) as f64,
                                    ),
                                    height: openmedia_video::DimensionValue::Pixels(
                                        (height - 300) as f64,
                                    ),
                                },
                                theme: "dark".to_string(),
                                timeline: None,
                            },
                        ],
                    });

                    let from = format!("scene_{}", i);
                    let to = scene_id;
                    transitions.push(openmedia_video::SceneTransition {
                        from,
                        to,
                        transition_type: custom_trans_type.clone(),
                        duration: custom_duration,
                        easing: custom_easing.clone(),
                    });
                }

                let total_duration = 2.0 + charts_arr.len() as f64 * duration;

                openmedia_video::VideoScene {
                    width,
                    height,
                    fps,
                    duration: total_duration,
                    background: "#0f172a".to_string(),
                    scenes,
                    transitions,
                    audio: parse_audio_config(&req.parameters),
                    custom_fonts: parse_custom_fonts(&req.parameters),
                }
            }
            "social_media" => {
                let title = req.parameters["title"]
                    .as_str()
                    .unwrap_or("Top Facts")
                    .to_string();
                let content_arr: &Vec<serde_json::Value> = req.parameters["content"]
                    .as_array()
                    .ok_or_else(|| "Missing parameters.content array".to_string())?;
                let duration = req.parameters["scene_duration"].as_f64().unwrap_or(3.0);
                let bg_color = req.parameters["background_color"]
                    .as_str()
                    .unwrap_or("#1e1b4b")
                    .to_string();
                let width = 1080;
                let height = 1920;
                let fps = req.parameters["fps"].as_u64().unwrap_or(30) as u32;

                let mut scenes = Vec::new();
                let mut transitions = Vec::new();

                scenes.push(openmedia_video::Scene {
                    id: "scene_0".to_string(),
                    start: 0.0,
                    end: 3.0,
                    elements: vec![openmedia_video::SceneElement::Text {
                        content: title.clone(),
                        style: openmedia_video::TextStyle {
                            font_family: "sans-serif".to_string(),
                            font_size: 72.0,
                            font_weight: 800,
                            color: "#fbbf24".to_string(),
                            text_align: "center".to_string(),
                            line_height: None,
                            letter_spacing: None,
                        },
                        position: openmedia_video::Position {
                            x: openmedia_video::DimensionValue::Pixels((width / 2) as f64),
                            y: openmedia_video::DimensionValue::Pixels((height / 2) as f64),
                        },
                        anchor: openmedia_video::Anchor::Center,
                        timeline: Some(openmedia_video::ElementTimeline {
                            keyframes: vec![
                                openmedia_video::Keyframe {
                                    time: 0.0,
                                    opacity: Some(0.0),
                                    x: None,
                                    y: None,
                                    scale: Some(0.5),
                                    scale_x: None,
                                    scale_y: None,
                                    rotation: None,
                                    easing: Some("ease_out".to_string()),
                                },
                                openmedia_video::Keyframe {
                                    time: 1.0,
                                    opacity: Some(1.0),
                                    x: None,
                                    y: None,
                                    scale: Some(1.0),
                                    scale_x: None,
                                    scale_y: None,
                                    rotation: None,
                                    easing: None,
                                },
                            ],
                        }),
                    }],
                });

                let (custom_trans_type, custom_duration, custom_easing) = parse_transition_params(
                    &req.parameters,
                    openmedia_video::TransitionType::SlideUp,
                );

                for (i, content_val) in content_arr.iter().enumerate() {
                    let point_text = content_val.as_str().unwrap_or("").to_string();
                    let scene_id = format!("scene_{}", i + 1);
                    let start = 3.0 + i as f64 * duration;
                    let end = start + duration;

                    scenes.push(openmedia_video::Scene {
                        id: scene_id.clone(),
                        start,
                        end,
                        elements: vec![
                            // Background Floating Circle 1
                            openmedia_video::SceneElement::Shape {
                                shape: openmedia_video::ShapeType::Circle,
                                size: openmedia_video::Size {
                                    width: openmedia_video::DimensionValue::Pixels(350.0),
                                    height: openmedia_video::DimensionValue::Pixels(350.0),
                                },
                                position: openmedia_video::Position {
                                    x: openmedia_video::DimensionValue::Pixels(100.0),
                                    y: openmedia_video::DimensionValue::Pixels(300.0),
                                },
                                style: openmedia_video::ShapeStyle {
                                    fill: Some("#312e81".to_string()),
                                    stroke: None,
                                    stroke_width: None,
                                    border_radius: None,
                                    opacity: Some(0.15),
                                },
                                timeline: Some(openmedia_video::ElementTimeline {
                                    keyframes: vec![
                                        openmedia_video::Keyframe {
                                            time: 0.0,
                                            opacity: Some(0.1),
                                            x: Some("-50".to_string()),
                                            y: Some("-30".to_string()),
                                            scale: Some(0.8),
                                            scale_x: None,
                                            scale_y: None,
                                            rotation: None,
                                            easing: None,
                                        },
                                        openmedia_video::Keyframe {
                                            time: duration,
                                            opacity: Some(0.15),
                                            x: Some("50".to_string()),
                                            y: Some("30".to_string()),
                                            scale: Some(1.2),
                                            scale_x: None,
                                            scale_y: None,
                                            rotation: None,
                                            easing: None,
                                        },
                                    ],
                                }),
                            },
                            // Background Floating Circle 2
                            openmedia_video::SceneElement::Shape {
                                shape: openmedia_video::ShapeType::Circle,
                                size: openmedia_video::Size {
                                    width: openmedia_video::DimensionValue::Pixels(450.0),
                                    height: openmedia_video::DimensionValue::Pixels(450.0),
                                },
                                position: openmedia_video::Position {
                                    x: openmedia_video::DimensionValue::Pixels(800.0),
                                    y: openmedia_video::DimensionValue::Pixels(1400.0),
                                },
                                style: openmedia_video::ShapeStyle {
                                    fill: Some("#4c1d95".to_string()),
                                    stroke: None,
                                    stroke_width: None,
                                    border_radius: None,
                                    opacity: Some(0.12),
                                },
                                timeline: Some(openmedia_video::ElementTimeline {
                                    keyframes: vec![
                                        openmedia_video::Keyframe {
                                            time: 0.0,
                                            opacity: Some(0.12),
                                            x: Some("40".to_string()),
                                            y: Some("50".to_string()),
                                            scale: Some(1.1),
                                            scale_x: None,
                                            scale_y: None,
                                            rotation: None,
                                            easing: None,
                                        },
                                        openmedia_video::Keyframe {
                                            time: duration,
                                            opacity: Some(0.08),
                                            x: Some("-40".to_string()),
                                            y: Some("-50".to_string()),
                                            scale: Some(0.9),
                                            scale_x: None,
                                            scale_y: None,
                                            rotation: None,
                                            easing: None,
                                        },
                                    ],
                                }),
                            },
                            // Card Container
                            openmedia_video::SceneElement::Shape {
                                shape: openmedia_video::ShapeType::RoundedRect,
                                size: openmedia_video::Size {
                                    width: openmedia_video::DimensionValue::Pixels(920.0),
                                    height: openmedia_video::DimensionValue::Pixels(1000.0),
                                },
                                position: openmedia_video::Position {
                                    x: openmedia_video::DimensionValue::Pixels(80.0),
                                    y: openmedia_video::DimensionValue::Pixels(460.0),
                                },
                                style: openmedia_video::ShapeStyle {
                                    fill: Some("#1e1b4b".to_string()),
                                    stroke: Some("#fbbf24".to_string()),
                                    stroke_width: Some(3.0),
                                    border_radius: Some(24.0),
                                    opacity: Some(0.85),
                                },
                                timeline: Some(openmedia_video::ElementTimeline {
                                    keyframes: vec![
                                        openmedia_video::Keyframe {
                                            time: 0.0,
                                            opacity: Some(0.0),
                                            x: None,
                                            y: None,
                                            scale: Some(0.85),
                                            scale_x: None,
                                            scale_y: None,
                                            rotation: None,
                                            easing: Some("ease_out".to_string()),
                                        },
                                        openmedia_video::Keyframe {
                                            time: 0.8,
                                            opacity: Some(0.85),
                                            x: None,
                                            y: None,
                                            scale: Some(1.0),
                                            scale_x: None,
                                            scale_y: None,
                                            rotation: None,
                                            easing: None,
                                        },
                                    ],
                                }),
                            },
                            // Title text (positioned inside card space)
                            openmedia_video::SceneElement::Text {
                                content: title.clone(),
                                style: openmedia_video::TextStyle {
                                    font_family: "sans-serif".to_string(),
                                    font_size: 48.0,
                                    font_weight: 700,
                                    color: "#fbbf24".to_string(),
                                    text_align: "center".to_string(),
                                    line_height: None,
                                    letter_spacing: None,
                                },
                                position: openmedia_video::Position {
                                    x: openmedia_video::DimensionValue::Pixels((width / 2) as f64),
                                    y: openmedia_video::DimensionValue::Pixels(580.0),
                                },
                                anchor: openmedia_video::Anchor::Center,
                                timeline: Some(openmedia_video::ElementTimeline {
                                    keyframes: vec![
                                        openmedia_video::Keyframe {
                                            time: 0.2,
                                            opacity: Some(0.0),
                                            x: None,
                                            y: Some("-20".to_string()),
                                            scale: None,
                                            scale_x: None,
                                            scale_y: None,
                                            rotation: None,
                                            easing: Some("ease_out".to_string()),
                                        },
                                        openmedia_video::Keyframe {
                                            time: 0.8,
                                            opacity: Some(1.0),
                                            x: None,
                                            y: Some("0".to_string()),
                                            scale: None,
                                            scale_x: None,
                                            scale_y: None,
                                            rotation: None,
                                            easing: None,
                                        },
                                    ],
                                }),
                            },
                            // Content text
                            openmedia_video::SceneElement::Text {
                                content: point_text,
                                style: openmedia_video::TextStyle {
                                    font_family: "sans-serif".to_string(),
                                    font_size: 56.0,
                                    font_weight: 600,
                                    color: "#ffffff".to_string(),
                                    text_align: "center".to_string(),
                                    line_height: None,
                                    letter_spacing: None,
                                },
                                position: openmedia_video::Position {
                                    x: openmedia_video::DimensionValue::Pixels((width / 2) as f64),
                                    y: openmedia_video::DimensionValue::Pixels((height / 2) as f64),
                                },
                                anchor: openmedia_video::Anchor::Center,
                                timeline: Some(openmedia_video::ElementTimeline {
                                    keyframes: vec![
                                        openmedia_video::Keyframe {
                                            time: 0.4,
                                            opacity: Some(0.0),
                                            x: None,
                                            y: Some("50".to_string()),
                                            scale: None,
                                            scale_x: None,
                                            scale_y: None,
                                            rotation: None,
                                            easing: Some("ease_out".to_string()),
                                        },
                                        openmedia_video::Keyframe {
                                            time: 1.0,
                                            opacity: Some(1.0),
                                            x: None,
                                            y: Some("0".to_string()),
                                            scale: None,
                                            scale_x: None,
                                            scale_y: None,
                                            rotation: None,
                                            easing: None,
                                        },
                                    ],
                                }),
                            },
                        ],
                    });

                    let from = format!("scene_{}", i);
                    let to = scene_id;
                    transitions.push(openmedia_video::SceneTransition {
                        from,
                        to,
                        transition_type: custom_trans_type.clone(),
                        duration: custom_duration,
                        easing: custom_easing.clone(),
                    });
                }

                let total_duration = 3.0 + content_arr.len() as f64 * duration;

                openmedia_video::VideoScene {
                    width,
                    height,
                    fps,
                    duration: total_duration,
                    background: bg_color,
                    scenes,
                    transitions,
                    audio: parse_audio_config(&req.parameters),
                    custom_fonts: parse_custom_fonts(&req.parameters),
                }
            }
            "product_showcase" => {
                let name = req.parameters["product_name"]
                    .as_str()
                    .unwrap_or("Product")
                    .to_string();
                let image_src = req.parameters["product_image"]
                    .as_str()
                    .ok_or_else(|| "Missing parameters.product_image path".to_string())?
                    .to_string();
                let features_arr: &Vec<serde_json::Value> =
                    req.parameters["features"]
                        .as_array()
                        .ok_or_else(|| "Missing parameters.features array".to_string())?;
                let duration = req.parameters["scene_duration"].as_f64().unwrap_or(3.0);
                let bg_color = req.parameters["background_color"]
                    .as_str()
                    .unwrap_or("#111827")
                    .to_string();
                let width = req.parameters["width"].as_u64().unwrap_or(1920) as u32;
                let height = req.parameters["height"].as_u64().unwrap_or(1080) as u32;
                let fps = req.parameters["fps"].as_u64().unwrap_or(30) as u32;

                let mut scenes = Vec::new();
                let mut transitions = Vec::new();

                scenes.push(openmedia_video::Scene {
                    id: "scene_0".to_string(),
                    start: 0.0,
                    end: 3.0,
                    elements: vec![
                        openmedia_video::SceneElement::Text {
                            content: name.clone(),
                            style: openmedia_video::TextStyle {
                                font_family: "sans-serif".to_string(),
                                font_size: 64.0,
                                font_weight: 700,
                                color: "#3b82f6".to_string(),
                                text_align: "center".to_string(),
                                line_height: None,
                                letter_spacing: None,
                            },
                            position: openmedia_video::Position {
                                x: openmedia_video::DimensionValue::Pixels((width / 2) as f64),
                                y: openmedia_video::DimensionValue::Pixels(150.0),
                            },
                            anchor: openmedia_video::Anchor::Center,
                            timeline: None,
                        },
                        openmedia_video::SceneElement::Image {
                            src: image_src.clone(),
                            position: openmedia_video::Position {
                                x: openmedia_video::DimensionValue::Pixels((width / 2) as f64),
                                y: openmedia_video::DimensionValue::Pixels(
                                    (height / 2 + 100) as f64,
                                ),
                            },
                            size: openmedia_video::Size {
                                width: openmedia_video::DimensionValue::Pixels(600.0),
                                height: openmedia_video::DimensionValue::Pixels(450.0),
                            },
                            fit: openmedia_video::ObjectFit::Contain,
                            timeline: Some(openmedia_video::ElementTimeline {
                                keyframes: vec![
                                    openmedia_video::Keyframe {
                                        time: 0.0,
                                        opacity: Some(0.0),
                                        x: None,
                                        y: None,
                                        scale: Some(0.8),
                                        scale_x: None,
                                        scale_y: None,
                                        rotation: None,
                                        easing: Some("ease_out".to_string()),
                                    },
                                    openmedia_video::Keyframe {
                                        time: 1.0,
                                        opacity: Some(1.0),
                                        x: None,
                                        y: None,
                                        scale: Some(1.0),
                                        scale_x: None,
                                        scale_y: None,
                                        rotation: None,
                                        easing: None,
                                    },
                                ],
                            }),
                        },
                    ],
                });

                let (custom_trans_type, custom_duration, custom_easing) = parse_transition_params(
                    &req.parameters,
                    openmedia_video::TransitionType::Crossfade,
                );

                for (i, feature_val) in features_arr.iter().enumerate() {
                    let feature_text = feature_val.as_str().unwrap_or("").to_string();
                    let scene_id = format!("scene_{}", i + 1);
                    let start = 3.0 + i as f64 * duration;
                    let end = start + duration;

                    scenes.push(openmedia_video::Scene {
                        id: scene_id.clone(),
                        start,
                        end,
                        elements: vec![
                            openmedia_video::SceneElement::Text {
                                content: name.clone(),
                                style: openmedia_video::TextStyle {
                                    font_family: "sans-serif".to_string(),
                                    font_size: 40.0,
                                    font_weight: 700,
                                    color: "#3b82f6".to_string(),
                                    text_align: "left".to_string(),
                                    line_height: None,
                                    letter_spacing: None,
                                },
                                position: openmedia_video::Position {
                                    x: openmedia_video::DimensionValue::Pixels(150.0),
                                    y: openmedia_video::DimensionValue::Pixels(100.0),
                                },
                                anchor: openmedia_video::Anchor::TopLeft,
                                timeline: None,
                            },
                            openmedia_video::SceneElement::Image {
                                src: image_src.clone(),
                                position: openmedia_video::Position {
                                    x: openmedia_video::DimensionValue::Pixels(450.0),
                                    y: openmedia_video::DimensionValue::Pixels((height / 2) as f64),
                                },
                                size: openmedia_video::Size {
                                    width: openmedia_video::DimensionValue::Pixels(600.0),
                                    height: openmedia_video::DimensionValue::Pixels(450.0),
                                },
                                fit: openmedia_video::ObjectFit::Contain,
                                timeline: None,
                            },
                            openmedia_video::SceneElement::Text {
                                content: feature_text,
                                style: openmedia_video::TextStyle {
                                    font_family: "sans-serif".to_string(),
                                    font_size: 52.0,
                                    font_weight: 600,
                                    color: "#ffffff".to_string(),
                                    text_align: "left".to_string(),
                                    line_height: None,
                                    letter_spacing: None,
                                },
                                position: openmedia_video::Position {
                                    x: openmedia_video::DimensionValue::Pixels(
                                        (width / 2 + 100) as f64,
                                    ),
                                    y: openmedia_video::DimensionValue::Pixels((height / 2) as f64),
                                },
                                anchor: openmedia_video::Anchor::CenterLeft,
                                timeline: Some(openmedia_video::ElementTimeline {
                                    keyframes: vec![
                                        openmedia_video::Keyframe {
                                            time: 0.0,
                                            opacity: Some(0.0),
                                            x: Some("-50".to_string()),
                                            y: None,
                                            scale: None,
                                            scale_x: None,
                                            scale_y: None,
                                            rotation: None,
                                            easing: Some("ease_out".to_string()),
                                        },
                                        openmedia_video::Keyframe {
                                            time: 0.8,
                                            opacity: Some(1.0),
                                            x: Some("0".to_string()),
                                            y: None,
                                            scale: None,
                                            scale_x: None,
                                            scale_y: None,
                                            rotation: None,
                                            easing: None,
                                        },
                                    ],
                                }),
                            },
                        ],
                    });

                    let from = format!("scene_{}", i);
                    let to = scene_id;
                    transitions.push(openmedia_video::SceneTransition {
                        from,
                        to,
                        transition_type: custom_trans_type.clone(),
                        duration: custom_duration,
                        easing: custom_easing.clone(),
                    });
                }

                let total_duration = 3.0 + features_arr.len() as f64 * duration;

                openmedia_video::VideoScene {
                    width,
                    height,
                    fps,
                    duration: total_duration,
                    background: bg_color,
                    scenes,
                    transitions,
                    audio: parse_audio_config(&req.parameters),
                    custom_fonts: parse_custom_fonts(&req.parameters),
                }
            }
            _ => {
                let template_path =
                    get_templates_dir().join(format!("{}.json", req.template_name.to_lowercase()));
                if template_path.exists() && template_path.is_file() {
                    let s = std::fs::read_to_string(&template_path).map_err(|e| {
                        format!(
                            "Failed to read custom template '{}': {}",
                            req.template_name, e
                        )
                    })?;
                    let custom_tmpl: serde_json::Value = serde_json::from_str(&s)
                        .map_err(|e| format!("Failed to parse custom template JSON: {}", e))?;

                    let scene_template = custom_tmpl.get("scene_template").ok_or_else(|| {
                        "Custom template missing 'scene_template' field".to_string()
                    })?;

                    let interpolated = interpolate_template(scene_template, &req.parameters)?;
                    let scene: openmedia_video::VideoScene = serde_json::from_value(interpolated)
                        .map_err(|e| {
                        format!("Failed to deserialize interpolated video scene: {}", e)
                    })?;
                    scene
                } else {
                    let width = req.parameters["width"].as_u64().unwrap_or(1920) as u32;
                    let height = req.parameters["height"].as_u64().unwrap_or(1080) as u32;
                    let fps = req.parameters["fps"].as_u64().unwrap_or(30) as u32;
                    openmedia_video::VideoScene {
                        width,
                        height,
                        fps,
                        duration: 3.0,
                        background: "#333333".to_string(),
                        scenes: vec![openmedia_video::Scene {
                            id: "scene_0".to_string(),
                            start: 0.0,
                            end: 3.0,
                            elements: vec![openmedia_video::SceneElement::Text {
                                content: format!("Template: {}", req.template_name),
                                style: openmedia_video::TextStyle {
                                    font_family: "sans-serif".to_string(),
                                    font_size: 36.0,
                                    font_weight: 400,
                                    color: "#ffffff".to_string(),
                                    text_align: "center".to_string(),
                                    line_height: None,
                                    letter_spacing: None,
                                },
                                position: openmedia_video::Position {
                                    x: openmedia_video::DimensionValue::Pixels((width / 2) as f64),
                                    y: openmedia_video::DimensionValue::Pixels((height / 2) as f64),
                                },
                                anchor: openmedia_video::Anchor::Center,
                                timeline: None,
                            }],
                        }],
                        transitions: vec![],
                        audio: parse_audio_config(&req.parameters),
                        custom_fonts: parse_custom_fonts(&req.parameters),
                    }
                }
            }
        };

        let video_spec = openmedia_video::render_video_scene(&scene, &output_path)
            .await
            .map_err(|e| e.to_string())?;

        serde_json::to_value(video_spec)
            .map(McpObject)
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[rmcp::tool(
        name = "video_extract_frames",
        description = "Extract keyframe images from a video file at specified time offsets."
    )]
    pub async fn video_extract_frames(
        &self,
        params: Parameters<VideoExtractFramesRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let input = std::path::Path::new(&req.video_path);
        if !input.exists() || !input.is_file() {
            return Err(format!("Input video file not found: {}", req.video_path));
        }

        let output_dir = std::path::Path::new(&req.output_dir);
        let _ = std::fs::create_dir_all(output_dir);
        let format = req.format.unwrap_or_else(|| "png".to_string());

        let mut outputs = Vec::new();

        for (i, offset) in req.offsets.iter().enumerate() {
            let filename = format!("frame_{}_{}.{}", i, uuid::Uuid::now_v7(), format);
            let output_path = output_dir.join(filename);

            let mut cmd = tokio::process::Command::new("ffmpeg");
            cmd.args([
                "-y",
                "-ss",
                &offset.to_string(),
                "-i",
                &req.video_path,
                "-vframes",
                "1",
                &output_path.to_string_lossy(),
            ]);

            cmd.stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());

            let mut child = cmd.spawn().map_err(|e| e.to_string())?;
            child.wait().await.map_err(|e| e.to_string())?;

            if output_path.exists() {
                let (w, h) = image::image_dimensions(&output_path).unwrap_or((0, 0));
                let file_size = std::fs::metadata(&output_path)
                    .map(|m| m.len())
                    .unwrap_or(0);
                outputs.push(openmedia_core::ImageOutput {
                    path: output_path,
                    width: w,
                    height: h,
                    seed: 0,
                    format: format.clone(),
                    file_size,
                    generation_id: uuid::Uuid::now_v7().to_string(),
                    clip_score: None,
                    aesthetic_score: None,
                    model_used: "ffmpeg".to_string(),
                    backend_used: "cpu".to_string(),
                    generation_time: 0.0,
                });
            }
        }

        serde_json::to_value(outputs)
            .map(McpObject)
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[rmcp::tool(
        name = "video_trim",
        description = "Trim a video file to a specified start and end time range."
    )]
    pub async fn video_trim(
        &self,
        params: Parameters<VideoTrimRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let input = std::path::Path::new(&req.video_path);
        if !input.exists() || !input.is_file() {
            return Err(format!("Input video file not found: {}", req.video_path));
        }

        let output_path = if let Some(out_p) = req.output_path {
            std::path::PathBuf::from(out_p)
        } else {
            let filename = format!("trimmed_{}.mp4", uuid::Uuid::now_v7());
            self.config.paths.output_dir.join(filename)
        };

        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);

        let duration = req.end_time - req.start_time;
        if duration <= 0.0 {
            return Err("End time must be greater than start time".to_string());
        }

        let mut cmd = tokio::process::Command::new("ffmpeg");
        cmd.args([
            "-y",
            "-ss",
            &req.start_time.to_string(),
            "-to",
            &req.end_time.to_string(),
            "-i",
            &req.video_path,
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            &output_path.to_string_lossy(),
        ]);

        cmd.stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());

        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        child.wait().await.map_err(|e| e.to_string())?;

        let file_size = std::fs::metadata(&output_path)
            .map(|m| m.len())
            .unwrap_or(0);
        let spec = openmedia_core::VideoSpec {
            path: output_path,
            width: 0,
            height: 0,
            duration,
            fps: 0,
            codec: "h264".to_string(),
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
            renderer_used: "ffmpeg".to_string(),
            total_frames: 0,
            generation_time: 0.0,
        };

        serde_json::to_value(spec)
            .map(McpObject)
            .map(Json)
            .map_err(|e| e.to_string())
    }

}
