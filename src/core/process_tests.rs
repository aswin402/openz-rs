use super::*;

#[test]
fn test_host_shell_command_builds() {
    let cmd = host_shell_command("echo hello");
    let program = cmd.get_program().to_string_lossy().to_string();
    match cfg!(target_os = "windows") {
        true => assert_eq!(program, "cmd"),
        false => assert_eq!(program, "sh"),
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

#[test]
fn test_quote_shell_arg_handles_spaces_and_quotes() {
    let arg = "hello world's \"test\"";
    let quoted = quote_shell_arg(arg);
    match cfg!(target_os = "windows") {
        true => {
            assert!(quoted.starts_with('"') && quoted.ends_with('"'));
            assert!(quoted.contains("\"\"test\"\""));
        }
        false => {
            assert!(quoted.starts_with('\'') && quoted.ends_with('\''));
            assert!(quoted.contains("'\\''"));
        }
    }
}
