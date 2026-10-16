// Copyright (C) 2026 ljl8086 <ljl8086@gmail.com>. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (MythManDay)
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
    rounding_mode TEXT NOT NULL DEFAULT 'ceil',
    min_granularity REAL NOT NULL DEFAULT 0.5,
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
    audio_path TEXT,
    is_invoiced INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS quick_notes (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    logged_at TEXT NOT NULL,
    content TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS active_timers (
    project_id TEXT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    session_start_at TEXT NOT NULL,
    current_started_at TEXT,
    accumulated_secs INTEGER NOT NULL DEFAULT 0,
    is_paused INTEGER NOT NULL DEFAULT 0,
    last_heartbeat TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS work_spans (
    id TEXT PRIMARY KEY,
    entry_id TEXT NOT NULL REFERENCES time_entries(id) ON DELETE CASCADE,
    span_order INTEGER NOT NULL DEFAULT 1,
    start_at TEXT NOT NULL,
    end_at TEXT NOT NULL,
    duration_secs INTEGER NOT NULL,
    span_type TEXT NOT NULL DEFAULT 'timer',
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS meetings (
    id TEXT PRIMARY KEY,
    entry_id TEXT NOT NULL REFERENCES time_entries(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    topic TEXT NOT NULL,
    start_at TEXT NOT NULL,
    end_at TEXT,
    duration_secs INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'in_progress',
    meeting_summary TEXT,
    quick_notes TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS meeting_audio_clips (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    clip_order INTEGER NOT NULL DEFAULT 1,
    audio_path TEXT NOT NULL,
    duration_secs INTEGER NOT NULL DEFAULT 0,
    transcript TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS meeting_materials (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    file_name TEXT NOT NULL,
    file_type TEXT NOT NULL DEFAULT 'document',
    file_path TEXT NOT NULL,
    file_size INTEGER NOT NULL DEFAULT 0,
    extracted_text TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_time_entries_project_start ON time_entries (project_id, start_time);
CREATE INDEX IF NOT EXISTS idx_work_spans_entry_id ON work_spans (entry_id);
CREATE INDEX IF NOT EXISTS idx_meetings_entry_id ON meetings (entry_id);
CREATE INDEX IF NOT EXISTS idx_meetings_project_id ON meetings (project_id);
CREATE INDEX IF NOT EXISTS idx_meeting_clips_meeting_id ON meeting_audio_clips (meeting_id);
CREATE INDEX IF NOT EXISTS idx_meeting_materials_meeting_id ON meeting_materials (meeting_id);
CREATE INDEX IF NOT EXISTS idx_quick_notes_project_id ON quick_notes (project_id);
"#;

