use super::*;

#[test]
fn attachment_policy_rejects_unsafe_mime_and_aggregate_overflow() {
    assert!(attachment_mime_allowed("image/png"));
    assert!(attachment_mime_allowed("application/pdf"));
    assert!(!attachment_mime_allowed("application/x-sh"));
    assert!(attachment_total_within_quota(0, MAX_ATTACHMENT_BYTES));
    assert!(!attachment_total_within_quota(
        MAX_ATTACHMENT_TOTAL_BYTES - 1,
        2,
    ));
}
