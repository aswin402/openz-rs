pub mod colors;
pub mod icons;
pub mod spinner;
pub mod menu;

pub use colors::*;
pub use icons::*;
pub use spinner::*;
pub use menu::*;

use std::io::Write;

#[macro_export]
macro_rules! tui_println {
    () => {
        $crate::agent::style::tui_println_fn("")
    };
    ($($arg:tt)*) => {
        $crate::agent::style::tui_println_fn(format!($($arg)*))
    };
}

#[macro_export]
macro_rules! tui_print {
    () => {
        $crate::agent::style::tui_print_fn("")
    };
    ($($arg:tt)*) => {
        $crate::agent::style::tui_print_fn(format!($($arg)*))
    };
}

pub fn tui_println_fn<T: AsRef<str>>(msg: T) {
    if is_silent() {
        return;
    }
    let s = msg.as_ref();
    let replaced = s.replace("\r\n", "\n").replace("\n", "\r\n");
    print!("{}\r\n", replaced);
    let _ = std::io::stdout().flush();
}

pub fn tui_print_fn<T: AsRef<str>>(msg: T) {
    if is_silent() {
        return;
    }
    let s = msg.as_ref();
    let replaced = s.replace("\r\n", "\n").replace("\n", "\r\n");
    print!("{}", replaced);
    let _ = std::io::stdout().flush();
}

#[macro_export]
macro_rules! tui_eprintln {
    () => {
        $crate::agent::style::tui_eprintln_fn("")
    };
    ($($arg:tt)*) => {
        $crate::agent::style::tui_eprintln_fn(format!($($arg)*))
    };
}

#[macro_export]
macro_rules! tui_eprint {
    () => {
        $crate::agent::style::tui_eprint_fn("")
    };
    ($($arg:tt)*) => {
        $crate::agent::style::tui_eprint_fn(format!($($arg)*))
    };
}

pub fn tui_eprintln_fn<T: AsRef<str>>(msg: T) {
    if is_silent() {
        return;
    }
    let s = msg.as_ref();
    let replaced = s.replace("\r\n", "\n").replace("\n", "\r\n");
    eprint!("{}\r\n", replaced);
    let _ = std::io::stderr().flush();
}

pub fn tui_eprint_fn<T: AsRef<str>>(msg: T) {
    if is_silent() {
        return;
    }
    let s = msg.as_ref();
    let replaced = s.replace("\r\n", "\n").replace("\n", "\r\n");
    eprint!("{}", replaced);
    let _ = std::io::stderr().flush();
}

/// Returns the prefix string for a tree trace line at the current delegation depth.
pub fn get_tree_prefix(is_leaf: bool) -> String {
    let depth = crate::tools::subagent::DELEGATION_DEPTH.try_with(|d| *d).unwrap_or(0);
    if depth == 0 {
        if is_leaf {
            "  └─ ".to_string()
        } else {
            "".to_string()
        }
    } else {
        let mut s = "  ".to_string();
        for _ in 0..(depth - 1) {
            s.push_str("│  ");
        }
        if is_leaf {
            s.push_str("└─ ");
        } else {
            s.push_str("├─ ");
        }
        s
    }
}

/// Converts a tool name into a clean, visual title.
pub fn get_tool_clean_name(name: &str) -> String {
    match name {
        "exec_command" => "Bash".to_string(),
        "write_file" => "Write".to_string(),
        "patch_file" | "replace_lines" => "Edit".to_string(),
        "read_file" => "Read".to_string(),
        _ => {
            let mut result = Vec::new();
            for word in name.split('_') {
                if word.is_empty() {
                    continue;
                }
                let mut chars = word.chars();
                if let Some(first) = chars.next() {
                    let capitalized = first.to_uppercase().to_string() + chars.as_str();
                    result.push(capitalized);
                }
            }
            result.join(" ")
        }
    }
}

/// Generates a styled spinner message indicating a tool is running under the tree bullet.
pub fn get_tree_spinner_msg(_name: &str, _formatted_args: &str) -> String {
    let depth = crate::tools::subagent::DELEGATION_DEPTH.try_with(|d| *d).unwrap_or(0);
    let msg = if depth == 0 {
        "  └─ Running...".to_string()
    } else {
        let mut s = "  ".to_string();
        for _ in 0..(depth - 1) {
            s.push_str("│  ");
        }
        s.push_str("│  └─ Running...");
        s
    };
    format!("{}{}{}", colors::AURA_SLATE, msg, colors::COLOR_RESET)
}

/// Generates the styled start indicator of a tool execution with tree prefixes.
pub fn get_tree_tool_start_msg(name: &str, formatted_args: &str) -> String {
    let prefix = get_tree_prefix(false);
    let clean_name = get_tool_clean_name(name);
    if formatted_args.is_empty() {
        format!(
            "{}{}{}{}● {}{}{}{}{}",
            colors::AURA_SLATE, prefix, colors::COLOR_RESET,
            colors::RED_ORANGE,
            colors::COLOR_RESET,
            colors::COLOR_BOLD, colors::LIGHT_WHITE, clean_name, colors::COLOR_RESET
        )
    } else {
        format!(
            "{}{}{}{}● {}{}{}{}{} {}{}{}",
            colors::AURA_SLATE, prefix, colors::COLOR_RESET,
            colors::RED_ORANGE,
            colors::COLOR_RESET,
            colors::COLOR_BOLD, colors::LIGHT_WHITE, clean_name, colors::COLOR_RESET,
            colors::AURA_SLATE, formatted_args, colors::COLOR_RESET
        )
    }
}

/// Prints the colored start indicator of a tool execution with tree prefixes.
pub fn print_tree_tool_start(name: &str, formatted_args: &str) {
    if is_silent() {
        return;
    }
    let output = get_tree_tool_start_msg(name, formatted_args);
    let replaced = output.replace("\r\n", "\n").replace("\n", "\r\n");
    print!("{}\r\n", replaced);
    let _ = std::io::stdout().flush();
}

#[cfg(test)]
mod tests {
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
        assert_eq!(get_tree_prefix(true), "  └─ ");
        assert_eq!(get_tree_prefix(false), "");
        
        let spinner = get_tree_spinner_msg("test", "");
        assert!(spinner.contains("  └─ Running..."));
        assert!(spinner.contains(colors::AURA_SLATE));
    }

    #[tokio::test]
    async fn test_tree_prefix_nested() {
        crate::tools::subagent::DELEGATION_DEPTH.scope(1, async {
            assert_eq!(get_tree_prefix(true), "  └─ ");
            assert_eq!(get_tree_prefix(false), "  ├─ ");
            
            let spinner = get_tree_spinner_msg("test", "");
            assert!(spinner.contains("  │  └─ Running..."));
        }).await;

        crate::tools::subagent::DELEGATION_DEPTH.scope(2, async {
            assert_eq!(get_tree_prefix(true), "  │  └─ ");
            assert_eq!(get_tree_prefix(false), "  │  ├─ ");
            
            let spinner = get_tree_spinner_msg("test", "");
            assert!(spinner.contains("  │  │  └─ Running..."));
        }).await;

        crate::tools::subagent::DELEGATION_DEPTH.scope(3, async {
            assert_eq!(get_tree_prefix(true), "  │  │  └─ ");
            assert_eq!(get_tree_prefix(false), "  │  │  ├─ ");
            
            let spinner = get_tree_spinner_msg("test", "");
            assert!(spinner.contains("  │  │  │  └─ Running..."));
        }).await;
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
}

