use super::db::*;
use crate::tools::Tool;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::sync::{Mutex, OnceLock};

static BRANCH_MUTEX: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn get_active_branch() -> &'static Mutex<Option<String>> {
    BRANCH_MUTEX.get_or_init(|| Mutex::new(None))
}

fn branch_db_path(branch_id: &str) -> std::path::PathBuf {
    let base = get_db_path();
    let base_str = base.to_string_lossy().to_string();
    std::path::PathBuf::from(format!("{}.branch_{}", base_str, branch_id))
}

// ─── Tool 10: CreateDatabaseBranchTool ──────────────────────────

pub struct CreateDatabaseBranchTool;

#[async_trait]
impl Tool for CreateDatabaseBranchTool {
    fn name(&self) -> &str {
        "create_database_branch"
    }

    fn description(&self) -> &str {
        "Create an isolated database branch for subagent/task execution. Branch ID must be unique."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "branchId": { "type": "string", "description": "Unique identifier for the branch" }
            },
            "required": ["branchId"]
        })
    }

    async fn call(&self, arguments: &Value) -> Result<Value> {
        if arguments["branchId"].as_str().is_none() {
            return Err(anyhow!("Missing 'branchId'"));
        }
        let branch_id = arguments["branchId"].as_str().unwrap();

        // Lock static db connection first
        let db_mutex =
            db_static().get_or_init(|| Mutex::new(Connection::open_in_memory().unwrap()));
        let mut db_guard = db_mutex.lock().map_err(|e| anyhow!("Lock error: {}", e))?;

        // Then lock active branch state
        let mut active = get_active_branch()
            .lock()
            .map_err(|e| anyhow!("Branch lock error: {}", e))?;
        if active.is_some() {
            return Err(anyhow!(
                "A database branch is already active. Commit or rollback first."
            ));
        }

        let src = get_db_path();
        let dst = branch_db_path(branch_id);

        // Flush WAL before copying
        let _ = db_guard.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");

        std::thread::sleep(std::time::Duration::from_millis(10));

        // If src doesn't exist yet (e.g. in tests), create a fresh DB with schema
        if !src.exists() {
            let fresh = init_db()?;
            drop(fresh);
        }

        std::fs::copy(&src, &dst)?;

        // Reset the static DB connection to point to branch file
        let branch_conn = Connection::open(&dst)?;
        branch_conn.execute_batch(&format!(
            "PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000; {}",
            SCHEMA_DDL
        ))?;
        *db_guard = branch_conn;

        *active = Some(branch_id.to_string());
        Ok(json!({ "status": format!("Created branch: {}", branch_id) }))
    }
}

// ─── Tool 11: CommitDatabaseBranchTool ──────────────────────────

pub struct CommitDatabaseBranchTool;

#[async_trait]
impl Tool for CommitDatabaseBranchTool {
    fn name(&self) -> &str {
        "commit_database_branch"
    }

    fn description(&self) -> &str {
        "Commit changes from the active database branch to the main database and delete the branch."
    }

    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }

    async fn call(&self, _arguments: &Value) -> Result<Value> {
        // Lock static db connection first
        let db_mutex =
            db_static().get_or_init(|| Mutex::new(Connection::open_in_memory().unwrap()));
        let mut db_guard = db_mutex.lock().map_err(|e| anyhow!("Lock error: {}", e))?;

        // Then lock active branch state
        let mut active = get_active_branch()
            .lock()
            .map_err(|e| anyhow!("Branch lock error: {}", e))?;
        let branch_id = active
            .as_ref()
            .ok_or_else(|| anyhow!("No active branch to commit."))?
            .clone();

        let branch_path = branch_db_path(&branch_id);
        let main_path = get_db_path();

        // Switch DB to in-memory to release file locks
        // Flush WAL first
        let _ = db_guard.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");

        let mem_conn = Connection::open_in_memory()?;
        *db_guard = mem_conn;

        // Copy branch file over main db
        std::fs::copy(&branch_path, &main_path)?;
        std::fs::remove_file(&branch_path)?;
        // Clean up WAL/SHM files
        let _ = std::fs::remove_file(main_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(main_path.with_extension("db-shm"));

        // Restore connection to updated main
        let main_conn = Connection::open(&main_path)?;
        main_conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;
        *db_guard = main_conn;

        *active = None;
        Ok(json!({ "status": format!("Committed branch: {}", branch_id) }))
    }
}

// ─── Tool 12: RollbackDatabaseBranchTool ────────────────────────

pub struct RollbackDatabaseBranchTool;

#[async_trait]
impl Tool for RollbackDatabaseBranchTool {
    fn name(&self) -> &str {
        "rollback_database_branch"
    }

    fn description(&self) -> &str {
        "Roll back changes from the active database branch, restoring the main database state and deleting the branch."
    }

    fn parameters(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }

    async fn call(&self, _arguments: &Value) -> Result<Value> {
        // Lock static db connection first
        let db_mutex =
            db_static().get_or_init(|| Mutex::new(Connection::open_in_memory().unwrap()));
        let mut db_guard = db_mutex.lock().map_err(|e| anyhow!("Lock error: {}", e))?;

        // Then lock active branch state
        let mut active = get_active_branch()
            .lock()
            .map_err(|e| anyhow!("Branch lock error: {}", e))?;
        let branch_id = active
            .as_ref()
            .ok_or_else(|| anyhow!("No active branch to rollback."))?
            .clone();

        let branch_path = branch_db_path(&branch_id);

        // Restore connection back to main database
        let main_path = get_db_path();
        let main_conn = Connection::open(&main_path)?;
        main_conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;
        *db_guard = main_conn;

        // Remove branch file
        if branch_path.exists() {
            std::fs::remove_file(&branch_path)?;
        }

        *active = None;
        Ok(json!({ "status": format!("Rolled back branch: {}", branch_id) }))
    }
}
