use crate::agent::style::spinner::with_spinner;
use crate::agent::style::*;
use crate::providers::LLMProvider;
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex, OnceLock,
};

pub struct WorktreeGuard {
    pub parent_dir: PathBuf,
    pub worktree_dir: PathBuf,
    pub active: bool,
    cleanup_id: Option<u64>,
}

#[derive(Debug, Clone)]
struct RegisteredWorktreeCleanup {
    id: u64,
    parent_dir: PathBuf,
    worktree_dir: PathBuf,
}

static ACTIVE_WORKTREE_CLEANUPS: OnceLock<Mutex<Vec<RegisteredWorktreeCleanup>>> = OnceLock::new();
static NEXT_WORKTREE_CLEANUP_ID: AtomicU64 = AtomicU64::new(1);

fn worktree_cleanup_registry() -> &'static Mutex<Vec<RegisteredWorktreeCleanup>> {
    ACTIVE_WORKTREE_CLEANUPS.get_or_init(|| Mutex::new(Vec::new()))
}

fn register_worktree_cleanup(
    parent_dir: PathBuf,
    worktree_dir: PathBuf,
) -> Option<u64> {
    if parent_dir == worktree_dir {
        return None;
    }
    let id = NEXT_WORKTREE_CLEANUP_ID.fetch_add(1, Ordering::SeqCst);
    let cleanup = RegisteredWorktreeCleanup {
        id,
        parent_dir,
        worktree_dir,
    };
    let mut guard = worktree_cleanup_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.push(cleanup);
    Some(id)
}

fn unregister_worktree_cleanup(id: Option<u64>) {
    let Some(id) = id else {
        return;
    };
    let mut guard = worktree_cleanup_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.retain(|cleanup| cleanup.id != id);
}

#[cfg(test)]
pub fn has_registered_worktree_cleanup_for_test(worktree_dir: &Path) -> bool {
    worktree_cleanup_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .any(|cleanup| cleanup.worktree_dir == worktree_dir)
}

pub fn cleanup_registered_worktrees() {
    let cleanups = {
        let mut guard = worktree_cleanup_registry()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        std::mem::take(&mut *guard)
    };

    for cleanup in cleanups {
        cleanup_isolated_workspace(&cleanup.parent_dir, &cleanup.worktree_dir);
    }
}

impl WorktreeGuard {
    pub fn new(parent_dir: PathBuf, worktree_dir: PathBuf) -> Self {
        let cleanup_id = register_worktree_cleanup(parent_dir.clone(), worktree_dir.clone());
        Self {
            parent_dir,
            worktree_dir,
            active: true,
            cleanup_id,
        }
    }

    pub fn deactivate(&mut self) {
        unregister_worktree_cleanup(self.cleanup_id.take());
        self.active = false;
    }
}

impl Drop for WorktreeGuard {
    fn drop(&mut self) {
        unregister_worktree_cleanup(self.cleanup_id.take());
        if self.active && self.worktree_dir != self.parent_dir {
            cleanup_isolated_workspace(&self.parent_dir, &self.worktree_dir);
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct WorktreeCleanupPolicy {
    pub max_age: std::time::Duration,
    pub max_count: usize,
    pub max_total_bytes: u64,
    pub min_free_bytes: u64,
}

impl Default for WorktreeCleanupPolicy {
    fn default() -> Self {
        Self {
            max_age: std::time::Duration::from_secs(30 * 60),
            max_count: 2,
            max_total_bytes: 2 * 1024 * 1024 * 1024,
            min_free_bytes: 5 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Debug)]
struct WorktreeCandidate {
    path: PathBuf,
    modified: std::time::SystemTime,
    size_bytes: u64,
}

fn is_openz_worktree_dir(path: &Path) -> bool {
    path.is_dir()
        && path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|name| name.starts_with("openz_worktree_"))
}

pub fn directory_size_bytes(path: &Path) -> u64 {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return 0,
    };

    if metadata.is_file() {
        return metadata.len();
    }
    if !metadata.is_dir() {
        return 0;
    }

    let mut total = 0;
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => return 0,
    };
    for entry in entries.flatten() {
        total += directory_size_bytes(&entry.path());
    }
    total
}

fn collect_worktree_candidates(worktrees_dir: &Path) -> Vec<WorktreeCandidate> {
    let mut candidates = Vec::new();
    if let Ok(entries) = std::fs::read_dir(worktrees_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !is_openz_worktree_dir(&path) {
                continue;
            }
            let metadata = match std::fs::metadata(&path) {
                Ok(metadata) => metadata,
                Err(_) => continue,
            };
            let modified = metadata
                .modified()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            candidates.push(WorktreeCandidate {
                size_bytes: directory_size_bytes(&path),
                path,
                modified,
            });
        }
    }
    candidates.sort_by(|a, b| a.modified.cmp(&b.modified));
    candidates
}

#[cfg(unix)]
fn available_bytes(path: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };
    if rc != 0 {
        return None;
    }
    let stat = unsafe { stat.assume_init() };
    Some(stat.f_bavail.saturating_mul(stat.f_frsize))
}

#[cfg(not(unix))]
fn available_bytes(_path: &Path) -> Option<u64> {
    None
}

fn safely_remove_worktree_dir(path: &Path) {
    if !path.exists() {
        return;
    }
    // Attempt git worktree remove --force first to unregister cleanly from git index
    let _ = std::process::Command::new("git")
        .args(["worktree", "remove", "--force", &path.to_string_lossy()])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    if path.exists() {
        let _ = std::fs::remove_dir_all(path);
    }
}

pub fn cleanup_worktrees_dir(worktrees_dir: &Path, policy: WorktreeCleanupPolicy) {
    if !worktrees_dir.exists() || !worktrees_dir.is_dir() {
        return;
    }

    let now = std::time::SystemTime::now();
    let candidates = collect_worktree_candidates(worktrees_dir);

    for candidate in &candidates {
        let is_expired = now
            .duration_since(candidate.modified)
            .map(|age| age > policy.max_age)
            .unwrap_or(false);
        if is_expired {
            tracing::warn!(
                path = %candidate.path.display(),
                "Removing expired OpenZ subagent worktree"
            );
            safely_remove_worktree_dir(&candidate.path);
        }
    }

    let mut candidates = collect_worktree_candidates(worktrees_dir);
    while candidates.len() > policy.max_count {
        if let Some(candidate) = candidates.first() {
            tracing::warn!(
                path = %candidate.path.display(),
                "Removing oldest OpenZ subagent worktree to satisfy count quota"
            );
            safely_remove_worktree_dir(&candidate.path);
        }
        candidates = collect_worktree_candidates(worktrees_dir);
    }

    loop {
        let total_bytes: u64 = candidates
            .iter()
            .map(|candidate| candidate.size_bytes)
            .sum();
        let free_ok = available_bytes(worktrees_dir)
            .map(|free| free >= policy.min_free_bytes)
            .unwrap_or(true);
        if (policy.max_total_bytes == 0 || total_bytes <= policy.max_total_bytes) && free_ok {
            break;
        }
        if let Some(candidate) = candidates.first() {
            tracing::warn!(
                path = %candidate.path.display(),
                total_bytes,
                "Removing oldest OpenZ subagent worktree to satisfy disk quota"
            );
            safely_remove_worktree_dir(&candidate.path);
        } else {
            break;
        }
        candidates = collect_worktree_candidates(worktrees_dir);
    }
}

#[cfg(test)]
pub fn set_directory_modified_time_for_test(
    path: &Path,
    modified: std::time::SystemTime,
) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let duration = modified
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        let times = [
            libc::timespec {
                tv_sec: duration.as_secs() as libc::time_t,
                tv_nsec: duration.subsec_nanos() as libc::c_long,
            },
            libc::timespec {
                tv_sec: duration.as_secs() as libc::time_t,
                tv_nsec: duration.subsec_nanos() as libc::c_long,
            },
        ];
        let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())?;
        let rc = unsafe { libc::utimensat(libc::AT_FDCWD, c_path.as_ptr(), times.as_ptr(), 0) };
        if rc != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = (path, modified);
        Ok(())
    }
}

const SCRATCH_WORKSPACE_MARKER: &str = ".openz_scratch_workspace";

fn openz_worktrees_dir() -> PathBuf {
    crate::config::loader::runtime_data_dir().join("worktrees")
}

pub(crate) fn simulation_space_teardown_message(
    run_success: bool,
    branch_id: &str,
    scratch_workspace: bool,
) -> String {
    if scratch_workspace {
        if run_success {
            "Scratch workspace completed; sync-back skipped.".to_string()
        } else {
            "Scratch workspace failed; sync-back skipped.".to_string()
        }
    } else if run_success {
        format!("Committed simulation space branch '{branch_id}'")
    } else {
        format!("Rolled back simulation space branch '{branch_id}'")
    }
}

pub fn current_workspace_root() -> PathBuf {
    crate::config::loader::ACTIVE_WORKSPACE
        .try_with(|workspace| workspace.clone())
        .unwrap_or_else(|_| {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        })
}

fn enforce_disk_quota() {
    let worktrees_dir = openz_worktrees_dir();
    cleanup_worktrees_dir(&worktrees_dir, WorktreeCleanupPolicy::default());
}

pub fn cleanup_stale_resources() {
    // 1. Run git worktree prune in both workspace root and current directory if it's a git repo
    let target_dirs = [
        current_workspace_root(),
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    ];
    for dir in &target_dirs {
        let git_check = std::process::Command::new("git")
            .args(["rev-parse", "--is-inside-work-tree"])
            .current_dir(dir)
            .output();
        if let Ok(out) = git_check {
            if out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "true" {
                let _ = std::process::Command::new("git")
                    .args(["worktree", "prune"])
                    .current_dir(dir)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
            }
        }
    }

    let ttl_seconds = WorktreeCleanupPolicy::default().max_age.as_secs();

    // 2. Clean dedicated directory (~/.openz/worktrees)
    let worktrees_dir = openz_worktrees_dir();
    cleanup_worktrees_dir(&worktrees_dir, WorktreeCleanupPolicy::default());

    // 3. Clean legacy /tmp/openz_worktree_* directories
    let tmp_dir = std::env::temp_dir();
    if let Ok(entries) = std::fs::read_dir(&tmp_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with("openz_worktree_") && is_older_than(&path, ttl_seconds) {
                    safely_remove_worktree_dir(&path);
                }
            }
        }
    }

    let seven_days_in_seconds = 7 * 24 * 3600;

    // 4. Clean tool_outputs (~/.openz/tool_outputs)
    let tool_outputs_dir = crate::config::loader::runtime_data_dir().join("tool_outputs");
    if tool_outputs_dir.exists() && tool_outputs_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&tool_outputs_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && is_older_than(&path, seven_days_in_seconds) {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }

    // 5. Clean traces (~/.openz/traces)
    let traces_dir = crate::config::loader::runtime_data_dir().join("traces");
    if traces_dir.exists() && traces_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&traces_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && is_older_than(&path, seven_days_in_seconds) {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }

    // 6. Clean cron_logs (~/.openz/cron_logs)
    let cron_logs_dir = crate::config::loader::runtime_data_dir().join("cron_logs");
    if cron_logs_dir.exists() && cron_logs_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&cron_logs_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && is_older_than(&path, seven_days_in_seconds) {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }
}

fn is_older_than(path: &Path, seconds: u64) -> bool {
    if let Ok(metadata) = std::fs::metadata(path) {
        if let Ok(modified) = metadata.modified() {
            if let Ok(elapsed) = modified.elapsed() {
                return elapsed.as_secs() > seconds;
            }
        }
    }
    false
}

pub fn create_isolated_workspace(parent_dir: &Path) -> Result<PathBuf> {
    enforce_disk_quota();

    let worktrees_dir = openz_worktrees_dir();
    // 1. Check if parent_dir is a git repository
    let git_check = std::process::Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(parent_dir)
        .output();

    let is_git = match git_check {
        Ok(out) => out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "true",
        Err(_) => false,
    };

    if !worktrees_dir.exists() {
        let _ = std::fs::create_dir_all(&worktrees_dir);
    }
    let temp_dir = worktrees_dir.join(format!(
        "openz_worktree_{}",
        &uuid::Uuid::new_v4().to_string()[..8]
    ));

    if !is_git && is_dangerous_fallback_copy_root(parent_dir) {
        return create_scratch_workspace(&temp_dir, parent_dir);
    }

    if is_git {
        // 2. Create git worktree
        let worktree_add = std::process::Command::new("git")
            .args(["worktree", "add", "--detach"])
            .arg(&temp_dir)
            .current_dir(parent_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        match worktree_add {
            Ok(status) if status.success() => {
                // 3. Sync uncommitted changes (modified, added, deleted, untracked files)
                if let Ok(status_out) = std::process::Command::new("git")
                    .args(["status", "--porcelain"])
                    .current_dir(parent_dir)
                    .output()
                {
                    let stdout = String::from_utf8_lossy(&status_out.stdout);
                    for line in stdout.lines() {
                        if line.len() < 4 {
                            continue;
                        }
                        let status_code = &line[..2];
                        let file_path_str = &line[3..];

                        let file_path = if status_code.starts_with('R') {
                            if let Some(pos) = file_path_str.find(" -> ") {
                                &file_path_str[pos + 4..]
                            } else {
                                file_path_str
                            }
                        } else {
                            file_path_str
                        };

                        let src = parent_dir.join(file_path);
                        let dst = temp_dir.join(file_path);

                        if status_code.contains('D') {
                            let _ = std::fs::remove_file(&dst);
                        } else {
                            if src.exists() {
                                if let Some(parent) = dst.parent() {
                                    let _ = std::fs::create_dir_all(parent);
                                }
                                let _ = std::fs::copy(&src, &dst);
                            }
                        }
                    }
                }
                return Ok(temp_dir);
            }
            _ => {
                // If git worktree add fails, fallback to recursive copy
            }
        }
    }

    // Fallback: Copy workspace files recursively (skipping heavy dirs)
    std::fs::create_dir_all(&temp_dir)?;
    copy_dir_recursive_filtered(parent_dir, &temp_dir)?;
    Ok(temp_dir)
}

fn should_skip_workspace_copy_dir(name: &str) -> bool {
    matches!(
        name,
        "target"
            | "node_modules"
            | ".git"
            | ".fastembed_cache"
            | ".sediment"
            | "logs"
            | ".openz"
            | ".cache"
            | ".local"
            | ".cargo"
            | ".rustup"
            | ".npm"
            | ".pnpm-store"
            | ".yarn"
            | ".bun"
            | ".gradle"
            | ".m2"
            | ".venv"
            | "venv"
            | "__pycache__"
            | "Downloads"
            | "snap"
            | "tmp"
            | "temp"
    )
}

fn is_dangerous_fallback_copy_root(path: &Path) -> bool {
    let canonical = crate::config::path_policy::canonicalize_with_missing_leaf(path);
    if canonical.parent().is_none() {
        return true;
    }

    if dirs::home_dir()
        .map(|home| {
            canonical == crate::config::path_policy::canonicalize_with_missing_leaf(&home)
        })
        .unwrap_or(false)
    {
        return true;
    }

    let runtime_dir = crate::config::path_policy::canonicalize_with_missing_leaf(
        &crate::config::loader::runtime_data_dir(),
    );
    canonical == runtime_dir || canonical.starts_with(runtime_dir.join("worktrees"))
}

fn create_scratch_workspace(
    worktree_dir: &Path,
    unsafe_parent_dir: &Path,
) -> Result<PathBuf> {
    std::fs::create_dir_all(worktree_dir)?;
    std::fs::write(
        worktree_dir.join(SCRATCH_WORKSPACE_MARKER),
        format!(
            "Scratch workspace created because active workspace '{}' is unsafe to copy.\n",
            unsafe_parent_dir.display()
        ),
    )?;
    Ok(worktree_dir.to_path_buf())
}

pub fn is_scratch_workspace(workspace_dir: &Path) -> bool {
    workspace_dir.join(SCRATCH_WORKSPACE_MARKER).is_file()
}

pub fn should_sync_changes_back(
    parent_dir: &Path,
    workspace_dir: &Path,
) -> bool {
    workspace_dir != parent_dir && !is_scratch_workspace(workspace_dir)
}

pub fn copy_dir_recursive_filtered(src: &Path, dst: &Path) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }

    if src.is_dir() {
        let name = src.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if should_skip_workspace_copy_dir(name) {
            return Ok(());
        }

        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let entry_path = entry.path();
            let Some(entry_name) = entry_path.file_name() else {
                continue;
            };
            copy_dir_recursive_filtered(&entry_path, &dst.join(entry_name))?;
        }
    } else {
        if let Ok(metadata) = src.symlink_metadata() {
            if metadata.file_type().is_file() {
                std::fs::copy(src, dst)?;
            }
        }
    }
    Ok(())
}

pub fn cleanup_isolated_workspace(parent_dir: &Path, worktree_dir: &Path) {
    let git_check = std::process::Command::new("git")
        .args(["worktree", "list"])
        .current_dir(parent_dir)
        .output();

    let is_worktree = match git_check {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let worktree_display = worktree_dir.to_string_lossy();
            stdout.contains(worktree_display.as_ref())
        }
        Err(_) => false,
    };

    if is_worktree {
        let _ = std::process::Command::new("git")
            .args(["worktree", "remove", "--force"])
            .arg(worktree_dir)
            .current_dir(parent_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        let _ = std::process::Command::new("git")
            .args(["worktree", "prune"])
            .current_dir(parent_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    } else {
        let _ = std::fs::remove_dir_all(worktree_dir);
    }
}

pub fn sync_changes_back(src_dir: &Path, dst_dir: &Path) -> Result<()> {
    let git_check = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(src_dir)
        .output();

    if let Ok(status_out) = git_check {
        let stdout = String::from_utf8_lossy(&status_out.stdout);
        for line in stdout.lines() {
            if line.len() < 4 {
                continue;
            }
            let status_code = &line[..2];
            let file_path_str = &line[3..];

            let file_path = if status_code.starts_with('R') {
                if let Some(pos) = file_path_str.find(" -> ") {
                    &file_path_str[pos + 4..]
                } else {
                    file_path_str
                }
            } else {
                file_path_str
            };

            let src = src_dir.join(file_path);
            let dst = dst_dir.join(file_path);

            if status_code.contains('D') {
                let _ = std::fs::remove_file(&dst);
            } else {
                if src.exists() {
                    if let Some(parent) = dst.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let _ = std::fs::copy(&src, &dst);
                }
            }
        }
    } else {
        copy_dir_recursive_filtered(src_dir, dst_dir)?;
    }
    Ok(())
}

pub async fn run_evolution_review(
    provider: &std::sync::Arc<dyn LLMProvider>,
    profile_name: &str,
    goal: &str,
    context: &str,
    summary: &str,
) -> Result<()> {
    let system_prompt = "You are a specialized Subagent Reviewer. Your task is to evaluate if a subagent successfully completed its task, and if so, extract any procedural skills or guidelines discovered during execution.\n\n\
        Review the subagent's goal, the context, and the summary of what it did and found.\n\n\
        Perform two tasks:\n\
        1. SUCCESS EVALUATION: Decide if the subagent succeeded in accomplishing the goal (true or false).\n\
        2. SKILL EXTRACTION: If the subagent succeeded, extract any reusable procedural guidelines, rules, tool usage lessons, or coding patterns it discovered. Avoid general descriptions; make them actionable instructions for future runs. Format the extracted guidelines in Markdown with a clear title (# Skill: ...), a description of when to use it, specific guidelines, and examples.\n\n\
        Provide your response as a raw JSON object with the following structure:\n\n\
        JSON Format:\n\
        {\n\
          \"success\": true,\n\
          \"skill_name\": \"cargo_check_workaround\",\n\
          \"skill_content\": \"# Skill: Cargo Check Workaround\\n\\nWhen cargo check fails with X, do Y...\"\n\
        }\n\n\
        Do not output any introductory or conversational text, only the raw JSON.";

    let user_prompt = format!(
        "Subagent Profile: {}\n\
         Goal: {}\n\
         Context: {}\n\
         Subagent Summary of Work:\n{}\n\n\
         Please review the above execution, evaluate success, and extract any reusable skills.",
        profile_name, goal, context, summary
    );

    let messages = vec![crate::session::Message {
        role: "user".to_string(),
        content: user_prompt,
        timestamp: Some(chrono::Utc::now().to_rfc3339()),
        extra: serde_json::Map::new(),
    }];

    let settings = crate::providers::GenerationSettings {
        temperature: 0.1,
        max_tokens: 1536,
        reasoning_effort: None,
    };

    let spinner_msg = format!(
        "{}◇ [Evolution] Evaluating subagent success & extracting skills...{}",
        AURA_PURPLE, COLOR_RESET
    );
    let resp = with_spinner(
        &spinner_msg,
        provider.chat(system_prompt, &messages, &[], &settings),
    )
    .await?;
    let content = resp
        .content
        .ok_or_else(|| anyhow!("No content returned from AI"))?;

    // Parse JSON
    let mut clean_json = content.trim();
    if let Some(stripped) = clean_json.strip_prefix("```json") {
        clean_json = stripped;
    } else if let Some(stripped) = clean_json.strip_prefix("```") {
        clean_json = stripped;
    }
    if clean_json.ends_with("```") {
        clean_json = clean_suffix_ticks(clean_json);
    }
    let clean_json = clean_json.trim();

    #[derive(serde::Deserialize)]
    struct ReviewRes {
        success: bool,
        skill_name: String,
        skill_content: String,
    }

    if let Ok(review) = serde_json::from_str::<ReviewRes>(clean_json) {
        if review.success {
            let s_name = review.skill_name.trim().to_lowercase().replace(' ', "_");
            let s_content = review.skill_content.trim();
            if !s_name.is_empty() && !s_content.is_empty() {
                crate::agent::skills::save_subagent_skill(profile_name, &s_name, s_content)?;
                crate::tui_println!(
                    "{}✓ [Evolution] Extracted and saved skill '{}' for subagent '{}'{}",
                    EMERALD_GREEN,
                    s_name,
                    profile_name,
                    COLOR_RESET
                );
            }
        } else {
            crate::tui_println!(
                "{}▲ [Evolution] Subagent task evaluation: Unsuccessful. No skill files updated.{}",
                AURA_GOLD,
                COLOR_RESET
            );
        }
    }

    Ok(())
}

fn clean_suffix_ticks(s: &str) -> &str {
    if let Some(stripped) = s.strip_suffix("```") {
        stripped
    } else {
        s
    }
}
