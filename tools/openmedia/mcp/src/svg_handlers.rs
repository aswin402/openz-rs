//! SVG layout, chart, and icon handlers.

use super::{
    ChartPointDto, CreateChartRequest, CreateIconRequest, CreateSvgRequest, Json, McpObject,
    OpenMediaServer, Parameters,
};
use rmcp::handler::server::tool::ToolRouter;

pub(crate) fn router() -> ToolRouter<OpenMediaServer> {
    OpenMediaServer::svg_router()
}

#[rmcp::tool_router(router = svg_router)]
impl OpenMediaServer {
    #[rmcp::tool(
        name = "create_svg",
        description = "Generate a custom SVG layout from a list of JSON-defined shapes and primitives and save to output directory. DESIGN TIPS: Use cohesive, non-primary color schemes (e.g. slate, teals, pastel gradients). Place text elements carefully and use relative or calculated coordinates."
    )]
    pub async fn create_svg(
        &self,
        params: Parameters<CreateSvgRequest>,
    ) -> Result<Json<McpObject>, String> {
        let mut req = params.0;
        req.elements = super::parse_json_value_field(&req.elements, "elements")?;
        let start_time = std::time::Instant::now();
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);

        let svg_content = openmedia_svg::build_svg_from_json(req.width, req.height, &req.elements)
            .map_err(|e| e.to_string())?;

        let filename = format!("{}.svg", uuid::Uuid::now_v7());
        let output_path = self.config.paths.output_dir.join(filename);

        std::fs::write(&output_path, &svg_content)
            .map_err(|e| format!("Failed to write SVG output: {}", e))?;

        let file_size = std::fs::metadata(&output_path)
            .map(|m| m.len())
            .unwrap_or(svg_content.len() as u64);

        let (w, h) = super::parse_svg_dimensions(&svg_content);
        let generation_time = start_time.elapsed().as_secs_f64();

        let output = openmedia_core::ImageOutput {
            path: output_path,
            width: w,
            height: h,
            seed: 0,
            format: "svg".to_string(),
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
            clip_score: None,
            aesthetic_score: None,
            model_used: "svg-builder".to_string(),
            backend_used: "svg-builder".to_string(),
            generation_time,
        };

        serde_json::to_value(output)
            .map(McpObject)
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[rmcp::tool(
        name = "create_chart",
        description = "Generate a custom bar, line, or pie chart from a list of data points and save to output directory. DESIGN TIPS: Match the theme ('dark' or 'light') to the parent scene. Maintain padding around margins (e.g. 50px-60px) to prevent label clipping."
    )]
    pub async fn create_chart(
        &self,
        params: Parameters<CreateChartRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let start_time = std::time::Instant::now();
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);

        let width = req.width.unwrap_or(800);
        let height = req.height.unwrap_or(600);

        let data: Vec<openmedia_svg::ChartPoint> = req
            .data
            .into_iter()
            .map(|p: ChartPointDto| openmedia_svg::ChartPoint {
                label: p.label,
                value: p.value,
            })
            .collect();

        let svg_content = openmedia_svg::create_chart(
            &req.chart_type,
            req.title.as_deref(),
            &data,
            width,
            height,
        )
        .map_err(|e| e.to_string())?;

        let filename = format!("{}.svg", uuid::Uuid::now_v7());
        let output_path = self.config.paths.output_dir.join(filename);

        std::fs::write(&output_path, &svg_content)
            .map_err(|e| format!("Failed to write SVG output: {}", e))?;

        let file_size = std::fs::metadata(&output_path)
            .map(|m| m.len())
            .unwrap_or(svg_content.len() as u64);

        let (w, h) = super::parse_svg_dimensions(&svg_content);
        let generation_time = start_time.elapsed().as_secs_f64();

        let output = openmedia_core::ImageOutput {
            path: output_path,
            width: w,
            height: h,
            seed: 0,
            format: "svg".to_string(),
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
            clip_score: None,
            aesthetic_score: None,
            model_used: "chart-builder".to_string(),
            backend_used: "chart-builder".to_string(),
            generation_time,
        };

        serde_json::to_value(output)
            .map(McpObject)
            .map(Json)
            .map_err(|e| e.to_string())
    }

    #[rmcp::tool(
        name = "create_icon",
        description = "Retrieve a custom scaled vector interface icon by name and save to output directory"
    )]
    pub async fn create_icon(
        &self,
        params: Parameters<CreateIconRequest>,
    ) -> Result<Json<McpObject>, String> {
        let req = params.0;
        let start_time = std::time::Instant::now();
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);

        let size = req.size.unwrap_or(24);
        let color = req.color.unwrap_or_else(|| "#ffffff".to_string());
        let stroke_width = req.stroke_width.unwrap_or(2.0);

        let svg_content = openmedia_svg::get_icon_svg(&req.name, size, &color, stroke_width)
            .ok_or_else(|| format!("Icon '{}' not found in library", req.name))?;

        let filename = format!("{}.svg", uuid::Uuid::now_v7());
        let output_path = self.config.paths.output_dir.join(filename);

        std::fs::write(&output_path, &svg_content)
            .map_err(|e| format!("Failed to write SVG output: {}", e))?;

        let file_size = std::fs::metadata(&output_path)
            .map(|m| m.len())
            .unwrap_or(svg_content.len() as u64);

        let (w, h) = super::parse_svg_dimensions(&svg_content);
        let generation_time = start_time.elapsed().as_secs_f64();

        let output = openmedia_core::ImageOutput {
            path: output_path,
            width: w,
            height: h,
            seed: 0,
            format: "svg".to_string(),
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
            clip_score: None,
            aesthetic_score: None,
            model_used: "icon-library".to_string(),
            backend_used: "icon-library".to_string(),
            generation_time,
        };

        serde_json::to_value(output)
            .map(McpObject)
            .map(Json)
            .map_err(|e| e.to_string())
    }
}
