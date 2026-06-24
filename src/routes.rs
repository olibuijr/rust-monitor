use axum::extract::Query;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::db;

pub async fn health() -> impl IntoResponse {
    Json(json!({"status": "ok"}))
}

#[derive(Serialize)]
struct MetricPoint {
    name: String,
    value: f64,
    ts: i64,
}

pub async fn status() -> impl IntoResponse {
    // Latest value of each metric
    let metrics = db::with_db(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT name, value, ts FROM metrics WHERE id IN (SELECT MAX(id) FROM metrics GROUP BY name) ORDER BY name",
            )
            .unwrap();
        stmt.query_map([], |row| {
            Ok(MetricPoint {
                name: row.get(0)?,
                value: row.get(1)?,
                ts: row.get(2)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>()
    });

    Json(json!({"metrics": metrics}))
}

#[derive(Deserialize)]
pub struct MetricsQuery {
    pub name: Option<String>,
    pub hours: Option<i64>,
}

pub async fn metrics(Query(q): Query<MetricsQuery>) -> impl IntoResponse {
    let hours = q.hours.unwrap_or(24);
    let cutoff = chrono::Utc::now().timestamp() - (hours * 3600);

    let metrics = db::with_db(|conn| {
        if let Some(name) = &q.name {
            let mut stmt = conn
                .prepare("SELECT name, value, ts FROM metrics WHERE name = ?1 AND ts >= ?2 ORDER BY ts")
                .unwrap();
            stmt.query_map(rusqlite::params![name, cutoff], |row| {
                Ok(MetricPoint {
                    name: row.get(0)?,
                    value: row.get(1)?,
                    ts: row.get(2)?,
                })
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect::<Vec<_>>()
        } else {
            let mut stmt = conn
                .prepare("SELECT name, value, ts FROM metrics WHERE ts >= ?1 ORDER BY ts LIMIT 1000")
                .unwrap();
            stmt.query_map(rusqlite::params![cutoff], |row| {
                Ok(MetricPoint {
                    name: row.get(0)?,
                    value: row.get(1)?,
                    ts: row.get(2)?,
                })
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect::<Vec<_>>()
        }
    });

    Json(json!({"metrics": metrics, "count": metrics.len()}))
}

#[derive(Deserialize)]
pub struct LogsQuery {
    pub source: Option<String>,
    pub hours: Option<i64>,
}

#[derive(Serialize)]
struct LogEntry {
    source: String,
    line: String,
    ts: i64,
}

pub async fn logs(Query(q): Query<LogsQuery>) -> impl IntoResponse {
    let hours = q.hours.unwrap_or(1);
    let cutoff = chrono::Utc::now().timestamp() - (hours * 3600);

    let entries = db::with_db(|conn| {
        if let Some(source) = &q.source {
            let mut stmt = conn
                .prepare("SELECT source, line, ts FROM logs WHERE source = ?1 AND ts >= ?2 ORDER BY ts DESC LIMIT 500")
                .unwrap();
            stmt.query_map(rusqlite::params![source, cutoff], |row| {
                Ok(LogEntry {
                    source: row.get(0)?,
                    line: row.get(1)?,
                    ts: row.get(2)?,
                })
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect::<Vec<_>>()
        } else {
            let mut stmt = conn
                .prepare("SELECT source, line, ts FROM logs WHERE ts >= ?1 ORDER BY ts DESC LIMIT 500")
                .unwrap();
            stmt.query_map(rusqlite::params![cutoff], |row| {
                Ok(LogEntry {
                    source: row.get(0)?,
                    line: row.get(1)?,
                    ts: row.get(2)?,
                })
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect::<Vec<_>>()
        }
    });

    Json(json!({"logs": entries, "count": entries.len()}))
}

#[derive(Deserialize)]
pub struct AlertsQuery {
    pub hours: Option<i64>,
}

#[derive(Serialize)]
struct AlertEvent {
    id: i64,
    rule_name: String,
    metric_name: String,
    threshold: f64,
    value: f64,
    triggered_at: i64,
    resolved_at: Option<i64>,
}

pub async fn alerts(Query(q): Query<AlertsQuery>) -> impl IntoResponse {
    let hours = q.hours.unwrap_or(24);
    let cutoff = chrono::Utc::now().timestamp() - (hours * 3600);

    let events = db::with_db(|conn| {
        let mut stmt = conn
            .prepare(
                "SELECT e.id, r.name, r.metric_name, r.threshold, e.value, e.triggered_at, e.resolved_at
                 FROM alert_events e JOIN alert_rules r ON e.rule_id = r.id
                 WHERE e.triggered_at >= ?1 OR e.resolved_at IS NULL
                 ORDER BY e.triggered_at DESC",
            )
            .unwrap();
        stmt.query_map(rusqlite::params![cutoff], |row| {
            Ok(AlertEvent {
                id: row.get(0)?,
                rule_name: row.get(1)?,
                metric_name: row.get(2)?,
                threshold: row.get(3)?,
                value: row.get(4)?,
                triggered_at: row.get(5)?,
                resolved_at: row.get(6)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>()
    });

    let active = events.iter().filter(|e| e.resolved_at.is_none()).count();
    Json(json!({"alerts": events, "active": active, "total": events.len()}))
}

