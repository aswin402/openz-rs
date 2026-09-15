use super::*;

#[test]
fn render_plan_flags_30_second_30fps_as_too_large_for_direct_default_render() {
    let plan = HtmlVideoRenderPlan::new(30.0, 30, 30, 1500).unwrap();
    assert_eq!(plan.total_frames, 900);
    assert!(plan.exceeds_default_direct_limit());
    assert!(plan.guidance().contains("render segments"));
    assert!(plan.guidance().contains("900 frames"));
}

#[test]
fn render_plan_allows_10_second_30fps_direct_render() {
    let plan = HtmlVideoRenderPlan::new(10.0, 30, 30, 1500).unwrap();
    assert_eq!(plan.total_frames, 300);
    assert!(!plan.exceeds_default_direct_limit());
}
