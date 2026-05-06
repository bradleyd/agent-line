use crate::state::{DeployEvent, LogLine, MetricPoint};

pub fn logs() -> Vec<LogLine> {
    vec![
        LogLine {
            minute: 0,
            service: "checkout",
            level: "INFO",
            message: "request volume steady at 820 rpm",
        },
        LogLine {
            minute: 2,
            service: "checkout",
            level: "INFO",
            message: "deployed checkout v1.18.0",
        },
        LogLine {
            minute: 4,
            service: "checkout",
            level: "WARN",
            message: "inventory quote latency exceeded 750ms",
        },
        LogLine {
            minute: 5,
            service: "inventory",
            level: "WARN",
            message: "database connection pool wait exceeded 500ms",
        },
        LogLine {
            minute: 6,
            service: "checkout",
            level: "ERROR",
            message: "cart submit failed: inventory quote timeout",
        },
        LogLine {
            minute: 7,
            service: "inventory",
            level: "ERROR",
            message: "pool exhausted: 64 active connections, 0 idle",
        },
        LogLine {
            minute: 8,
            service: "payments",
            level: "INFO",
            message: "authorization latency unchanged",
        },
        LogLine {
            minute: 9,
            service: "gateway",
            level: "WARN",
            message: "checkout upstream 5xx rate above alert threshold",
        },
        LogLine {
            minute: 11,
            service: "checkout",
            level: "INFO",
            message: "feature flag inventory_parallel_quotes disabled by operator",
        },
        LogLine {
            minute: 13,
            service: "inventory",
            level: "INFO",
            message: "pool pressure recovered: 21 active connections, 43 idle",
        },
    ]
}

pub fn deploys() -> Vec<DeployEvent> {
    vec![
        DeployEvent {
            minute: 2,
            service: "checkout",
            version: "v1.18.0",
            summary: "enabled parallel inventory quote requests for cart submit",
        },
        DeployEvent {
            minute: 18,
            service: "search",
            version: "v4.3.7",
            summary: "index ranking tweak after the incident window",
        },
    ]
}

pub fn metrics() -> Vec<MetricPoint> {
    let mut points = Vec::new();

    for minute in 0..15 {
        let incident = (4..=10).contains(&minute);
        let recovery = (11..=14).contains(&minute);

        points.push(MetricPoint {
            minute,
            service: "checkout",
            metric: "5xx_rate_pct",
            value: if incident {
                7.8 + f64::from(minute - 4) * 0.4
            } else if recovery {
                1.7 - f64::from(minute - 11) * 0.3
            } else {
                0.3
            },
        });

        points.push(MetricPoint {
            minute,
            service: "checkout",
            metric: "p95_latency_ms",
            value: if incident {
                940.0 + f64::from(minute - 4) * 35.0
            } else if recovery {
                480.0 - f64::from(minute - 11) * 40.0
            } else {
                220.0
            },
        });

        points.push(MetricPoint {
            minute,
            service: "inventory",
            metric: "db_pool_wait_ms",
            value: if incident {
                610.0 + f64::from(minute - 4) * 80.0
            } else if recovery {
                180.0 - f64::from(minute - 11) * 35.0
            } else {
                24.0
            },
        });

        points.push(MetricPoint {
            minute,
            service: "inventory",
            metric: "active_db_connections",
            value: if incident {
                58.0 + f64::from(minute - 4)
            } else if recovery {
                37.0 - f64::from(minute - 11) * 4.0
            } else {
                18.0
            },
        });

        points.push(MetricPoint {
            minute,
            service: "payments",
            metric: "p95_latency_ms",
            value: if incident { 225.0 } else { 215.0 },
        });
    }

    points
}
