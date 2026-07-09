use crate::config::loader;
use anyhow::Result;
use std::path::Path;

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

    // ── Close ──
    println!(
        "🩺 Doctor check complete. All runtime state resolves under: {}",
        data_dir.display()
    );
    Ok(())
}
