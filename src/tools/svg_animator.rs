use crate::tools::Tool;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::fmt::Write as FmtWrite;
use std::fs;

pub struct SvgAnimatorTool;

/// Internal representation of an SVG element with optional SMIL animations
#[derive(Debug)]
struct SvgElement {
    tag: String,
    attrs: Vec<(String, String)>,
    children: Vec<SvgElement>,
    text_content: Option<String>,
}

impl SvgElement {
    fn new(tag: &str) -> Self {
        Self {
            tag: tag.to_string(),
            attrs: Vec::new(),
            children: Vec::new(),
            text_content: None,
        }
    }

    fn attr(mut self, key: &str, val: &str) -> Self {
        if let Some(pos) = self.attrs.iter().position(|(k, _)| k == key) {
            self.attrs[pos].1 = val.to_string();
        } else {
            self.attrs.push((key.to_string(), val.to_string()));
        }
        self
    }

    fn child(mut self, child: SvgElement) -> Self {
        self.children.push(child);
        self
    }

    fn text(mut self, text: &str) -> Self {
        self.text_content = Some(text.to_string());
        self
    }

    fn to_svg_string(&self, indent: usize) -> String {
        let pad = "  ".repeat(indent);
        let mut out = String::new();
        let attrs: String = self
            .attrs
            .iter()
            .map(|(k, v)| format!(" {}=\"{}\"", k, escape_xml(v)))
            .collect();

        if self.children.is_empty() && self.text_content.is_none() {
            let _ = writeln!(out, "{}<{}{} />", pad, self.tag, attrs);
        } else {
            let _ = write!(out, "{}<{}{}>", pad, self.tag, attrs);
            if let Some(ref txt) = self.text_content {
                let _ = writeln!(out, "{}</{}>", txt, self.tag);
            } else {
                let _ = writeln!(out);
                for child in &self.children {
                    out.push_str(&child.to_svg_string(indent + 1));
                }
                let _ = writeln!(out, "{}</{}>", pad, self.tag);
            }
        }
        out
    }
}

/// Build an SMIL `<animate>` element
fn smil_animate(attr_name: &str, from: &str, to: &str, dur: &str, repeat: &str) -> SvgElement {
    SvgElement::new("animate")
        .attr("attributeName", attr_name)
        .attr("from", from)
        .attr("to", to)
        .attr("dur", dur)
        .attr("repeatCount", repeat)
        .attr("fill", "freeze")
}

/// Build an SMIL `<animateTransform>` element
fn smil_animate_transform(
    transform_type: &str,
    from: &str,
    to: &str,
    dur: &str,
    repeat: &str,
    add_itive: &str,
) -> SvgElement {
    SvgElement::new("animateTransform")
        .attr("attributeName", "transform")
        .attr("attributeType", "XML")
        .attr("type", transform_type)
        .attr("from", from)
        .attr("to", to)
        .attr("dur", dur)
        .attr("repeatCount", repeat)
        .attr("additive", add_itive)
}

/// Build an SMIL `<animateMotion>` element  
fn smil_animate_motion(path: &str, dur: &str, repeat: &str) -> SvgElement {
    SvgElement::new("animateMotion")
        .attr("path", path)
        .attr("dur", dur)
        .attr("repeatCount", repeat)
}

/// Build an SMIL `<set>` element
fn smil_set(attr_name: &str, to: &str, begin: &str, dur: &str) -> SvgElement {
    SvgElement::new("set")
        .attr("attributeName", attr_name)
        .attr("to", to)
        .attr("begin", begin)
        .attr("dur", dur)
}

fn parse_animation(anim: &Value) -> Vec<SvgElement> {
    let mut anims = Vec::new();
    let anim_type = anim.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let dur = anim.get("dur").and_then(|v| v.as_str()).unwrap_or("1s");
    let repeat = anim
        .get("repeat")
        .and_then(|v| v.as_str())
        .unwrap_or("indefinite");

    match anim_type {
        "fade_in" => {
            anims.push(smil_animate("opacity", "0", "1", dur, repeat));
        }
        "fade_out" => {
            anims.push(smil_animate("opacity", "1", "0", dur, repeat));
        }
        "blink" => {
            anims.push(smil_animate("opacity", "1", "0", dur, repeat));
        }
        "pulse" => {
            let cx = anim.get("cx").and_then(|v| v.as_str()).unwrap_or("50 50");
            let from_s = anim
                .get("from_scale")
                .and_then(|v| v.as_str())
                .unwrap_or("1");
            let to_s = anim
                .get("to_scale")
                .and_then(|v| v.as_str())
                .unwrap_or("1.2");
            anims.push(
                smil_animate_transform(
                    "scale",
                    &format!("{} {}", from_s, from_s),
                    &format!("{} {}", to_s, to_s),
                    dur,
                    repeat,
                    "replace",
                )
                .attr("begin", "0s"),
            );
            // Counter-scale to keep it centered around a point requires translate trick
            let _ = cx; // cx hint for user context but SMIL scale is from origin
        }
        "rotate" => {
            let from = anim
                .get("from")
                .and_then(|v| v.as_str())
                .unwrap_or("0 50 50");
            let to = anim
                .get("to")
                .and_then(|v| v.as_str())
                .unwrap_or("360 50 50");
            anims.push(smil_animate_transform(
                "rotate", from, to, dur, repeat, "replace",
            ));
        }
        "translate" => {
            let from = anim.get("from").and_then(|v| v.as_str()).unwrap_or("0 0");
            let to = anim.get("to").and_then(|v| v.as_str()).unwrap_or("100 0");
            anims.push(smil_animate_transform(
                "translate",
                from,
                to,
                dur,
                repeat,
                "replace",
            ));
        }
        "scale" => {
            let from = anim.get("from").and_then(|v| v.as_str()).unwrap_or("1 1");
            let to = anim.get("to").and_then(|v| v.as_str()).unwrap_or("2 2");
            anims.push(smil_animate_transform(
                "scale", from, to, dur, repeat, "replace",
            ));
        }
        "color_cycle" => {
            let attr = anim.get("attr").and_then(|v| v.as_str()).unwrap_or("fill");
            let from = anim
                .get("from")
                .and_then(|v| v.as_str())
                .unwrap_or("#ff0000");
            let to = anim.get("to").and_then(|v| v.as_str()).unwrap_or("#0000ff");
            anims.push(smil_animate(attr, from, to, dur, repeat));
        }
        "motion_path" => {
            let path = anim
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("M0,0 Q50,-50 100,0");
            anims.push(smil_animate_motion(path, dur, repeat));
        }
        "stroke_dash" => {
            // Classic SVG draw-on animation
            let total = anim.get("length").and_then(|v| v.as_str()).unwrap_or("300");
            anims.push(smil_animate("stroke-dashoffset", total, "0", dur, repeat));
        }
        "typewriter" => {
            // Reveal text via clip-path / visibility width trick using set
            let begin = anim.get("begin").and_then(|v| v.as_str()).unwrap_or("0s");
            anims.push(smil_set("visibility", "visible", begin, dur));
        }
        _ => {
            // Generic attribute animation
            let attr = anim
                .get("attr")
                .and_then(|v| v.as_str())
                .unwrap_or("opacity");
            let from = anim.get("from").and_then(|v| v.as_str()).unwrap_or("0");
            let to = anim.get("to").and_then(|v| v.as_str()).unwrap_or("1");
            anims.push(smil_animate(attr, from, to, dur, repeat));
        }
    }
    anims
}

fn parse_element(elem: &Value) -> Option<SvgElement> {
    let shape = elem.get("shape").and_then(|v| v.as_str())?;

    let mut el = match shape {
        "rect" => {
            let x = elem.get("x").and_then(|v| v.as_str()).unwrap_or("0");
            let y = elem.get("y").and_then(|v| v.as_str()).unwrap_or("0");
            let w = elem.get("width").and_then(|v| v.as_str()).unwrap_or("100");
            let h = elem.get("height").and_then(|v| v.as_str()).unwrap_or("100");
            let rx = elem.get("rx").and_then(|v| v.as_str()).unwrap_or("0");
            let fill = elem
                .get("fill")
                .and_then(|v| v.as_str())
                .unwrap_or("#4A90E2");
            let stroke = elem
                .get("stroke")
                .and_then(|v| v.as_str())
                .unwrap_or("none");
            let stroke_w = elem
                .get("stroke_width")
                .and_then(|v| v.as_str())
                .unwrap_or("1");
            let opacity = elem.get("opacity").and_then(|v| v.as_str()).unwrap_or("1");
            SvgElement::new("rect")
                .attr("x", x)
                .attr("y", y)
                .attr("width", w)
                .attr("height", h)
                .attr("rx", rx)
                .attr("fill", fill)
                .attr("stroke", stroke)
                .attr("stroke-width", stroke_w)
                .attr("opacity", opacity)
        }
        "circle" => {
            let cx = elem.get("cx").and_then(|v| v.as_str()).unwrap_or("50");
            let cy = elem.get("cy").and_then(|v| v.as_str()).unwrap_or("50");
            let r = elem.get("r").and_then(|v| v.as_str()).unwrap_or("40");
            let fill = elem
                .get("fill")
                .and_then(|v| v.as_str())
                .unwrap_or("#E74C3C");
            let stroke = elem
                .get("stroke")
                .and_then(|v| v.as_str())
                .unwrap_or("none");
            let stroke_w = elem
                .get("stroke_width")
                .and_then(|v| v.as_str())
                .unwrap_or("1");
            let opacity = elem.get("opacity").and_then(|v| v.as_str()).unwrap_or("1");
            SvgElement::new("circle")
                .attr("cx", cx)
                .attr("cy", cy)
                .attr("r", r)
                .attr("fill", fill)
                .attr("stroke", stroke)
                .attr("stroke-width", stroke_w)
                .attr("opacity", opacity)
        }
        "ellipse" => {
            let cx = elem.get("cx").and_then(|v| v.as_str()).unwrap_or("100");
            let cy = elem.get("cy").and_then(|v| v.as_str()).unwrap_or("60");
            let rx = elem.get("rx").and_then(|v| v.as_str()).unwrap_or("80");
            let ry = elem.get("ry").and_then(|v| v.as_str()).unwrap_or("40");
            let fill = elem
                .get("fill")
                .and_then(|v| v.as_str())
                .unwrap_or("#9B59B6");
            let opacity = elem.get("opacity").and_then(|v| v.as_str()).unwrap_or("1");
            SvgElement::new("ellipse")
                .attr("cx", cx)
                .attr("cy", cy)
                .attr("rx", rx)
                .attr("ry", ry)
                .attr("fill", fill)
                .attr("opacity", opacity)
        }
        "line" => {
            let x1 = elem.get("x1").and_then(|v| v.as_str()).unwrap_or("0");
            let y1 = elem.get("y1").and_then(|v| v.as_str()).unwrap_or("0");
            let x2 = elem.get("x2").and_then(|v| v.as_str()).unwrap_or("100");
            let y2 = elem.get("y2").and_then(|v| v.as_str()).unwrap_or("100");
            let stroke = elem
                .get("stroke")
                .and_then(|v| v.as_str())
                .unwrap_or("#2C3E50");
            let stroke_w = elem
                .get("stroke_width")
                .and_then(|v| v.as_str())
                .unwrap_or("2");
            SvgElement::new("line")
                .attr("x1", x1)
                .attr("y1", y1)
                .attr("x2", x2)
                .attr("y2", y2)
                .attr("stroke", stroke)
                .attr("stroke-width", stroke_w)
        }
        "polyline" => {
            let points = elem
                .get("points")
                .and_then(|v| v.as_str())
                .unwrap_or("0,50 25,0 50,50 75,0 100,50");
            let stroke = elem
                .get("stroke")
                .and_then(|v| v.as_str())
                .unwrap_or("#1ABC9C");
            let stroke_w = elem
                .get("stroke_width")
                .and_then(|v| v.as_str())
                .unwrap_or("2");
            let fill = elem.get("fill").and_then(|v| v.as_str()).unwrap_or("none");
            SvgElement::new("polyline")
                .attr("points", points)
                .attr("stroke", stroke)
                .attr("stroke-width", stroke_w)
                .attr("fill", fill)
        }
        "polygon" => {
            let points = elem
                .get("points")
                .and_then(|v| v.as_str())
                .unwrap_or("50,0 100,100 0,100");
            let fill = elem
                .get("fill")
                .and_then(|v| v.as_str())
                .unwrap_or("#F39C12");
            let stroke = elem
                .get("stroke")
                .and_then(|v| v.as_str())
                .unwrap_or("none");
            let stroke_w = elem
                .get("stroke_width")
                .and_then(|v| v.as_str())
                .unwrap_or("1");
            SvgElement::new("polygon")
                .attr("points", points)
                .attr("fill", fill)
                .attr("stroke", stroke)
                .attr("stroke-width", stroke_w)
        }
        "path" => {
            let d = elem
                .get("d")
                .and_then(|v| v.as_str())
                .unwrap_or("M10,80 Q95,10 180,80");
            let fill = elem.get("fill").and_then(|v| v.as_str()).unwrap_or("none");
            let stroke = elem
                .get("stroke")
                .and_then(|v| v.as_str())
                .unwrap_or("#E74C3C");
            let stroke_w = elem
                .get("stroke_width")
                .and_then(|v| v.as_str())
                .unwrap_or("3");
            let opacity = elem.get("opacity").and_then(|v| v.as_str()).unwrap_or("1");
            // For stroke-dash draw-on effects, allow stroke-dasharray
            let dash = elem
                .get("stroke_dasharray")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let dash_offset = elem
                .get("stroke_dashoffset")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let mut e = SvgElement::new("path")
                .attr("d", d)
                .attr("fill", fill)
                .attr("stroke", stroke)
                .attr("stroke-width", stroke_w)
                .attr("opacity", opacity);
            if !dash.is_empty() {
                e = e.attr("stroke-dasharray", dash);
            }
            if !dash_offset.is_empty() {
                e = e.attr("stroke-dashoffset", dash_offset);
            }
            e
        }
        "text" => {
            let x = elem.get("x").and_then(|v| v.as_str()).unwrap_or("50");
            let y = elem.get("y").and_then(|v| v.as_str()).unwrap_or("50");
            let content = elem
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("Hello");
            let font_size = elem
                .get("font_size")
                .and_then(|v| v.as_str())
                .unwrap_or("24");
            let font_family = elem
                .get("font_family")
                .and_then(|v| v.as_str())
                .unwrap_or("sans-serif");
            let fill = elem
                .get("fill")
                .and_then(|v| v.as_str())
                .unwrap_or("#2C3E50");
            let text_anchor = elem
                .get("text_anchor")
                .and_then(|v| v.as_str())
                .unwrap_or("middle");
            let dominant_baseline = elem
                .get("dominant_baseline")
                .and_then(|v| v.as_str())
                .unwrap_or(if text_anchor == "middle" {
                    "middle"
                } else {
                    "auto"
                });
            let opacity = elem.get("opacity").and_then(|v| v.as_str()).unwrap_or("1");
            SvgElement::new("text")
                .attr("x", x)
                .attr("y", y)
                .attr("font-size", font_size)
                .attr("font-family", font_family)
                .attr("fill", fill)
                .attr("text-anchor", text_anchor)
                .attr("dominant-baseline", dominant_baseline)
                .attr("opacity", opacity)
                .text(content)
        }
        "group" => {
            let transform = elem.get("transform").and_then(|v| v.as_str()).unwrap_or("");
            let opacity = elem.get("opacity").and_then(|v| v.as_str()).unwrap_or("1");
            let mut g = SvgElement::new("g").attr("opacity", opacity);
            if !transform.is_empty() {
                g = g.attr("transform", transform);
            }
            // Recursively process group children
            if let Some(children) = elem.get("children").and_then(|v| v.as_array()) {
                for child in children {
                    if let Some(child_el) = parse_element(child) {
                        g = g.child(child_el);
                    }
                }
            }
            g
        }
        _ => return None,
    };

    // Apply id if provided
    if let Some(id) = elem.get("id").and_then(|v| v.as_str()) {
        el = el.attr("id", id);
    }

    // Apply common SVG attributes if provided
    if let Some(class) = elem.get("class").and_then(|v| v.as_str()) {
        el = el.attr("class", class);
    }
    if let Some(style) = elem.get("style").and_then(|v| v.as_str()) {
        el = el.attr("style", style);
    }
    if let Some(transform) = elem.get("transform").and_then(|v| v.as_str()) {
        el = el.attr("transform", transform);
    }
    if let Some(filter) = elem.get("filter").and_then(|v| v.as_str()) {
        el = el.attr("filter", filter);
    }
    if let Some(clip_path) = elem.get("clip_path").and_then(|v| v.as_str()) {
        el = el.attr("clip-path", clip_path);
    }
    if let Some(mask) = elem.get("mask").and_then(|v| v.as_str()) {
        el = el.attr("mask", mask);
    }

    // Apply animations
    if let Some(animations) = elem.get("animations").and_then(|v| v.as_array()) {
        for anim in animations {
            for anim_el in parse_animation(anim) {
                el = el.child(anim_el);
            }
        }
    }

    Some(el)
}

fn build_svg_document(args: &Value) -> Result<String> {
    if let Some(raw) = args.get("raw_svg").and_then(|v| v.as_str()) {
        let trimmed = raw.trim();
        if trimmed.starts_with("<?xml")
            || (trimmed.starts_with("<svg") && trimmed.contains("</svg>"))
        {
            return Ok(trimmed.to_string());
        }

        let width = args.get("width").and_then(|v| v.as_u64()).unwrap_or(400);
        let height = args.get("height").and_then(|v| v.as_u64()).unwrap_or(400);
        let bg = args
            .get("background")
            .and_then(|v| v.as_str())
            .unwrap_or("#ffffff");
        let title = args
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Generated SVG");
        let view_box = args.get("viewBox").and_then(|v| v.as_str()).unwrap_or("");
        let preserve_ar = args
            .get("preserveAspectRatio")
            .and_then(|v| v.as_str())
            .unwrap_or("xMidYMid meet");

        let vb_attr = if view_box.is_empty() {
            format!("0 0 {} {}", width, height)
        } else {
            view_box.to_string()
        };

        let mut svg = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg"
     xmlns:xlink="http://www.w3.org/1999/xlink"
     width="{width}" height="{height}"
     viewBox="{vb}"
     preserveAspectRatio="{par}">
  <title>{title}</title>
  <rect width="100%" height="100%" fill="{bg}"/>
"#,
            width = width,
            height = height,
            vb = vb_attr,
            par = preserve_ar,
            title = escape_xml(title),
            bg = bg
        );
        svg.push_str(trimmed);
        svg.push_str("\n</svg>\n");
        return Ok(svg);
    }

    let width = args.get("width").and_then(|v| v.as_u64()).unwrap_or(400);
    let height = args.get("height").and_then(|v| v.as_u64()).unwrap_or(400);
    let bg = args
        .get("background")
        .and_then(|v| v.as_str())
        .unwrap_or("#ffffff");
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Generated SVG");
    let view_box = args.get("viewBox").and_then(|v| v.as_str()).unwrap_or("");
    let preserve_ar = args
        .get("preserveAspectRatio")
        .and_then(|v| v.as_str())
        .unwrap_or("xMidYMid meet");

    // Defs: gradients, filters, clip paths
    let mut defs_content = String::new();
    if let Some(defs) = args.get("defs").and_then(|v| v.as_array()) {
        for def in defs {
            let def_type = def.get("type").and_then(|v| v.as_str()).unwrap_or("");
            match def_type {
                "linearGradient" => {
                    let id = def.get("id").and_then(|v| v.as_str()).unwrap_or("grad1");
                    let x1 = def.get("x1").and_then(|v| v.as_str()).unwrap_or("0%");
                    let y1 = def.get("y1").and_then(|v| v.as_str()).unwrap_or("0%");
                    let x2 = def.get("x2").and_then(|v| v.as_str()).unwrap_or("100%");
                    let y2 = def.get("y2").and_then(|v| v.as_str()).unwrap_or("0%");
                    let _ = writeln!(
                        defs_content,
                        "    <linearGradient id=\"{}\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\">",
                        id, x1, y1, x2, y2
                    );
                    if let Some(stops) = def.get("stops").and_then(|v| v.as_array()) {
                        for stop in stops {
                            let offset =
                                stop.get("offset").and_then(|v| v.as_str()).unwrap_or("0%");
                            let color =
                                stop.get("color").and_then(|v| v.as_str()).unwrap_or("#000");
                            let opacity =
                                stop.get("opacity").and_then(|v| v.as_str()).unwrap_or("1");
                            let _ = writeln!(defs_content, "      <stop offset=\"{}\" style=\"stop-color:{};stop-opacity:{}\"/>", offset, color, opacity);
                        }
                    }
                    let _ = writeln!(defs_content, "    </linearGradient>");
                }
                "radialGradient" => {
                    let id = def.get("id").and_then(|v| v.as_str()).unwrap_or("radgrad1");
                    let cx = def.get("cx").and_then(|v| v.as_str()).unwrap_or("50%");
                    let cy = def.get("cy").and_then(|v| v.as_str()).unwrap_or("50%");
                    let r = def.get("r").and_then(|v| v.as_str()).unwrap_or("50%");
                    let _ = writeln!(
                        defs_content,
                        "    <radialGradient id=\"{}\" cx=\"{}\" cy=\"{}\" r=\"{}\">",
                        id, cx, cy, r
                    );
                    if let Some(stops) = def.get("stops").and_then(|v| v.as_array()) {
                        for stop in stops {
                            let offset =
                                stop.get("offset").and_then(|v| v.as_str()).unwrap_or("0%");
                            let color =
                                stop.get("color").and_then(|v| v.as_str()).unwrap_or("#000");
                            let opacity =
                                stop.get("opacity").and_then(|v| v.as_str()).unwrap_or("1");
                            let _ = writeln!(defs_content, "      <stop offset=\"{}\" style=\"stop-color:{};stop-opacity:{}\"/>", offset, color, opacity);
                        }
                    }
                    let _ = writeln!(defs_content, "    </radialGradient>");
                }
                "filter" => {
                    let id = def.get("id").and_then(|v| v.as_str()).unwrap_or("f1");
                    let filter_type = def
                        .get("filter_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("blur");
                    match filter_type {
                        "blur" => {
                            let std = def
                                .get("stdDeviation")
                                .and_then(|v| v.as_str())
                                .unwrap_or("3");
                            let _ = writeln!(defs_content, "    <filter id=\"{}\"><feGaussianBlur stdDeviation=\"{}\"/></filter>", id, std);
                        }
                        "shadow" => {
                            let dx = def.get("dx").and_then(|v| v.as_str()).unwrap_or("3");
                            let dy = def.get("dy").and_then(|v| v.as_str()).unwrap_or("3");
                            let blur = def.get("blur").and_then(|v| v.as_str()).unwrap_or("4");
                            let color = def
                                .get("color")
                                .and_then(|v| v.as_str())
                                .unwrap_or("rgba(0,0,0,0.5)");
                            let _ = writeln!(defs_content, "    <filter id=\"{}\" x=\"-20%\" y=\"-20%\" width=\"140%\" height=\"140%\"><feDropShadow dx=\"{}\" dy=\"{}\" stdDeviation=\"{}\" flood-color=\"{}\"/></filter>", id, dx, dy, blur, color);
                        }
                        _ => {}
                    }
                }
                "clipPath" => {
                    let id = def.get("id").and_then(|v| v.as_str()).unwrap_or("clip1");
                    let shape_def = def.get("shape").and_then(|v| v.as_str()).unwrap_or("rect");
                    let _ = writeln!(defs_content, "    <clipPath id=\"{}\">", id);
                    match shape_def {
                        "rect" => {
                            let x = def.get("x").and_then(|v| v.as_str()).unwrap_or("0");
                            let y = def.get("y").and_then(|v| v.as_str()).unwrap_or("0");
                            let w = def.get("width").and_then(|v| v.as_str()).unwrap_or("100");
                            let h = def.get("height").and_then(|v| v.as_str()).unwrap_or("100");
                            let _ = writeln!(
                                defs_content,
                                "      <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
                                x, y, w, h
                            );
                        }
                        "circle" => {
                            let cx = def.get("cx").and_then(|v| v.as_str()).unwrap_or("50");
                            let cy = def.get("cy").and_then(|v| v.as_str()).unwrap_or("50");
                            let r = def.get("r").and_then(|v| v.as_str()).unwrap_or("40");
                            let _ = writeln!(
                                defs_content,
                                "      <circle cx=\"{}\" cy=\"{}\" r=\"{}\"/>",
                                cx, cy, r
                            );
                        }
                        _ => {}
                    }
                    let _ = writeln!(defs_content, "    </clipPath>");
                }
                _ => {}
            }
        }
    }

    // CSS styles (inline or from custom_css field)
    let custom_css = args
        .get("custom_css")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Build SVG header
    let vb_attr = if view_box.is_empty() {
        format!("0 0 {} {}", width, height)
    } else {
        view_box.to_string()
    };

    let mut svg = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg"
     xmlns:xlink="http://www.w3.org/1999/xlink"
     width="{width}" height="{height}"
     viewBox="{vb}"
     preserveAspectRatio="{par}">
  <title>{title}</title>
"#,
        width = width,
        height = height,
        vb = vb_attr,
        par = preserve_ar,
        title = escape_xml(title),
    );

    // Defs block
    if !defs_content.is_empty() || !custom_css.is_empty() {
        svg.push_str("  <defs>\n");
        if !custom_css.is_empty() {
            svg.push_str("    <style type=\"text/css\">\n");
            svg.push_str("    <![CDATA[\n");
            svg.push_str(custom_css);
            svg.push('\n');
            svg.push_str("    ]]>\n");
            svg.push_str("    </style>\n");
        }
        svg.push_str(&defs_content);
        svg.push_str("  </defs>\n");
    }

    // Background rect
    svg.push_str(&format!(
        "  <rect width=\"100%\" height=\"100%\" fill=\"{}\"/>\n",
        bg
    ));

    // Elements
    if let Some(elements) = args.get("elements").and_then(|v| v.as_array()) {
        for elem in elements {
            if let Some(el) = parse_element(elem) {
                svg.push_str(&el.to_svg_string(1));
            }
        }
    }

    svg.push_str("</svg>\n");
    Ok(svg)
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[async_trait::async_trait]
impl Tool for SvgAnimatorTool {
    fn name(&self) -> &str {
        "create_animated_svg"
    }

    fn description(&self) -> &str {
        "Create a sophisticated animated SVG file from a declarative JSON specification. Supports shapes (rect, circle, ellipse, line, polyline, polygon, path, text, group), gradients, filters, clip-paths, and a rich set of SMIL animations (fade_in, fade_out, blink, rotate, translate, scale, pulse, color_cycle, motion_path, stroke_dash, typewriter, and generic attribute animations). The output is a self-contained .svg file that animates natively in any browser."
    }

    fn parameters(&self) -> Value {
        serde_json::from_str(r#"{
            "type": "object",
            "properties": {
                "width": {
                    "type": "integer",
                    "description": "Canvas width in pixels (default: 400)"
                },
                "height": {
                    "type": "integer",
                    "description": "Canvas height in pixels (default: 400)"
                },
                "background": {
                    "type": "string",
                    "description": "Background fill color or gradient url (default: '#ffffff')"
                },
                "title": {
                    "type": "string",
                    "description": "SVG document title for accessibility"
                },
                "viewBox": {
                    "type": "string",
                    "description": "Optional SVG viewBox override (e.g. '0 0 200 200'). Defaults to '0 0 {width} {height}'."
                },
                "preserveAspectRatio": {
                    "type": "string",
                    "description": "SVG preserveAspectRatio value (default: 'xMidYMid meet')"
                },
                "defs": {
                    "type": "array",
                    "description": "Array of reusable SVG definitions: linearGradient, radialGradient, filter (blur/shadow), clipPath.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "description": "One of: linearGradient, radialGradient, filter, clipPath" },
                            "id": { "type": "string", "description": "Unique id for this def, referenced as url(#id) in elements" },
                            "stops": {
                                "type": "array",
                                "description": "Color stops for gradients",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "offset": { "type": "string" },
                                        "color": { "type": "string" },
                                        "opacity": { "type": "string" }
                                    }
                                }
                            },
                            "filter_type": { "type": "string", "description": "blur | shadow (for filter type)" },
                            "stdDeviation": { "type": "string" },
                            "dx": { "type": "string" },
                            "dy": { "type": "string" },
                            "blur": { "type": "string" },
                            "color": { "type": "string" },
                            "x1": { "type": "string" },
                            "y1": { "type": "string" },
                            "x2": { "type": "string" },
                            "y2": { "type": "string" },
                            "cx": { "type": "string" },
                            "cy": { "type": "string" },
                            "r": { "type": "string" },
                            "shape": { "type": "string", "description": "Shape for clipPath: rect | circle" },
                            "width": { "type": "string" },
                            "height": { "type": "string" }
                        }
                    }
                },
                "custom_css": {
                    "type": "string",
                    "description": "Optional inline CSS to embed in a <style> block inside <defs>"
                },
                "elements": {
                    "type": "array",
                    "description": "Array of SVG elements to render, each with optional animations.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "shape": {
                                "type": "string",
                                "description": "Element type: rect | circle | ellipse | line | polyline | polygon | path | text | group"
                            },
                            "id": { "type": "string", "description": "Optional element id" },
                            "fill": { "type": "string", "description": "Fill color or gradient url(#id)" },
                            "stroke": { "type": "string" },
                            "stroke_width": { "type": "string" },
                            "opacity": { "type": "string", "description": "Element opacity 0-1" },
                            "filter": { "type": "string", "description": "Filter url(#id)" },
                            "clip_path": { "type": "string", "description": "clipPath url(#id)" },
                            "transform": { "type": "string", "description": "SVG transform string (for groups)" },
                            "x": { "type": "string" },
                            "y": { "type": "string" },
                            "width": { "type": "string" },
                            "height": { "type": "string" },
                            "rx": { "type": "string", "description": "Border radius for rect" },
                            "cx": { "type": "string" },
                            "cy": { "type": "string" },
                            "r": { "type": "string" },
                            "ry": { "type": "string" },
                            "x1": { "type": "string" },
                            "y1": { "type": "string" },
                            "x2": { "type": "string" },
                            "y2": { "type": "string" },
                            "points": { "type": "string", "description": "Space/comma-separated points for polyline/polygon" },
                            "d": { "type": "string", "description": "SVG path data string" },
                            "stroke_dasharray": { "type": "string", "description": "Dash pattern for path draw-on animation" },
                            "stroke_dashoffset": { "type": "string", "description": "Initial dash offset for draw-on animation" },
                            "content": { "type": "string", "description": "Text content for text element" },
                            "font_size": { "type": "string" },
                            "font_family": { "type": "string" },
                            "text_anchor": { "type": "string", "description": "start | middle | end" },
                            "dominant_baseline": { "type": "string", "description": "auto | middle | central | hanging | text-before-edge | text-after-edge" },
                            "children": {
                                "type": "array",
                                "description": "Child elements (for group shape)",
                                "items": { "type": "object" }
                            },
                            "animations": {
                                "type": "array",
                                "description": "SMIL animation definitions for this element.",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "type": {
                                            "type": "string",
                                            "description": "Animation type: fade_in | fade_out | blink | rotate | translate | scale | pulse | color_cycle | motion_path | stroke_dash | typewriter | generic"
                                        },
                                        "dur": { "type": "string", "description": "Duration e.g. '2s', '500ms'" },
                                        "repeat": { "type": "string", "description": "repeatCount: 'indefinite' or a number string" },
                                        "from": { "type": "string", "description": "Start value" },
                                        "to": { "type": "string", "description": "End value" },
                                        "attr": { "type": "string", "description": "Attribute name for generic/color_cycle animation" },
                                        "path": { "type": "string", "description": "SVG path for motion_path animation" },
                                        "length": { "type": "string", "description": "Total path length for stroke_dash animation" },
                                        "begin": { "type": "string", "description": "Begin time for typewriter/set animations" },
                                        "cx": { "type": "string", "description": "Center point hint for pulse animation" },
                                        "from_scale": { "type": "string" },
                                        "to_scale": { "type": "string" }
                                    },
                                    "required": ["type"]
                                }
                            }
                        },
                        "required": ["shape"]
                    }
                },
                "raw_svg": {
                    "type": "string",
                    "description": "Optional raw SVG elements or complete SVG document string. If provided, allows writing custom code (embedded CSS transitions/animations, paths, gradients, interactive shapes) directly."
                },
                "output_path": {
                    "type": "string",
                    "description": "File path to save the generated SVG (default: 'output.svg')"
                }
            },
            "required": ["output_path"]
        }"#).unwrap_or_else(|_| json!({"type": "object", "properties": {}}))
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        if arguments.get("elements").is_none() && arguments.get("raw_svg").is_none() {
            return Err(anyhow!(
                "Either 'elements' or 'raw_svg' parameter must be provided."
            ));
        }

        let output_path_str = arguments
            .get("output_path")
            .and_then(|v| v.as_str())
            .unwrap_or("output.svg");
        let output_path = crate::config::resolve_path(output_path_str);

        let svg_content =
            build_svg_document(arguments).map_err(|e| anyhow!("SVG generation failed: {}", e))?;

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&output_path, &svg_content)
            .map_err(|e| anyhow!("Failed to write SVG file: {}", e))?;

        let element_count = arguments
            .get("elements")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);

        let anim_count: usize = arguments
            .get("elements")
            .and_then(|v| v.as_array())
            .map(|elems| {
                elems
                    .iter()
                    .map(|e| {
                        e.get("animations")
                            .and_then(|a| a.as_array())
                            .map(|a| a.len())
                            .unwrap_or(0)
                    })
                    .sum()
            })
            .unwrap_or(0);

        Ok(json!({
            "status": "success",
            "output_path": output_path.to_string_lossy(),
            "message": if arguments.get("raw_svg").is_some() {
                format!("Raw SVG successfully written and saved to '{}'.", output_path_str)
            } else {
                format!(
                    "Animated SVG successfully created at '{}' with {} element(s) and {} animation(s).",
                    output_path_str, element_count, anim_count
                )
            },
            "size_bytes": svg_content.len(),
            "element_count": element_count,
            "animation_count": anim_count
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_animated_svg_basic() -> Result<()> {
        let tool = SvgAnimatorTool;
        let temp_dir =
            std::env::temp_dir().join(format!("openz_svg_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir)?;
        let output_file = temp_dir.join("test_anim.svg");

        let args = json!({
            "width": 300,
            "height": 300,
            "background": "#1a1a2e",
            "title": "Test Animated SVG",
            "output_path": output_file.to_str().unwrap(),
            "defs": [
                {
                    "type": "linearGradient",
                    "id": "grad1",
                    "x1": "0%", "y1": "0%", "x2": "100%", "y2": "0%",
                    "stops": [
                        { "offset": "0%", "color": "#4A90E2", "opacity": "1" },
                        { "offset": "100%", "color": "#E74C3C", "opacity": "1" }
                    ]
                }
            ],
            "elements": [
                {
                    "shape": "circle",
                    "cx": "150",
                    "cy": "150",
                    "r": "60",
                    "fill": "url(#grad1)",
                    "animations": [
                        { "type": "pulse", "dur": "1.5s", "repeat": "indefinite", "from_scale": "0.9", "to_scale": "1.1" }
                    ]
                },
                {
                    "shape": "text",
                    "x": "150",
                    "y": "250",
                    "content": "Hello SVG!",
                    "font_size": "20",
                    "fill": "#ffffff",
                    "text_anchor": "middle",
                    "animations": [
                        { "type": "fade_in", "dur": "2s", "repeat": "1" }
                    ]
                }
            ]
        });

        let res = tool.call(&args).await?;
        assert_eq!(res["status"], "success");
        assert!(output_file.exists());

        let content = fs::read_to_string(&output_file)?;
        assert!(content.contains("<svg"));
        assert!(content.contains("<circle"));
        assert!(content.contains("animateTransform"));
        assert!(content.contains("animate"));
        assert!(content.contains("dominant-baseline=\"middle\""));

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[tokio::test]
    async fn test_create_svg_with_path_draw_on() -> Result<()> {
        let tool = SvgAnimatorTool;
        let temp_dir =
            std::env::temp_dir().join(format!("openz_svg_draw_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir)?;
        let output_file = temp_dir.join("draw_on.svg");

        let args = json!({
            "width": 400,
            "height": 200,
            "background": "#f0f0f0",
            "output_path": output_file.to_str().unwrap(),
            "elements": [
                {
                    "shape": "path",
                    "d": "M10,100 Q100,20 200,100 T390,100",
                    "stroke": "#E74C3C",
                    "stroke_width": "3",
                    "fill": "none",
                    "stroke_dasharray": "500",
                    "stroke_dashoffset": "500",
                    "animations": [
                        { "type": "stroke_dash", "dur": "3s", "repeat": "indefinite", "length": "500" }
                    ]
                }
            ]
        });

        let res = tool.call(&args).await?;
        assert_eq!(res["status"], "success");
        assert!(output_file.exists());

        let content = fs::read_to_string(&output_file)?;
        assert!(content.contains("stroke-dashoffset"));

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[tokio::test]
    async fn test_create_svg_raw_string() -> Result<()> {
        let tool = SvgAnimatorTool;
        let temp_dir =
            std::env::temp_dir().join(format!("openz_svg_raw_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir)?;
        let output_file = temp_dir.join("raw.svg");

        let args = json!({
            "width": 200,
            "height": 200,
            "background": "#ff00ff",
            "output_path": output_file.to_str().unwrap(),
            "raw_svg": "<circle cx=\"100\" cy=\"100\" r=\"50\" fill=\"white\" />"
        });

        let res = tool.call(&args).await?;
        assert_eq!(res["status"], "success");
        assert!(output_file.exists());

        let content = fs::read_to_string(&output_file)?;
        assert!(content.contains("<svg"));
        assert!(content.contains("fill=\"#ff00ff\""));
        assert!(content.contains("<circle cx=\"100\""));

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
