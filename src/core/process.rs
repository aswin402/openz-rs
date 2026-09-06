//! Shared cross-platform process utilities for spawning host shell commands.

/// Builds a synchronous `std::process::Command` targeting the host shell.
/// Uses `cmd.exe /C <cmd>` on Windows and `sh -c <cmd>` on Unix.
pub fn host_shell_command(cmd: &str) -> std::process::Command {
    let mut command = if cfg!(target_os = "windows") {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", cmd]);
        c
    } else {
        let mut c = std::process::Command::new("sh");
        c.args(["-c", cmd]);
        c
    };
    crate::config::loader::set_command_cwd(&mut command);
    command
}

/// Builds an asynchronous `tokio::process::Command` targeting the host shell.
/// Uses `cmd.exe /C <cmd>` on Windows and `sh -c <cmd>` on Unix.
pub fn host_tokio_shell_command(cmd: &str) -> tokio::process::Command {
    let mut command = if cfg!(target_os = "windows") {
        let mut c = tokio::process::Command::new("cmd");
        c.args(["/C", cmd]);
        c
    } else {
        let mut c = tokio::process::Command::new("sh");
        c.args(["-c", cmd]);
        c
    };
    crate::config::loader::set_tokio_command_cwd(&mut command);
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_shell_command_builds() {
        let cmd = host_shell_command("echo hello");
        let program = cmd.get_program().to_string_lossy().to_string();
        if cfg!(target_os = "windows") {
            assert_eq!(program, "cmd");
        } else {
            assert_eq!(program, "sh");
        }
    }

    #[tokio::test]
    async fn test_host_tokio_shell_command_runs() {
        let mut cmd = host_tokio_shell_command("echo hello_openz_process");
        let output = cmd.output().await.expect("shell command should run");
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("hello_openz_process"));
    }
}
