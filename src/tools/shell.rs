use crate::tools::Tool;
use anyhow::{anyhow, Result};
use std::process::{Command, Stdio};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

/// Apply Linux seccomp guard + resource limits to a command via pre_exec.
/// Runs inside the child process after fork() but before exec().
/// On non-Linux platforms this is a no-op.
#[cfg(target_os = "linux")]
unsafe fn apply_seccomp_guard() -> Result<(), std::io::Error> {
    // Prevent privilege escalation via setuid/setgid/capabilities
    if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
        return Err(std::io::Error::last_os_error());
    }

    // Kill child if parent dies (no orphaned processes)
    if libc::prctl(
        libc::PR_SET_PDEATHSIG,
        libc::SIGKILL as libc::c_ulong,
        0,
        0,
        0,
    ) != 0
    {
        return Err(std::io::Error::last_os_error());
    }

    // Limit CPU time (30 seconds soft, 60 hard)
    let rlim_cpu = libc::rlimit {
        rlim_cur: 30,
        rlim_max: 60,
    };
    if libc::setrlimit(libc::RLIMIT_CPU, &rlim_cpu) != 0 {
        return Err(std::io::Error::last_os_error());
    }

    // Limit virtual memory (256 MB soft, 512 MB hard)
    let rlim_as = libc::rlimit {
        rlim_cur: 256 * 1024 * 1024,
        rlim_max: 512 * 1024 * 1024,
    };
    if libc::setrlimit(libc::RLIMIT_AS, &rlim_as) != 0 {
        return Err(std::io::Error::last_os_error());
    }

    // Limit output file size (10 MB)
    let rlim_fsize = libc::rlimit {
        rlim_cur: 10 * 1024 * 1024,
        rlim_max: 10 * 1024 * 1024,
    };
    if libc::setrlimit(libc::RLIMIT_FSIZE, &rlim_fsize) != 0 {
        return Err(std::io::Error::last_os_error());
    }

    // Apply seccomp BPF filter (allowlist-based, x86_64 only)
    allowlist_seccomp_filter()?;

    Ok(())
}

/// Build and install a seccomp BPF allowlist filter.
/// Denies dangerous syscalls (networking, module loading, ptrace, etc.)
/// while allowing normal file I/O, memory, and process operations.
#[cfg(target_os = "linux")]
unsafe fn allowlist_seccomp_filter() -> Result<(), std::io::Error> {
    #[cfg(not(target_arch = "x86_64"))]
    {
        tracing::warn!(
            "seccomp sandbox disabled on this CPU architecture until architecture-specific syscall tables are implemented"
        );
        return Ok(());
    }

    // x86_64 syscall numbers to allow
    const ALLOWED_SYSCALLS: &[u32] = &[
        0,   // read
        1,   // write
        2,   // open
        3,   // close
        4,   // stat
        5,   // fstat
        6,   // lstat
        7,   // poll
        8,   // lseek
        9,   // mmap
        10,  // mprotect
        11,  // munmap
        12,  // brk
        13,  // rt_sigaction
        14,  // sigprocmask
        15,  // rt_sigreturn
        16,  // ioctl
        17,  // pread64
        18,  // pwrite64
        19,  // readv
        20,  // writev
        21,  // access
        22,  // pipe
        23,  // select
        24,  // sched_yield
        25,  // mremap
        26,  // msync
        27,  // mincore
        28,  // madvise
        29,  // shmget
        30,  // shmat
        31,  // shmctl
        32,  // dup
        33,  // dup2
        35,  // nanosleep
        39,  // getpid
        56,  // clone
        57,  // fork
        58,  // vfork
        59,  // execve
        60,  // exit
        61,  // wait4
        62,  // kill (needed for process management)
        72,  // fcntl
        78,  // getdents
        79,  // getcwd
        80,  // chdir
        81,  // fchdir
        82,  // rename
        83,  // mkdir
        84,  // rmdir
        85,  // creat
        86,  // link
        87,  // unlink
        88,  // symlink
        89,  // readlink
        90,  // chmod
        91,  // fchmod
        92,  // chown
        93,  // fchown
        95,  // umask
        96,  // getpriority
        97,  // setpriority
        102, // getuid
        104, // getgid
        107, // geteuid
        108, // getegid
        110, // getppid
        125, // capget
        126, // capset
        131, // sigaltstack
        135, // personality
        137, // statfs
        138, // fstatfs
        157, // prctl
        158, // arch_prctl
        186, // gettid
        202, // futex
        217, // getdents64
        218, // set_tid_address
        228, // clock_gettime
        231, // exit_group
        232, // epoll_wait
        233, // epoll_ctl
        234, // tgkill
        240, // sched_getaffinity
        241, // sched_setaffinity
        257, // openat
        258, // mkdirat
        259, // mknodat
        260, // fchownat
        262, // newfstatat
        263, // unlinkat
        264, // renameat
        265, // linkat
        267, // faccessat
        268, // readlinkat
        273, // set_robust_list
        274, // get_robust_list
        281, // eventfd2
        282, // epoll_create1
        283, // dup3
        284, // pipe2
        291, // inotify_init1
        292, // inotify_add_watch
        293, // inotify_rm_watch
        302, // prlimit64
        318, // getrandom
        332, // statx
        334, // rseq
        436, // close_range
    ];

    // Build BPF program:
    // 1. Load architecture from seccomp_data (offset 4)
    // 2. Check if x86_64 (0xC000003E)
    // 3. If not, KILL
    // 4. Load syscall number
    // 5. Check against allowlist
    // 6. If not found, KILL
    // 7. If found, ALLOW

    // AUDIT_ARCH_X86_64 = 0xC000003E (little-endian)
    const AUDIT_ARCH_X86_64: u32 = 0xC000003E;

    let n_allowed = ALLOWED_SYSCALLS.len();

    // BPF layout:
    // 0: LD W ABS 4                    → load arch
    // 1: JEQ #AUDIT_ARCH_X86_64, +1, +0  -> if x86_64, skip next kill
    // 2: RET #SECCOMP_RET_KILL         → wrong arch
    // 3: LD W ABS 0                    → load syscall num
    // 4..4+N-1: for each syscall in allowlist:
    //    JEQ #syscall, +(N - idx + 1), +1  → if match, jump to ALLOW; else continue
    // 4+N: RET #SECCOMP_RET_KILL       → not allowed
    // 4+N+1: RET #SECCOMP_RET_ALLOW    → allowed

    let n_instr = 4 + n_allowed + 2;
    let mut filter: Vec<libc::sock_filter> = Vec::with_capacity(n_instr);

    // Arch check
    filter.push(libc::sock_filter {
        code: (libc::BPF_LD | libc::BPF_W | libc::BPF_ABS) as u16,
        jt: 0,
        jf: 0,
        k: 4, // offset of arch in seccomp_data
    });
    // If arch == AUDIT_ARCH_X86_64, skip the kill instruction and load syscall number.
    filter.push(libc::sock_filter {
        code: (libc::BPF_JMP | libc::BPF_JEQ | libc::BPF_K) as u16,
        jt: 1,
        jf: 0,
        k: AUDIT_ARCH_X86_64,
    });
    // Unsupported arch
    filter.push(libc::sock_filter {
        code: (libc::BPF_RET | libc::BPF_K) as u16,
        jt: 0,
        jf: 0,
        k: libc::SECCOMP_RET_KILL,
    });

    // Load syscall number (offset 0 in seccomp_data)
    filter.push(libc::sock_filter {
        code: (libc::BPF_LD | libc::BPF_W | libc::BPF_ABS) as u16,
        jt: 0,
        jf: 0,
        k: 0,
    });

    // For each allowed syscall: check and jump to ALLOW if match
    // Jump offset: (remaining checks) + 1 (KILL) to reach ALLOW
    for (idx, &syscall) in ALLOWED_SYSCALLS.iter().enumerate() {
        let remaining = n_allowed - idx;
        filter.push(libc::sock_filter {
            code: (libc::BPF_JMP | libc::BPF_JEQ | libc::BPF_K) as u16,
            jt: (remaining) as u8, // jump to ALLOW (skip remaining checks + KILL)
            jf: 0,
            k: syscall,
        });
    }

    // Default: kill (not in allowlist)
    filter.push(libc::sock_filter {
        code: (libc::BPF_RET | libc::BPF_K) as u16,
        jt: 0,
        jf: 0,
        k: libc::SECCOMP_RET_KILL,
    });

    // ALLOW (target of matching JEQ jumps)
    filter.push(libc::sock_filter {
        code: (libc::BPF_RET | libc::BPF_K) as u16,
        jt: 0,
        jf: 0,
        k: libc::SECCOMP_RET_ALLOW,
    });

    let prog = libc::sock_fprog {
        len: filter.len() as u16,
        filter: filter.as_ptr() as *mut libc::sock_filter,
    };

    if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
        return Err(std::io::Error::last_os_error());
    }

    // Use seccomp(2) syscall directly — prctl(PR_SET_SECCOMP, ...) has a
    // long-standing kernel quirk on some x86_64 builds where SECCOMP_SET_MODE_FILTER
    // is mis-handled during execve, causing SIGKILL instead of SIGSYS even with
    // SECCOMP_RET_ALLOW filters. The seccomp(2) syscall avoids this entirely.
    let ret = unsafe {
        libc::syscall(
            libc::SYS_seccomp,
            libc::SECCOMP_SET_MODE_FILTER as u64,
            0u64,
            &prog as *const libc::sock_fprog,
        )
    };
    if ret != 0 {
        return Err(std::io::Error::last_os_error());
    }

    Ok(())
}

/// Apply sandbox guard to a Command on Linux. No-op on other platforms.
#[cfg(target_os = "linux")]
fn sandbox_command(cmd: &mut Command, enable_sandbox: bool) {
    if !enable_sandbox {
        return;
    }
    unsafe {
        cmd.pre_exec(|| match apply_seccomp_guard() {
            Ok(()) => Ok(()),
            Err(e) => {
                // Log but don't block execution — seccomp is a best-effort layer
                tracing::warn!("seccomp guard failed (allowing execution anyway): {}", e);
                Ok(())
            }
        });
    }
}

#[cfg(not(target_os = "linux"))]
fn sandbox_command(_cmd: &mut Command, _enable_sandbox: bool) {
    // seccomp is Linux-specific; no sandboxing on this platform
}

pub struct ExecCommandTool;

#[async_trait::async_trait]
impl Tool for ExecCommandTool {
    fn name(&self) -> &str {
        "exec_command"
    }

    fn description(&self) -> &str {
        "Run a shell command on the host system and return its output."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "The shell command to execute" }
            },
            "required": ["command"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let command_str = arguments
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'command' argument"))?;

        // 1. Try to parse command line to see if it targets a WASM script or skill
        let parsed_args = parse_command_line(command_str);
        if !parsed_args.is_empty() {
            if let Some(wasm_file) = find_wasm_file(&parsed_args[0]) {
                let path = wasm_file.clone();
                let wasm_args = parsed_args[1..].to_vec();

                // Execute in spawn_blocking to avoid blocking tokio executor thread
                let wasm_res = tokio::task::spawn_blocking(move || {
                    crate::tools::wasm_sandbox::execute_wasm(&path, wasm_args)
                })
                .await?;

                match wasm_res {
                    Ok(val) => {
                        let stdout = val
                            .get("stdout")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let stderr = val
                            .get("stderr")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let status_code =
                            val.get("exit_code").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                        return Ok(serde_json::json!({
                            "status_code": status_code,
                            "stdout": stdout,
                            "stderr": stderr
                        }));
                    }
                    Err(e) => {
                        return Ok(serde_json::json!({
                            "status_code": 1,
                            "stdout": "".to_string(),
                            "stderr": format!("WASM execution error: {}", e)
                        }));
                    }
                }
            }
        }

        if let Some(detach_kind) = detach_command_kind(command_str) {
            spawn_detached_command(command_str, detach_kind)?;
            return Ok(serde_json::json!({
                "status_code": 0,
                "stdout": "",
                "stderr": "",
                "detached": true,
                "user_visible": true,
                "do_not_retry": true,
                "message": "Command launched in the background because it opens a desktop app or long-running server. Treat this as complete; do not try alternate launch methods unless the user says it failed."
            }));
        }

        // 2. Fallback to standard raw host shell execution
        let mut std_cmd = if cfg!(target_os = "windows") {
            let mut c = std::process::Command::new("cmd");
            c.args(["/C", command_str]);
            c
        } else {
            let mut c = std::process::Command::new("sh");
            c.args(["-c", command_str]);
            c
        };
        crate::config::loader::set_command_cwd(&mut std_cmd);
        let enable_sandbox = crate::config::loader::load_config()
            .map(|c| c.agents.defaults.enable_sandbox)
            .unwrap_or(false);
        sandbox_command(&mut std_cmd, enable_sandbox);

        let mut tokio_cmd = tokio::process::Command::from(std_cmd);
        tokio_cmd.kill_on_drop(true);

        let timeout_secs = crate::config::loader::load_config()
            .map(|c| c.agents.defaults.tool_timeout_secs)
            .unwrap_or(120);

        let output_res = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            tokio_cmd.output(),
        )
        .await;

        let output = match output_res {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => {
                return Ok(serde_json::json!({
                    "status_code": -1,
                    "stdout": "",
                    "stderr": format!("Command execution timed out after {} seconds", timeout_secs)
                }));
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let status_code = output.status.code().unwrap_or(-1);

        Ok(serde_json::json!({
            "status_code": status_code,
            "stdout": stdout,
            "stderr": stderr
        }))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DetachedCommandKind {
    DesktopApp,
    DevServer,
}

#[cfg(test)]
fn should_detach_command(command_line: &str) -> bool {
    detach_command_kind(command_line).is_some()
}

fn detach_command_kind(command_line: &str) -> Option<DetachedCommandKind> {
    let trimmed = command_line.trim();
    if trimmed.is_empty() || contains_shell_control_operator(trimmed) {
        return None;
    }
    let args = parse_command_line(trimmed);
    let first_idx = first_program_arg_index(&args)?;
    let first = command_basename(&args[first_idx]);
    let second = args
        .get(first_idx + 1)
        .map(|s| s.as_str())
        .unwrap_or_default();

    if (first == "gio" && second == "open")
        || matches!(
            first.as_str(),
            "xdg-open"
                | "gnome-open"
                | "kde-open"
                | "kioclient5"
                | "kioclient"
                | "open"
                | "start"
                | "firefox"
                | "google-chrome"
                | "google-chrome-stable"
                | "chromium"
                | "chromium-browser"
                | "brave"
                | "brave-browser"
                | "vlc"
                | "mpv"
                | "totem"
                | "eog"
                | "loupe"
                | "ristretto"
                | "feh"
                | "display"
                | "code"
                | "libreoffice"
                | "soffice"
        )
    {
        return Some(DetachedCommandKind::DesktopApp);
    }

    if is_long_running_dev_server(&args[first_idx..]) {
        return Some(DetachedCommandKind::DevServer);
    }

    None
}

fn contains_shell_control_operator(command_line: &str) -> bool {
    let mut in_double_quote = false;
    let mut in_single_quote = false;
    for c in command_line.chars() {
        match c {
            '"' if !in_single_quote => in_double_quote = !in_double_quote,
            '\'' if !in_double_quote => in_single_quote = !in_single_quote,
            ';' | '|' | '&' | '`' | '$' | '<' | '>' | '\n'
                if !in_double_quote && !in_single_quote =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn first_program_arg_index(args: &[String]) -> Option<usize> {
    args.iter().position(|arg| !looks_like_env_assignment(arg))
}

fn looks_like_env_assignment(arg: &str) -> bool {
    let Some((name, _)) = arg.split_once('=') else {
        return false;
    };
    !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !name.chars().next().is_some_and(|c| c.is_ascii_digit())
}

fn command_basename(program: &str) -> String {
    program
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(program)
        .to_ascii_lowercase()
}

fn is_long_running_dev_server(args: &[String]) -> bool {
    if args.is_empty() {
        return false;
    }
    let first = command_basename(&args[0]);
    let rest: Vec<String> = args
        .iter()
        .skip(1)
        .map(|s| s.to_ascii_lowercase())
        .collect();

    matches!(first.as_str(), "vite" | "next" | "astro")
        || first == "npx"
            && rest
                .iter()
                .any(|s| matches!(s.as_str(), "vite" | "next" | "astro"))
        || (matches!(first.as_str(), "npm" | "pnpm" | "yarn" | "bun")
            && rest
                .iter()
                .any(|s| matches!(s.as_str(), "dev" | "start" | "preview" | "serve")))
        || ((first == "python3" || first == "python")
            && rest
                .windows(2)
                .any(|w| w[0] == "-m" && w[1] == "http.server"))
}

fn spawn_detached_command(command_line: &str, kind: DetachedCommandKind) -> Result<()> {
    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = Command::new("cmd");
        c.args(["/C", command_line]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-c", command_line]);
        c
    };
    crate::config::loader::set_command_cwd(&mut cmd);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let child = cmd.spawn()?;
    match kind {
        DetachedCommandKind::DesktopApp => {
            std::thread::spawn(move || {
                let mut child = child;
                let _ = child.wait();
            });
        }
        DetachedCommandKind::DevServer => crate::shutdown::register_child(child),
    }
    Ok(())
}

fn parse_command_line(cmd: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_double_quote = false;
    let mut in_single_quote = false;
    let chars = cmd.chars().peekable();

    for c in chars {
        match c {
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
            }
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
            }
            ' ' | '\t' if !in_double_quote && !in_single_quote => {
                if !current.is_empty() {
                    args.push(current);
                    current = String::new();
                }
            }
            _ => {
                current.push(c);
            }
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

fn find_wasm_file(program: &str) -> Option<std::path::PathBuf> {
    let path = crate::config::resolve_path(program);

    // Check if the path exists and is a WASM file
    if path.exists() && path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("wasm")
    {
        return Some(path);
    }

    // Try appending .wasm if not already present
    if path.extension().and_then(|s| s.to_str()) != Some("wasm") {
        let mut wasm_path = path.clone();
        wasm_path.set_extension("wasm");
        if wasm_path.exists() && wasm_path.is_file() {
            return Some(wasm_path);
        }
    }

    // Check the global skills directory (~/.openz/skills/)
    let skills_dir = crate::agent::skills::get_skills_dir();
    if let Some(file_name) = std::path::Path::new(program).file_name() {
        let skill_path = skills_dir.join(file_name);
        if skill_path.exists()
            && skill_path.is_file()
            && skill_path.extension().and_then(|s| s.to_str()) == Some("wasm")
        {
            return Some(skill_path);
        }
        if skill_path.extension().and_then(|s| s.to_str()) != Some("wasm") {
            let mut skill_wasm_path = skill_path.clone();
            skill_wasm_path.set_extension("wasm");
            if skill_wasm_path.exists() && skill_wasm_path.is_file() {
                return Some(skill_wasm_path);
            }
        }

        // Check explicit workspace-local OpenZ skills (.openz/skills/).
        let workspace_skills_dir = crate::agent::skills::get_workspace_skills_dir();
        if workspace_skills_dir.exists() && workspace_skills_dir.is_dir() {
            let workspace_skill_path = workspace_skills_dir.join(file_name);
            if workspace_skill_path.exists()
                && workspace_skill_path.is_file()
                && workspace_skill_path.extension().and_then(|s| s.to_str()) == Some("wasm")
            {
                return Some(workspace_skill_path);
            }
            if workspace_skill_path.extension().and_then(|s| s.to_str()) != Some("wasm") {
                let mut workspace_skill_wasm_path = workspace_skill_path.clone();
                workspace_skill_wasm_path.set_extension("wasm");
                if workspace_skill_wasm_path.exists() && workspace_skill_wasm_path.is_file() {
                    return Some(workspace_skill_wasm_path);
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod exec_command_tests {
    use super::*;

    #[test]
    fn detects_desktop_launchers() {
        assert!(should_detach_command("xdg-open /tmp/demo.svg"));
        assert!(should_detach_command("gio open /tmp/demo.svg"));
        assert!(should_detach_command("vlc /tmp/video.mp4"));
        assert!(should_detach_command("firefox https://example.com"));
        assert!(should_detach_command("code ."));
    }

    #[test]
    fn detects_dev_servers_without_detaching_normal_commands() {
        assert!(should_detach_command("bun run dev --host 127.0.0.1"));
        assert!(should_detach_command("npm run preview"));
        assert!(should_detach_command("python3 -m http.server 5173"));
        assert!(should_detach_command("PORT=5173 npm run dev"));
        assert!(should_detach_command("npx vite --host 127.0.0.1"));
        assert!(!should_detach_command("cargo test"));
        assert!(!should_detach_command("python3 script.py"));
    }

    #[test]
    fn does_not_detach_shell_control_chains() {
        assert!(!should_detach_command(
            "firefox https://example.com && echo done"
        ));
        assert!(!should_detach_command(
            "xdg-open /tmp/demo.svg; rm -rf /tmp/nope"
        ));
        assert!(!should_detach_command("gio info /tmp/demo.svg"));
    }
}

pub struct PythonSandboxTool;

#[async_trait::async_trait]
impl Tool for PythonSandboxTool {
    fn name(&self) -> &str {
        "python_sandbox"
    }

    fn description(&self) -> &str {
        "Execute a Python script for data analysis, calculations, or chart drawing in a secure, sandboxed environment. Networking is disabled."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "code": {
                    "type": "string",
                    "description": "The complete Python 3 script code to execute."
                }
            },
            "required": ["code"]
        })
    }

    async fn call(&self, arguments: &serde_json::Value) -> Result<serde_json::Value> {
        let code = arguments
            .get("code")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'code' parameter"))?;

        let temp_dir = std::env::temp_dir();
        let file_name = format!("sandbox_{}.py", uuid::Uuid::new_v4());
        let temp_path = temp_dir.join(file_name);

        std::fs::write(&temp_path, code)?;

        let mut std_cmd = std::process::Command::new("python3");
        crate::config::loader::set_command_cwd(&mut std_cmd);
        std_cmd.arg(&temp_path);

        let enable_sandbox = crate::config::loader::load_config()
            .map(|c| c.agents.defaults.enable_sandbox)
            .unwrap_or(false);
        sandbox_command(&mut std_cmd, enable_sandbox);

        let mut tokio_cmd = tokio::process::Command::from(std_cmd);
        tokio_cmd.kill_on_drop(true);
        let output_res =
            tokio::time::timeout(std::time::Duration::from_secs(60), tokio_cmd.output()).await;
        let _ = std::fs::remove_file(&temp_path);

        let output = match output_res {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => {
                return Ok(serde_json::json!({
                    "status": "error",
                    "stdout": "",
                    "stderr": "Python execution timed out after 60 seconds",
                    "exit_code": -1
                }));
            }
        };
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        Ok(serde_json::json!({
            "status": if exit_code == 0 { "success" } else { "error" },
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": exit_code
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_command_line() {
        let cmd = "my_program arg1 \"arg 2\" 'arg 3'";
        let args = parse_command_line(cmd);
        assert_eq!(args.len(), 4);
        assert_eq!(args[0], "my_program");
        assert_eq!(args[1], "arg1");
        assert_eq!(args[2], "arg 2");
        assert_eq!(args[3], "arg 3");
    }

    #[test]
    fn test_find_wasm_file_nonexistent() {
        let path = find_wasm_file("nonexistent_wasm_file_12345");
        assert!(path.is_none());
    }

    #[test]
    fn test_find_wasm_file_uses_workspace_openz_skills_not_repo_skills() {
        let original = std::env::current_dir().unwrap();
        let temp_dir =
            std::env::temp_dir().join(format!("openz_wasm_skills_{}", uuid::Uuid::new_v4()));
        let repo_skills = temp_dir.join("skills");
        let workspace_skills = temp_dir.join(".openz").join("skills");
        std::fs::create_dir_all(&repo_skills).unwrap();
        std::fs::create_dir_all(&workspace_skills).unwrap();
        std::fs::write(repo_skills.join("tool.wasm"), b"repo").unwrap();
        std::fs::write(workspace_skills.join("tool.wasm"), b"workspace").unwrap();
        std::env::set_current_dir(&temp_dir).unwrap();

        let found = find_wasm_file("tool");

        std::env::set_current_dir(original).unwrap();
        let _ = std::fs::remove_dir_all(&temp_dir);

        assert!(
            found
                .as_ref()
                .map(|path| path.ends_with(".openz/skills/tool.wasm"))
                .unwrap_or(false),
            "expected workspace .openz/skills WASM, got {found:?}"
        );
    }

    #[tokio::test]
    async fn test_exec_command_fallback() {
        let tool = ExecCommandTool;
        let args = serde_json::json!({
            "command": "echo 'hello openz'"
        });
        let res = tool.call(&args).await.unwrap();
        assert!(res.get("status_code").is_some());
        let stdout = res["stdout"].as_str().unwrap();
        assert!(stdout.contains("hello openz"));
    }

    #[tokio::test]
    async fn test_exec_command_wasm() {
        let temp_dir = std::env::temp_dir();
        let wasm_path = temp_dir.join("test_exec_command_wasm_temp_file_12345.wasm");

        let wasm_bytes: &[u8] = &[
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x04, 0x01, 0x60, 0x00, 0x00,
            0x03, 0x02, 0x01, 0x00, 0x07, 0x0a, 0x01, 0x06, 0x5f, 0x73, 0x74, 0x61, 0x72, 0x74,
            0x00, 0x00, 0x0a, 0x04, 0x01, 0x02, 0x00, 0x0b,
        ];

        std::fs::write(&wasm_path, wasm_bytes).unwrap();

        let tool = ExecCommandTool;
        let args = serde_json::json!({
            "command": format!("{} arg1 arg2", wasm_path.to_string_lossy())
        });

        let res = tool.call(&args).await.unwrap();

        // Clean up
        let _ = std::fs::remove_file(wasm_path);

        assert_eq!(res["status_code"].as_i64().unwrap(), 0);
        assert!(res.get("stdout").is_some());
        assert!(res.get("stderr").is_some());
    }
}
