#[test]
fn test_icon_retrieval() {
    let svg = openmedia_svg::get_icon_svg("home", 32, "#ff0000", 2.5).unwrap();
    assert!(svg.contains("viewBox=\"0 0 24 24\""));
    assert!(svg.contains("stroke=\"#ff0000\""));
    assert!(svg.contains("stroke-width=\"2.5\""));
    assert!(svg.contains("width=\"32\""));
    assert!(svg.contains("height=\"32\""));
    assert!(svg.contains("<polyline"));

    // Retrieve an invalid icon
    let invalid = openmedia_svg::get_icon_svg("invalid-icon-name-12345", 32, "#ff0000", 2.5);
    assert!(invalid.is_none());
}

#[test]
fn test_multiple_icons_retrieval() {
    let list = vec!["user", "settings", "play", "pause", "check", "star"];
    for name in list {
        let svg = openmedia_svg::get_icon_svg(name, 24, "#000000", 2.0);
        assert!(svg.is_some(), "Icon '{}' should be found", name);
    }
}
