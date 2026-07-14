use openmedia_core::{ImageOutput, OpenMediaError, Result, SvgOutput};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod chart;
pub mod icons;
pub mod schema;

pub use chart::{create_chart, ChartPoint};
pub use icons::get_icon_svg;

#[derive(Debug, Clone)]
pub struct SvgBuilder {
    pub width: u32,
    pub height: u32,
    pub viewbox: Option<String>,
    pub elements: Vec<SvgElement>,
    pub defs: Vec<SvgDef>,
    pub styles: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum SvgElement {
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        rx: Option<f64>,
        ry: Option<f64>,
        attrs: Attributes,
    },
    Circle {
        cx: f64,
        cy: f64,
        r: f64,
        attrs: Attributes,
    },
    Ellipse {
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        attrs: Attributes,
    },
    Line {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        attrs: Attributes,
    },
    Polyline {
        points: Vec<(f64, f64)>,
        attrs: Attributes,
    },
    Polygon {
        points: Vec<(f64, f64)>,
        attrs: Attributes,
    },
    Path {
        d: String,
        attrs: Attributes,
    },
    Text {
        x: f64,
        y: f64,
        content: String,
        attrs: Attributes,
    },
    Group {
        elements: Vec<SvgElement>,
        attrs: Attributes,
    },
    Use {
        href: String,
        x: f64,
        y: f64,
        attrs: Attributes,
    },
    Image {
        href: String,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        attrs: Attributes,
    },
}

#[derive(Debug, Clone)]
pub enum SvgDef {
    LinearGradient {
        id: String,
        x1: String,
        y1: String,
        x2: String,
        y2: String,
        stops: Vec<GradientStop>,
    },
    RadialGradient {
        id: String,
        cx: String,
        cy: String,
        r: String,
        stops: Vec<GradientStop>,
    },
    ClipPath {
        id: String,
        elements: Vec<SvgElement>,
    },
    Filter {
        id: String,
        primitives: Vec<FilterPrimitive>,
    },
    Symbol {
        id: String,
        viewbox: String,
        elements: Vec<SvgElement>,
    },
}

#[derive(Debug, Clone)]
pub struct GradientStop {
    pub offset: String,
    pub color: String,
    pub opacity: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct FilterPrimitive {
    pub name: String,
    pub attrs: Attributes,
}

pub type Attributes = HashMap<String, String>;

pub struct RectBuilder<'a> {
    builder: &'a mut SvgBuilder,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    rx: Option<f64>,
    ry: Option<f64>,
    attrs: Attributes,
}

impl<'a> RectBuilder<'a> {
    pub fn fill(mut self, color: &str) -> Self {
        self.attrs.insert("fill".to_string(), color.to_string());
        self
    }

    pub fn stroke(mut self, color: &str) -> Self {
        self.attrs.insert("stroke".to_string(), color.to_string());
        self
    }

    pub fn stroke_width(mut self, width: f64) -> Self {
        self.attrs
            .insert("stroke-width".to_string(), width.to_string());
        self
    }

    pub fn rx(mut self, rx: f64) -> Self {
        self.rx = Some(rx);
        self
    }

    pub fn ry(mut self, ry: f64) -> Self {
        self.ry = Some(ry);
        self
    }

    pub fn finish(self) -> &'a mut SvgBuilder {
        self.builder.elements.push(SvgElement::Rect {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
            rx: self.rx,
            ry: self.ry,
            attrs: self.attrs,
        });
        self.builder
    }
}

pub struct CircleBuilder<'a> {
    builder: &'a mut SvgBuilder,
    cx: f64,
    cy: f64,
    r: f64,
    attrs: Attributes,
}

impl<'a> CircleBuilder<'a> {
    pub fn fill(mut self, color: &str) -> Self {
        self.attrs.insert("fill".to_string(), color.to_string());
        self
    }

    pub fn stroke(mut self, color: &str) -> Self {
        self.attrs.insert("stroke".to_string(), color.to_string());
        self
    }

    pub fn stroke_width(mut self, width: f64) -> Self {
        self.attrs
            .insert("stroke-width".to_string(), width.to_string());
        self
    }

    pub fn finish(self) -> &'a mut SvgBuilder {
        self.builder.elements.push(SvgElement::Circle {
            cx: self.cx,
            cy: self.cy,
            r: self.r,
            attrs: self.attrs,
        });
        self.builder
    }
}

pub struct TextBuilder<'a> {
    builder: &'a mut SvgBuilder,
    x: f64,
    y: f64,
    content: String,
    attrs: Attributes,
}

impl<'a> TextBuilder<'a> {
    pub fn fill(mut self, color: &str) -> Self {
        self.attrs.insert("fill".to_string(), color.to_string());
        self
    }

    pub fn font_size(mut self, size: f64) -> Self {
        self.attrs.insert("font-size".to_string(), size.to_string());
        self
    }

    pub fn font_family(mut self, family: &str) -> Self {
        self.attrs
            .insert("font-family".to_string(), family.to_string());
        self
    }

    pub fn finish(self) -> &'a mut SvgBuilder {
        self.builder.elements.push(SvgElement::Text {
            x: self.x,
            y: self.y,
            content: self.content,
            attrs: self.attrs,
        });
        self.builder
    }
}

pub struct PathBuilder<'a> {
    builder: &'a mut SvgBuilder,
    d: String,
    attrs: Attributes,
}

impl<'a> PathBuilder<'a> {
    pub fn fill(mut self, color: &str) -> Self {
        self.attrs.insert("fill".to_string(), color.to_string());
        self
    }

    pub fn stroke(mut self, color: &str) -> Self {
        self.attrs.insert("stroke".to_string(), color.to_string());
        self
    }

    pub fn stroke_width(mut self, width: f64) -> Self {
        self.attrs
            .insert("stroke-width".to_string(), width.to_string());
        self
    }

    pub fn opacity(mut self, opacity: f64) -> Self {
        self.attrs
            .insert("opacity".to_string(), opacity.to_string());
        self
    }

    pub fn finish(self) -> &'a mut SvgBuilder {
        self.builder.elements.push(SvgElement::Path {
            d: self.d,
            attrs: self.attrs,
        });
        self.builder
    }
}

pub struct GroupBuilder<'a> {
    builder: &'a mut SvgBuilder,
    elements: Vec<SvgElement>,
    attrs: Attributes,
}

impl<'a> GroupBuilder<'a> {
    pub fn finish(self) -> &'a mut SvgBuilder {
        self.builder.elements.push(SvgElement::Group {
            elements: self.elements,
            attrs: self.attrs,
        });
        self.builder
    }
}

pub struct GradientBuilder<'a> {
    builder: &'a mut SvgBuilder,
    id: String,
    is_radial: bool,
    stops: Vec<GradientStop>,
}

impl<'a> GradientBuilder<'a> {
    pub fn stop(mut self, offset: &str, color: &str) -> Self {
        self.stops.push(GradientStop {
            offset: offset.to_string(),
            color: color.to_string(),
            opacity: None,
        });
        self
    }

    pub fn finish(self) -> &'a mut SvgBuilder {
        if self.is_radial {
            self.builder.defs.push(SvgDef::RadialGradient {
                id: self.id,
                cx: "50%".to_string(),
                cy: "50%".to_string(),
                r: "50%".to_string(),
                stops: self.stops,
            });
        } else {
            self.builder.defs.push(SvgDef::LinearGradient {
                id: self.id,
                x1: "0%".to_string(),
                y1: "0%".to_string(),
                x2: "100%".to_string(),
                y2: "0%".to_string(),
                stops: self.stops,
            });
        }
        self.builder
    }
}

impl SvgBuilder {
    /// Create a new SVG builder with given dimensions
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            viewbox: None,
            elements: Vec::new(),
            defs: Vec::new(),
            styles: Vec::new(),
        }
    }

    /// Set custom viewBox
    pub fn viewbox(&mut self, viewbox: &str) -> &mut Self {
        self.viewbox = Some(viewbox.to_string());
        self
    }

    /// Add a rectangle
    pub fn rect(&mut self, x: f64, y: f64, width: f64, height: f64) -> RectBuilder<'_> {
        RectBuilder {
            builder: self,
            x,
            y,
            width,
            height,
            rx: None,
            ry: None,
            attrs: Attributes::new(),
        }
    }

    /// Add a circle
    pub fn circle(&mut self, cx: f64, cy: f64, r: f64) -> CircleBuilder<'_> {
        CircleBuilder {
            builder: self,
            cx,
            cy,
            r,
            attrs: Attributes::new(),
        }
    }

    /// Add a text element
    pub fn text(&mut self, x: f64, y: f64, content: &str) -> TextBuilder<'_> {
        TextBuilder {
            builder: self,
            x,
            y,
            content: content.to_string(),
            attrs: Attributes::new(),
        }
    }

    /// Add a path element
    pub fn path(&mut self, d: &str) -> PathBuilder<'_> {
        PathBuilder {
            builder: self,
            d: d.to_string(),
            attrs: Attributes::new(),
        }
    }

    /// Start a group
    pub fn group(&mut self) -> GroupBuilder<'_> {
        GroupBuilder {
            builder: self,
            elements: Vec::new(),
            attrs: Attributes::new(),
        }
    }

    /// Define a linear gradient
    pub fn linear_gradient(&mut self, id: &str) -> GradientBuilder<'_> {
        GradientBuilder {
            builder: self,
            id: id.to_string(),
            is_radial: false,
            stops: Vec::new(),
        }
    }

    /// Define a radial gradient
    pub fn radial_gradient(&mut self, id: &str) -> GradientBuilder<'_> {
        GradientBuilder {
            builder: self,
            id: id.to_string(),
            is_radial: true,
            stops: Vec::new(),
        }
    }

    /// Add inline CSS styles
    pub fn style(&mut self, css: &str) -> &mut Self {
        self.styles.push(css.to_string());
        self
    }

    /// Build the final SVG string
    pub fn build(self) -> String {
        let mut svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\"",
            self.width, self.height
        );
        if let Some(vb) = &self.viewbox {
            svg.push_str(&format!(" viewBox=\"{}\"", vb));
        }
        svg.push_str(">\n");

        if !self.styles.is_empty() {
            svg.push_str("  <style>\n");
            for style in &self.styles {
                svg.push_str(&format!("    {}\n", style));
            }
            svg.push_str("  </style>\n");
        }

        if !self.defs.is_empty() {
            svg.push_str("  <defs>\n");
            for def in &self.defs {
                match def {
                    SvgDef::LinearGradient {
                        id,
                        x1,
                        y1,
                        x2,
                        y2,
                        stops,
                    } => {
                        svg.push_str(&format!("    <linearGradient id=\"{}\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\">\n", id, x1, y1, x2, y2));
                        for stop in stops {
                            let opacity_attr = stop
                                .opacity
                                .map(|o| format!(" stop-opacity=\"{}\"", o))
                                .unwrap_or_default();
                            svg.push_str(&format!(
                                "      <stop offset=\"{}\" stop-color=\"{}\"{} />\n",
                                stop.offset, stop.color, opacity_attr
                            ));
                        }
                        svg.push_str("    </linearGradient>\n");
                    }
                    SvgDef::RadialGradient {
                        id,
                        cx,
                        cy,
                        r,
                        stops,
                    } => {
                        svg.push_str(&format!(
                            "    <radialGradient id=\"{}\" cx=\"{}\" cy=\"{}\" r=\"{}\">\n",
                            id, cx, cy, r
                        ));
                        for stop in stops {
                            let opacity_attr = stop
                                .opacity
                                .map(|o| format!(" stop-opacity=\"{}\"", o))
                                .unwrap_or_default();
                            svg.push_str(&format!(
                                "      <stop offset=\"{}\" stop-color=\"{}\"{} />\n",
                                stop.offset, stop.color, opacity_attr
                            ));
                        }
                        svg.push_str("    </radialGradient>\n");
                    }
                    _ => {}
                }
            }
            svg.push_str("  </defs>\n");
        }

        fn serialize_element(elem: &SvgElement, indent: usize) -> String {
            let ind = " ".repeat(indent);
            match elem {
                SvgElement::Rect {
                    x,
                    y,
                    width,
                    height,
                    rx,
                    ry,
                    attrs,
                } => {
                    let mut extra = String::new();
                    if let Some(r) = rx {
                        extra.push_str(&format!(" rx=\"{}\"", r));
                    }
                    if let Some(r) = ry {
                        extra.push_str(&format!(" ry=\"{}\"", r));
                    }
                    let attrs_str = serialize_attrs(attrs);
                    format!(
                        "{}<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}{} />\n",
                        ind, x, y, width, height, extra, attrs_str
                    )
                }
                SvgElement::Circle { cx, cy, r, attrs } => {
                    let attrs_str = serialize_attrs(attrs);
                    format!(
                        "{}<circle cx=\"{}\" cy=\"{}\" r=\"{}\"{} />\n",
                        ind, cx, cy, r, attrs_str
                    )
                }
                SvgElement::Ellipse {
                    cx,
                    cy,
                    rx,
                    ry,
                    attrs,
                } => {
                    let attrs_str = serialize_attrs(attrs);
                    format!(
                        "{}<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"{} />\n",
                        ind, cx, cy, rx, ry, attrs_str
                    )
                }
                SvgElement::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    attrs,
                } => {
                    let attrs_str = serialize_attrs(attrs);
                    format!(
                        "{}<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"{} />\n",
                        ind, x1, y1, x2, y2, attrs_str
                    )
                }
                SvgElement::Polyline { points, attrs } => {
                    let pts: Vec<String> =
                        points.iter().map(|(x, y)| format!("{},{}", x, y)).collect();
                    let attrs_str = serialize_attrs(attrs);
                    format!(
                        "{}<polyline points=\"{}\"{} />\n",
                        ind,
                        pts.join(" "),
                        attrs_str
                    )
                }
                SvgElement::Polygon { points, attrs } => {
                    let pts: Vec<String> =
                        points.iter().map(|(x, y)| format!("{},{}", x, y)).collect();
                    let attrs_str = serialize_attrs(attrs);
                    format!(
                        "{}<polygon points=\"{}\"{} />\n",
                        ind,
                        pts.join(" "),
                        attrs_str
                    )
                }
                SvgElement::Path { d, attrs } => {
                    let attrs_str = serialize_attrs(attrs);
                    format!("{}<path d=\"{}\"{} />\n", ind, d, attrs_str)
                }
                SvgElement::Text {
                    x,
                    y,
                    content,
                    attrs,
                } => {
                    let attrs_str = serialize_attrs(attrs);
                    format!(
                        "{}<text x=\"{}\" y=\"{}\"{}>{}</text>\n",
                        ind, x, y, attrs_str, content
                    )
                }
                SvgElement::Group { elements, attrs } => {
                    let attrs_str = serialize_attrs(attrs);
                    let mut inner = format!("{}<g{}>\n", ind, attrs_str);
                    for sub in elements {
                        inner.push_str(&serialize_element(sub, indent + 2));
                    }
                    inner.push_str(&format!("{}</g>\n", ind));
                    inner
                }
                SvgElement::Use { href, x, y, attrs } => {
                    let attrs_str = serialize_attrs(attrs);
                    format!(
                        "{}<use href=\"{}\" x=\"{}\" y=\"{}\"{} />\n",
                        ind, href, x, y, attrs_str
                    )
                }
                SvgElement::Image {
                    href,
                    x,
                    y,
                    width,
                    height,
                    attrs,
                } => {
                    let attrs_str = serialize_attrs(attrs);
                    format!(
                        "{}<image href=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{} />\n",
                        ind, href, x, y, width, height, attrs_str
                    )
                }
            }
        }

        fn serialize_attrs(attrs: &Attributes) -> String {
            let mut s = String::new();
            for (k, v) in attrs {
                s.push_str(&format!(" {}=\"{}\"", k, v));
            }
            s
        }

        for elem in &self.elements {
            svg.push_str(&serialize_element(elem, 2));
        }

        svg.push_str("</svg>");
        svg
    }

    /// Build and write to a file
    pub fn build_to_file(self, path: &std::path::Path) -> Result<SvgOutput> {
        let width = self.width;
        let height = self.height;
        let content = self.build();
        std::fs::write(path, &content)?;
        let file_size = content.len() as u64;
        Ok(SvgOutput {
            path: path.to_path_buf(),
            width,
            height,
            content: Some(content),
            file_size,
            generation_id: uuid::Uuid::now_v7().to_string(),
        })
    }
}

/// Chart type selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartType {
    Bar,
    Line,
    Pie,
    Area,
    Scatter,
    Radar,
    Heatmap,
    Treemap,
    Gauge,
}

/// Configuration for chart generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartConfig {
    pub chart_type: ChartType,
    pub data: serde_json::Value,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub width: u32,
    pub height: u32,
    pub theme: ChartTheme,
    pub legend: LegendConfig,
    pub grid: bool,
    pub animate: bool,
    pub padding: Padding,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartTheme {
    pub background: String,
    pub text_color: String,
    pub grid_color: String,
    pub axis_color: String,
    pub palette: Vec<String>,
    pub font_family: String,
    pub font_size: f32,
}

impl ChartTheme {
    pub fn dark() -> Self {
        Self {
            background: "#1a1a2e".into(),
            text_color: "#e0e0e0".into(),
            grid_color: "#333355".into(),
            axis_color: "#555577".into(),
            palette: vec![
                "#e94560".into(),
                "#0f3460".into(),
                "#16213e".into(),
                "#533483".into(),
                "#e94560".into(),
                "#f5b461".into(),
                "#61c0bf".into(),
                "#bbbbbb".into(),
            ],
            font_family: "Inter, sans-serif".into(),
            font_size: 14.0,
        }
    }

    pub fn light() -> Self {
        Self {
            background: "#ffffff".into(),
            text_color: "#333333".into(),
            grid_color: "#e0e0e0".into(),
            axis_color: "#999999".into(),
            palette: vec![
                "#2563eb".into(),
                "#dc2626".into(),
                "#16a34a".into(),
                "#9333ea".into(),
                "#ea580c".into(),
                "#0891b2".into(),
                "#4f46e5".into(),
                "#64748b".into(),
            ],
            font_family: "Inter, sans-serif".into(),
            font_size: 14.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegendConfig {
    pub show: bool,
    pub position: LegendPosition,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegendPosition {
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Padding {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

/// Diagram type for technical diagrams
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagramType {
    Flowchart,
    Sequence,
    Architecture,
    ErDiagram,
    Tree,
    MindMap,
    Gantt,
    Timeline,
    Network,
}

/// Generate a chart as SVG
pub fn generate_chart(config: &ChartConfig) -> Result<String> {
    let chart_type_str = match config.chart_type {
        ChartType::Bar => "bar",
        ChartType::Line => "line",
        ChartType::Pie => "pie",
        ChartType::Area => "area",
        ChartType::Scatter => "scatter",
        ChartType::Radar => "radar",
        _ => "bar",
    };
    let chart_points: Vec<ChartPoint> =
        serde_json::from_value(config.data.clone()).map_err(|e| {
            OpenMediaError::InvalidParameter {
                param: "data".to_string(),
                reason: format!("Failed to parse chart data as ChartPoint array: {}", e),
            }
        })?;
    create_chart(
        chart_type_str,
        config.title.as_deref(),
        &chart_points,
        config.width,
        config.height,
    )
}

/// Build SVG XML string from JSON elements
pub fn build_svg_from_json(
    width: u32,
    height: u32,
    elements: &serde_json::Value,
) -> Result<String> {
    let json_elements: Vec<schema::JsonElement> = serde_json::from_value(elements.clone())
        .map_err(|e| OpenMediaError::InvalidParameter {
            param: "elements".to_string(),
            reason: e.to_string(),
        })?;

    let mut builder = SvgBuilder::new(width, height);
    for elem in json_elements {
        match elem {
            schema::JsonElement::Rect {
                x,
                y,
                width,
                height,
                rx,
                ry,
                fill,
                stroke,
                stroke_width,
                opacity,
            } => {
                let mut rect = builder.rect(x, y, width, height);
                if let Some(f) = fill {
                    rect = rect.fill(&f);
                }
                if let Some(s) = stroke {
                    rect = rect.stroke(&s);
                }
                if let Some(sw) = stroke_width {
                    rect = rect.stroke_width(sw);
                }
                if let Some(opacity) = opacity {
                    rect.attrs
                        .insert("opacity".to_string(), opacity.to_string());
                }
                if let Some(rx_val) = rx {
                    rect = rect.rx(rx_val);
                }
                if let Some(ry_val) = ry {
                    rect = rect.ry(ry_val);
                }
                rect.finish();
            }
            schema::JsonElement::Circle {
                cx,
                cy,
                r,
                fill,
                stroke,
                stroke_width,
                opacity,
            } => {
                let mut circle = builder.circle(cx, cy, r);
                if let Some(f) = fill {
                    circle = circle.fill(&f);
                }
                if let Some(s) = stroke {
                    circle = circle.stroke(&s);
                }
                if let Some(sw) = stroke_width {
                    circle = circle.stroke_width(sw);
                }
                if let Some(opacity) = opacity {
                    circle
                        .attrs
                        .insert("opacity".to_string(), opacity.to_string());
                }
                circle.finish();
            }
            schema::JsonElement::Line {
                x1,
                y1,
                x2,
                y2,
                stroke,
                stroke_width,
                stroke_linecap,
                opacity,
            } => {
                let mut attrs = Attributes::new();
                attrs.insert(
                    "stroke".to_string(),
                    stroke.unwrap_or_else(|| "#000000".to_string()),
                );
                attrs.insert(
                    "stroke-width".to_string(),
                    stroke_width.unwrap_or(1.0).to_string(),
                );
                if let Some(linecap) = stroke_linecap {
                    attrs.insert("stroke-linecap".to_string(), linecap);
                }
                if let Some(opacity) = opacity {
                    attrs.insert("opacity".to_string(), opacity.to_string());
                }
                builder.elements.push(SvgElement::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    attrs,
                });
            }
            schema::JsonElement::Text {
                x,
                y,
                content,
                fill,
                font_size,
                font_family,
                font_weight,
                text_anchor,
                dominant_baseline,
                opacity,
            } => {
                let mut text = builder.text(x, y, &content);
                let default_middle_baseline = matches!(text_anchor.as_deref(), Some("middle"));
                let dominant_baseline = dominant_baseline
                    .or_else(|| default_middle_baseline.then(|| "middle".to_string()));
                if let Some(f) = fill {
                    text = text.fill(&f);
                }
                if let Some(sz) = font_size {
                    text = text.font_size(sz);
                }
                if let Some(fam) = font_family {
                    text = text.font_family(&fam);
                }
                if let Some(weight) = font_weight {
                    text.attrs
                        .insert("font-weight".to_string(), weight.to_string());
                }
                if let Some(anchor) = text_anchor {
                    text.attrs.insert("text-anchor".to_string(), anchor);
                }
                if let Some(baseline) = dominant_baseline {
                    text.attrs.insert("dominant-baseline".to_string(), baseline);
                }
                if let Some(opacity) = opacity {
                    text.attrs
                        .insert("opacity".to_string(), opacity.to_string());
                }
                text.finish();
            }
        }
    }
    Ok(builder.build())
}

/// Rasterize an SVG string into an ImageOutput with specified dimensions and background color.
pub fn rasterize(
    svg_content: &str,
    width: Option<u32>,
    height: Option<u32>,
    bg_color: Option<&str>,
    format: &str,
    output_path: &std::path::Path,
) -> Result<ImageOutput> {
    use resvg::usvg;
    use std::time::Instant;
    let start_time = Instant::now();

    // Parse the SVG
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_str(svg_content, &opt)
        .map_err(|e| OpenMediaError::InvalidSvgInput(e.to_string()))?;

    // Determine dimensions and scale factor
    let svg_size = tree.size();
    let (w, h, factor) = match (width, height) {
        (Some(w), Some(h)) => {
            let factor_w = w as f32 / svg_size.width();
            let factor_h = h as f32 / svg_size.height();
            let factor = factor_w.min(factor_h);
            (w, h, factor)
        }
        (Some(w), None) => {
            let factor = w as f32 / svg_size.width();
            (w, (svg_size.height() * factor).round() as u32, factor)
        }
        (None, Some(h)) => {
            let factor = h as f32 / svg_size.height();
            ((svg_size.width() * factor).round() as u32, h, factor)
        }
        (None, None) => (
            svg_size.width().round() as u32,
            svg_size.height().round() as u32,
            1.0,
        ),
    };

    let mut pixmap =
        tiny_skia::Pixmap::new(w, h).ok_or_else(|| OpenMediaError::InvalidDimensions {
            width: w,
            height: h,
            reason: "Failed to allocate pixmap".to_string(),
        })?;

    // Fill background if specified
    if let Some(color_str) = bg_color {
        let clean_hex = color_str.trim_start_matches('#');
        if let Ok(val) = u32::from_str_radix(clean_hex, 16) {
            let r = ((val >> 16) & 0xFF) as f32 / 255.0;
            let g = ((val >> 8) & 0xFF) as f32 / 255.0;
            let b = (val & 0xFF) as f32 / 255.0;
            let a = if clean_hex.len() == 8 {
                ((val >> 24) & 0xFF) as f32 / 255.0
            } else {
                1.0
            };
            if let Some(skia_color) = tiny_skia::Color::from_rgba(r, g, b, a) {
                pixmap.fill(skia_color);
            }
        }
    }

    // Render using resvg
    let transform = tiny_skia::Transform::from_scale(factor, factor);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // Demultiply alpha for formats other than PNG (or always for general image saving)
    let mut pixels = pixmap.data().to_vec();
    for chunk in pixels.chunks_exact_mut(4) {
        let a = chunk[3];
        if a > 0 && a < 255 {
            let alpha_factor = 255.0 / a as f32;
            chunk[0] = (chunk[0] as f32 * alpha_factor).min(255.0) as u8;
            chunk[1] = (chunk[1] as f32 * alpha_factor).min(255.0) as u8;
            chunk[2] = (chunk[2] as f32 * alpha_factor).min(255.0) as u8;
        }
    }

    // Save output based on format
    let clean_format = format.to_lowercase();
    match clean_format.as_str() {
        "png" => {
            pixmap
                .save_png(output_path)
                .map_err(|e| OpenMediaError::ImageEncodeError {
                    format: "png".to_string(),
                    reason: e.to_string(),
                })?;
        }
        "jpeg" | "jpg" => {
            let img = image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(w, h, {
                // Drop alpha channel
                let mut rgb_pixels = Vec::with_capacity((w * h * 3) as usize);
                for chunk in pixels.chunks_exact(4) {
                    rgb_pixels.push(chunk[0]);
                    rgb_pixels.push(chunk[1]);
                    rgb_pixels.push(chunk[2]);
                }
                rgb_pixels
            })
            .ok_or_else(|| OpenMediaError::ImageEncodeError {
                format: "jpeg".to_string(),
                reason: "Failed to create RGB image buffer".to_string(),
            })?;
            img.save(output_path)
                .map_err(|e| OpenMediaError::ImageEncodeError {
                    format: "jpeg".to_string(),
                    reason: e.to_string(),
                })?;
        }
        "webp" => {
            let img = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(w, h, pixels).ok_or_else(
                || OpenMediaError::ImageEncodeError {
                    format: "webp".to_string(),
                    reason: "Failed to create RGBA image buffer".to_string(),
                },
            )?;
            img.save(output_path)
                .map_err(|e| OpenMediaError::ImageEncodeError {
                    format: "webp".to_string(),
                    reason: e.to_string(),
                })?;
        }
        other => {
            return Err(OpenMediaError::InvalidParameter {
                param: "output_format".to_string(),
                reason: format!("Unsupported format: {}", other),
            });
        }
    }

    let file_size = std::fs::metadata(output_path)?.len();
    let generation_time = start_time.elapsed().as_secs_f64();

    Ok(ImageOutput {
        path: output_path.to_path_buf(),
        width: w,
        height: h,
        seed: 0,
        format: clean_format,
        file_size,
        generation_id: uuid::Uuid::now_v7().to_string(),
        clip_score: None,
        aesthetic_score: None,
        model_used: "resvg".to_string(),
        backend_used: "resvg".to_string(),
        generation_time,
    })
}

/// Render a Mermaid diagram into SVG XML content natively
pub fn render_mermaid(
    code: &str,
    theme: Option<mermaid_rs_renderer::Theme>,
    layout: Option<mermaid_rs_renderer::LayoutConfig>,
) -> std::result::Result<String, String> {
    let options = mermaid_rs_renderer::RenderOptions {
        theme: theme.unwrap_or_else(mermaid_rs_renderer::Theme::modern),
        layout: layout.unwrap_or_default(),
    };
    mermaid_rs_renderer::render_with_options(code, options)
        .map_err(|e| format!("Failed to render Mermaid diagram: {:?}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_mermaid() {
        let code = "flowchart LR\n  A --> B";
        let result = render_mermaid(code, None, None);
        assert!(result.is_ok());
        let svg = result.unwrap();
        assert!(svg.contains("<svg") || svg.contains("svg"));
    }

    #[test]
    fn test_render_mermaid_with_custom_theme() {
        let code = "flowchart TD\n  A --> B";
        let mut theme = mermaid_rs_renderer::Theme::modern();
        theme.primary_color = "#ff0000".to_string(); // Bright Red
        let result = render_mermaid(code, Some(theme), None);
        assert!(result.is_ok());
        let svg = result.unwrap();
        assert!(svg.contains("#ff0000"));
    }

    #[test]
    fn test_render_mermaid_with_layout_spacing() {
        let code = "flowchart LR\n  A --> B";
        let mut layout = mermaid_rs_renderer::LayoutConfig::default();
        layout.node_spacing = 150.0;
        let result = render_mermaid(code, None, Some(layout));
        assert!(result.is_ok());
    }

    #[test]
    fn test_svg_builder() {
        let mut builder = SvgBuilder::new(500, 500);
        builder
            .viewbox("0 0 100 100")
            .rect(10.0, 20.0, 30.0, 40.0)
            .fill("red")
            .stroke("black")
            .stroke_width(2.0)
            .rx(5.0)
            .ry(5.0)
            .finish()
            .circle(50.0, 50.0, 15.0)
            .fill("blue")
            .finish();

        let output = builder.build();
        assert!(output.contains("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"500\" height=\"500\" viewBox=\"0 0 100 100\">"));
        assert!(
            output.contains("<rect x=\"10\" y=\"20\" width=\"30\" height=\"40\" rx=\"5\" ry=\"5\"")
        );
        assert!(output.contains("<circle cx=\"50\" cy=\"50\" r=\"15\""));

        // Check attributes are present
        assert!(output.contains("fill=\"red\"") || output.contains("stroke=\"black\""));
        assert!(output.contains("fill=\"blue\""));
    }

    #[test]
    fn test_build_svg_from_json_supports_line_and_text_alignment_attrs() {
        let elements = serde_json::json!([
            {
                "type": "line",
                "x1": 10.0,
                "y1": 20.0,
                "x2": 120.0,
                "y2": 80.0,
                "stroke": "#00e5ff",
                "stroke_width": 6.0,
                "stroke_linecap": "round"
            },
            {
                "type": "text",
                "x": 100.0,
                "y": 140.0,
                "content": "OpenZ",
                "fill": "#ffffff",
                "font_size": 42.0,
                "font_family": "JetBrains Mono",
                "font_weight": 800,
                "text_anchor": "middle"
            }
        ]);

        let svg = build_svg_from_json(200, 180, &elements).unwrap();
        assert!(svg.contains("<line x1=\"10\" y1=\"20\" x2=\"120\" y2=\"80\""));
        assert!(svg.contains("stroke-width=\"6\""));
        assert!(svg.contains("stroke-linecap=\"round\""));
        assert!(svg.contains("font-weight=\"800\""));
        assert!(svg.contains("text-anchor=\"middle\""));
        assert!(svg.contains("dominant-baseline=\"middle\""));
    }

    #[test]
    fn test_rasterize_svg() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <rect width="100" height="100" fill="red"/>
        </svg>"#;
        let temp_dir = std::env::temp_dir();
        let out_path = temp_dir.join("test_rasterize.png");
        let result = rasterize(svg, Some(200), None, Some("#00ff00"), "png", &out_path);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.width, 200);
        assert_eq!(output.height, 200);
        assert!(out_path.exists());
        let _ = std::fs::remove_file(out_path);
    }

    #[test]
    fn test_new_chart_and_transition_variants_parsing() {
        let area_json = "\"area\"";
        let area: ChartType = serde_json::from_str(area_json).unwrap();
        assert!(matches!(area, ChartType::Area));

        let scatter_json = "\"scatter\"";
        let scatter: ChartType = serde_json::from_str(scatter_json).unwrap();
        assert!(matches!(scatter, ChartType::Scatter));

        let radar_json = "\"radar\"";
        let radar: ChartType = serde_json::from_str(radar_json).unwrap();
        assert!(matches!(radar, ChartType::Radar));
    }
}
