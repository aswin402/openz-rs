use super::{is_ctrl_exit_key, is_turn_cancel_key};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

#[test]
fn ctrl_exit_key_accepts_common_terminal_encodings() {
    assert!(is_ctrl_exit_key(&key(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL
    )));
    assert!(is_ctrl_exit_key(&key(
        KeyCode::Char('C'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT
    )));
    assert!(is_ctrl_exit_key(&key(
        KeyCode::Char('d'),
        KeyModifiers::CONTROL
    )));
    assert!(is_ctrl_exit_key(&key(
        KeyCode::Char('\u{3}'),
        KeyModifiers::NONE
    )));
    assert!(is_ctrl_exit_key(&key(
        KeyCode::Char('\u{4}'),
        KeyModifiers::NONE
    )));
}

#[test]
fn ctrl_exit_key_rejects_printable_chars_and_key_releases() {
    assert!(!is_ctrl_exit_key(&key(
        KeyCode::Char('c'),
        KeyModifiers::NONE
    )));
    assert!(!is_ctrl_exit_key(&key(
        KeyCode::Char('v'),
        KeyModifiers::CONTROL
    )));

    let release = KeyEvent {
        code: KeyCode::Char('c'),
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Release,
        state: crossterm::event::KeyEventState::NONE,
    };
    assert!(!is_ctrl_exit_key(&release));
}

#[test]
fn turn_cancel_key_accepts_esc_and_ctrl_exit_keys() {
    assert!(is_turn_cancel_key(&key(KeyCode::Esc, KeyModifiers::NONE)));
    assert!(is_turn_cancel_key(&key(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL
    )));
    assert!(!is_turn_cancel_key(&key(
        KeyCode::Char('x'),
        KeyModifiers::NONE
    )));
}
