// Copyright (C) 2026 ljl8086 <ljl8086@126.com>. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (MythManDay)
// Version: V1.0
// Description: Multi-channel concurrent time-tracking & man-day billing engine.

use crate::schema::CREATE_TABLES_SQL;
use chrono::{DateTime, Utc};
use myth_core::models::{BillingType, Client, DayCalcMode, Project, QuickNote, TimeEntry};
use rusqlite::{params, Connection, Result};
use std::path::Path;
use uuid::Uuid;

pub struct Database {
    conn: Connection,
}

unsafe impl Send for Database {}
unsafe impl Sync for Database {}

impl Database {
    /// 打开或创建 SQLite 文件数据库
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        let _ = conn.busy_timeout(std::time::Duration::from_secs(5));
        let _ = conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;");
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// 打开内存数据库（用于单元测试）
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(CREATE_TABLES_SQL)?;
        let _ = self.conn.execute("ALTER TABLE time_entries ADD COLUMN audio_path TEXT", []);
        let _ = self.conn.execute("ALTER TABLE projects ADD COLUMN rounding_mode TEXT NOT NULL DEFAULT 'ceil'", []);
        let _ = self.conn.execute("ALTER TABLE projects ADD COLUMN min_granularity REAL NOT NULL DEFAULT 0.5", []);
        Ok(())
    }

    // --- 客户 CRUD ---
    pub fn insert_client(&self, client: &Client) -> Result<()> {
        self.conn.execute(
            "INSERT INTO clients (id, name, default_currency, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                client.id.to_string(),
                client.name,
                client.default_currency,
                client.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn list_clients(&self) -> Result<Vec<Client>> {
        let mut stmt = self.conn.prepare("SELECT id, name, default_currency, created_at FROM clients ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], |row| {
            let id_str: String = row.get(0)?;
            let created_at_str: String = row.get(3)?;
            Ok(Client {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                name: row.get(1)?,
                default_currency: row.get(2)?,
                created_at: DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;

        let mut clients = Vec::new();
        for r in rows {
            clients.push(r?);
        }
        Ok(clients)
    }

    // --- 项目 CRUD ---
    pub fn insert_project(&self, project: &Project) -> Result<()> {
        let billing_type_str = match project.billing_type {
            BillingType::Daily => "daily",
            BillingType::Hourly => "hourly",
            BillingType::FixedCap => "fixed_cap",
            BillingType::NonBillable => "non_billable",
        };
        let day_calc_mode_str = match project.day_calc_mode {
            DayCalcMode::HalfStep => "half_step",
            DayCalcMode::Exact => "exact",
            DayCalcMode::FullDay => "full_day",
        };
        let rounding_mode_str = match project.rounding_mode {
            myth_core::models::RoundingMode::Ceil => "ceil",
            myth_core::models::RoundingMode::HalfUp => "half_up",
            myth_core::models::RoundingMode::Floor => "floor",
        };

        self.conn.execute(
            r#"INSERT INTO projects (
                id, client_id, name, git_repo_path, billing_type, currency,
                daily_rate, hourly_rate, hours_per_day, day_calc_mode,
                rounding_mode, min_granularity,
                rounding_minutes, budget_hours, is_active, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)"#,
            params![
                project.id.to_string(),
                project.client_id.map(|id| id.to_string()),
                project.name,
                project.git_repo_path,
                billing_type_str,
                project.currency,
                project.daily_rate,
                project.hourly_rate,
                project.hours_per_day,
                day_calc_mode_str,
                rounding_mode_str,
                project.min_granularity,
                project.rounding_minutes,
                project.budget_hours,
                if project.is_active { 1 } else { 0 },
                project.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn list_projects(&self) -> Result<Vec<Project>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, client_id, name, git_repo_path, billing_type, currency,
                      daily_rate, hourly_rate, hours_per_day, day_calc_mode,
                      rounding_mode, min_granularity,
                      rounding_minutes, budget_hours, is_active, created_at
               FROM projects WHERE is_active = 1 ORDER BY created_at DESC"#,
        )?;

        let rows = stmt.query_map([], |row| {
            let id_str: String = row.get(0)?;
            let client_id_str: Option<String> = row.get(1)?;
            let billing_type_str: String = row.get(4)?;
            let day_calc_mode_str: String = row.get(9)?;
            let rounding_mode_str: Option<String> = row.get(10).ok();
            let min_granularity: f64 = row.get(11).unwrap_or(0.5);
            let is_active_int: i32 = row.get(14)?;
            let created_at_str: String = row.get(15)?;

            let billing_type = match billing_type_str.as_str() {
                "hourly" => BillingType::Hourly,
                "fixed_cap" => BillingType::FixedCap,
                "non_billable" => BillingType::NonBillable,
                _ => BillingType::Daily,
            };

            let day_calc_mode = match day_calc_mode_str.as_str() {
                "exact" => DayCalcMode::Exact,
                "full_day" => DayCalcMode::FullDay,
                _ => DayCalcMode::HalfStep,
            };

            let rounding_mode = match rounding_mode_str.as_deref() {
                Some("half_up") | Some("round") => myth_core::models::RoundingMode::HalfUp,
                Some("floor") => myth_core::models::RoundingMode::Floor,
                _ => myth_core::models::RoundingMode::Ceil,
            };

            Ok(Project {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                client_id: client_id_str.and_then(|s| Uuid::parse_str(&s).ok()),
                name: row.get(2)?,
                git_repo_path: row.get(3)?,
                billing_type,
                currency: row.get(5)?,
                daily_rate: row.get(6)?,
                hourly_rate: row.get(7)?,
                hours_per_day: row.get(8)?,
                day_calc_mode,
                rounding_mode,
                min_granularity,
                rounding_minutes: row.get(12)?,
                budget_hours: row.get(13)?,
                is_active: is_active_int == 1,
                created_at: DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;

        let mut projects = Vec::new();
        for r in rows {
            projects.push(r?);
        }
        Ok(projects)
    }

    pub fn get_project(&self, id: &Uuid) -> Result<Option<Project>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, client_id, name, git_repo_path, billing_type, currency,
                      daily_rate, hourly_rate, hours_per_day, day_calc_mode,
                      rounding_mode, min_granularity,
                      rounding_minutes, budget_hours, is_active, created_at
               FROM projects WHERE id = ?1"#,
        )?;

        let mut rows = stmt.query(params![id.to_string()])?;
        if let Some(row) = rows.next()? {
            let id_str: String = row.get(0)?;
            let client_id_str: Option<String> = row.get(1)?;
            let billing_type_str: String = row.get(4)?;
            let day_calc_mode_str: String = row.get(9)?;
            let rounding_mode_str: Option<String> = row.get(10).ok();
            let min_granularity: f64 = row.get(11).unwrap_or(0.5);
            let is_active_int: i32 = row.get(14)?;
            let created_at_str: String = row.get(15)?;

            let billing_type = match billing_type_str.as_str() {
                "hourly" => BillingType::Hourly,
                "fixed_cap" => BillingType::FixedCap,
                "non_billable" => BillingType::NonBillable,
                _ => BillingType::Daily,
            };

            let day_calc_mode = match day_calc_mode_str.as_str() {
                "exact" => DayCalcMode::Exact,
                "full_day" => DayCalcMode::FullDay,
                _ => DayCalcMode::HalfStep,
            };

            let rounding_mode = match rounding_mode_str.as_deref() {
                Some("half_up") | Some("round") => myth_core::models::RoundingMode::HalfUp,
                Some("floor") => myth_core::models::RoundingMode::Floor,
                _ => myth_core::models::RoundingMode::Ceil,
            };

            Ok(Some(Project {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                client_id: client_id_str.and_then(|s| Uuid::parse_str(&s).ok()),
                name: row.get(2)?,
                git_repo_path: row.get(3)?,
                billing_type,
                currency: row.get(5)?,
                daily_rate: row.get(6)?,
                hourly_rate: row.get(7)?,
                hours_per_day: row.get(8)?,
                day_calc_mode,
                rounding_mode,
                min_granularity,
                rounding_minutes: row.get(12)?,
                budget_hours: row.get(13)?,
                is_active: is_active_int == 1,
                created_at: DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn update_project(&self, project: &Project) -> Result<()> {
        let billing_type_str = match project.billing_type {
            BillingType::Daily => "daily",
            BillingType::Hourly => "hourly",
            BillingType::FixedCap => "fixed_cap",
            BillingType::NonBillable => "non_billable",
        };
        let day_calc_mode_str = match project.day_calc_mode {
            DayCalcMode::HalfStep => "half_step",
            DayCalcMode::Exact => "exact",
            DayCalcMode::FullDay => "full_day",
        };
        let rounding_mode_str = match project.rounding_mode {
            myth_core::models::RoundingMode::Ceil => "ceil",
            myth_core::models::RoundingMode::HalfUp => "half_up",
            myth_core::models::RoundingMode::Floor => "floor",
        };

        self.conn.execute(
            r#"UPDATE projects SET
                client_id = ?1, name = ?2, git_repo_path = ?3, billing_type = ?4,
                currency = ?5, daily_rate = ?6, hourly_rate = ?7, hours_per_day = ?8,
                day_calc_mode = ?9, rounding_mode = ?10, min_granularity = ?11,
                rounding_minutes = ?12, budget_hours = ?13, is_active = ?14
               WHERE id = ?15"#,
            params![
                project.client_id.map(|id| id.to_string()),
                project.name,
                project.git_repo_path,
                billing_type_str,
                project.currency,
                project.daily_rate,
                project.hourly_rate,
                project.hours_per_day,
                day_calc_mode_str,
                rounding_mode_str,
                project.min_granularity,
                project.rounding_minutes,
                project.budget_hours,
                if project.is_active { 1 } else { 0 },
                project.id.to_string(),
            ],
        )?;
        Ok(())
    }

    // --- 工时流水 CRUD ---
    pub fn insert_time_entry(&self, entry: &TimeEntry) -> Result<()> {
        self.conn.execute(
            r#"INSERT OR REPLACE INTO time_entries (
                id, project_id, start_time, end_time, raw_duration_secs,
                billable_hours, billable_days, rate_at_moment, total_amount,
                summary, audio_path, is_invoiced, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"#,
            params![
                entry.id.to_string(),
                entry.project_id.to_string(),
                entry.start_time.to_rfc3339(),
                entry.end_time.to_rfc3339(),
                entry.raw_duration_secs as i64,
                entry.billable_hours,
                entry.billable_days,
                entry.rate_at_moment,
                entry.total_amount,
                entry.summary,
                entry.audio_path,
                if entry.is_invoiced { 1 } else { 0 },
                entry.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn get_time_entry(&self, id: &Uuid) -> Result<Option<TimeEntry>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, project_id, start_time, end_time, raw_duration_secs,
                      billable_hours, billable_days, rate_at_moment, total_amount,
                      summary, audio_path, is_invoiced, created_at
               FROM time_entries WHERE id = ?1"#,
        )?;

        let mut rows = stmt.query(params![id.to_string()])?;
        if let Some(row) = rows.next()? {
            let id_str: String = row.get(0)?;
            let proj_id_str: String = row.get(1)?;
            let start_str: String = row.get(2)?;
            let end_str: String = row.get(3)?;
            let raw_secs: i64 = row.get(4)?;
            let is_invoiced_int: i32 = row.get(11)?;
            let created_at_str: String = row.get(12)?;

            Ok(Some(TimeEntry {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                project_id: Uuid::parse_str(&proj_id_str).unwrap_or_default(),
                start_time: DateTime::parse_from_rfc3339(&start_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                end_time: DateTime::parse_from_rfc3339(&end_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                raw_duration_secs: raw_secs.max(0) as u64,
                billable_hours: row.get(5)?,
                billable_days: row.get(6)?,
                rate_at_moment: row.get(7)?,
                total_amount: row.get(8)?,
                summary: row.get(9)?,
                audio_path: row.get(10)?,
                is_invoiced: is_invoiced_int == 1,
                created_at: DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn list_entries_for_project(&self, project_id: &Uuid) -> Result<Vec<TimeEntry>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, project_id, start_time, end_time, raw_duration_secs,
                      billable_hours, billable_days, rate_at_moment, total_amount,
                      summary, audio_path, is_invoiced, created_at
               FROM time_entries WHERE project_id = ?1 ORDER BY start_time DESC"#,
        )?;

        let rows = stmt.query_map(params![project_id.to_string()], |row| {
            let id_str: String = row.get(0)?;
            let proj_id_str: String = row.get(1)?;
            let start_str: String = row.get(2)?;
            let end_str: String = row.get(3)?;
            let raw_secs: i64 = row.get(4)?;
            let is_invoiced_int: i32 = row.get(11)?;
            let created_at_str: String = row.get(12)?;

            Ok(TimeEntry {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                project_id: Uuid::parse_str(&proj_id_str).unwrap_or_default(),
                start_time: DateTime::parse_from_rfc3339(&start_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                end_time: DateTime::parse_from_rfc3339(&end_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                raw_duration_secs: raw_secs.max(0) as u64,
                billable_hours: row.get(5)?,
                billable_days: row.get(6)?,
                rate_at_moment: row.get(7)?,
                total_amount: row.get(8)?,
                summary: row.get(9)?,
                audio_path: row.get(10)?,
                is_invoiced: is_invoiced_int == 1,
                created_at: DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;

        let mut entries = Vec::new();
        for r in rows {
            entries.push(r?);
        }
        Ok(entries)
    }

    pub fn set_entry_invoiced(&self, entry_id: &Uuid, is_invoiced: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE time_entries SET is_invoiced = ?1 WHERE id = ?2",
            params![if is_invoiced { 1 } else { 0 }, entry_id.to_string()],
        )?;
        Ok(())
    }

    pub fn batch_set_entries_invoiced(&self, entry_ids: &[Uuid], is_invoiced: bool) -> Result<()> {
        let val = if is_invoiced { 1 } else { 0 };
        for id in entry_ids {
            self.conn.execute(
                "UPDATE time_entries SET is_invoiced = ?1 WHERE id = ?2",
                params![val, id.to_string()],
            )?;
        }
        Ok(())
    }

    // --- 便签 CRUD ---
    pub fn insert_quick_note(&self, note: &QuickNote) -> Result<()> {
        self.conn.execute(
            "INSERT INTO quick_notes (id, project_id, logged_at, content) VALUES (?1, ?2, ?3, ?4)",
            params![
                note.id.to_string(),
                note.project_id.to_string(),
                note.logged_at.to_rfc3339(),
                note.content,
            ],
        )?;
        Ok(())
    }

    pub fn list_quick_notes(&self, project_id: &Uuid) -> Result<Vec<QuickNote>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_id, logged_at, content FROM quick_notes WHERE project_id = ?1 ORDER BY logged_at DESC"
        )?;

        let rows = stmt.query_map(params![project_id.to_string()], |row| {
            let id_str: String = row.get(0)?;
            let proj_id_str: String = row.get(1)?;
            let logged_at_str: String = row.get(2)?;

            Ok(QuickNote {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                project_id: Uuid::parse_str(&proj_id_str).unwrap_or_default(),
                logged_at: DateTime::parse_from_rfc3339(&logged_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                content: row.get(3)?,
            })
        })?;

        let mut notes = Vec::new();
        for r in rows {
            notes.push(r?);
        }
        Ok(notes)
    }

    pub fn list_all_quick_notes(&self) -> Result<Vec<QuickNote>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_id, logged_at, content FROM quick_notes ORDER BY logged_at DESC"
        )?;

        let rows = stmt.query_map([], |row| {
            let id_str: String = row.get(0)?;
            let proj_id_str: String = row.get(1)?;
            let logged_at_str: String = row.get(2)?;

            Ok(QuickNote {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                project_id: Uuid::parse_str(&proj_id_str).unwrap_or_default(),
                logged_at: DateTime::parse_from_rfc3339(&logged_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                content: row.get(3)?,
            })
        })?;

        let mut notes = Vec::new();
        for r in rows {
            notes.push(r?);
        }
        Ok(notes)
    }

    pub fn delete_quick_note(&self, id: &Uuid) -> Result<()> {
        self.conn.execute("DELETE FROM quick_notes WHERE id = ?1", params![id.to_string()])?;
        Ok(())
    }

    pub fn delete_project(&self, id: &Uuid) -> Result<()> {
        self.conn.execute("DELETE FROM projects WHERE id = ?1", params![id.to_string()])?;
        Ok(())
    }

    // --- 系统设置 Key-Value ---
    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare("SELECT value FROM app_settings WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    // --- 活动通道持久化 (崩溃/断电恢复) ---
    pub fn upsert_active_timer(&self, record: &ActiveTimerRecord) -> Result<()> {
        self.conn.execute(
            r#"INSERT OR REPLACE INTO active_timers (
                project_id, session_start_at, current_started_at,
                accumulated_secs, is_paused, last_heartbeat
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
            params![
                record.project_id.to_string(),
                record.session_start_at.to_rfc3339(),
                record.current_started_at.map(|dt| dt.to_rfc3339()),
                record.accumulated_secs as i64,
                if record.is_paused { 1 } else { 0 },
                record.last_heartbeat.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn delete_active_timer(&self, project_id: &Uuid) -> Result<()> {
        self.conn.execute(
            "DELETE FROM active_timers WHERE project_id = ?1",
            params![project_id.to_string()],
        )?;
        Ok(())
    }

    pub fn list_active_timers(&self) -> Result<Vec<ActiveTimerRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT project_id, session_start_at, current_started_at, accumulated_secs, is_paused, last_heartbeat FROM active_timers"
        )?;

        let rows = stmt.query_map([], |row| {
            let pid_str: String = row.get(0)?;
            let s_start_str: String = row.get(1)?;
            let c_start_str: Option<String> = row.get(2)?;
            let acc_secs: i64 = row.get(3)?;
            let is_paused_int: i32 = row.get(4)?;
            let hb_str: String = row.get(5)?;

            let session_start_at = DateTime::parse_from_rfc3339(&s_start_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            let current_started_at = c_start_str.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
            });

            let last_heartbeat = DateTime::parse_from_rfc3339(&hb_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(ActiveTimerRecord {
                project_id: Uuid::parse_str(&pid_str).unwrap_or_default(),
                session_start_at,
                current_started_at,
                accumulated_secs: acc_secs.max(0) as u64,
                is_paused: is_paused_int != 0,
                last_heartbeat,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }
}

#[derive(Debug, Clone)]
pub struct ActiveTimerRecord {
    pub project_id: Uuid,
    pub session_start_at: DateTime<Utc>,
    pub current_started_at: Option<DateTime<Utc>>,
    pub accumulated_secs: u64,
    pub is_paused: bool,
    pub last_heartbeat: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_lifecycle() {
        let db = Database::open_in_memory().unwrap();

        // 插入项目
        let mut project = Project::new("测试电商项目");
        project.daily_rate = 2500.0;
        db.insert_project(&project).unwrap();

        let list = db.list_projects().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "测试电商项目");
        assert_eq!(list[0].daily_rate, 2500.0);

        // 插入便签
        let note = QuickNote::new(project.id, "电话讨论支付回调逻辑");
        db.insert_quick_note(&note).unwrap();

        let notes = db.list_quick_notes(&project.id).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].content, "电话讨论支付回调逻辑");

        // 插入带录音的工时条目
        let entry1 = TimeEntry {
            id: Uuid::new_v4(),
            project_id: project.id,
            start_time: Utc::now(),
            end_time: Utc::now(),
            raw_duration_secs: 3600,
            billable_hours: 1.0,
            billable_days: 0.125,
            rate_at_moment: 312.5,
            total_amount: 312.5,
            summary: Some("技术方案对齐与决议研讨".to_string()),
            audio_path: Some("/tmp/meeting_rec.wav".to_string()),
            is_invoiced: false,
            created_at: Utc::now(),
        };
        db.insert_time_entry(&entry1).unwrap();

        // 插入不带录音的常规工时条目
        let entry2 = TimeEntry {
            id: Uuid::new_v4(),
            project_id: project.id,
            start_time: Utc::now(),
            end_time: Utc::now(),
            raw_duration_secs: 1800,
            billable_hours: 0.5,
            billable_days: 0.0625,
            rate_at_moment: 312.5,
            total_amount: 156.25,
            summary: Some("功能模块代码编写".to_string()),
            audio_path: None,
            is_invoiced: false,
            created_at: Utc::now(),
        };
        db.insert_time_entry(&entry2).unwrap();

        // 验证读取
        let fetched1 = db.get_time_entry(&entry1.id).unwrap().unwrap();
        assert_eq!(fetched1.audio_path, Some("/tmp/meeting_rec.wav".to_string()));
        assert_eq!(fetched1.summary.as_deref(), Some("技术方案对齐与决议研讨"));

        let fetched2 = db.get_time_entry(&entry2.id).unwrap().unwrap();
        assert_eq!(fetched2.audio_path, None);
        assert_eq!(fetched2.summary.as_deref(), Some("功能模块代码编写"));

        let entries = db.list_entries_for_project(&project.id).unwrap();
        assert_eq!(entries.len(), 2);
    }
}
