// Copyright (C) 2026 ParaDay Authors. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (ParaDay)
// Version: V1.0
// Description: Multi-channel concurrent time-tracking & man-day billing engine.

use crate::schema::CREATE_TABLES_SQL;
use chrono::{DateTime, Utc};
use paraday_core::models::{BillingType, Client, DayCalcMode, Project, QuickNote, TimeEntry};
use rusqlite::{params, Connection, Result};
use std::path::Path;
use uuid::Uuid;

pub struct Database {
    conn: Connection,
}

impl Database {
    /// 打开或创建 SQLite 文件数据库
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
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

        self.conn.execute(
            r#"INSERT INTO projects (
                id, client_id, name, git_repo_path, billing_type, currency,
                daily_rate, hourly_rate, hours_per_day, day_calc_mode,
                rounding_minutes, budget_hours, is_active, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"#,
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
                      rounding_minutes, budget_hours, is_active, created_at
               FROM projects WHERE is_active = 1 ORDER BY created_at DESC"#,
        )?;

        let rows = stmt.query_map([], |row| {
            let id_str: String = row.get(0)?;
            let client_id_str: Option<String> = row.get(1)?;
            let billing_type_str: String = row.get(4)?;
            let day_calc_mode_str: String = row.get(9)?;
            let is_active_int: i32 = row.get(12)?;
            let created_at_str: String = row.get(13)?;

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
                rounding_minutes: row.get(10)?,
                budget_hours: row.get(11)?,
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

    // --- 工时流水 CRUD ---
    pub fn insert_time_entry(&self, entry: &TimeEntry) -> Result<()> {
        self.conn.execute(
            r#"INSERT INTO time_entries (
                id, project_id, start_time, end_time, raw_duration_secs,
                billable_hours, billable_days, rate_at_moment, total_amount,
                summary, is_invoiced, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"#,
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
                if entry.is_invoiced { 1 } else { 0 },
                entry.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn list_entries_for_project(&self, project_id: &Uuid) -> Result<Vec<TimeEntry>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, project_id, start_time, end_time, raw_duration_secs,
                      billable_hours, billable_days, rate_at_moment, total_amount,
                      summary, is_invoiced, created_at
               FROM time_entries WHERE project_id = ?1 ORDER BY start_time DESC"#,
        )?;

        let rows = stmt.query_map(params![project_id.to_string()], |row| {
            let id_str: String = row.get(0)?;
            let proj_id_str: String = row.get(1)?;
            let start_str: String = row.get(2)?;
            let end_str: String = row.get(3)?;
            let raw_secs: i64 = row.get(4)?;
            let is_invoiced_int: i32 = row.get(10)?;
            let created_at_str: String = row.get(11)?;

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
    }
}
