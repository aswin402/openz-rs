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
mod tests {
    use super::*;

    #[test]
    fn test_apply_standard_pragmas() {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        apply_standard_pragmas(&conn).expect("apply standard pragmas");

        let busy_timeout: i64 = conn
            .query_row("PRAGMA busy_timeout;", [], |row| row.get(0))
            .expect("query busy_timeout");
        assert_eq!(busy_timeout, 5000);

        let synchronous: i64 = conn
            .query_row("PRAGMA synchronous;", [], |row| row.get(0))
            .expect("query synchronous");
        assert_eq!(synchronous, 1); // 1 = NORMAL
    }

    #[test]
    fn test_apply_standard_pragmas_with_extra() {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        apply_standard_pragmas_with_extra(&conn, "PRAGMA foreign_keys = ON;")
            .expect("apply standard pragmas with foreign keys");

        let fk: i64 = conn
            .query_row("PRAGMA foreign_keys;", [], |row| row.get(0))
            .expect("query foreign_keys");
        assert_eq!(fk, 1);
    }
}
