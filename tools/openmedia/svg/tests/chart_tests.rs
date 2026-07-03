#[test]
fn test_bar_chart_generation() {
    let data = vec![
        openmedia_svg::ChartPoint {
            label: "A".to_string(),
            value: 10.0,
        },
        openmedia_svg::ChartPoint {
            label: "B".to_string(),
            value: 20.0,
        },
    ];
    let svg = openmedia_svg::create_chart("bar", Some("My Bar Chart"), &data, 800, 600).unwrap();
    assert!(svg.contains("My Bar Chart"));
    assert!(svg.contains("<rect"));
}

#[test]
fn test_line_chart_generation() {
    let data = vec![
        openmedia_svg::ChartPoint {
            label: "Jan".to_string(),
            value: 15.0,
        },
        openmedia_svg::ChartPoint {
            label: "Feb".to_string(),
            value: 25.0,
        },
    ];
    let svg = openmedia_svg::create_chart("line", Some("My Line Chart"), &data, 800, 600).unwrap();
    assert!(svg.contains("My Line Chart"));
    assert!(svg.contains("<circle cx="));
    assert!(svg.contains("<path d="));
}

#[test]
fn test_pie_chart_generation() {
    let data = vec![
        openmedia_svg::ChartPoint {
            label: "Red".to_string(),
            value: 30.0,
        },
        openmedia_svg::ChartPoint {
            label: "Blue".to_string(),
            value: 70.0,
        },
    ];
    let svg = openmedia_svg::create_chart("pie", Some("My Pie Chart"), &data, 800, 600).unwrap();
    assert!(svg.contains("My Pie Chart"));
    assert!(svg.contains("Red: 30"));
    assert!(svg.contains("Blue: 70"));
    assert!(svg.contains("<path d="));
}

#[test]
fn test_area_chart_generation() {
    let data = vec![
        openmedia_svg::ChartPoint { label: "Jan".to_string(), value: 10.0 },
        openmedia_svg::ChartPoint { label: "Feb".to_string(), value: 20.0 },
    ];
    let svg = openmedia_svg::create_chart("area", Some("Area Test"), &data, 800, 600).unwrap();
    assert!(svg.contains("<path d="));
}

#[test]
fn test_scatter_chart_generation() {
    let data = vec![
        openmedia_svg::ChartPoint { label: "Jan".to_string(), value: 10.0 },
        openmedia_svg::ChartPoint { label: "Feb".to_string(), value: 20.0 },
    ];
    let svg = openmedia_svg::create_chart("scatter", Some("Scatter Test"), &data, 800, 600).unwrap();
    assert!(svg.contains("<circle cx="));
}

#[test]
fn test_radar_chart_generation() {
    let data = vec![
        openmedia_svg::ChartPoint { label: "A".to_string(), value: 80.0 },
        openmedia_svg::ChartPoint { label: "B".to_string(), value: 60.0 },
        openmedia_svg::ChartPoint { label: "C".to_string(), value: 90.0 },
    ];
    let svg = openmedia_svg::create_chart("radar", Some("Radar Test"), &data, 800, 600).unwrap();
    assert!(svg.contains("Radar Test"));
    assert!(svg.contains("<path d="));
}

