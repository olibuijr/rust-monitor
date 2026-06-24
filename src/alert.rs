use crate::db;
use chrono::Utc;
use std::io::Write;

struct AlertRule {
    id: i64,
    name: String,
    metric_name: String,
    operator: String,
    threshold: f64,
    duration_secs: i64,
}

pub fn evaluate_alerts(alert_log_path: &str) {
    let rules = db::with_db(|conn| {
        let mut stmt = conn
            .prepare("SELECT id, name, metric_name, operator, threshold, duration_secs FROM alert_rules WHERE enabled = 1")
            .unwrap();
        stmt.query_map([], |row| {
            Ok(AlertRule {
                id: row.get(0)?,
                name: row.get(1)?,
                metric_name: row.get(2)?,
                operator: row.get(3)?,
                threshold: row.get(4)?,
                duration_secs: row.get(5)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>()
    });

    let now = Utc::now().timestamp();

    for rule in &rules {
        let cutoff = now - rule.duration_secs;

        let samples: Vec<f64> = db::with_db(|conn| {
            let mut stmt = conn
                .prepare("SELECT value FROM metrics WHERE name = ?1 AND ts >= ?2 ORDER BY ts")
                .unwrap();
            stmt.query_map(rusqlite::params![rule.metric_name, cutoff], |row| row.get(0))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect()
        });

        if samples.is_empty() {
            continue;
        }

        let all_violating = samples.iter().all(|v| violates(*v, &rule.operator, rule.threshold));
        let latest = *samples.last().unwrap();

        let has_open_alert: bool = db::with_db(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM alert_events WHERE rule_id = ?1 AND resolved_at IS NULL",
                rusqlite::params![rule.id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
                > 0
        });

        if all_violating && !has_open_alert {
            // Trigger alert
            db::with_db(|conn| {
                conn.execute(
                    "INSERT INTO alert_events (rule_id, triggered_at, value) VALUES (?1, ?2, ?3)",
                    rusqlite::params![rule.id, now, latest],
                )
                .ok();
            });

            write_alert_log(
                alert_log_path,
                &format!(
                    "ALERT rule=\"{}\" metric=\"{}\" value={:.1} threshold={:.1}",
                    rule.name, rule.metric_name, latest, rule.threshold
                ),
            );

            tracing::warn!(
                rule = rule.name,
                metric = rule.metric_name,
                value = latest,
                threshold = rule.threshold,
                "alert triggered"
            );
        } else if !all_violating && has_open_alert {
            // Resolve alert
            db::with_db(|conn| {
                conn.execute(
                    "UPDATE alert_events SET resolved_at = ?1 WHERE rule_id = ?2 AND resolved_at IS NULL",
                    rusqlite::params![now, rule.id],
                )
                .ok();
            });

            write_alert_log(
                alert_log_path,
                &format!(
                    "RESOLVED rule=\"{}\" metric=\"{}\" value={:.1}",
                    rule.name, rule.metric_name, latest
                ),
            );

            tracing::info!(rule = rule.name, metric = rule.metric_name, "alert resolved");
        }
    }
}

fn violates(value: f64, operator: &str, threshold: f64) -> bool {
    match operator {
        "gt" => value > threshold,
        "lt" => value < threshold,
        "eq" => (value - threshold).abs() < f64::EPSILON,
        _ => false,
    }
}

fn write_alert_log(path: &str, message: &str) {
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let timestamp = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{timestamp} {message}");
    }
}
