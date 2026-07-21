use crate::agent::style::colors::{
    AURA_GOLD, AURA_GREEN, AURA_SLATE, COLOR_RESET, EMERALD_GREEN, AURA_PURPLE,
};
use crate::agent::style::spinner::with_spinner;
use crate::config::resolve_path;
use crate::providers::LLMProvider;
use crate::tools::subagent::cancellation_token::CancellationToken;
use crate::tools::subagent::lifecycle::{cancellation_result_json, SubagentRunStatus, status_json};
use crate::tools::subagent::scan_for_images;
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex, OnceLock,
};

pub struct SubagentRunContext {
    pub parent_dir: PathBuf,
    pub workspace_dir: PathBuf,
    pub workspace_isolation: String,
    pub workspace_isolation_reason: Option<String>,
    pub branch_id: String,
    pub has_branch: bool,
    pub clean_goal: String,
    pub clean_context: String,
    pub subagent_prompt: String,
    pub cancellation_token: CancellationToken,
}

impl SubagentRunContext {
    pub async fn prepare(
        goal: &str,
        context: &str,
        system_prompt: Option<&str>,
        needs_workspace: bool,
        cancellation_token: CancellationToken,
    ) -> Result<Self> {
        let clean_goal = ensure_markdown_images(goal);
        let clean_context = ensure_markdown_images(context);

        let mut subagent_prompt = if let Some(sys) = system_prompt {
            format!(
                "You are a specialized subagent operating under the following profile guidelines:\n\n\
                {}\n\n\
                TASK:\n{}\n\n\
                CONTEXT:\n{}\n\n\
                When finished, provide a clear, concise summary of what you did and found.",
                sys, clean_goal, clean_context
            )
        } else {
            format!(
                "You are a focused subagent. Complete the following task using the tools available.\n\n\
                TASK:\n{}\n\n\
                CONTEXT:\n{}\n\n\
                When finished, provide a clear, concise summary of what you did and found.",
                clean_goal, clean_context
            )
        };

        // Scan goal/context for image paths
        let image_paths = scan_for_images(&clean_goal, &clean_context);
        for img in image_paths {
            subagent_prompt.push_str(&format!(" ![](file://{})", img));
        }

        // Branching logic
        let branch_id = format!("branch_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let mut has_branch = false;
        {
            let tool = crate::tools::graph_memory::CreateDatabaseBranchTool;
            if let Ok(_) = tool.call(&serde_json::json!({ "branchId": branch_id })).await {
                crate::tui_println!("{}  ✓ Isolated simulation space branch '{}' created{}", EMERALD_GREEN, branch_id, COLOR_RESET);
                has_branch = true;
            }
        }

        // Workspace setup
        let parent_dir = current_workspace_root();
        let mut workspace_isolation = if needs_workspace {
            "isolated_worktree".to_string()
        } else {
            "not_required".to_string()
        };
        let mut workspace_isolation_reason = None;

        let workspace_dir = if !needs_workspace {
            parent_dir.clone()
        } else {
            let parent_dir_clone = parent_dir.clone();
            let workspace_res = tokio::task::spawn_blocking(move || {
                create_isolated_workspace(&parent_dir_clone)
            })
            .await;

            match workspace_res {
                Ok(Ok(dir)) => {
                    crate::tui_println!("{}  ✓ Isolated workspace worktree created at {:?}{}", EMERALD_GREEN, dir, COLOR_RESET);
                    dir
                }
                Ok(Err(e)) => {
                    let reason = e.to_string();
                    workspace_isolation = "fallback_active_workspace".to_string();
                    workspace_isolation_reason = Some(reason.clone());
                    crate::tui_println!("{}⚠️  Failed to create isolated workspace ({}). Running in active workspace without isolation.{}", AURA_GOLD, reason, COLOR_RESET);
                    parent_dir.clone()
                }
                Err(e) => {
                    let reason = format!("join error: {:?}", e);
                    workspace_isolation = "fallback_active_workspace".to_string();
                    workspace_isolation_reason = Some(reason.clone());
                    crate::tui_println!("{}⚠️  Failed to create isolated workspace ({}). Running in active workspace without isolation.{}", AURA_GOLD, reason, COLOR_RESET);
                    parent_dir.clone()
                }
            }
        };

        Ok(Self {
            parent_dir,
            workspace_dir,
            workspace_isolation,
            workspace_isolation_reason,
            branch_id,
            has_branch,
            clean_goal,
            clean_context,
            subagent_prompt,
            cancellation_token,
        })
    }

    pub async fn handle_teardown(
        &self,
        run_success: bool,
    ) {
        if self.has_branch {
            if run_success {
                if let Err(e) = crate::tools::graph_memory::CommitDatabaseBranchTool.call(&serde_json::json!({})).await {
                    tracing::warn!("Failed to commit database branch: {:?}", e);
                } else {
                    crate::tui_println!("{}  ✓ Committed simulation space branch '{}'{}", EMERALD_GREEN, self.branch_id, COLOR_RESET);
                }
            } else {
                if let Err(e) = crate::tools::graph_memory::RollbackDatabaseBranchTool.call(&serde_json::json!({})).await {
                    tracing::warn!("Failed to rollback database branch: {:?}", e);
                } else {
                    crate::tui_println!("{}  ✓ Rolled back simulation space branch '{}'{}", AURA_GOLD, self.branch_id, COLOR_RESET);
                }
            }
        }

        if run_success && self.workspace_dir != self.parent_dir {
            if let Err(e) = sync_changes_back(&self.workspace_dir, &self.parent_dir) {
                if !crate::agent::style::is_silent() {
                    let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                    crate::tui_println!("{}{}{}↶ Failed to sync changes back to active workspace: {}{}", AURA_SLATE, leaf_prefix, AURA_GOLD, e, COLOR_RESET);
                }
            } else {
                if !crate::agent::style::is_silent() {
                    let leaf_prefix = crate::agent::style::get_tree_prefix(true);
                    crate::tui_println!("{}{}{}✓ Synchronized changes back to active workspace{}", AURA_SLATE, leaf_prefix, AURA_GREEN, COLOR_RESET);
                }
            }
        }
    }

    pub fn format_cancellation_result(
        &self,
        tool_name: &str,
        subagent_name: Option<&str>,
        session_id: &str,
        model: &str,
        error_text: &str,
    ) -> serde_json::Value {
        let mut cancelled = cancellation_result_json(
            tool_name,
            subagent_name,
            session_id,
            model,
            error_text,
        );
        if let Some(obj) = cancelled.as_object_mut() {
            obj.insert(
                "workspaceIsolation".to_string(),
                serde_json::Value::String(self.workspace_isolation.clone()),
            );
            obj.insert(
                "workspaceIsolationReason".to_string(),
                self.workspace_isolation_reason.clone().map(serde_json::Value::String).unwrap_or(serde_json::Value::Null),
            );
        }
        cancelled
    }

    pub fn format_success_result(
        &self,
        session_id: &str,
        model: &str,
        summary: &str,
    ) -> serde_json::Value {
        serde_json::json!({
            "status": "success",
            "lifecycle": status_json(&SubagentRunStatus::Completed),
            "session_id": session_id,
            "model_used": model,
            "workspaceIsolation": self.workspace_isolation,
            "workspaceIsolationReason": self.workspace_isolation_reason,
            "summary": summary
        })
    }

    pub fn format_failure_result(
        &self,
        status: &SubagentRunStatus,
        error_msg: &str,
    ) -> serde_json::Value {
        serde_json::json!({
            "status": "error",
            "lifecycle": status_json(status),
            "workspaceIsolation": self.workspace_isolation,
            "workspaceIsolationReason": self.workspace_isolation_reason,
            "error": error_msg
        })
    }
}

pub struct CancelGuard {
    token: CancellationToken,
    pub completed: bool,
}

impl CancelGuard {
    pub fn new(token: CancellationToken) -> Self {
        Self { token, completed: false }
    }
    pub fn complete(&mut self) {
        self.completed = true;
    }
}

impl Drop for CancelGuard {
    fn drop(&mut self) {
        if !self.completed {
            self.token.cancel();
        }
    }
}

pub fn ensure_markdown_images(text: &str) -> String {
    let re = match regex::Regex::new(
        r"(?i)(file://[^\s\)\(]+\.(?:png|jpg|jpeg|webp|gif)|https?://[^\s\)\(]+\.(?:png|jpg|jpeg|webp|gif)|/[^\s\)\(]+\.(?:png|jpg|jpeg|webp|gif))",
    ) {
        Ok(r) => r,
        Err(_) => return text.to_string(),
    };

    let mut result = text.to_string();
    let mut matches: Vec<_> = re.find_iter(text).collect();
    matches.reverse();

    for mat in matches {
        let start = mat.start();
        let end = mat.end();
        let matched_str = mat.as_str();

        let mut already_formatted = false;
        if start > 0 {
            let before = &text[..start];
            if before.ends_with('(') || before.ends_with("](") {
                already_formatted = true;
            }
        }

        if !already_formatted {
            let replacement = format!("![]({})", matched_str);
            result.replace_range(start..end, &replacement);
        }
    }
    result
}

pub struct WorktreeGuard {
    pub parent_dir: std::path::PathBuf,
    pub worktree_dir: std::path::PathBuf,
    pub active: bool,
    cleanup_id: Option<u64>,
}

#[derive(Debug, Clone)]
struct RegisteredWorktreeCleanup {
    id: u64,
    parent_dir: std::path::PathBuf,
    worktree_dir: std::path::PathBuf,
}

static ACTIVE_WORKTREE_CLEANUPS: OnceLock<Mutex<Vec<RegisteredWorktreeCleanup>>> = OnceLock::new();
static NEXT_WORKTREE_CLEANUP_ID: AtomicU64 = AtomicU64::new(1);

fn worktree_cleanup_registry() -> &'static Mutex<Vec<RegisteredWorktreeCleanup>> {
    ACTIVE_WORKTREE_CLEANUPS.get_or_init(|| Mutex::new(Vec::new()))
}

fn register_worktree_cleanup(
    parent_dir: std::path::PathBuf,
    worktree_dir: std::path::PathBuf,
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
pub fn has_registered_worktree_cleanup_for_test(worktree_dir: &std::path::Path) -> bool {
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
    pub fn new(parent_dir: std::path::PathBuf, worktree_dir: std::path::PathBuf) -> Self {
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
        if self.active {
            cleanup_isolated_workspace(&self.parent_dir, &self.worktree_dir);
            unregister_worktree_cleanup(self.cleanup_id);
        }
    }
}

fn is_openz_worktree_dir(path: &std::path::Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.starts_with("openz_worktree_"))
        .unwrap_or(false)
}

pub fn directory_size_bytes(path: &std::path::Path) -> u64 {
    let mut size = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_dir() {
                    size += directory_size_bytes(&entry.path());
                } else {
                    size += meta.len();
                }
            }
        }
    }
    size
}

struct WorktreeCandidate {
    path: std::path::PathBuf,
    modified: std::time::SystemTime,
    size_bytes: u64,
}

fn collect_worktree_candidates(worktrees_dir: &std::path::Path) -> Vec<WorktreeCandidate> {
    let mut candidates = Vec::new();
    if let Ok(entries) = std::fs::read_dir(worktrees_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if is_openz_worktree_dir(&path) && path.is_dir() {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(modified) = meta.modified() {
                        let size_bytes = directory_size_bytes(&path);
                        candidates.push(WorktreeCandidate {
                            path,
                            modified,
                            size_bytes,
                        });
                    }
                }
            }
        }
    }
    candidates.sort_by_key(|c| c.modified);
    candidates
}

#[cfg(unix)]
fn available_bytes(path: &std::path::Path) -> Option<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let c_path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    if unsafe { libc::statvfs(c_path.as_ptr(), stats.as_mut_ptr()) } == 0 {
        let stats = unsafe { stats.assume_init() };
        Some(stats.f_frsize * stats.f_bavail)
    } else {
        None
    }
}

#[cfg(not(unix))]
fn available_bytes(_path: &std::path::Path) -> Option<u64> {
    None
}

#[derive(Debug, Clone)]
pub struct WorktreeCleanupPolicy {
    pub max_worktrees: usize,
    pub max_total_size_bytes: u64,
    pub min_free_space_bytes: u64,
    pub max_age_seconds: u64,
}

impl Default for WorktreeCleanupPolicy {
    fn default() -> Self {
        Self {
            max_worktrees: 10,
            max_total_size_bytes: 4 * 1024 * 1024 * 1024, // 4 GB
            min_free_space_bytes: 1024 * 1024 * 1024,      // 1 GB safety margin
            max_age_seconds: 24 * 3600,                    // 24 hours
        }
    }
}

pub fn cleanup_worktrees_dir(worktrees_dir: &std::path::Path, policy: WorktreeCleanupPolicy) {
    if !worktrees_dir.exists() {
        return;
    }
    let candidates = collect_worktree_candidates(worktrees_dir);
    let mut total_size: u64 = candidates.iter().map(|c| c.size_bytes).sum();
    let mut count = candidates.len();

    for candidate in &candidates {
        let age = std::time::SystemTime::now()
            .duration_since(candidate.modified)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let space_low = available_bytes(worktrees_dir)
            .map(|free| free < policy.min_free_space_bytes)
            .unwrap_or(false);

        let should_remove = count > policy.max_worktrees
            || total_size > policy.max_total_size_bytes
            || age > policy.max_age_seconds
            || space_low;

        if should_remove {
            tracing::info!(
                "Pruning worktree {:?} (age: {}s, size: {} bytes, count: {}, space_low: {})",
                candidate.path,
                age,
                candidate.size_bytes,
                count,
                space_low
            );
            cleanup_isolated_workspace(worktrees_dir, &candidate.path);
            total_size = total_size.saturating_sub(candidate.size_bytes);
            count = count.saturating_sub(1);
        }
    }
}

pub fn set_directory_modified_time_for_test(
    path: &std::path::Path,
    modified: std::time::SystemTime,
) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let meta = path.metadata()?;
        let atime = meta.atime();
        let mtime = modified
            .duration_since(std::time::SystemTime::UNIX_EPOCH)?
            .as_secs();
        let times = [
            libc::timeval {
                tv_sec: atime,
                tv_usec: 0,
            },
            libc::timeval {
                tv_sec: mtime as libc::time_t,
                tv_usec: 0,
            },
        ];
        let c_path = std::ffi::CString::new(path.as_os_str().to_string_lossy().as_bytes())?;
        if unsafe { libc::utimes(c_path.as_ptr(), times.as_ptr()) } != 0 {
            return Err(anyhow::anyhow!("Failed to set modified time via utimes"));
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let file = std::fs::File::open(path)?;
        file.set_modified(modified)?;
        Ok(())
    }
}

fn openz_worktrees_dir() -> std::path::PathBuf {
    resolve_path("~/.openz/worktrees")
}

pub fn current_workspace_root() -> std::path::PathBuf {
    crate::config::loader::ACTIVE_WORKSPACE
        .try_with(|workspace| workspace.clone())
        .unwrap_or_else(|_| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        })
}

fn enforce_disk_quota() {
    let dir = openz_worktrees_dir();
    cleanup_worktrees_dir(&dir, WorktreeCleanupPolicy::default());
}

pub fn cleanup_stale_resources() {
    enforce_disk_quota();
    let dir = openz_worktrees_dir();
    if !dir.exists() {
        return;
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if is_openz_worktree_dir(&path) && path.is_dir() {
                if is_older_than(&path, 3600 * 2) {
                    cleanup_isolated_workspace(&current_workspace_root(), &path);
                }
            }
        }
    }
}

fn is_older_than(path: &std::path::Path, seconds: u64) -> bool {
    if let Ok(meta) = path.metadata() {
        if let Ok(modified) = meta.modified() {
            if let Ok(elapsed) = std::time::SystemTime::now().duration_since(modified) {
                return elapsed.as_secs() > seconds;
            }
        }
    }
    false
}

pub fn create_isolated_workspace(parent_dir: &std::path::Path) -> Result<std::path::PathBuf> {
    enforce_disk_quota();

    let git_check = std::process::Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(parent_dir)
        .output();

    let is_git = match git_check {
        Ok(out) => out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "true",
        Err(_) => false,
    };

    if !is_git && is_dangerous_fallback_copy_root(parent_dir) {
        return Err(anyhow!(
            "Refusing to recursively copy unsafe workspace root '{}'. cd into a project git repository before launching OpenZ, or set the agent workspace to a safe project directory. Running subagents in active workspace disables isolation.",
            parent_dir.display()
        ));
    }

    let uuid_str = uuid::Uuid::new_v4().to_string();
    let slug = format!("openz_worktree_{}", &uuid_str[..8]);
    let worktree_dir = openz_worktrees_dir().join(slug);

    if !worktree_dir.exists() {
        std::fs::create_dir_all(&worktree_dir)?;
    }

    let has_git = parent_dir.join(".git").exists();
    if has_git {
        let output = std::process::Command::new("git")
            .current_dir(parent_dir)
            .args([
                "worktree",
                "add",
                "--detach",
                &worktree_dir.to_string_lossy(),
                "HEAD",
            ])
            .output();

        match output {
            Ok(out) => {
                if out.status.success() {
                    return Ok(worktree_dir);
                } else {
                    let err_msg = String::from_utf8_lossy(&out.stderr).to_string();
                    if err_msg.contains("already has a worktree")
                        || err_msg.contains("is already checked out")
                    {
                        let _ = std::process::Command::new("git")
                            .current_dir(parent_dir)
                            .args(["worktree", "prune"])
                            .output();
                    }
                    tracing::warn!(
                        "git worktree add failed: {}. Falling back to copy-based isolation.",
                        err_msg
                    );
                }
            }
            Err(e) => {
                tracing::warn!(
                    "git command failed to launch: {:?}. Falling back to copy-based isolation.",
                    e
                );
            }
        }
    }

    copy_dir_recursive_filtered(parent_dir, &worktree_dir)?;
    Ok(worktree_dir)
}

fn should_skip_workspace_copy_dir(name: &str) -> bool {
    let n = name.to_lowercase();
    n.starts_with('.')
        || n == "node_modules"
        || n == "target"
        || n == "venv"
        || n == "env"
        || n == "build"
        || n == "dist"
        || n == "tmp"
        || n == "out"
        || n == "cache"
        || n == "pkg"
        || n == "vendor"
        || n == "gems"
        || n == "coverage"
        || n == "logs"
        || n == "public"
        || n == "static"
        || n == "assets"
        || n == "media"
        || n == "images"
        || n == "uploads"
        || n == "downloads"
        || n == "videos"
        || n == "storage"
        || n == "data"
        || n == "db"
        || n == "database"
        || n == "worktrees"
        || n == "tool_outputs"
}

fn canonical_or_original(path: &std::path::Path) -> std::path::PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn is_dangerous_fallback_copy_root(path: &std::path::Path) -> bool {
    let resolved = canonical_or_original(path);
    let home = resolve_path("~");
    let canonical_home = canonical_or_original(&home);

    resolved == canonical_home
        || resolved == PathBuf::from("/")
        || resolved == PathBuf::from("/home")
        || resolved == PathBuf::from("/Users")
}

pub fn copy_dir_recursive_filtered(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    if !dst.exists() {
        std::fs::create_dir_all(dst)?;
    }

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        if should_skip_workspace_copy_dir(name) {
            continue;
        }

        let dst_path = dst.join(name);
        if path.is_dir() {
            copy_dir_recursive_filtered(&path, &dst_path)?;
        } else {
            std::fs::copy(&path, &dst_path)?;
        }
    }
    Ok(())
}

pub fn cleanup_isolated_workspace(parent_dir: &std::path::Path, worktree_dir: &std::path::Path) {
    if parent_dir == worktree_dir {
        return;
    }
    if !worktree_dir.exists() {
        return;
    }

    let has_git = parent_dir.join(".git").exists();
    if has_git {
        let output = std::process::Command::new("git")
            .current_dir(parent_dir)
            .args([
                "worktree",
                "remove",
                "--force",
                &worktree_dir.to_string_lossy(),
            ])
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                let _ = std::fs::remove_dir_all(worktree_dir);
                return;
            }
        }
    }

    let _ = std::fs::remove_dir_all(worktree_dir);
}

pub fn sync_changes_back(src_dir: &std::path::Path, dst_dir: &std::path::Path) -> Result<()> {
    if src_dir == dst_dir {
        return Ok(());
    }

    if !src_dir.exists() {
        return Ok(());
    }

    for entry in std::fs::read_dir(src_dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        if should_skip_workspace_copy_dir(name) {
            continue;
        }

        let dst_path = dst_dir.join(name);
        if path.is_dir() {
            sync_changes_back(&path, &dst_path)?;
        } else {
            let needs_copy = if dst_path.exists() {
                let src_meta = path.metadata()?;
                let dst_meta = dst_path.metadata()?;
                src_meta.len() != dst_meta.len()
                    || src_meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                        > dst_meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            } else {
                true
            };

            if needs_copy {
                if let Some(parent) = dst_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(&path, &dst_path)?;
            }
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
