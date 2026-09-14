use super::valid_telegram_target;

#[test]
fn accepts_explicit_chat_ids_and_usernames() {
    assert!(valid_telegram_target("1404322011"));
    assert!(valid_telegram_target("-1001234567890"));
    assert!(valid_telegram_target("@example_user"));
}

#[test]
fn rejects_phone_numbers_and_ambiguous_targets() {
    assert!(!valid_telegram_target("+918870020639"));
    assert!(!valid_telegram_target("918870020639"));
    assert!(!valid_telegram_target("1404322011 extra"));
    assert!(!valid_telegram_target("@a"));
}
