// Copyright (C) 2026 ParaDay Authors. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (ParaDay)
// Version: V1.0
// Description: Multi-channel concurrent time-tracking & man-day billing engine.

pub const CREATE_TABLES_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS clients (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    default_currency TEXT NOT NULL DEFAULT 'CNY',
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    client_id TEXT REFERENCES clients(id) ON DELETE SET NULL,
    name TEXT NOT NULL,
    git_repo_path TEXT,
    billing_type TEXT NOT NULL DEFAULT 'daily',
    currency TEXT NOT NULL DEFAULT 'CNY',
    daily_rate REAL NOT NULL DEFAULT 0.0,
    hourly_rate REAL NOT NULL DEFAULT 0.0,
    hours_per_day REAL NOT NULL DEFAULT 8.0,
    day_calc_mode TEXT NOT NULL DEFAULT 'half_step',
    rounding_minutes INTEGER NOT NULL DEFAULT 1,
    budget_hours REAL NOT NULL DEFAULT 0.0,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS time_entries (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    start_time TEXT NOT NULL,
    end_time TEXT NOT NULL,
    raw_duration_secs INTEGER NOT NULL,
    billable_hours REAL NOT NULL,
    billable_days REAL NOT NULL,
    rate_at_moment REAL NOT NULL,
    total_amount REAL NOT NULL,
    summary TEXT,
    is_invoiced INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS quick_notes (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    logged_at TEXT NOT NULL,
    content TEXT NOT NULL
);
"#;
