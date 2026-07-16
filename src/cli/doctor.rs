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
        let Some(size) = self.size_bytes else {
            return DiskStatus::Missing;
        };
        if size >= self.critical_bytes {
            DiskStatus::Critical
        } else if size >= self.warn_bytes {
            DiskStatus::Warn
        } else {
            DiskStatus::Ok
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
    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn path_size_bytes(path: &Path) -> Option<u64> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if metadata.is_file() {
        return Some(metadata.len());
    }
    if !metadata.is_dir() {
        return Some(0);
    }

    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.is_dir() {
                stack.push(path);
            } else if meta.is_file() {
                total = total.saturating_add(meta.len());
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

fn nearest_existing_path(path: &Path) -> &Path {
    let mut current = path;
    while !current.exists() {
        let Some(parent) = current.parent() else {
            return path;
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
            Some("Run: ./localupdate.sh --clean-target  (or cargo clean)"),
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
            Some("Cargo registry cache is rebuildable/downloadable; clean selectively if disk remains low"),
        ),
        DiskItem::new(
            "Cargo git cache",
            cargo_dir.join("git"),
            2,
            5,
            Some("Cargo git cache is rebuildable/downloadable; clean selectively if disk remains low"),
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
        if matches!(status, DiskStatus::Critical)
            || (matches!(status, DiskStatus::Warn) && matches!(worst, DiskStatus::Ok))
        {
            worst = status;
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
        if matches!(status, DiskStatus::Warn | DiskStatus::Critical) {
            if let Some(cleanup) = item.cleanup {
                println!("      ↳ {cleanup}");
            }
        }
    }

    let free_probe = nearest_existing_path(&cwd);
    match available_bytes(free_probe) {
        Some(bytes) => {
            println!(
                "   • Free space on current filesystem: {}",
                format_bytes(bytes)
            );
            if bytes < gib(5) {
                println!("🚨 Critical: less than 5 GiB free. Clean target/ or archive caches before running large builds/media tools.");
            } else if bytes < gib(20) {
                println!("⚠️  Warning: less than 20 GiB free. Large builds, video renders, and crawls may fail.");
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

    if let Ok(entries) = std::fs::read_dir(data_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.path().is_file() && name.starts_with("graph_memory.db.branch") {
                let src = entry.path();
                let dst = archive_dir.join(&name);
                let _ = std::fs::create_dir_all(&archive_dir);
                if std::fs::rename(&src, &dst).is_ok() {
                    count += 1;
                }
            }
        }
    }
    count
}

/// `openz doctor` — verify runtime databases live under the global data dir
/// (~/.openz), relocate any stray artifacts found in the working directory,
/// and prune stale graph-memory branch databases. No data is ever deleted.
pub async fn handle_doctor() -> Result<()> {
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

    if !diag.has_root_runtime_dbs() {
        println!("✅ No stray runtime databases found in the working directory.");
    } else {
        println!("⚠️  Found runtime DB artifacts in the working directory:");
        for p in &diag.found {
            println!("   • {}", p.display());
        }
        println!();
        println!("🔧 Relocating artifacts to the global data dir...");

        let moves = loader::migrate_root_runtime_dbs();
        if moves.is_empty() {
            println!("ℹ️  No files were moved (already relocated or still open).");
        } else {
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
    println!();

    // ── Step 2: Prune stale graph-memory branch databases ──
    println!("🔍 Checking for stale graph-memory branch databases...");
    let pruned = archive_stale_graph_branches(&data_dir);
    if pruned > 0 {
        println!(
            "✅ Archived {} stale branch database(s) to {}/legacy-root-backup/pruned-branches-*/.",
            pruned,
            data_dir.display()
        );
    } else {
        println!("✅ No stale graph-memory branch databases found.");
    }
    println!();

    // ── Step 3: Disk/cache pressure report ──
    print_disk_report(&data_dir);
    println!();

    // ── Close ──
    println!(
        "🩺 Doctor check complete. Runtime state resolves under: {}",
        data_dir.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bytes_uses_binary_units() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1024), "1.0 KiB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MiB");
        assert_eq!(format_bytes(5 * 1024 * 1024 * 1024), "5.0 GiB");
    }

    #[test]
    fn disk_item_status_respects_thresholds() {
        let base = DiskItem::new("test", Path::new("target").to_path_buf(), 20, 50, None);
        let missing = base.clone();
        assert_eq!(missing.status(), DiskStatus::Missing);

        let mut ok = base.clone();
        ok.size_bytes = Some(gib(1));
        assert_eq!(ok.status(), DiskStatus::Ok);

        let mut warn = base.clone();
        warn.size_bytes = Some(gib(20));
        assert_eq!(warn.status(), DiskStatus::Warn);

        let mut critical = base;
        critical.size_bytes = Some(gib(50));
        assert_eq!(critical.status(), DiskStatus::Critical);
    }
}
