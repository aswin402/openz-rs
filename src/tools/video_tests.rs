use super::*;
use std::collections::BTreeMap;
use wavyte::{
    Anim, Asset, BlendMode, Canvas, Clip, ClipProps, Fps, FrameIndex, FrameRange, PathAsset,
    Track, Transform2D, Vec2,
};

#[tokio::test]
async fn test_generate_video() -> Result<()> {
    let tool = VideoGeneratorTool;
    let temp_dir =
        std::env::temp_dir().join(format!("openz_video_test_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir)?;
    let output_file = temp_dir.join("test_video.mp4");

    // Build a simple 30-frame composition
    let mut assets = BTreeMap::<String, Asset>::new();
    assets.insert(
        "rect".to_string(),
        Asset::Path(PathAsset {
            svg_path_d: "M0,0 L100,0 L100,100 L0,100 Z".to_string(),
        }),
    );

    let comp = Composition {
        fps: Fps::new(30, 1).unwrap(),
        canvas: Canvas {
            width: 256,
            height: 256,
        },
        duration: FrameIndex(30),
        assets,
        tracks: vec![Track {
            name: "main".to_string(),
            z_base: 0,
            layout_mode: wavyte::LayoutMode::Absolute,
            layout_gap_px: 0.0,
            layout_padding: wavyte::Edges::default(),
            layout_align_x: wavyte::LayoutAlignX::Start,
            layout_align_y: wavyte::LayoutAlignY::Start,
            layout_grid_columns: 2,
            clips: vec![Clip {
                id: "a_rect".to_string(),
                asset: "rect".to_string(),
                range: FrameRange::new(FrameIndex(0), FrameIndex(30)).unwrap(),
                props: ClipProps {
                    transform: Anim::constant(Transform2D {
                        translate: Vec2::new(78.0, 78.0),
                        scale: Vec2::new(1.0, 1.0),
                        ..Transform2D::default()
                    }),
                    opacity: Anim::constant(1.0),
                    blend: BlendMode::Normal,
                },
                z_offset: 0,
                effects: vec![],
                transition_in: None,
                transition_out: None,
            }],
        }],
        seed: 42,
    };

    let comp_json = serde_json::to_string(&comp)?;

    let args = json!({
        "composition_json": comp_json,
        "output_path": output_file.to_str().unwrap()
    });

    // The test environment might fail if ffmpeg is missing, but here we verified it is present.
    let res = tool.call(&args).await?;
    assert_eq!(res["status"], "success");
    assert!(output_file.exists());

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}
