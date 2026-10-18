use super::SvgBuilder;
use crate::tools::openmedia::core::{OpenMediaError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartPoint {
    pub label: String,
    pub value: f64,
}

struct CartesianLayout {
    margin_left: f64,
    margin_top: f64,
    plot_width: f64,
    plot_height: f64,
    max_val: f64,
}

fn prepare_cartesian_grid(
    builder: &mut SvgBuilder,
    data: &[ChartPoint],
    width: u32,
    height: u32,
) -> CartesianLayout {
    let margin_left = 60.0;
    let margin_right = 40.0;
    let margin_top = 70.0;
    let margin_bottom = 60.0;

    let plot_width = width as f64 - margin_left - margin_right;
    let plot_height = height as f64 - margin_top - margin_bottom;

    let raw_max = data.iter().map(|p| p.value).fold(0.0f64, f64::max);
    let max_val = match raw_max <= 0.0 {
        true => 1.0,
        false => raw_max,
    };

    let grid_count = 5;
    for i in 0..=grid_count {
        let ratio = i as f64 / grid_count as f64;
        let y_val = margin_top + plot_height * (1.0 - ratio);
        let label_val = ratio * max_val;

        builder
            .path(&format!(
                "M {} {} L {} {}",
                margin_left,
                y_val,
                width as f64 - margin_right,
                y_val
            ))
            .stroke("#333355")
            .stroke_width(1.0)
            .fill("none")
            .finish();

        builder
            .text(10.0, y_val + 4.0, &format!("{:.1}", label_val))
            .fill("#94a3b8")
            .font_size(10.0)
            .font_family("sans-serif")
            .finish();
    }

    CartesianLayout {
        margin_left,
        margin_top,
        plot_width,
        plot_height,
        max_val,
    }
}

fn point_x(i: usize, n: usize, margin_left: f64, plot_width: f64) -> f64 {
    match n > 1 {
        true => margin_left + (i as f64 / (n - 1) as f64) * plot_width,
        false => margin_left + plot_width / 2.0,
    }
}

fn path_cmd_prefix(index: usize) -> &'static str {
    match index == 0 {
        true => "M",
        false => " L",
    }
}

pub fn create_chart(
    chart_type: &str,
    title: Option<&str>,
    data: &[ChartPoint],
    width: u32,
    height: u32,
) -> Result<String> {
    match data.is_empty() {
        true => {
            return Err(OpenMediaError::InvalidParameter {
                param: "data".to_string(),
                reason: "Chart data cannot be empty".to_string(),
            });
        }
        false => {}
    }

    let mut builder = SvgBuilder::new(width, height);

    // Dynamic background
    builder
        .rect(0.0, 0.0, width as f64, height as f64)
        .fill("#1a1a2e")
        .finish();

    // Standard title
    match title {
        Some(t) => {
            builder
                .text(width as f64 / 2.0 - (t.len() as f64 * 4.0), 35.0, t)
                .fill("#ffffff")
                .font_size(18.0)
                .font_family("sans-serif")
                .finish();
        }
        None => {}
    }

    let palette = [
        "#2563eb", "#dc2626", "#16a34a", "#9333ea", "#ea580c", "#0891b2", "#4f46e5",
    ];

    match chart_type.to_lowercase().as_str() {
        "bar" => {
            let layout = prepare_cartesian_grid(&mut builder, data, width, height);

            // Draw bars
            let n = data.len();
            let spacing = 0.25;
            let bar_width = layout.plot_width / n as f64;
            let inner_bar_width = bar_width * (1.0 - spacing);

            for (i, p) in data.iter().enumerate() {
                let bar_height = (p.value / layout.max_val) * layout.plot_height;
                let x = layout.margin_left + i as f64 * bar_width + (bar_width * spacing / 2.0);
                let y = layout.margin_top + layout.plot_height - bar_height;
                let color = palette[i % palette.len()];

                builder
                    .rect(x, y, inner_bar_width, bar_height)
                    .fill(color)
                    .finish();

                // Draw X-axis label
                let label_x = x + inner_bar_width / 2.0 - (p.label.len() as f64 * 3.0);
                builder
                    .text(
                        label_x,
                        layout.margin_top + layout.plot_height + 20.0,
                        &p.label,
                    )
                    .fill("#94a3b8")
                    .font_size(10.0)
                    .font_family("sans-serif")
                    .finish();
            }
        }
        "line" => {
            let layout = prepare_cartesian_grid(&mut builder, data, width, height);
            let n = data.len();
            let mut path_d = String::new();

            // First pass: generate the path line
            for (i, p) in data.iter().enumerate() {
                let x = point_x(i, n, layout.margin_left, layout.plot_width);
                let y =
                    layout.margin_top + layout.plot_height - (p.value / layout.max_val) * layout.plot_height;
                path_d.push_str(&format!("{} {} {}", path_cmd_prefix(i), x, y));
            }

            builder
                .path(&path_d)
                .stroke("#3b82f6")
                .stroke_width(3.0)
                .fill("none")
                .finish();

            // Second pass: draw marker circles and labels
            for (i, p) in data.iter().enumerate() {
                let x = point_x(i, n, layout.margin_left, layout.plot_width);
                let y =
                    layout.margin_top + layout.plot_height - (p.value / layout.max_val) * layout.plot_height;

                builder
                    .circle(x, y, 5.0)
                    .fill("#ffffff")
                    .stroke("#3b82f6")
                    .stroke_width(2.0)
                    .finish();

                // Draw X-axis label
                let label_x = x - (p.label.len() as f64 * 3.0);
                builder
                    .text(
                        label_x,
                        layout.margin_top + layout.plot_height + 20.0,
                        &p.label,
                    )
                    .fill("#94a3b8")
                    .font_size(10.0)
                    .font_family("sans-serif")
                    .finish();
            }
        }
        "pie" => {
            let cx = width as f64 * 0.4;
            let cy = height as f64 / 2.0 + 10.0;
            let r = (width.min(height) as f64 * 0.6) / 2.0;

            let total: f64 = data.iter().map(|p| p.value).sum();

            match total <= 0.0 {
                true => {
                    builder.circle(cx, cy, r).fill("#475569").finish();
                }
                false => {
                    let mut current_angle = -std::f64::consts::FRAC_PI_2;
                    for (i, p) in data.iter().enumerate() {
                        let slice_angle = (p.value / total) * 2.0 * std::f64::consts::PI;
                        let next_angle = current_angle + slice_angle;

                        let x1 = cx + r * current_angle.cos();
                        let y1 = cy + r * current_angle.sin();
                        let x2 = cx + r * next_angle.cos();
                        let y2 = cy + r * next_angle.sin();

                        let large_arc_flag = u8::from(slice_angle > std::f64::consts::PI);

                        let d = format!(
                            "M {} {} L {} {} A {} {} 0 {} 1 {} {} Z",
                            cx, cy, x1, y1, r, r, large_arc_flag, x2, y2
                        );

                        let color = palette[i % palette.len()];

                        builder
                            .path(&d)
                            .fill(color)
                            .stroke("#1a1a2e")
                            .stroke_width(1.5)
                            .finish();

                        current_angle = next_angle;
                    }
                }
            }

            // Draw Legend
            let legend_x = width as f64 * 0.7;
            let n = data.len();
            let mut legend_y = cy - (n as f64 * 25.0 / 2.0);

            for (i, p) in data.iter().enumerate() {
                let color = palette[i % palette.len()];

                builder
                    .rect(legend_x, legend_y, 15.0, 15.0)
                    .fill(color)
                    .finish();

                let percentage = match total > 0.0 {
                    true => (p.value / total) * 100.0,
                    false => 0.0,
                };
                builder
                    .text(
                        legend_x + 25.0,
                        legend_y + 12.0,
                        &format!("{}: {} ({:.1}%)", p.label, p.value, percentage),
                    )
                    .fill("#ffffff")
                    .font_size(11.0)
                    .font_family("sans-serif")
                    .finish();

                legend_y += 25.0;
            }
        }
        "area" => {
            let layout = prepare_cartesian_grid(&mut builder, data, width, height);
            let n = data.len();
            let y_base = layout.margin_top + layout.plot_height;

            // 1. Build area polygon path
            let mut poly_points = format!("M {} {}", layout.margin_left, y_base);
            for (i, p) in data.iter().enumerate() {
                let x = point_x(i, n, layout.margin_left, layout.plot_width);
                let y =
                    layout.margin_top + layout.plot_height - (p.value / layout.max_val) * layout.plot_height;
                poly_points.push_str(&format!(" L {} {}", x, y));
            }
            let last_x = point_x(n.saturating_sub(1), n, layout.margin_left, layout.plot_width);
            poly_points.push_str(&format!(" L {} {} Z", last_x, y_base));

            builder
                .path(&poly_points)
                .fill("#3b82f6")
                .opacity(0.3)
                .finish();

            // 2. Draw line and markers
            let mut line_path = String::new();
            for (i, p) in data.iter().enumerate() {
                let x = point_x(i, n, layout.margin_left, layout.plot_width);
                let y =
                    layout.margin_top + layout.plot_height - (p.value / layout.max_val) * layout.plot_height;

                line_path.push_str(&format!("{} {} {}", path_cmd_prefix(i), x, y));

                builder
                    .circle(x, y, 4.0)
                    .fill("#ffffff")
                    .stroke("#3b82f6")
                    .stroke_width(2.0)
                    .finish();

                // Labels
                let label_x = x - (p.label.len() as f64 * 3.0);
                builder
                    .text(label_x, y_base + 20.0, &p.label)
                    .fill("#94a3b8")
                    .font_size(10.0)
                    .font_family("sans-serif")
                    .finish();
            }
            builder
                .path(&line_path)
                .stroke("#3b82f6")
                .stroke_width(3.0)
                .fill("none")
                .finish();
        }
        "scatter" => {
            let layout = prepare_cartesian_grid(&mut builder, data, width, height);
            let n = data.len();
            let y_base = layout.margin_top + layout.plot_height;

            for (i, p) in data.iter().enumerate() {
                let x = point_x(i, n, layout.margin_left, layout.plot_width);
                let y =
                    layout.margin_top + layout.plot_height - (p.value / layout.max_val) * layout.plot_height;

                let color = palette[i % palette.len()];
                builder
                    .circle(x, y, 6.0)
                    .fill(color)
                    .stroke("#ffffff")
                    .stroke_width(1.5)
                    .finish();

                // Value label above dot
                builder
                    .text(x - 10.0, y - 10.0, &format!("{:.1}", p.value))
                    .fill("#ffffff")
                    .font_size(9.0)
                    .font_family("sans-serif")
                    .finish();

                // X-axis labels
                let label_x = x - (p.label.len() as f64 * 3.0);
                builder
                    .text(label_x, y_base + 20.0, &p.label)
                    .fill("#94a3b8")
                    .font_size(10.0)
                    .font_family("sans-serif")
                    .finish();
            }
        }
        "radar" => {
            let cx = width as f64 / 2.0;
            let cy = height as f64 / 2.0 + 20.0;
            let r = (width.min(height) as f64 * 0.7) / 2.0;

            let raw_max = data.iter().map(|p| p.value).fold(0.0f64, f64::max);
            let max_val = match raw_max <= 0.0 {
                true => 1.0,
                false => raw_max,
            };
            let n = data.len();

            // Draw concentric grids (5 rings)
            let ring_count = 5;
            for i in 1..=ring_count {
                let ratio = i as f64 / ring_count as f64;
                let ring_r = r * ratio;
                let mut grid_path = String::new();

                for j in 0..n {
                    let angle = j as f64 * (2.0 * std::f64::consts::PI / n as f64)
                        - std::f64::consts::FRAC_PI_2;
                    let x = cx + ring_r * angle.cos();
                    let y = cy + ring_r * angle.sin();
                    grid_path.push_str(&format!("{} {} {}", path_cmd_prefix(j), x, y));
                }
                grid_path.push_str(" Z");
                builder
                    .path(&grid_path)
                    .stroke("#333355")
                    .stroke_width(1.0)
                    .fill("none")
                    .finish();
            }

            // Draw spokes (lines from center to outer ring vertices)
            let mut data_path = String::new();
            for (j, item) in data.iter().enumerate() {
                let angle = j as f64 * (2.0 * std::f64::consts::PI / n as f64)
                    - std::f64::consts::FRAC_PI_2;
                let outer_x = cx + r * angle.cos();
                let outer_y = cy + r * angle.sin();

                builder
                    .path(&format!("M {} {} L {} {}", cx, cy, outer_x, outer_y))
                    .stroke("#333355")
                    .stroke_width(1.0)
                    .fill("none")
                    .finish();

                // Labels
                let label_dist = r + 20.0;
                let label_x = cx + label_dist * angle.cos() - (item.label.len() as f64 * 3.0);
                let label_y = cy + label_dist * angle.sin() + 4.0;
                builder
                    .text(label_x, label_y, &item.label)
                    .fill("#94a3b8")
                    .font_size(11.0)
                    .font_family("sans-serif")
                    .finish();

                // Data point coord
                let val_r = r * (item.value / max_val);
                let val_x = cx + val_r * angle.cos();
                let val_y = cy + val_r * angle.sin();

                data_path.push_str(&format!("{} {} {}", path_cmd_prefix(j), val_x, val_y));
            }
            data_path.push_str(" Z");

            // Draw data polygon area
            builder
                .path(&data_path)
                .fill("#3b82f6")
                .opacity(0.35)
                .finish();
            builder
                .path(&data_path)
                .stroke("#3b82f6")
                .stroke_width(2.5)
                .fill("none")
                .finish();

            // Draw circles on data vertices
            for (j, item) in data.iter().enumerate() {
                let angle = j as f64 * (2.0 * std::f64::consts::PI / n as f64)
                    - std::f64::consts::FRAC_PI_2;
                let val_r = r * (item.value / max_val);
                let val_x = cx + val_r * angle.cos();
                let val_y = cy + val_r * angle.sin();

                builder
                    .circle(val_x, val_y, 4.0)
                    .fill("#ffffff")
                    .stroke("#3b82f6")
                    .stroke_width(2.0)
                    .finish();
            }
        }
        other => {
            return Err(OpenMediaError::InvalidParameter {
                param: "chart_type".to_string(),
                reason: format!("Unsupported chart type: {}", other),
            });
        }
    }

    Ok(builder.build())
}
