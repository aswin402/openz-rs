//! SVG animation and Lottie conversion handlers.

use super::{
    AnimateMorphRequest, AnimateSvgRequest, AnimateTimelineRequest, GenerateSpinnerRequest, Json,
    LottieToSvgRequest, McpObject, OpenMediaServer, Parameters, SvgToLottieRequest,
};
use rmcp::handler::server::tool::ToolRouter;

pub(crate) fn router() -> ToolRouter<OpenMediaServer> {
    OpenMediaServer::animation_router()
}

#[rmcp::tool_router(router = animation_router)]
impl OpenMediaServer {
    #[rmcp::tool(
        name = "animate_svg",
        description = "Apply animation presets (such as fade_in, spin, bounce, pulse, typewriter, draw_path) to SVG elements."
    )]
    pub async fn animate_svg(
        &self,
        params: Parameters<AnimateSvgRequest>,
    ) -> Result<Json<McpObject>, String> {
        use openmedia_animate::*;
        let req = params.0;
        let svg_content = if req.svg.trim().starts_with('<') {
            req.svg
        } else {
            let path = std::path::Path::new(&req.svg);
            if path.exists() && path.is_file() {
                std::fs::read_to_string(path).map_err(|e| e.to_string())?
            } else {
                req.svg
            }
        };

        let preset = super::parse_preset(&req.preset);
        let duration = req.duration.unwrap_or(1.0);
        let delay = req.delay.unwrap_or(0.0);
        let easing = super::parse_easing(req.easing.as_deref());
        let extra_params = req.params.clone().unwrap_or(serde_json::Value::Null);

        let output = preset
            .generate(duration, delay, &easing, &extra_params)
            .map_err(|e| e.to_string())?;

        let (animated_svg, animation_count) = match output {
            AnimationOutput::Smil(anims) => {
                let animation_count = anims.len() as u32;
                let mut xml_block = String::new();
                for anim in anims {
                    xml_block.push_str("  ");
                    xml_block.push_str(&anim.to_xml(Some(&req.element_id)));
                    xml_block.push('\n');
                }
                (
                    super::inject_style_or_xml(svg_content, &xml_block),
                    animation_count,
                )
            }
            AnimationOutput::Css(keyframes) => {
                let animated_svg =
                    super::inject_css_class(&svg_content, &req.element_id, &keyframes.name);
                let style_block = format!("  <style>\n    {}\n  </style>\n", keyframes.to_css());
                (
                    super::inject_style_or_xml(animated_svg, &style_block),
                    1,
                )
            }
            AnimationOutput::Combined { smil, css } => {
                let animated_svg =
                    super::inject_css_class(&svg_content, &req.element_id, &css.name);
                let mut xml_block = format!("  <style>\n    {}\n  </style>\n", css.to_css());
                for anim in &smil {
                    xml_block.push_str("  ");
                    xml_block.push_str(&anim.to_xml(Some(&req.element_id)));
                    xml_block.push('\n');
                }
                (
                    super::inject_style_or_xml(animated_svg, &xml_block),
                    (smil.len() + 1) as u32,
                )
            }
        };

        let filename = format!("{}.svg", uuid::Uuid::now_v7());
        let output_path = self.config.paths.output_dir.join(filename);
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);
        std::fs::write(&output_path, &animated_svg).map_err(|e| e.to_string())?;

        let file_size = std::fs::metadata(&output_path)
            .map(|m| m.len())
            .unwrap_or(0);
        let (width, height) = super::parse_svg_dimensions(&animated_svg);

        let result = openmedia_core::AnimatedSvgOutput {
            path: output_path,
            width,
            height,
            duration,
            animation_count,
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
        };

        serde_json::to_value(result)
            .map(McpObject)
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[rmcp::tool(
        name = "animate_create_timeline",
        description = "Sequentially or concurrently coordinate animations of multiple elements."
    )]
    pub async fn animate_create_timeline(
        &self,
        params: Parameters<AnimateTimelineRequest>,
    ) -> Result<Json<McpObject>, String> {
        use openmedia_animate::*;
        let req = params.0;
        let svg_content = if req.svg.trim().starts_with('<') {
            req.svg
        } else {
            let path = std::path::Path::new(&req.svg);
            if path.exists() && path.is_file() {
                std::fs::read_to_string(path).map_err(|e| e.to_string())?
            } else {
                req.svg
            }
        };

        let mode = match req.mode.to_lowercase().as_str() {
            "sequential" => TimelineMode::Sequential,
            "staggered" => TimelineMode::Staggered {
                delay: req.stagger_delay.unwrap_or(0.2),
            },
            _ => TimelineMode::Parallel,
        };

        let mut timeline = AnimationTimeline::new(mode);

        for entry in &req.entries {
            let preset = super::parse_preset(&entry.preset);
            let easing = super::parse_easing(entry.easing.as_deref());
            let entry_params = entry.params.clone().unwrap_or(serde_json::Value::Null);

            let out = preset
                .generate(entry.duration, entry.offset, &easing, &entry_params)
                .map_err(|e| e.to_string())?;

            match out {
                AnimationOutput::Smil(anims) => {
                    for anim in anims {
                        timeline.add(&entry.element_id, anim);
                    }
                }
                AnimationOutput::Css(_keyframes) => {
                    let anim = SmilAnimation::Animate {
                        attribute_name: "opacity".to_string(),
                        from: "0".to_string(),
                        to: "1".to_string(),
                        dur: entry.duration,
                        begin: entry.offset,
                        fill: AnimationFill::Freeze,
                        repeat_count: RepeatCount::Definite(1),
                        easing,
                    };
                    timeline.add(&entry.element_id, anim);
                }
                AnimationOutput::Combined { smil, .. } => {
                    for anim in smil {
                        timeline.add(&entry.element_id, anim);
                    }
                }
            }
        }

        let timeline_xml = timeline.to_svg();
        let animated_svg = super::inject_style_or_xml(svg_content, &timeline_xml);

        let filename = format!("{}.svg", uuid::Uuid::now_v7());
        let output_path = self.config.paths.output_dir.join(filename);
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);
        std::fs::write(&output_path, &animated_svg).map_err(|e| e.to_string())?;

        let file_size = std::fs::metadata(&output_path)
            .map(|m| m.len())
            .unwrap_or(0);
        let (width, height) = super::parse_svg_dimensions(&animated_svg);

        let result = openmedia_core::AnimatedSvgOutput {
            path: output_path,
            width,
            height,
            duration: timeline.total_duration,
            animation_count: timeline.animations.len() as u32,
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
        };

        serde_json::to_value(result)
            .map(McpObject)
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[rmcp::tool(
        name = "animate_morph_paths",
        description = "Interpolate morph frames between two path data strings."
    )]
    pub async fn animate_morph_paths(
        &self,
        params: Parameters<AnimateMorphRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let duration = req.duration.unwrap_or(3.0);
        let easing = super::parse_easing(req.easing.as_deref());

        let frames = openmedia_animate::morph_paths(&req.from_path, &req.to_path, 30, &easing)
            .map_err(|e| e.to_string())?;

        let values_attr = frames.join("; ");
        let animated_svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 800 600\" width=\"800\" height=\"600\">\n  \
               <path d=\"{}\" fill=\"none\" stroke=\"#8b5cf6\" stroke-width=\"4\">\n    \
                 <animate attributeName=\"d\" values=\"{}\" dur=\"{}s\" repeatCount=\"indefinite\" />\n  \
               </path>\n\
             </svg>",
            req.from_path, values_attr, duration
        );

        let filename = format!("{}.svg", uuid::Uuid::now_v7());
        let output_path = self.config.paths.output_dir.join(filename);
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);
        std::fs::write(&output_path, &animated_svg).map_err(|e| e.to_string())?;

        let file_size = std::fs::metadata(&output_path)
            .map(|m| m.len())
            .unwrap_or(0);

        let result = openmedia_core::AnimatedSvgOutput {
            path: output_path,
            width: 800,
            height: 600,
            duration,
            animation_count: 1,
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
        };

        serde_json::to_value(result)
            .map(McpObject)
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[rmcp::tool(
        name = "animate_generate_spinner",
        description = "Generate beautiful animated loading spinner SVGs."
    )]
    pub async fn animate_generate_spinner(
        &self,
        params: Parameters<GenerateSpinnerRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let color = req.color.unwrap_or_else(|| "#8b5cf6".to_string());
        let size = req.size.unwrap_or(60);

        let animated_svg = match req.spinner_type.to_lowercase().as_str() {
            "ring" => {
                format!(
                    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 50 50\">\n  \
                       <path d=\"M 25 5 A 20 20 0 0 1 45 25\" fill=\"none\" stroke=\"{}\" stroke-width=\"4\" stroke-linecap=\"round\">\n    \
                         <animateTransform attributeName=\"transform\" type=\"rotate\" from=\"0 25 25\" to=\"360 25 25\" dur=\"1s\" repeatCount=\"indefinite\" />\n  \
                       </path>\n  \
                       <circle cx=\"25\" cy=\"25\" r=\"20\" fill=\"none\" stroke=\"{}\" stroke-width=\"4\" opacity=\"0.2\" />\n\
                     </svg>",
                    size, size, color, color
                )
            }
            "dots" => {
                format!(
                    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 120 30\">\n  \
                       <circle cx=\"20\" cy=\"15\" r=\"8\" fill=\"{}\">\n    \
                         <animate attributeName=\"cy\" values=\"15; 5; 15\" dur=\"1s\" begin=\"0s\" repeatCount=\"indefinite\" />\n    \
                         <animate attributeName=\"opacity\" values=\"0.3; 1; 0.3\" dur=\"1s\" begin=\"0s\" repeatCount=\"indefinite\" />\n  \
                       </circle>\n  \
                       <circle cx=\"60\" cy=\"15\" r=\"8\" fill=\"{}\">\n    \
                         <animate attributeName=\"cy\" values=\"15; 5; 15\" dur=\"1s\" begin=\"0.25s\" repeatCount=\"indefinite\" />\n    \
                         <animate attributeName=\"opacity\" values=\"0.3; 1; 0.3\" dur=\"1s\" begin=\"0.25s\" repeatCount=\"indefinite\" />\n  \
                       </circle>\n  \
                       <circle cx=\"100\" cy=\"15\" r=\"8\" fill=\"{}\">\n    \
                         <animate attributeName=\"cy\" values=\"15; 5; 15\" dur=\"1s\" begin=\"0.5s\" repeatCount=\"indefinite\" />\n    \
                         <animate attributeName=\"opacity\" values=\"0.3; 1; 0.3\" dur=\"1s\" begin=\"0.5s\" repeatCount=\"indefinite\" />\n  \
                       </circle>\n\
                     </svg>",
                    size, size, color, color, color
                )
            }
            "bars" => {
                format!(
                    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 50 50\">\n  \
                       <rect x=\"10\" y=\"15\" width=\"6\" height=\"20\" fill=\"{}\">\n    \
                         <animate attributeName=\"height\" values=\"20; 40; 20\" dur=\"1s\" begin=\"0s\" repeatCount=\"indefinite\" />\n    \
                         <animate attributeName=\"y\" values=\"15; 5; 15\" dur=\"1s\" begin=\"0s\" repeatCount=\"indefinite\" />\n  \
                       </rect>\n  \
                       <rect x=\"22\" y=\"15\" width=\"6\" height=\"20\" fill=\"{}\">\n    \
                         <animate attributeName=\"height\" values=\"20; 40; 20\" dur=\"1s\" begin=\"0.2s\" repeatCount=\"indefinite\" />\n    \
                         <animate attributeName=\"y\" values=\"15; 5; 15\" dur=\"1s\" begin=\"0.2s\" repeatCount=\"indefinite\" />\n  \
                       </rect>\n  \
                       <rect x=\"34\" y=\"15\" width=\"6\" height=\"20\" fill=\"{}\">\n    \
                         <animate attributeName=\"height\" values=\"20; 40; 20\" dur=\"1s\" begin=\"0.4s\" repeatCount=\"indefinite\" />\n    \
                         <animate attributeName=\"y\" values=\"15; 5; 15\" dur=\"1s\" begin=\"0.4s\" repeatCount=\"indefinite\" />\n  \
                       </rect>\n\
                     </svg>",
                    size, size, color, color, color
                )
            }
            _ => {
                format!(
                    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 50 50\">\n  \
                       <circle cx=\"25\" cy=\"25\" r=\"20\" fill=\"none\" stroke=\"{}\" stroke-width=\"4\" stroke-dasharray=\"31.4 31.4\" stroke-linecap=\"round\">\n    \
                         <animateTransform attributeName=\"transform\" type=\"rotate\" from=\"0 25 25\" to=\"360 25 25\" dur=\"1.2s\" repeatCount=\"indefinite\" />\n  \
                       </circle>\n\
                     </svg>",
                    size, size, color
                )
            }
        };

        let filename = format!("{}.svg", uuid::Uuid::now_v7());
        let output_path = self.config.paths.output_dir.join(filename);
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);
        std::fs::write(&output_path, &animated_svg).map_err(|e| e.to_string())?;

        let file_size = std::fs::metadata(&output_path)
            .map(|m| m.len())
            .unwrap_or(0);

        let result = openmedia_core::AnimatedSvgOutput {
            path: output_path,
            width: size,
            height: size,
            duration: 1.0,
            animation_count: 1,
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
        };

        serde_json::to_value(result)
            .map(McpObject)
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[rmcp::tool(
        name = "animate_from_lottie",
        description = "Import Lottie JSON and convert to an animated SVG."
    )]
    pub async fn animate_from_lottie(
        &self,
        params: Parameters<LottieToSvgRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let lottie_json = if req.lottie_json.trim().starts_with('{') {
            req.lottie_json
        } else {
            let path = std::path::Path::new(&req.lottie_json);
            if path.exists() && path.is_file() {
                std::fs::read_to_string(path).map_err(|e| e.to_string())?
            } else {
                req.lottie_json
            }
        };

        let animated_svg =
            openmedia_animate::lottie_to_svg(&lottie_json).map_err(|e| e.to_string())?;

        let filename = format!("{}.svg", uuid::Uuid::now_v7());
        let output_path = self.config.paths.output_dir.join(filename);
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);
        std::fs::write(&output_path, &animated_svg).map_err(|e| e.to_string())?;

        let file_size = std::fs::metadata(&output_path)
            .map(|m| m.len())
            .unwrap_or(0);
        let (width, height) = super::parse_svg_dimensions(&animated_svg);

        let result = openmedia_core::AnimatedSvgOutput {
            path: output_path,
            width,
            height,
            duration: 3.0,
            animation_count: 1,
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
        };

        serde_json::to_value(result)
            .map(McpObject)
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[rmcp::tool(name = "animate_to_lottie", description = "Export SVG to Lottie JSON.")]
    pub async fn animate_to_lottie(
        &self,
        params: Parameters<SvgToLottieRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let svg_content = if req.svg.trim().starts_with('<') {
            req.svg
        } else {
            let path = std::path::Path::new(&req.svg);
            if path.exists() && path.is_file() {
                std::fs::read_to_string(path).map_err(|e| e.to_string())?
            } else {
                req.svg
            }
        };

        let lottie_json_str =
            openmedia_animate::svg_to_lottie(&svg_content).map_err(|e| e.to_string())?;

        let lottie_val: serde_json::Value =
            serde_json::from_str(&lottie_json_str).map_err(|e| e.to_string())?;

        Ok(Json(McpObject(lottie_val)))
    }
}
