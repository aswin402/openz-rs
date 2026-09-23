//! SVG layout, chart, and icon handlers.

use crate::tools::openmedia::server::*;

impl OpenMediaServer {
    pub async fn create_svg(&self, mut req: CreateSvgRequest) -> Result<serde_json::Value, String> {
        req.elements = super::helpers::parse_json_value_field(&req.elements, "elements")?;
        let start_time = std::time::Instant::now();

        let svg_content = crate::tools::openmedia::svg::build_svg_from_json(req.width, req.height, &req.elements)
            .map_err(|e| e.to_string())?;

        let output = super::helpers::save_svg_to_output(
            &self.config.paths.output_dir,
            &svg_content,
            "svg-builder",
            "svg-builder",
            start_time,
        )?;

        serde_json::to_value(output).map_err(|e| e.to_string())
    }

    pub async fn create_chart(&self, req: CreateChartRequest) -> Result<serde_json::Value, String> {
        let start_time = std::time::Instant::now();
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

        let output = super::helpers::save_svg_to_output(
            &self.config.paths.output_dir,
            &svg_content,
            "chart-builder",
            "chart-builder",
            start_time,
        )?;

        serde_json::to_value(output).map_err(|e| e.to_string())
    }

    pub async fn create_icon(&self, req: CreateIconRequest) -> Result<serde_json::Value, String> {
        let start_time = std::time::Instant::now();
        let size = req.size.unwrap_or(24);
        let color = req.color.unwrap_or_else(|| "#ffffff".to_string());
        let stroke_width = req.stroke_width.unwrap_or(2.0);

        let svg_content = crate::tools::openmedia::svg::get_icon_svg(&req.name, size, &color, stroke_width)
            .ok_or_else(|| format!("Icon '{}' not found in library", req.name))?;

        let output = super::helpers::save_svg_to_output(
            &self.config.paths.output_dir,
            &svg_content,
            "icon-library",
            "icon-library",
            start_time,
        )?;

        serde_json::to_value(output).map_err(|e| e.to_string())
    }
}

