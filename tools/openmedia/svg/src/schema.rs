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
    },
    Circle {
        cx: f64,
        cy: f64,
        r: f64,
        fill: Option<String>,
        stroke: Option<String>,
    },
    Text {
        x: f64,
        y: f64,
        content: String,
        fill: Option<String>,
        font_size: Option<f64>,
        font_family: Option<String>,
    },
}
