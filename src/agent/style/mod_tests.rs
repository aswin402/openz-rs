use super::*;

#[test]
fn test_clean_names() {
    assert_eq!(get_tool_clean_name("exec_command"), "Bash");
    assert_eq!(get_tool_clean_name("write_file"), "Write");
    assert_eq!(get_tool_clean_name("patch_file"), "Edit");
    assert_eq!(get_tool_clean_name("replace_lines"), "Edit");
    assert_eq!(get_tool_clean_name("read_file"), "Read");
    assert_eq!(get_tool_clean_name("delegate_task"), "Delegate Task");
    assert_eq!(get_tool_clean_name("my_custom_tool"), "My Custom Tool");
    assert_eq!(get_tool_clean_name("a_b_c"), "A B C");
}

#[test]
fn test_tree_prefix_depth_0() {
    // Without scoping, DELEGATION_DEPTH should default to 0
    assert_eq!(get_tree_prefix(true), "  L ");
    assert_eq!(get_tree_prefix(false), "");

    let spinner = get_tree_spinner_msg("test", "");
    assert!(spinner.contains("  L "));
    assert!(spinner.contains("Running..."));
    assert!(spinner.contains(colors::AURA_SLATE));
}

#[tokio::test]
async fn test_tree_prefix_nested() {
    crate::tools::subagent::DELEGATION_DEPTH
        .scope(1, async {
            assert_eq!(get_tree_prefix(true), "  L ");
            assert_eq!(get_tree_prefix(false), "  L ");

            let spinner = get_tree_spinner_msg("test", "");
            assert!(spinner.contains("  L "));
            assert!(spinner.contains("Running..."));
        })
        .await;

    crate::tools::subagent::DELEGATION_DEPTH
        .scope(2, async {
            assert_eq!(get_tree_prefix(true), "    L ");
            assert_eq!(get_tree_prefix(false), "    L ");

            let spinner = get_tree_spinner_msg("test", "");
            assert!(spinner.contains("    L "));
            assert!(spinner.contains("Running..."));
        })
        .await;

    crate::tools::subagent::DELEGATION_DEPTH
        .scope(3, async {
            assert_eq!(get_tree_prefix(true), "      L ");
            assert_eq!(get_tree_prefix(false), "      L ");

            let spinner = get_tree_spinner_msg("test", "");
            assert!(spinner.contains("      L "));
            assert!(spinner.contains("Running..."));
        })
        .await;
}

#[test]
fn test_tree_tool_start_msg() {
    // Without args
    let msg = get_tree_tool_start_msg("exec_command", "");
    assert!(msg.contains("Bash"));
    assert!(msg.contains('●'));

    let bullet_idx = msg.find('●').unwrap();
    // RED_ORANGE should precede bullet
    assert!(msg[..bullet_idx].ends_with(colors::RED_ORANGE));
    // COLOR_RESET should follow bullet (followed by a space)
    let after_bullet = &msg[bullet_idx + '●'.len_utf8()..];
    assert!(after_bullet.starts_with(" "));
    assert!(after_bullet[1..].starts_with(colors::COLOR_RESET));

    // With args
    let msg_args = get_tree_tool_start_msg("write_file", "--force");
    assert!(msg_args.contains("Write"));
    assert!(msg_args.contains("--force"));
    assert!(msg_args.contains('●'));

    let bullet_idx_args = msg_args.find('●').unwrap();
    assert!(msg_args[..bullet_idx_args].ends_with(colors::RED_ORANGE));
    let after_bullet_args = &msg_args[bullet_idx_args + '●'.len_utf8()..];
    assert!(after_bullet_args.starts_with(" "));
    assert!(after_bullet_args[1..].starts_with(colors::COLOR_RESET));
}

#[test]
fn test_wrap_line() {
    let line = "hello world this is a test of wrapping";
    let wrapped = wrap_line(line, 10);
    assert_eq!(
        wrapped,
        vec!["hello", "world this", "is a test", "of", "wrapping"]
    );

    let long_word = "supercalifragilistic";
    let wrapped_long = wrap_line(long_word, 10);
    assert_eq!(wrapped_long, vec!["supercalif", "ragilistic"]);
}

#[test]
fn test_clean_tool_args_msg() {
    assert_eq!(
        clean_tool_args_msg("web_search", "\x1b[1mWebSearch\x1b[0m"),
        ""
    );
    assert_eq!(
        clean_tool_args_msg(
            "web_search",
            "\x1b[1mWebSearch\x1b[0m(\x1b[38;2;107;122;153mquery: \"ZeroClaw\"\x1b[0m)"
        ),
        "query: \"ZeroClaw\""
    );
    assert_eq!(
        clean_tool_args_msg("exec_command", "\x1b[1mBash\x1b[0m(cargo build)"),
        "cargo build"
    );
    assert_eq!(clean_tool_args_msg("write_file", "--force"), "--force");
}

#[test]
fn test_strip_ansi_escapes() {
    assert_eq!(strip_ansi_escapes("\x1b[1mRead\x1b[0m"), "Read");
    assert_eq!(
        strip_ansi_escapes("\x1b[38;2;255;0;0mError\x1b[0m details"),
        "Error details"
    );
}
