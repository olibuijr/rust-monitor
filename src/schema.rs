use rusqlite::Connection;

pub fn create_tables(conn: &Connection) {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS metrics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            value REAL NOT NULL,
            ts INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_metrics_name_ts ON metrics(name, ts);

        CREATE TABLE IF NOT EXISTS logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source TEXT NOT NULL,
            line TEXT NOT NULL,
            ts INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_logs_source_ts ON logs(source, ts);

        CREATE TABLE IF NOT EXISTS alert_rules (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            metric_name TEXT NOT NULL,
            operator TEXT NOT NULL,
            threshold REAL NOT NULL,
            duration_secs INTEGER NOT NULL,
            enabled INTEGER NOT NULL DEFAULT 1
        );

        CREATE TABLE IF NOT EXISTS alert_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            rule_id INTEGER NOT NULL REFERENCES alert_rules(id),
            triggered_at INTEGER NOT NULL,
            resolved_at INTEGER,
            value REAL NOT NULL
        );
        ",
    )
    .expect("failed to create tables");
}

pub fn seed_alert_rules(conn: &Connection) {
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM alert_rules", [], |row| row.get(0))
        .unwrap_or(0);
    if count > 0 {
        return;
    }

    let rules = [
        ("High CPU", "cpu.usage", "gt", 90.0, 300),
        ("High Memory", "mem.used_pct", "gt", 90.0, 300),
        ("Disk Almost Full", "disk./.used_pct", "gt", 85.0, 60),
    ];

    for (name, metric, op, threshold, duration) in &rules {
        conn.execute(
            "INSERT INTO alert_rules (name, metric_name, operator, threshold, duration_secs) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![name, metric, op, threshold, duration],
        )
        .expect("failed to seed alert rule");
    }

    tracing::info!("seeded {} default alert rules", rules.len());
}
