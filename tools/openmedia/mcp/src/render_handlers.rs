//! Core model, SVG, Mermaid, and HTML rendering handlers.

use super::{
    GenerateMermaidRequest, HtmlToImageRequest, Json, McpObject, ModelDownloadRequest,
    OpenMediaServer, Parameters, RasterizeSvgRequest,
};
use rmcp::handler::server::tool::ToolRouter;

pub(crate) fn router() -> ToolRouter<OpenMediaServer> {
    OpenMediaServer::render_router()
}

#[rmcp::tool_router(router = render_router)]
impl OpenMediaServer {
    #[rmcp::tool(description = "Ping the media generation server to check status and health")]
    pub async fn ping(&self) -> String {
        format!(
            "pong (CPU: {}, GPU: {:?})",
            self.hardware.cpu.brand,
            self.hardware.gpu.as_ref().map(|g| &g.name)
        )
    }

    #[rmcp::tool(
        name = "model_download",
        description = "Download a specified model file (CLIP text/vision or Aesthetic predictor) from Hugging Face Hub with progress tracking."
    )]
    pub async fn model_download(
        &self,
        params: Parameters<ModelDownloadRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let reporter = openmedia_core::StderrProgressReporter::new(req.id.clone());

        let path = self
            .model_registry
            .download_model(&req.id, &reporter)
            .await
            .map_err(|e| e.to_string())?;

        let response = serde_json::json!({
            "status": "success",
            "model_id": req.id,
            "path": path.to_string_lossy(),
        });

        Ok(Json(McpObject(response)))
    }

    #[rmcp::tool(
        name = "rasterize_svg",
        description = "Rasterize an SVG string or file path into a PNG, JPEG, or WebP image."
    )]
    pub async fn rasterize_svg(
        &self,
        params: Parameters<RasterizeSvgRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let format = req.output_format.unwrap_or_else(|| "png".to_string());
        let filename = format!("{}.{}", uuid::Uuid::now_v7(), format);
        let output_path = self.config.paths.output_dir.join(filename);

        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);

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

        let output = openmedia_svg::rasterize(
            &svg_content,
            req.width,
            req.height,
            req.background_color.as_deref(),
            &format,
            &output_path,
        )
        .map_err(|e| e.to_string())?;

        serde_json::to_value(output)
            .map(McpObject)
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[rmcp::tool(
        name = "diagram_generate_mermaid",
        description = "Generate a flowchart, sequence diagram, or architecture diagram from a Mermaid string and save it to the output directory as SVG, PNG, JPEG, or WebP."
    )]
    pub async fn diagram_generate_mermaid(
        &self,
        params: Parameters<GenerateMermaidRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let format = req
            .output_format
            .clone()
            .unwrap_or_else(|| "svg".to_string());
        let clean_format = format.trim().to_lowercase();

        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);

        let mut layout_config = mermaid_rs_renderer::LayoutConfig::default();
        if let Some(spacing) = req.node_spacing {
            layout_config.node_spacing = spacing;
        }
        if let Some(spacing) = req.rank_spacing {
            layout_config.rank_spacing = spacing;
        }
        if let Some(ratio) = req.preferred_aspect_ratio {
            layout_config.preferred_aspect_ratio = Some(ratio);
        }

        let mut final_theme = if let Some(ref preset) = req.theme {
            super::resolve_theme_preset(preset)
        } else {
            mermaid_rs_renderer::Theme::modern()
        };

        if let Some(ref overrides) = req.custom_theme {
            super::override_theme_fields(&mut final_theme, overrides);
        }

        let svg_content =
            openmedia_svg::render_mermaid(&req.code, Some(final_theme), Some(layout_config))
                .map_err(|e| format!("Failed to render Mermaid diagram: {}", e))?;

        let start_time = std::time::Instant::now();
        let filename = format!("{}.{}", uuid::Uuid::now_v7(), clean_format);
        let output_path = self.config.paths.output_dir.join(filename);

        let output = if clean_format == "svg" {
            std::fs::write(&output_path, &svg_content)
                .map_err(|e| format!("Failed to write SVG output: {}", e))?;

            let file_size = std::fs::metadata(&output_path)
                .map(|m| m.len())
                .unwrap_or(svg_content.len() as u64);

            let (w, h) = super::parse_svg_dimensions(&svg_content);
            let generation_time = start_time.elapsed().as_secs_f64();

            openmedia_core::ImageOutput {
                path: output_path,
                width: w,
                height: h,
                seed: 0,
                format: clean_format,
                file_size,
                generation_id: uuid::Uuid::now_v7().to_string(),
                clip_score: None,
                aesthetic_score: None,
                model_used: "mermaid-rs-renderer".to_string(),
                backend_used: "mermaid-rs-renderer".to_string(),
                generation_time,
            }
        } else {
            openmedia_svg::rasterize(
                &svg_content,
                req.width,
                req.height,
                req.background_color.as_deref(),
                &clean_format,
                &output_path,
            )
            .map_err(|e| format!("Failed to rasterize Mermaid SVG: {}", e))?
        };

        serde_json::to_value(output)
            .map(McpObject)
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[rmcp::tool(
        name = "html_to_image",
        description = "Render an HTML/CSS string or file path into a PNG, JPEG, or WebP screenshot."
    )]
    pub async fn html_to_image(
        &self,
        params: Parameters<HtmlToImageRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let format = req.output_format.unwrap_or_else(|| "png".to_string());
        let filename = format!("{}.{}", uuid::Uuid::now_v7(), format);
        let output_path = self.config.paths.output_dir.join(filename);

        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);

        let output = openmedia_video::html_to_image(
            &req.html,
            req.width,
            req.height,
            req.device_scale_factor,
            &format,
            &output_path,
        )
        .await
        .map_err(|e| e.to_string())?;

        serde_json::to_value(output)
            .map(McpObject)
            .map(Json)
            .map_err(|e| e.to_string())
    }
}
