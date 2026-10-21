use super::*;
use crate::tools::wavyte::animation::anim::{InterpMode, Keyframe, Keyframes, SampleCtx};
use crate::tools::wavyte::foundation::core::FrameIndex;

#[test]
fn sequence_switches_at_boundary() {
    let a = Anim::constant(1.0);
    let b = Anim::Keyframes(Keyframes {
        keys: vec![Keyframe {
            frame: FrameIndex(0),
            value: 10.0,
            ease: crate::tools::wavyte::animation::ease::Ease::Linear,
        }],
        mode: InterpMode::Hold,
        default: None,
    });

    let s = sequence(a, 5, b);
    assert_eq!(s.sample(SampleCtx::test_ctx(4)).unwrap(), 1.0);
    assert_eq!(s.sample(SampleCtx::test_ctx(5)).unwrap(), 10.0);
}
