use super::*;

#[test]
fn registered_telegram_commands_use_telegram_safe_names() {
    assert!(TELEGRAM_COMMANDS
        .iter()
        .all(|(command, _)| valid_telegram_command_name(command)));
    let payload = telegram_commands_payload();
    assert_eq!(
        payload["commands"].as_array().unwrap().len(),
        TELEGRAM_COMMANDS.len()
    );
}

#[test]
fn telegram_stop_command_is_classified_as_cancel() {
    assert_eq!(
        telegram_command_action("/stop"),
        TelegramCommandAction::Stop
    );
    assert_eq!(
        telegram_command_action("/stop now"),
        TelegramCommandAction::Stop
    );
    assert_eq!(
        telegram_command_action("/cancel"),
        TelegramCommandAction::Stop
    );
    assert_eq!(
        telegram_command_action("/tui-esc"),
        TelegramCommandAction::Stop
    );
    assert_eq!(
        telegram_command_action("/tui-cancel"),
        TelegramCommandAction::Stop
    );
    assert_eq!(
        telegram_command_action("/remote"),
        TelegramCommandAction::RemoteMode
    );
    assert_eq!(
        telegram_command_action("hello"),
        TelegramCommandAction::None
    );
}
