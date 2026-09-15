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
