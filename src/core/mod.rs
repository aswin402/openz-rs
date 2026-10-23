pub mod heal;
pub mod http;
pub mod inventory;
pub mod process;
pub mod secrets;
pub mod sqlite;
pub mod vault;

/// Generate a short 8-hex-character UUID v4 identifier.
pub fn short_uuid() -> String {
    uuid::Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(8)
        .collect()
}

