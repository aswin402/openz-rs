use crate::config::loader;
use anyhow::Result;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiskStatus {
    Ok,
    Warn,
    Critical,
    Missing,
}

#[derive(Debug, Clone)]
struct DiskItem {
    label: &'static str,
    path: std::path::PathBuf,
    size_bytes: Option<u64>,
    warn_bytes: u64,
    critical_bytes: u64,
    cleanup: Option<&'static str>,
}

impl DiskItem {
    fn new(
        label: &'static str,
        path: std::path::PathBuf,
        warn_gib: u64,
        critical_gib: u64,
        cleanup: Option<&'static str>,
    ) -> Self {
        Self {
            label,
            path,
            size_bytes: None,
            warn_bytes: gib(warn_gib),
            critical_bytes: gib(critical_gib),
            cleanup,
        }
    }

    fn measure(mut self) -> Self {
        self.size_bytes = path_size_bytes(&self.path);
        self
    }

    fn status(&self) -> DiskStatus {
        let size = match self.size_bytes {
            Some(size) => size,
            None => return DiskStatus::Missing,
        };
        match (size >= self.critical_bytes, size >= self.warn_bytes) {
            (true, _) => DiskStatus::Critical,
            (false, true) => DiskStatus::Warn,
            (false, false) => DiskStatus::Ok,
        }
    }
}

fn gib(n: u64) -> u64 {
    n.saturating_mul(1024 * 1024 * 1024)
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    match unit {
        0 => format!("{} {}", bytes, UNITS[unit]),
        _ => format!("{value:.1} {}", UNITS[unit]),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SecretScrubResult {
    text: String,
    replacements: usize,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct SecretScrubReport {
    files_scanned: usize,
    files_changed: usize,
    replacements: usize,
}

fn scrub_secret_text(text: &str) -> SecretScrubResult {
    let (text, replacements) = crate::core::secrets::scrub_secret_text(text);
    SecretScrubResult {
        text,
        replacements,
    }
}

fn write_scrubbed_file_atomically(path: &Path, scrubbed_text: &str) -> Result<()> {
    let is_session_json = path.extension().and_then(|e| e.to_str()) == Some("json")
        && path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            == Some("sessions");

    let payload = match is_session_json
        .then(|| serde_json::from_str::<crate::session::Session>(scrubbed_text).ok())
        .flatten()
    {
        Some(mut session) => {
            session.populate_hashes();
            serde_json::to_string_pretty(&session).unwrap_or_else(|_| scrubbed_text.to_string())
        }
        None => scrubbed_text.to_string(),
    };

    let tmp_path = path.with_extension("scrub.tmp");
    std::fs::write(&tmp_path, &payload)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

fn scrub_secret_file(path: &Path) -> Result<SecretScrubResult> {
    let original = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => match err.kind() {
            std::io::ErrorKind::InvalidData => {
                return Ok(SecretScrubResult {
                    text: String::new(),
                    replacements: 0,
                });
            }
            _ => return Err(err.into()),
        },
    };
    let result = scrub_secret_text(&original);
    match result.replacements > 0 && result.text != original {
        true => write_scrubbed_file_atomically(path, &result.text)?,
        false => {}
    }
    Ok(result)
}

fn scrub_secret_roots(data_dir: &Path) -> Vec<std::path::PathBuf> {
    [
        "sessions",
        "traces",
        "tool_outputs",
        "data",
        "memory",
        "shared",
        "active_tui",
    ]
    .into_iter()
    .map(|name| data_dir.join(name))
    .collect()
}

fn scrub_secrets_in_runtime_state(data_dir: &Path) -> Result<SecretScrubReport> {
    let mut report = SecretScrubReport::default();
    let mut stack = scrub_secret_roots(data_dir);
    while let Some(path) = stack.pop() {
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        match (
            meta.file_type().is_symlink(),
            meta.is_dir(),
            meta.is_file(),
        ) {
            (true, _, _) => continue,
            (false, true, _) => {
                let entries = match std::fs::read_dir(&path) {
                    Ok(entries) => entries,
                    Err(_) => continue,
                };
                for entry in entries.flatten() {
                    stack.push(entry.path());
                }
                continue;
            }
            (false, false, false) => continue,
            (false, false, true) => {}
        }

        report.files_scanned = report.files_scanned.saturating_add(1);
        let result = scrub_secret_file(&path)?;
        match result.replacements > 0 {
            true => {
                report.files_changed = report.files_changed.saturating_add(1);
                report.replacements = report.replacements.saturating_add(result.replacements);
            }
            false => {}
        }
    }
    Ok(report)
}

fn path_size_bytes(path: &Path) -> Option<u64> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    match (metadata.is_file(), metadata.is_dir()) {
        (true, _) => return Some(metadata.len()),
        (false, false) => return Some(0),
        (false, true) => {}
    }

    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let meta = match std::fs::symlink_metadata(&path) {
                Ok(meta) => meta,
                Err(_) => continue,
            };
            match (meta.is_dir(), meta.is_file()) {
                (true, _) => stack.push(path),
                (false, true) => total = total.saturating_add(meta.len()),
                (false, false) => {}
            }
        }
    }
    Some(total)
}

#[cfg(unix)]
fn available_bytes(path: &Path) -> Option<u64> {
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

#[cfg(not(unix))]
fn available_bytes(_path: &Path) -> Option<u64> {
    None
}

fn nearest_existing_path(path: &Path) -> &Path {
    let mut current = path;
    while !current.exists() {
        let parent = match current.parent() {
            Some(parent) => parent,
            None => return path,
        };
        current = parent;
    }
    current
}

fn status_icon(status: DiskStatus) -> &'static str {
    match status {
        DiskStatus::Ok => "✅",
        DiskStatus::Warn => "⚠️ ",
        DiskStatus::Critical => "🚨",
        DiskStatus::Missing => "ℹ️ ",
    }
}

fn print_disk_report(data_dir: &Path) {
    println!("💽 Disk and cache usage report...");
    let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    let home = dirs::home_dir().unwrap_or_else(|| Path::new("~").to_path_buf());
    let cargo_dir = home.join(".cargo");

    let items = vec![
        DiskItem::new(
            "repo target/ build cache",
            cwd.join("target"),
            20,
            50,
            Some("Run: openz doctor --clean-target  (or cargo clean)"),
        ),
        DiskItem::new(
            "OpenZ runtime data ~/.openz",
            data_dir.to_path_buf(),
            5,
            20,
            Some("Inspect: openz logs --tail 100; prune old tool outputs/sessions if needed"),
        ),
        DiskItem::new(
            "OpenZ tool outputs",
            data_dir.join("tool_outputs"),
            1,
            5,
            Some("Large tool outputs are usually safe to archive after review"),
        ),
        DiskItem::new(
            "OpenZ traces",
            data_dir.join("traces"),
            1,
            5,
            Some("Old trace files can be archived after debugging"),
        ),
        DiskItem::new(
            "SearchXyz repo cache",
            home.join(".searchxyz"),
            5,
            20,
            Some("Use SearchXyz source cleanup for repos you no longer need"),
        ),
        DiskItem::new(
            "Cargo registry cache",
            cargo_dir.join("registry"),
            5,
            15,
            Some(
                "Cargo registry cache is rebuildable/downloadable; clean selectively if disk remains low",
            ),
        ),
        DiskItem::new(
            "Cargo git cache",
            cargo_dir.join("git"),
            2,
            5,
            Some(
                "Cargo git cache is rebuildable/downloadable; clean selectively if disk remains low",
            ),
        ),
        DiskItem::new(
            "Cargo installed binaries",
            cargo_dir.join("bin"),
            2,
            5,
            Some("Review ~/.cargo/bin for old binaries before deleting anything"),
        ),
    ];

    let mut worst = DiskStatus::Ok;
    for item in items.into_iter().map(DiskItem::measure) {
        let status = item.status();
        let should_promote = matches!(status, DiskStatus::Critical)
            || (matches!(status, DiskStatus::Warn) && matches!(worst, DiskStatus::Ok));
        match should_promote {
            true => worst = status,
            false => {}
        }
        let size = item
            .size_bytes
            .map(format_bytes)
            .unwrap_or_else(|| "not present".to_string());
        println!(
            "   {} {:<30} {:>12}  {}",
            status_icon(status),
            item.label,
            size,
            item.path.display()
        );
        match (
            matches!(status, DiskStatus::Warn | DiskStatus::Critical),
            item.cleanup,
        ) {
            (true, Some(cleanup)) => println!("      ↳ {cleanup}"),
            _ => {}
        }
    }

    let free_probe = nearest_existing_path(&cwd);
    match available_bytes(free_probe) {
        Some(bytes) => {
            println!(
                "   • Free space on current filesystem: {}",
                format_bytes(bytes)
            );
            match (bytes < gib(5), bytes < gib(20)) {
                (true, _) => {
                    println!(
                        "🚨 Critical: less than 5 GiB free. Clean target/ or archive caches before running large builds/media tools."
                    );
                }
                (false, true) => {
                    println!(
                        "⚠️  Warning: less than 20 GiB free. Large builds, video renders, and crawls may fail."
                    );
                }
                (false, false) => {}
            }
        }
        None => println!("   • Free space on current filesystem: unavailable on this platform"),
    }

    match worst {
        DiskStatus::Critical => println!("🚨 Disk report found critical cache growth."),
        DiskStatus::Warn => println!("⚠️  Disk report found cache growth worth cleaning soon."),
        _ => println!("✅ Disk report found no oversized OpenZ caches."),
    }
}

/// Archive stale graph-memory branch databases to the legacy-root-backup
/// directory. These are leftover from the graph-memory branch/rollback feature
/// and are no longer active. Data is preserved (not deleted).
fn archive_stale_graph_branches(data_dir: &Path) -> usize {
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%S");
    let archive_dir = data_dir.join(format!("legacy-root-backup/pruned-branches-{}", stamp));
    let mut count = 0;

    match std::fs::read_dir(data_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                match entry.path().is_file() && name.starts_with("graph_memory.db.branch") {
                    true => {
                        let src = entry.path();
                        let dst = archive_dir.join(&name);
                        let _ = std::fs::create_dir_all(&archive_dir);
                        match std::fs::rename(&src, &dst) {
                            Ok(()) => count += 1,
                            Err(_) => {}
                        }
                    }
                    false => {}
                }
            }
        }
        Err(_) => {}
    }
    count
}

/// Purges the target build cache at `target_path` and returns reclaimed bytes.
pub fn clean_target_cache_at(target_path: &Path) -> Result<u64> {
    match target_path.exists() {
        false => return Ok(0),
        true => {}
    }
    let size = path_size_bytes(target_path).unwrap_or(0);
    match target_path.is_file() {
        true => std::fs::remove_file(target_path)?,
        false => std::fs::remove_dir_all(target_path)?,
    }
    Ok(size)
}

/// `openz doctor` — verify runtime databases live under the global data dir
/// (~/.openz), relocate any stray artifacts found in the working directory,
/// and prune stale graph-memory branch databases. No data is ever deleted.
pub async fn handle_doctor(scrub_secrets: bool, clean_target: bool) -> Result<()> {
    println!("🩺 OpenZ doctor — runtime database placement check");
    println!("────────────────────────────────────────────");

    let data_dir = loader::runtime_data_dir();
    println!("Global runtime data dir: {}", data_dir.display());
    for name in loader::RUNTIME_DB_FILENAMES {
        println!(
            "   • {} -> {}",
            name,
            loader::runtime_db_path(name).display()
        );
    }
    println!();

    // ── Step 1: Check for stray runtime DBs in the working directory ──
    let diag = loader::check_root_runtime_dbs();

    match diag.has_root_runtime_dbs() {
        false => {
            println!("✅ No stray runtime databases found in the working directory.");
        }
        true => {
            println!("⚠️  Found runtime DB artifacts in the working directory:");
            for p in &diag.found {
                println!("   • {}", p.display());
            }
            println!();
            println!("🔧 Relocating artifacts to the global data dir...");

            let moves = loader::migrate_root_runtime_dbs();
            match moves.is_empty() {
                true => {
                    println!("ℹ️  No files were moved (already relocated or still open).");
                }
                false => {
                    for (from, to) in &moves {
                        println!("   ↳ {}  →  {}", from.display(), to.display());
                    }
                    println!();
                    println!(
                        "✅ Relocated {} file(s). Archived under {}/legacy-root-backup/.",
                        moves.len(),
                        data_dir.display()
                    );
                }
            }
        }
    }
    println!();

    // ── Step 2: Prune stale graph-memory branch databases ──
    println!("🔍 Checking for stale graph-memory branch databases...");
    let pruned = archive_stale_graph_branches(&data_dir);
    match pruned {
        0 => println!("✅ No stale graph-memory branch databases found."),
        _ => println!(
            "✅ Archived {} stale branch database(s) to {}/legacy-root-backup/pruned-branches-*/.",
            pruned,
            data_dir.display()
        ),
    }
    println!();

    // ── Step 3: Repair research-memory topic aliases ──
    println!("🔍 Checking research brief topic aliases...");
    match crate::tools::shared_memory::repair_research_brief_topics().await {
        Ok(0) => println!("✅ No research brief topic aliases needed repair."),
        Ok(repaired) => {
            println!(
                "✅ Repaired {} research brief topic alias(es) using saved source evidence.",
                repaired
            );
        }
        Err(err) => println!("⚠️  Research brief topic repair skipped: {err}"),
    }
    println!();

    // ── Step 4: Optional historical secret scrub ──
    match scrub_secrets {
        true => {
            println!("🔐 Scrubbing historical secrets from runtime transcripts...");
            match scrub_secrets_in_runtime_state(&data_dir) {
                Ok(report) => {
                    println!(
                        "✅ Secret scrub complete: {} replacement(s) across {} changed file(s), {} file(s) scanned.",
                        report.replacements, report.files_changed, report.files_scanned
                    );
                    println!("   Config files and backups are not scanned by this action.");
                }
                Err(err) => println!("⚠️  Secret scrub failed: {err}"),
            }
            println!();
        }
        false => {}
    }

    // ── Step 5: Optional target build cache cleanup ──
    match clean_target {
        true => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
            let target_dir = cwd.join("target");
            println!("🧹 Cleaning build cache at {}...", target_dir.display());
            match clean_target_cache_at(&target_dir) {
                Ok(0) => println!("ℹ️  No build cache found to clean."),
                Ok(reclaimed) => {
                    println!(
                        "✅ Cleaned target build cache: reclaimed {}.",
                        format_bytes(reclaimed)
                    );
                }
                Err(err) => println!("⚠️  Failed to clean build cache: {err}"),
            }
            println!();
        }
        false => {}
    }

    // ── Step 6: Disk/cache pressure report ──
    print_disk_report(&data_dir);
    println!();

    // ── Step 7: OpenZ Vault deliverables verification ──
    println!("📦 Checking OpenZ Vault deliverables workspace...");
    let config = loader::load_config().unwrap_or_default();
    match crate::core::vault::ensure_vault_initialized(&config) {
        Ok(vault_path) => {
            let summary = crate::core::vault::scan_vault(&config);
            println!(
                "✅ OpenZ Vault is active at: {} ({} deliverable(s) across {} categories)",
                vault_path.display(),
                summary.total_items,
                summary.categories.len()
            );
            for cat in &summary.categories {
                match cat.item_count > 0 {
                    true => println!(
                        "   • {}/{}: {} item(s)",
                        config.vault.name, cat.name, cat.item_count
                    ),
                    false => {}
                }
            }
        }
        Err(err) => println!("⚠️  OpenZ Vault initialization error: {err}"),
    }
    println!();

    // ── Close ──
    println!(
        "🩺 Doctor check complete. Runtime state resolves under: {}",
        data_dir.display()
    );
    Ok(())
}

#[cfg(test)]
#[path = "doctor_tests.rs"]
mod tests;

