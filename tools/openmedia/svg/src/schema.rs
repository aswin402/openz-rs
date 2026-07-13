use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum JsonElement {
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        rx: Option<f64>,
        ry: Option<f64>,
        fill: Option<String>,
        stroke: Option<String>,
        stroke_width: Option<f64>,
        opacity: Option<f64>,
    },
    Circle {
        cx: f64,
        cy: f64,
        r: f64,
        fill: Option<String>,
        stroke: Option<String>,
        stroke_width: Option<f64>,
        opacity: Option<f64>,
    },
    Line {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        stroke: Option<String>,
        stroke_width: Option<f64>,
        stroke_linecap: Option<String>,
        opacity: Option<f64>,
    },
    Text {
        x: f64,
        y: f64,
        content: String,
        fill: Option<String>,
        font_size: Option<f64>,
        font_family: Option<String>,
        font_weight: Option<u16>,
        text_anchor: Option<String>,
        opacity: Option<f64>,
    },
}
