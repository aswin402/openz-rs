use super::*;

#[test]
fn test_server_init() {
    let server = get_server();
    assert!(server.get_db_conn().is_ok());
}
