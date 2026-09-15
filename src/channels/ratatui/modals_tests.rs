use super::*;

#[test]
fn test_centered_rect_bounds() {
    let parent = Rect::new(0, 0, 100, 100);
    let centered = centered_rect(50, 50, parent);
    assert_eq!(centered.width, 50);
    assert_eq!(centered.height, 50);
    assert_eq!(centered.x, 25);
    assert_eq!(centered.y, 25);
}
