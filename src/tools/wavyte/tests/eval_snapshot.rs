use wavyte::{Composition, Evaluator, FrameIndex, digest_u64};

#[test]
fn eval_snapshot_is_deterministic() {
    let s = include_str!("data/simple_comp.json");
    let comp: Composition = serde_json::from_str(s).unwrap();

    let mut digest = 0u64;
    for f in 0..20u64 {
        let g = Evaluator::eval_frame(&comp, FrameIndex(f)).unwrap();
        let bytes = serde_json::to_vec(&g).unwrap();
        digest ^= digest_u64(&bytes);
    }

    // Updated when semantics change (intentionally should be rare).
    let expected: u64 = 17_822_534_885_193_820_641;
    assert_eq!(digest, expected);
}
