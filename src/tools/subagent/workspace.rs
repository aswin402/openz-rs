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
    match parent_dir == worktree_dir {
        true => None,
        false => {
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
    }
}

fn unregister_worktree_cleanup(id: Option<u64>) {
    match id {
        Some(id) => {
            let mut guard = worktree_cleanup_registry()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.retain(|cleanup| cleanup.id != id);
        }
        None => {}
    }
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
        match self.active && self.worktree_dir != self.parent_dir {
            true => cleanup_isolated_workspace(&self.parent_dir, &self.worktree_dir),
            false => {}
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

    match (metadata.is_file(), metadata.is_dir()) {
        (true, _) => metadata.len(),
        (false, false) => 0,
        (false, true) => match std::fs::read_dir(path) {
            Ok(entries) => entries
                .flatten()
                .map(|entry| directory_size_bytes(&entry.path()))
                .sum(),
            Err(_) => 0,
        },
    }
}

fn collect_worktree_candidates(worktrees_dir: &Path) -> Vec<WorktreeCandidate> {
    let mut candidates = Vec::new();
    match std::fs::read_dir(worktrees_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                match is_openz_worktree_dir(&path) {
                    false => continue,
                    true => {}
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
        Err(_) => {}
    }
    candidates.sort_by(|a, b| a.modified.cmp(&b.modified));
    candidates
}

use crate::core::process::available_bytes;

fn safely_remove_worktree_dir(path: &Path) {
    match path.exists() {
        false => return,
        true => {}
    }
    // Attempt git worktree remove --force first to unregister cleanly from git index
    let _ = std::process::Command::new("git")
        .args(["worktree", "remove", "--force", &path.to_string_lossy()])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    match path.exists() {
        true => {
            let _ = std::fs::remove_dir_all(path);
        }
        false => {}
    }
}

pub fn cleanup_worktrees_dir(worktrees_dir: &Path, policy: WorktreeCleanupPolicy) {
    match worktrees_dir.exists() && worktrees_dir.is_dir() {
        false => return,
        true => {}
    }

    let now = std::time::SystemTime::now();
    let candidates = collect_worktree_candidates(worktrees_dir);

    for candidate in &candidates {
        let is_expired = now
            .duration_since(candidate.modified)
            .map(|age| age > policy.max_age)
            .unwrap_or(false);
        match is_expired {
            true => {
                tracing::warn!(
                    path = %candidate.path.display(),
                    "Removing expired OpenZ subagent worktree"
                );
                safely_remove_worktree_dir(&candidate.path);
            }
            false => {}
        }
    }

    let mut candidates = collect_worktree_candidates(worktrees_dir);
    while candidates.len() > policy.max_count {
        match candidates.first() {
            Some(candidate) => {
                tracing::warn!(
                    path = %candidate.path.display(),
                    "Removing oldest OpenZ subagent worktree to satisfy count quota"
                );
                safely_remove_worktree_dir(&candidate.path);
            }
            None => {}
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
        match (policy.max_total_bytes == 0 || total_bytes <= policy.max_total_bytes) && free_ok {
            true => break,
            false => {}
        }
        match candidates.first() {
            Some(candidate) => {
                tracing::warn!(
                    path = %candidate.path.display(),
                    total_bytes,
                    "Removing oldest OpenZ subagent worktree to satisfy disk quota"
                );
                safely_remove_worktree_dir(&candidate.path);
            }
            None => break,
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
        match rc == 0 {
            true => Ok(()),
            false => Err(std::io::Error::last_os_error().into()),
        }
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
    match (scratch_workspace, run_success) {
        (true, true) => "Scratch workspace completed; sync-back skipped.".to_string(),
        (true, false) => "Scratch workspace failed; sync-back skipped.".to_string(),
        (false, true) => format!("Committed simulation space branch '{branch_id}'"),
        (false, false) => format!("Rolled back simulation space branch '{branch_id}'"),
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

fn prune_stale_files_in_dir(dir: &Path, max_age_seconds: u64) {
    match dir.exists() && dir.is_dir() {
        false => return,
        true => {}
    }
    match std::fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                match path.is_file() && is_older_than(&path, max_age_seconds) {
                    true => {
                        let _ = std::fs::remove_file(&path);
                    }
                    false => {}
                }
            }
        }
        Err(_) => {}
    }
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
        let is_git_worktree = git_check
            .map(|out| out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "true")
            .unwrap_or(false);
        match is_git_worktree {
            true => {
                let _ = std::process::Command::new("git")
                    .args(["worktree", "prune"])
                    .current_dir(dir)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
            }
            false => {}
        }
    }

    let ttl_seconds = WorktreeCleanupPolicy::default().max_age.as_secs();

    // 2. Clean dedicated directory (~/.openz/worktrees)
    let worktrees_dir = openz_worktrees_dir();
    cleanup_worktrees_dir(&worktrees_dir, WorktreeCleanupPolicy::default());

    // 3. Clean legacy /tmp/openz_worktree_* directories
    let tmp_dir = std::env::temp_dir();
    match std::fs::read_dir(&tmp_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                let should_remove = path.is_dir()
                    && path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .starts_with("openz_worktree_")
                    && is_older_than(&path, ttl_seconds);
                match should_remove {
                    true => safely_remove_worktree_dir(&path),
                    false => {}
                }
            }
        }
        Err(_) => {}
    }

    let seven_days_in_seconds = 7 * 24 * 3600;
    let runtime_dir = crate::config::loader::runtime_data_dir();
    for subdir in ["tool_outputs", "traces", "cron_logs"] {
        prune_stale_files_in_dir(&runtime_dir.join(subdir), seven_days_in_seconds);
    }
}

fn is_older_than(path: &Path, seconds: u64) -> bool {
    std::fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.elapsed().ok())
        .map(|elapsed| elapsed.as_secs() > seconds)
        .unwrap_or(false)
}

fn sync_git_porcelain_changes(src_dir: &Path, dst_dir: &Path, stdout: &str) {
    for line in stdout.lines().filter(|line| line.len() >= 4) {
        let status_code = &line[..2];
        let file_path_str = &line[3..];
        let file_path = match status_code.starts_with('R') {
            true => file_path_str
                .find(" -> ")
                .map(|pos| &file_path_str[pos + 4..])
                .unwrap_or(file_path_str),
            false => file_path_str,
        };

        let src = src_dir.join(file_path);
        let dst = dst_dir.join(file_path);

        match (status_code.contains('D'), src.exists()) {
            (true, _) => {
                let _ = std::fs::remove_file(&dst);
            }
            (false, true) => {
                match dst.parent() {
                    Some(parent) => {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    None => {}
                }
                let _ = std::fs::copy(&src, &dst);
            }
            (false, false) => {}
        }
    }
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

    match !worktrees_dir.exists() {
        true => {
            let _ = std::fs::create_dir_all(&worktrees_dir);
        }
        false => {}
    }
    let temp_dir = worktrees_dir.join(format!(
        "openz_worktree_{}",
        crate::core::short_uuid()
    ));

    match !is_git && is_dangerous_fallback_copy_root(parent_dir) {
        true => return create_scratch_workspace(&temp_dir, parent_dir),
        false => {}
    }

    match is_git {
        true => {
            // 2. Create git worktree
            let worktree_add = std::process::Command::new("git")
                .args(["worktree", "add", "--detach"])
                .arg(&temp_dir)
                .current_dir(parent_dir)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();

            match worktree_add.map(|s| s.success()).unwrap_or(false) {
                true => {
                    // 3. Sync uncommitted changes (modified, added, deleted, untracked files)
                    match std::process::Command::new("git")
                        .args(["status", "--porcelain"])
                        .current_dir(parent_dir)
                        .output()
                    {
                        Ok(status_out) => {
                            let stdout = String::from_utf8_lossy(&status_out.stdout);
                            sync_git_porcelain_changes(parent_dir, &temp_dir, &stdout);
                        }
                        Err(_) => {}
                    }
                    return Ok(temp_dir);
                }
                false => {}
            }
        }
        false => {}
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
    let is_root = canonical.parent().is_none();
    let is_home = dirs::home_dir()
        .map(|home| canonical == crate::config::path_policy::canonicalize_with_missing_leaf(&home))
        .unwrap_or(false);
    let runtime_dir = crate::config::path_policy::canonicalize_with_missing_leaf(
        &crate::config::loader::runtime_data_dir(),
    );
    is_root
        || is_home
        || canonical == runtime_dir
        || canonical.starts_with(runtime_dir.join("worktrees"))
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
    match src.exists() {
        false => return Ok(()),
        true => {}
    }

    match src.is_dir() {
        true => {
            let name = src.file_name().and_then(|n| n.to_str()).unwrap_or("");
            match should_skip_workspace_copy_dir(name) {
                true => return Ok(()),
                false => {}
            }

            std::fs::create_dir_all(dst)?;
            for entry in std::fs::read_dir(src)? {
                let entry = entry?;
                let entry_path = entry.path();
                match entry_path.file_name() {
                    Some(entry_name) => {
                        copy_dir_recursive_filtered(&entry_path, &dst.join(entry_name))?;
                    }
                    None => {}
                }
            }
        }
        false => {
            let is_regular_file = src
                .symlink_metadata()
                .map(|m| m.file_type().is_file())
                .unwrap_or(false);
            match is_regular_file {
                true => {
                    std::fs::copy(src, dst)?;
                }
                false => {}
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

    match is_worktree {
        true => {
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
        }
        false => {
            let _ = std::fs::remove_dir_all(worktree_dir);
        }
    }
}

pub fn sync_changes_back(src_dir: &Path, dst_dir: &Path) -> Result<()> {
    let git_check = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(src_dir)
        .output();

    match git_check {
        Ok(status_out) => {
            let stdout = String::from_utf8_lossy(&status_out.stdout);
            sync_git_porcelain_changes(src_dir, dst_dir, &stdout);
        }
        Err(_) => {
            copy_dir_recursive_filtered(src_dir, dst_dir)?;
        }
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

    let clean_json = super::schema_retry::strip_json_fences(&content);

    #[derive(serde::Deserialize)]
    struct ReviewRes {
        success: bool,
        skill_name: String,
        skill_content: String,
    }

    match serde_json::from_str::<ReviewRes>(clean_json) {
        Ok(review) => match review.success {
            true => {
                let s_name = review.skill_name.trim().to_lowercase().replace(' ', "_");
                let s_content = review.skill_content.trim();
                match !s_name.is_empty() && !s_content.is_empty() {
                    true => {
                        crate::agent::skills::save_subagent_skill(profile_name, &s_name, s_content)?;
                        crate::tui_println!(
                            "{}✓ [Evolution] Extracted and saved skill '{}' for subagent '{}'{}",
                            EMERALD_GREEN,
                            s_name,
                            profile_name,
                            COLOR_RESET
                        );
                    }
                    false => {}
                }
            }
            false => {
                crate::tui_println!(
                    "{}▲ [Evolution] Subagent task evaluation: Unsuccessful. No skill files updated.{}",
                    AURA_GOLD,
                    COLOR_RESET
                );
            }
        },
        Err(_) => {}
    }

    Ok(())
}

#[cfg(test)]
#[path = "workspace_tests.rs"]
mod tests;

