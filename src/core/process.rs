//! Shared cross-platform process utilities for spawning host shell commands.

/// Builds a synchronous `std::process::Command` targeting the host shell.
/// Uses `cmd.exe /C <cmd>` on Windows and `sh -c <cmd>` on Unix.
pub fn host_shell_command(cmd: &str) -> std::process::Command {
    let mut command = match cfg!(target_os = "windows") {
        true => {
            let mut c = std::process::Command::new("cmd");
            c.args(["/C", cmd]);
            c
        }
        false => {
            let mut c = std::process::Command::new("sh");
            c.args(["-c", cmd]);
            c
        }
    };
    crate::config::loader::set_command_cwd(&mut command);
    command
}

/// Builds an asynchronous `tokio::process::Command` targeting the host shell.
/// Uses `cmd.exe /C <cmd>` on Windows and `sh -c <cmd>` on Unix.
pub fn host_tokio_shell_command(cmd: &str) -> tokio::process::Command {
    let mut command = match cfg!(target_os = "windows") {
        true => {
            let mut c = tokio::process::Command::new("cmd");
            c.args(["/C", cmd]);
            c
        }
        false => {
            let mut c = tokio::process::Command::new("sh");
            c.args(["-c", cmd]);
            c
        }
    };
    crate::config::loader::set_tokio_command_cwd(&mut command);
    command
}

/// Properly quote a shell argument for the host shell.
/// Uses double-quotes with internal `"` escaped as `""` on Windows `cmd.exe`.
/// Uses single-quotes with internal `'` escaped as `'\''` on Unix POSIX `sh`.
pub fn quote_shell_arg(arg: &str) -> String {
    match cfg!(target_os = "windows") {
        true => {
            let mut s = String::with_capacity(arg.len() + 2);
            s.push('"');
            for c in arg.chars() {
                match c {
                    '"' => s.push_str("\"\""),
                    other => s.push(other),
                }
            }
            s.push('"');
            s
        }
        false => {
            let mut s = String::with_capacity(arg.len() + 2);
            s.push('\'');
            for c in arg.chars() {
                match c {
                    '\'' => s.push_str("'\\''"),
                    other => s.push(other),
                }
            }
            s.push('\'');
            s
        }
    }
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
