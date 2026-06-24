use crate::config;
use crate::schema;
use rusqlite::Connection;
use std::sync::{LazyLock, Mutex};

static DB: LazyLock<Mutex<Connection>> = LazyLock::new(|| {
    let cfg = config::get();
    if let Some(parent) = std::path::Path::new(&cfg.db_path).parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let conn = Connection::open(&cfg.db_path).expect("failed to open SQLite database");
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")
        .expect("failed to set pragmas");
    schema::create_tables(&conn);
    schema::seed_alert_rules(&conn);
    Mutex::new(conn)
});

pub fn with_db<F, R>(f: F) -> R
where
    F: FnOnce(&Connection) -> R,
{
    let conn = DB.lock().unwrap();
    f(&conn)
}
