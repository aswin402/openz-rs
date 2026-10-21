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

/// Check whether a host process ID is currently alive.
pub fn process_is_alive(pid: u32) -> bool {
    match (pid == 0, pid == std::process::id()) {
        (true, _) => false,
        (false, true) => true,
        (false, false) => {
            #[cfg(unix)]
            {
                match pid > i32::MAX as u32 {
                    true => false,
                    false => {
                        let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
                        result == 0
                            || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
                    }
                }
            }
            #[cfg(not(unix))]
            {
                false
            }
        }
    }
}

/// Return available free disk space in bytes for the filesystem containing `path`.
#[cfg(unix)]
pub fn available_bytes(path: &std::path::Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };
    match rc == 0 {
        true => {
            let stat = unsafe { stat.assume_init() };
            Some(stat.f_bavail.saturating_mul(stat.f_frsize))
        }
        false => None,
    }
}

/// Return available free disk space in bytes for the filesystem containing `path`.
#[cfg(not(unix))]
pub fn available_bytes(_path: &std::path::Path) -> Option<u64> {
    None
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
