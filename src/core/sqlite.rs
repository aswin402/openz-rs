use rusqlite::{Connection, Result};

/// Canonical standard pragmas used across OpenZ SQLite stores:
/// - WAL mode for concurrent read/write access
/// - 5000ms busy timeout to prevent transient locked-database errors
/// - -2000 cache size (~2MB memory cache per connection)
/// - mmap_size=0 for zero mmap overhead on low-resource environments
/// - synchronous=NORMAL for optimal performance in WAL mode
/// - wal_autocheckpoint=1000 for regular WAL truncation
pub const STANDARD_SQLITE_PRAGMAS: &str =
    "PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000; PRAGMA cache_size=-2000; PRAGMA mmap_size=0; PRAGMA synchronous=NORMAL; PRAGMA wal_autocheckpoint=1000;";

/// Apply the standard SQLite pragmas to an active connection.
pub fn apply_standard_pragmas(conn: &Connection) -> Result<()> {
    conn.execute_batch(STANDARD_SQLITE_PRAGMAS)
}

/// Apply the standard SQLite pragmas with additional SQL statements (e.g. `PRAGMA foreign_keys = ON;`).
pub fn apply_standard_pragmas_with_extra(conn: &Connection, extra_sql: &str) -> Result<()> {
    let trimmed = extra_sql.trim();
    if trimmed.is_empty() {
        apply_standard_pragmas(conn)
    } else {
        let combined = format!("{STANDARD_SQLITE_PRAGMAS} {trimmed}");
        conn.execute_batch(&combined)
    }
}

#[cfg(test)]
#[path = "sqlite_tests.rs"]
mod tests;
