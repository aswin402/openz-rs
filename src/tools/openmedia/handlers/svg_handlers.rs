//! SVG layout, chart, and icon handlers.

use crate::tools::openmedia::core::ImageOutput;
use crate::tools::openmedia::server::*;

impl OpenMediaServer {
    pub async fn create_svg(&self, mut req: CreateSvgRequest) -> Result<serde_json::Value, String> {
        req.elements = super::helpers::parse_json_value_field(&req.elements, "elements")?;
        let start_time = std::time::Instant::now();
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);

        let svg_content = crate::tools::openmedia::svg::build_svg_from_json(req.width, req.height, &req.elements)
            .map_err(|e| e.to_string())?;

        let filename = format!("{}.svg", uuid::Uuid::now_v7());
        let output_path = self.config.paths.output_dir.join(filename);

        std::fs::write(&output_path, &svg_content)
            .map_err(|e| format!("Failed to write SVG output: {}", e))?;

        let file_size = std::fs::metadata(&output_path)
            .map(|m| m.len())
            .unwrap_or(svg_content.len() as u64);

        let (w, h) = super::helpers::parse_svg_dimensions(&svg_content);
        let generation_time = start_time.elapsed().as_secs_f64();

        let output = ImageOutput {
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

        serde_json::to_value(output).map_err(|e| e.to_string())
    }

    pub async fn create_chart(&self, req: CreateChartRequest) -> Result<serde_json::Value, String> {
        let start_time = std::time::Instant::now();
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);

        let width = req.width.unwrap_or(800);
        let height = req.height.unwrap_or(600);

        let data: Vec<crate::tools::openmedia::svg::ChartPoint> = req
            .data
            .into_iter()
            .map(|p: ChartPointDto| crate::tools::openmedia::svg::ChartPoint {
                label: p.label,
                value: p.value,
            })
            .collect();

        let svg_content = crate::tools::openmedia::svg::create_chart(
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

        let (w, h) = super::helpers::parse_svg_dimensions(&svg_content);
        let generation_time = start_time.elapsed().as_secs_f64();

        let output = ImageOutput {
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

        serde_json::to_value(output).map_err(|e| e.to_string())
    }

    pub async fn create_icon(&self, req: CreateIconRequest) -> Result<serde_json::Value, String> {
        let start_time = std::time::Instant::now();
        let _ = std::fs::create_dir_all(&self.config.paths.output_dir);

        let size = req.size.unwrap_or(24);
        let color = req.color.unwrap_or_else(|| "#ffffff".to_string());
        let stroke_width = req.stroke_width.unwrap_or(2.0);

        let svg_content = crate::tools::openmedia::svg::get_icon_svg(&req.name, size, &color, stroke_width)
            .ok_or_else(|| format!("Icon '{}' not found in library", req.name))?;

        let filename = format!("{}.svg", uuid::Uuid::now_v7());
        let output_path = self.config.paths.output_dir.join(filename);

        std::fs::write(&output_path, &svg_content)
            .map_err(|e| format!("Failed to write SVG output: {}", e))?;

        let file_size = std::fs::metadata(&output_path)
            .map(|m| m.len())
            .unwrap_or(svg_content.len() as u64);

        let (w, h) = super::helpers::parse_svg_dimensions(&svg_content);
        let generation_time = start_time.elapsed().as_secs_f64();

        let output = ImageOutput {
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

        serde_json::to_value(output).map_err(|e| e.to_string())
    }
}
