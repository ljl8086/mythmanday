// Copyright (C) 2026 ljl8086 <ljl8086@126.com>. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (MythManDay - Tauri Host)
// Version: V1.0

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::{Local, TimeZone, Utc};
use myth_ai::{AiConfig, AiSummarizer};
use myth_audio::{AudioPlayer, AudioRecorder};
use myth_core::billing::BillingEngine;
use myth_core::models::{BillingType, DayCalcMode, Project, QuickNote, TimeEntry};
use myth_core::timer::MultiChannelManager;
use myth_db::{ActiveTimerRecord, Database};
use myth_git::GitScanner;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

pub struct AppState {
    pub db: Arc<Mutex<Database>>,
    pub timer_manager: Arc<Mutex<MultiChannelManager>>,
    pub audio_recorder: Arc<Mutex<AudioRecorder>>,
    pub audio_player: Arc<Mutex<AudioPlayer>>,
    pub active_recording: Arc<Mutex<Option<(Uuid, String, std::path::PathBuf)>>>,
    pub db_dir: std::path::PathBuf,
}

unsafe impl Send for AppState {}
unsafe impl Sync for AppState {}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProjectDto {
    pub id: String,
    pub name: String,
    pub client: String,
    pub rate_info: String,
    pub timer_text: String,
    pub is_running: bool,
    pub is_paused: bool,
    pub today_duration: String,
    pub today_days: String,
    pub entries_count: usize,
    pub currency: String,
    pub daily_rate: f64,
    pub hourly_rate: f64,
    pub billing_type: String,
    pub rounding_mode: String,
    pub min_granularity: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EntryDto {
    pub id: String,
    pub project_id: String,
    pub project_name: String,
    pub duration_text: String,
    pub days_text: String,
    pub amount_text: String,
    pub summary_text: String,
    pub time_range: String,
    pub date_str: String,
    pub has_audio: bool,
    pub audio_path: String,
    pub raw_secs: u64,
    pub is_invoiced: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DayGroupDto {
    pub date_str: String,
    pub display_date: String,
    pub entries_count: usize,
    pub total_duration: String,
    pub total_days: String,
    pub total_amount: String,
    pub entries: Vec<EntryDto>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GlobalStatsDto {
    pub total_days: String,
    pub total_amount: String,
    pub running_count: usize,
    pub is_recording_audio: bool,
    pub active_recording_project_name: String,
    pub is_audio_paused: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NoteDto {
    pub id: String,
    pub project_id: String,
    pub content: String,
    pub time_text: String,
}

fn format_duration(secs: u64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

fn get_client_name(db: &Database, client_id: Option<Uuid>) -> String {
    if let Some(cid) = client_id {
        if let Ok(clients) = db.list_clients() {
            if let Some(c) = clients.iter().find(|c| c.id == cid) {
                return c.name.clone();
            }
        }
    }
    "商业客户".to_string()
}

fn dirs_or_local_data_dir() -> std::path::PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        std::path::PathBuf::from(home).join(".config").join("mythmanday")
    } else {
        std::path::PathBuf::from("./data")
    }
}

// ======================= Tauri Commands =======================

#[tauri::command]
fn get_projects(state: State<AppState>) -> Result<Vec<ProjectDto>, String> {
    let projects = state.db.lock().unwrap().list_projects().map_err(|e| e.to_string())?;
    let mgr = state.timer_manager.lock().unwrap();
    let mut list = Vec::new();

    for p in projects {
        let status = mgr.get_status(&p.id);
        let is_running = matches!(status, myth_core::timer::TimerStatus::Running { .. });
        let is_paused = matches!(status, myth_core::timer::TimerStatus::Paused { .. });
        let elapsed = mgr.get_elapsed(&p.id);

        let entries = state.db.lock().unwrap().list_entries_for_project(&p.id).unwrap_or_default();
        let today_str = Local::now().format("%Y-%m-%d").to_string();
        let today_entries: Vec<_> = entries
            .iter()
            .filter(|e| e.start_time.with_timezone(&Local).format("%Y-%m-%d").to_string() == today_str)
            .collect();
        let today_secs: u64 = today_entries.iter().map(|e| e.raw_duration_secs).sum();
        let today_days: f64 = today_entries.iter().map(|e| e.billable_days).sum();

        let rounding_desc = match p.rounding_mode {
            myth_core::models::RoundingMode::Ceil => "进位",
            myth_core::models::RoundingMode::HalfUp => "四舍五入",
            myth_core::models::RoundingMode::Floor => "舍去",
        };

        let rate_info = match p.billing_type {
            BillingType::Daily => {
                let r = (p.daily_rate / p.hours_per_day.max(1.0)).round();
                format!(
                    "{:.0} {}/天 (约{:.0}/h · 最小{:.1}天 · {})",
                    p.daily_rate, p.currency, r, p.min_granularity, rounding_desc
                )
            }
            BillingType::Hourly => {
                format!(
                    "{:.0} {}/小时 (最小{:.1}h · {})",
                    p.hourly_rate, p.currency, p.min_granularity, rounding_desc
                )
            }
            BillingType::FixedCap => format!("固定总包 {:.0} {}", p.daily_rate, p.currency),
            BillingType::NonBillable => "自用/不计费".to_string(),
        };

        list.push(ProjectDto {
            id: p.id.to_string(),
            name: p.name,
            client: get_client_name(&state.db.lock().unwrap(), p.client_id),
            rate_info,
            timer_text: format_duration(elapsed),
            is_running,
            is_paused,
            today_duration: format_duration(today_secs),
            today_days: format!("{:.2} 天", today_days),
            entries_count: entries.len(),
            currency: p.currency,
            daily_rate: p.daily_rate,
            hourly_rate: p.hourly_rate,
            billing_type: match p.billing_type {
                BillingType::Hourly => "hourly".to_string(),
                BillingType::FixedCap => "fixed_cap".to_string(),
                BillingType::NonBillable => "non_billable".to_string(),
                BillingType::Daily => "daily".to_string(),
            },
            rounding_mode: match p.rounding_mode {
                myth_core::models::RoundingMode::HalfUp => "half_up".to_string(),
                myth_core::models::RoundingMode::Floor => "floor".to_string(),
                myth_core::models::RoundingMode::Ceil => "ceil".to_string(),
            },
            min_granularity: p.min_granularity,
        });
    }

    Ok(list)
}

#[tauri::command]
fn get_daily_groups(
    state: State<AppState>,
    project_id: String,
    month: Option<String>,
) -> Result<Vec<DayGroupDto>, String> {
    let pid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let entries = state.db.lock().unwrap().list_entries_for_project(&pid).unwrap_or_default();
    let projs = state.db.lock().unwrap().list_projects().unwrap_or_default();
    let proj = projs.iter().find(|p| p.id == pid);
    let currency = proj.map(|p| p.currency.as_str()).unwrap_or("CNY");

    let mut days_map: BTreeMap<String, Vec<EntryDto>> = BTreeMap::new();
    for e in entries {
        let start_local = e.start_time.with_timezone(&Local);
        let end_local = e.end_time.with_timezone(&Local);
        let date_str = start_local.format("%Y-%m-%d").to_string();

        // 年月过滤：如果指定了 month (如 "2026-10")，只保留匹配月份
        if let Some(ref m) = month {
            if !m.is_empty() && !date_str.starts_with(m) {
                continue;
            }
        }

        let audio_path = e.audio_path.clone().unwrap_or_default();
        let has_audio = !audio_path.is_empty() && std::path::Path::new(&audio_path).exists();

        let item = EntryDto {
            id: e.id.to_string(),
            project_id: project_id.clone(),
            project_name: proj.map(|p| p.name.clone()).unwrap_or_default(),
            duration_text: format_duration(e.raw_duration_secs),
            days_text: format!("{:.2} 天", e.billable_days),
            amount_text: format!("{} {:.2}", currency, e.total_amount),
            summary_text: e.summary.unwrap_or_else(|| "持续代码开发与测试交付".to_string()),
            time_range: format!("{}-{}", start_local.format("%H:%M"), end_local.format("%H:%M")),
            date_str: date_str.clone(),
            has_audio,
            audio_path,
            raw_secs: e.raw_duration_secs,
            is_invoiced: e.is_invoiced,
        };
        days_map.entry(date_str).or_default().push(item);
    }

    let today_str = Local::now().format("%Y-%m-%d").to_string();
    let mut groups = Vec::new();

    // 倒序展示：最新一天排在最前
    for (d_str, day_items) in days_map.into_iter().rev() {
        let count = day_items.len();
        let display_date = if d_str == today_str {
            format!("{} (今天)", d_str)
        } else {
            d_str.clone()
        };

        let day_secs: u64 = day_items.iter().map(|e| e.raw_secs).sum();
        let day_days: f64 = day_items
            .iter()
            .map(|e| {
                e.days_text
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0)
            })
            .sum();
        let day_amount: f64 = day_items
            .iter()
            .map(|e| {
                let parts: Vec<&str> = e.amount_text.split_whitespace().collect();
                if parts.len() >= 2 {
                    parts[1].parse::<f64>().unwrap_or(0.0)
                } else {
                    0.0
                }
            })
            .sum();

        groups.push(DayGroupDto {
            date_str: d_str,
            display_date,
            entries_count: count,
            total_duration: format_duration(day_secs),
            total_days: format!("{:.2} 天", day_days),
            total_amount: format!("{} {:.2}", currency, day_amount),
            entries: day_items,
        });
    }

    Ok(groups)
}

#[tauri::command]
fn get_global_stats(state: State<AppState>) -> Result<GlobalStatsDto, String> {
    let projs = state.db.lock().unwrap().list_projects().unwrap_or_default();
    let today_str = Local::now().format("%Y-%m-%d").to_string();
    let mut total_days = 0.0f64;
    let mut total_amount = 0.0f64;

    for p in &projs {
        if let Ok(entries) = state.db.lock().unwrap().list_entries_for_project(&p.id) {
            for e in entries {
                if e.start_time.with_timezone(&Local).format("%Y-%m-%d").to_string() == today_str {
                    total_days += e.billable_days;
                    total_amount += e.total_amount;
                }
            }
        }
    }

    let mgr = state.timer_manager.lock().unwrap();
    let running_count = mgr.active_channel_ids().len();
    let audio_rec = state.audio_recorder.lock().unwrap();
    let is_recording_audio = audio_rec.is_recording();
    let is_audio_paused = audio_rec.is_paused();
    let active_rec = state.active_recording.lock().unwrap();
    let active_recording_project_name = active_rec
        .as_ref()
        .map(|(_, topic, _)| topic.clone())
        .unwrap_or_default();

    Ok(GlobalStatsDto {
        total_days: format!("{:.2} 人天", total_days),
        total_amount: format!("¥ {:.2}", total_amount),
        running_count,
        is_recording_audio,
        active_recording_project_name,
        is_audio_paused,
    })
}

#[tauri::command]
fn start_timer(state: State<AppState>, project_id: String) -> Result<(), String> {
    let pid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let mut mgr = state.timer_manager.lock().unwrap();
    mgr.start_channel(pid);
    let now = Utc::now();
    let session_start = mgr.get_timer(&pid).and_then(|t| t.session_start_at).unwrap_or(now);
    let _ = state.db.lock().unwrap().upsert_active_timer(&ActiveTimerRecord {
        project_id: pid,
        session_start_at: session_start,
        current_started_at: Some(now),
        accumulated_secs: mgr.get_elapsed(&pid),
        is_paused: false,
        last_heartbeat: now,
    });
    Ok(())
}

#[tauri::command]
fn pause_timer(state: State<AppState>, project_id: String) -> Result<(), String> {
    let pid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let mut mgr = state.timer_manager.lock().unwrap();
    mgr.pause_channel(pid);
    let elapsed = mgr.get_elapsed(&pid);
    let session_start = mgr.get_timer(&pid).and_then(|t| t.session_start_at).unwrap_or_else(Utc::now);
    let _ = state.db.lock().unwrap().upsert_active_timer(&ActiveTimerRecord {
        project_id: pid,
        session_start_at: session_start,
        current_started_at: None,
        accumulated_secs: elapsed,
        is_paused: true,
        last_heartbeat: Utc::now(),
    });
    Ok(())
}

#[tauri::command]
fn resume_timer(state: State<AppState>, project_id: String) -> Result<(), String> {
    let pid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let mut mgr = state.timer_manager.lock().unwrap();
    mgr.start_channel(pid);
    let now = Utc::now();
    let elapsed = mgr.get_elapsed(&pid);
    let session_start = mgr.get_timer(&pid).and_then(|t| t.session_start_at).unwrap_or(now);
    let _ = state.db.lock().unwrap().upsert_active_timer(&ActiveTimerRecord {
        project_id: pid,
        session_start_at: session_start,
        current_started_at: Some(now),
        accumulated_secs: elapsed,
        is_paused: false,
        last_heartbeat: now,
    });
    Ok(())
}

#[tauri::command]
fn stop_timer(state: State<AppState>, project_id: String) -> Result<String, String> {
    let pid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let stop_res = {
        let mut mgr = state.timer_manager.lock().unwrap();
        mgr.stop_channel(pid)
    };

    if let Some((start_at, end_at, elapsed_secs)) = stop_res {
        let _ = state.db.lock().unwrap().delete_active_timer(&pid);

        // 处理录音
        let mut saved_audio: Option<String> = None;
        let mut rec_info = state.active_recording.lock().unwrap();
        if let Some((rec_pid, _, rec_path)) = rec_info.take() {
            if rec_pid == pid {
                let mut recorder = state.audio_recorder.lock().unwrap();
                if recorder.is_recording() {
                    if let Ok(p) = recorder.stop_recording() {
                        saved_audio = Some(p.display().to_string());
                    }
                } else if rec_path.exists() {
                    saved_audio = Some(rec_path.display().to_string());
                }
            } else {
                *rec_info = Some((rec_pid, "".to_string(), rec_path));
            }
        }

        let projs = state.db.lock().unwrap().list_projects().map_err(|e| e.to_string())?;
        if let Some(p) = projs.iter().find(|proj| proj.id == pid) {
            let billable_hours = BillingEngine::calculate_billable_hours(elapsed_secs, p.rounding_minutes);
            let billable_days = BillingEngine::calculate_billable_days(billable_hours, p.hours_per_day, p.day_calc_mode);
            let (rate, amount) = BillingEngine::calculate_amount(p, billable_hours, billable_days);

            let mut git_summary = myth_git::GitActivitySummary {
                commits: vec![],
                diff_stat: "".to_string(),
                has_uncommitted_changes: false,
                uncommitted_files: vec![],
            };
            if let Some(ref git_path) = p.git_repo_path {
                if let Ok(scanned) = GitScanner::extract_activity(git_path, start_at, end_at) {
                    git_summary = scanned;
                }
            }

            let notes = state.db.lock().unwrap().list_quick_notes(&pid).unwrap_or_default();
            let user_notes: Vec<&str> = notes.iter().map(|n| n.content.as_str()).filter(|s| !s.starts_with('[')).collect();

            let summary_desc = if saved_audio.is_some() || p.name.contains("会议") {
                if !user_notes.is_empty() {
                    format!("会议沟通与决议：{}", user_notes.join("；"))
                } else {
                    format!("会议沟通与技术决策：{}", p.name)
                }
            } else if !git_summary.commits.is_empty() {
                format!("代码研发与提交 {} 次：{}", git_summary.commits.len(), git_summary.commits.join("; "))
            } else if !user_notes.is_empty() {
                format!("阶段性研发与联调：{}", user_notes.join("；"))
            } else {
                "完成阶段性功能需求研发与验证".to_string()
            };

            let entry_id = Uuid::new_v4();
            let entry = TimeEntry {
                id: entry_id,
                project_id: pid,
                start_time: start_at,
                end_time: end_at,
                raw_duration_secs: elapsed_secs,
                billable_hours,
                billable_days,
                rate_at_moment: rate,
                total_amount: amount,
                summary: Some(summary_desc),
                audio_path: saved_audio,
                is_invoiced: false,
                created_at: Utc::now(),
            };

            state.db.lock().unwrap().insert_time_entry(&entry).map_err(|e| e.to_string())?;
            return Ok(format!("工时已归档：{:.2} 人天 ({})", billable_days, format_duration(elapsed_secs)));
        }
    }

    Ok("工时未发生变动或已结束".to_string())
}

#[tauri::command]
fn start_quick_meeting(
    state: State<AppState>,
    project_id: Option<String>,
    topic: String,
    record_audio: bool,
) -> Result<String, String> {
    let (pid, proj_name) = if let Some(pid_str) = project_id {
        if pid_str.trim().is_empty() {
            let mut p = Project::new(&format!("🎙️ {}", topic.trim()));
            p.billing_type = BillingType::Hourly;
            p.hourly_rate = 350.0;
            p.hours_per_day = 8.0;
            p.day_calc_mode = DayCalcMode::Exact;
            state.db.lock().unwrap().insert_project(&p).map_err(|e| e.to_string())?;
            (p.id, p.name)
        } else {
            let p_uuid = Uuid::parse_str(&pid_str).map_err(|e| e.to_string())?;
            let projs = state.db.lock().unwrap().list_projects().unwrap_or_default();
            let name = projs.iter().find(|p| p.id == p_uuid).map(|p| p.name.clone()).unwrap_or_else(|| topic.clone());
            (p_uuid, name)
        }
    } else {
        let mut p = Project::new(&format!("🎙️ {}", topic.trim()));
        p.billing_type = BillingType::Hourly;
        p.hourly_rate = 350.0;
        p.hours_per_day = 8.0;
        p.day_calc_mode = DayCalcMode::Exact;
        state.db.lock().unwrap().insert_project(&p).map_err(|e| e.to_string())?;
        (p.id, p.name)
    };

    let mut mgr = state.timer_manager.lock().unwrap();
    mgr.start_channel(pid);
    let now = Utc::now();
    let _ = state.db.lock().unwrap().upsert_active_timer(&ActiveTimerRecord {
        project_id: pid,
        session_start_at: now,
        current_started_at: Some(now),
        accumulated_secs: mgr.get_elapsed(&pid),
        is_paused: false,
        last_heartbeat: now,
    });

    if record_audio {
        let rec_dir = state.db_dir.join("recordings");
        let _ = std::fs::create_dir_all(&rec_dir);
        let timestamp_str = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let wav_file = rec_dir.join(format!("meeting_{}_{}.wav", pid, timestamp_str));
        let mut recorder = state.audio_recorder.lock().unwrap();
        if let Ok(_) = recorder.start_recording(&wav_file) {
            *state.active_recording.lock().unwrap() = Some((pid, topic.clone(), wav_file));
        }
    }

    Ok(format!("已启动【{}】会议沟通", proj_name))
}

#[tauri::command]
fn toggle_audio_pause(state: State<AppState>) -> Result<bool, String> {
    let mut recorder = state.audio_recorder.lock().unwrap();
    if recorder.is_recording() {
        if recorder.is_paused() {
            recorder.resume_recording();
            Ok(false)
        } else {
            recorder.pause_recording();
            Ok(true)
        }
    } else {
        Ok(false)
    }
}

#[tauri::command]
fn play_audio(state: State<AppState>, audio_path: String) -> Result<(), String> {
    let p = std::path::Path::new(&audio_path);
    state.audio_player.lock().unwrap().play(p).map_err(|e| e.to_string())
}

#[tauri::command]
fn stop_audio(state: State<AppState>) -> Result<(), String> {
    state.audio_player.lock().unwrap().stop();
    Ok(())
}

#[tauri::command]
fn create_project(
    state: State<AppState>,
    name: String,
    client: String,
    billing_type: Option<String>,
    rate: Option<f64>,
    daily_rate: Option<f64>,
    currency: String,
    rounding_mode: Option<String>,
    min_granularity: Option<f64>,
    git_repo: Option<String>,
) -> Result<String, String> {
    if name.trim().is_empty() {
        return Err("项目名称不能为空".to_string());
    }
    let mut p = Project::new(name.trim());
    
    let b_type = match billing_type.as_deref() {
        Some("hourly") => BillingType::Hourly,
        Some("fixed_cap") => BillingType::FixedCap,
        Some("non_billable") => BillingType::NonBillable,
        _ => BillingType::Daily,
    };
    p.billing_type = b_type;

    let target_rate = rate.or(daily_rate).unwrap_or(0.0);
    match b_type {
        BillingType::Hourly => {
            p.hourly_rate = target_rate;
            p.daily_rate = target_rate * 8.0; // 默认 8h 等效日薪
        }
        BillingType::Daily => {
            p.daily_rate = target_rate;
            p.hourly_rate = target_rate / 8.0;
        }
        _ => {
            p.daily_rate = target_rate;
            p.hourly_rate = target_rate / 8.0;
        }
    }

    p.currency = if currency.trim().is_empty() { "CNY".to_string() } else { currency };
    p.git_repo_path = git_repo.filter(|s| !s.trim().is_empty());

    p.rounding_mode = match rounding_mode.as_deref() {
        Some("half_up") | Some("round") => myth_core::models::RoundingMode::HalfUp,
        Some("floor") => myth_core::models::RoundingMode::Floor,
        _ => myth_core::models::RoundingMode::Ceil,
    };

    let gran = min_granularity.unwrap_or(0.5);
    p.min_granularity = if gran > 0.0 { gran } else { 0.5 };

    if !client.trim().is_empty() {
        let c = myth_core::models::Client::new(client.trim(), &p.currency);
        let _ = state.db.lock().unwrap().insert_client(&c);
        p.client_id = Some(c.id);
    }

    state.db.lock().unwrap().insert_project(&p).map_err(|e| e.to_string())?;
    Ok(p.id.to_string())
}

#[tauri::command]
fn update_project(
    state: State<AppState>,
    id: String,
    name: String,
    client: String,
    billing_type: Option<String>,
    rate: Option<f64>,
    currency: String,
    rounding_mode: Option<String>,
    min_granularity: Option<f64>,
    git_repo: Option<String>,
) -> Result<(), String> {
    let pid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let mut guard = state.db.lock().unwrap();
    let mut p = guard.get_project(&pid).map_err(|e| e.to_string())?
        .ok_or_else(|| "项目不存在".to_string())?;

    if !name.trim().is_empty() {
        p.name = name.trim().to_string();
    }

    let b_type = match billing_type.as_deref() {
        Some("hourly") => BillingType::Hourly,
        Some("fixed_cap") => BillingType::FixedCap,
        Some("non_billable") => BillingType::NonBillable,
        _ => BillingType::Daily,
    };
    p.billing_type = b_type;

    if let Some(target_rate) = rate {
        match b_type {
            BillingType::Hourly => {
                p.hourly_rate = target_rate;
                p.daily_rate = target_rate * 8.0;
            }
            BillingType::Daily => {
                p.daily_rate = target_rate;
                p.hourly_rate = target_rate / 8.0;
            }
            _ => {
                p.daily_rate = target_rate;
                p.hourly_rate = target_rate / 8.0;
            }
        }
    }

    if !currency.trim().is_empty() {
        p.currency = currency.trim().to_uppercase();
    }
    p.git_repo_path = git_repo.filter(|s| !s.trim().is_empty());

    if let Some(rm) = rounding_mode.as_deref() {
        p.rounding_mode = match rm {
            "half_up" | "round" => myth_core::models::RoundingMode::HalfUp,
            "floor" => myth_core::models::RoundingMode::Floor,
            _ => myth_core::models::RoundingMode::Ceil,
        };
    }

    if let Some(gran) = min_granularity {
        p.min_granularity = if gran > 0.0 { gran } else { 0.5 };
    }

    if !client.trim().is_empty() {
        let c = myth_core::models::Client::new(client.trim(), &p.currency);
        let _ = guard.insert_client(&c);
        p.client_id = Some(c.id);
    }

    guard.update_project(&p).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn add_project_note(state: State<AppState>, project_id: String, content: String) -> Result<(), String> {
    let pid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let note = QuickNote::new(pid, content.trim());
    state.db.lock().unwrap().insert_quick_note(&note).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn list_project_notes(state: State<AppState>, project_id: String) -> Result<Vec<NoteDto>, String> {
    let pid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let notes = state.db.lock().unwrap().list_quick_notes(&pid).unwrap_or_default();
    Ok(notes
        .into_iter()
        .map(|n| NoteDto {
            id: n.id.to_string(),
            project_id: n.project_id.to_string(),
            content: n.content,
            time_text: n.logged_at.with_timezone(&Local).format("%H:%M").to_string(),
        })
        .collect())
}

#[tauri::command]
fn delete_note(state: State<AppState>, note_id: String) -> Result<(), String> {
    let nid = Uuid::parse_str(&note_id).map_err(|e| e.to_string())?;
    state.db.lock().unwrap().delete_quick_note(&nid).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn log_manual_entry(
    state: State<AppState>,
    project_id: String,
    date: String,
    start_time: String,
    end_time: String,
    summary: String,
) -> Result<String, String> {
    let pid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let proj = state
        .db
        .lock()
        .unwrap()
        .list_projects()
        .unwrap_or_default()
        .into_iter()
        .find(|p| p.id == pid)
        .ok_or_else(|| "项目不存在".to_string())?;

    // 解析日期与时间 (格式: 2026-10-08, 09:30, 12:00)
    let start_str = format!("{} {}:00", date.trim(), start_time.trim());
    let end_str = format!("{} {}:00", date.trim(), end_time.trim());

    let start_naive = chrono::NaiveDateTime::parse_from_str(&start_str, "%Y-%m-%d %H:%M:%S")
        .map_err(|_| "开始时间格式错误，需为 YYYY-MM-DD 与 HH:MM".to_string())?;
    let end_naive = chrono::NaiveDateTime::parse_from_str(&end_str, "%Y-%m-%d %H:%M:%S")
        .map_err(|_| "结束时间格式错误，需为 YYYY-MM-DD 与 HH:MM".to_string())?;

    let start_local = match Local.from_local_datetime(&start_naive) {
        chrono::LocalResult::Single(dt) => dt,
        chrono::LocalResult::Ambiguous(dt, _) => dt,
        chrono::LocalResult::None => return Err("无效的开始时间".to_string()),
    };
    let end_local = match Local.from_local_datetime(&end_naive) {
        chrono::LocalResult::Single(dt) => dt,
        chrono::LocalResult::Ambiguous(dt, _) => dt,
        chrono::LocalResult::None => return Err("无效的结束时间".to_string()),
    };

    if end_local <= start_local {
        return Err("结束时间必须晚于开始时间".to_string());
    }

    let diff = end_local.signed_duration_since(start_local);
    let duration_secs = diff.num_seconds().max(0) as u64;

    let billable_hours = BillingEngine::calculate_billable_hours(duration_secs, proj.rounding_minutes);
    let billable_days = BillingEngine::calculate_billable_days(billable_hours, proj.hours_per_day, proj.day_calc_mode);
    let (rate_snapshot, total_amount) = BillingEngine::calculate_amount(&proj, billable_hours, billable_days);

    let mut entry = TimeEntry {
        id: Uuid::new_v4(),
        project_id: pid,
        start_time: start_local.with_timezone(&Utc),
        end_time: end_local.with_timezone(&Utc),
        raw_duration_secs: duration_secs,
        billable_hours,
        billable_days,
        rate_at_moment: rate_snapshot,
        total_amount,
        summary: Some(if summary.trim().is_empty() { "补录工时投入与研发交付".to_string() } else { summary.trim().to_string() }),
        audio_path: None,
        is_invoiced: false,
        created_at: Utc::now(),
    };

    state.db.lock().unwrap().insert_time_entry(&entry).map_err(|e| e.to_string())?;
    Ok("工时补录成功！".to_string())
}

#[tauri::command]
fn log_manual_meeting(
    state: State<AppState>,
    project_id: Option<String>,
    date: String,
    start_time: String,
    end_time: String,
    topic: String,
    rate: f64,
    source_audio_path: Option<String>,
) -> Result<String, String> {
    let pid = if let Some(ref p_str) = project_id {
        if p_str.trim().is_empty() {
            let mut p = Project::new(&format!("🎙️ {}", topic.trim()));
            p.billing_type = BillingType::Hourly;
            p.hourly_rate = if rate > 0.0 { rate } else { 350.0 };
            p.hours_per_day = 8.0;
            p.day_calc_mode = DayCalcMode::Exact;
            state.db.lock().unwrap().insert_project(&p).map_err(|e| e.to_string())?;
            p.id
        } else {
            Uuid::parse_str(p_str).map_err(|e| e.to_string())?
        }
    } else {
        let mut p = Project::new(&format!("🎙️ {}", topic.trim()));
        p.billing_type = BillingType::Hourly;
        p.hourly_rate = if rate > 0.0 { rate } else { 350.0 };
        p.hours_per_day = 8.0;
        p.day_calc_mode = DayCalcMode::Exact;
        state.db.lock().unwrap().insert_project(&p).map_err(|e| e.to_string())?;
        p.id
    };

    let start_str = format!("{} {}:00", date.trim(), start_time.trim());
    let end_str = format!("{} {}:00", date.trim(), end_time.trim());

    let start_naive = chrono::NaiveDateTime::parse_from_str(&start_str, "%Y-%m-%d %H:%M:%S")
        .map_err(|_| "开始时间格式错误，需为 YYYY-MM-DD 与 HH:MM".to_string())?;
    let end_naive = chrono::NaiveDateTime::parse_from_str(&end_str, "%Y-%m-%d %H:%M:%S")
        .map_err(|_| "结束时间格式错误，需为 YYYY-MM-DD 与 HH:MM".to_string())?;

    let start_local = match Local.from_local_datetime(&start_naive) {
        chrono::LocalResult::Single(dt) => dt,
        chrono::LocalResult::Ambiguous(dt, _) => dt,
        chrono::LocalResult::None => return Err("无效的开始时间".to_string()),
    };
    let end_local = match Local.from_local_datetime(&end_naive) {
        chrono::LocalResult::Single(dt) => dt,
        chrono::LocalResult::Ambiguous(dt, _) => dt,
        chrono::LocalResult::None => return Err("无效的结束时间".to_string()),
    };

    if end_local <= start_local {
        return Err("结束时间必须晚于开始时间".to_string());
    }

    let diff = end_local.signed_duration_since(start_local);
    let duration_secs = diff.num_seconds().max(0) as u64;

    let hours = duration_secs as f64 / 3600.0;
    let billable_hours = (hours * 10.0).ceil() / 10.0; // 保留0.1小时步长
    let billable_days = (billable_hours / 8.0 * 100.0).round() / 100.0;
    let actual_rate = if rate > 0.0 { rate } else { 350.0 };
    let total_amount = (billable_hours * actual_rate * 100.0).round() / 100.0;

    // 处理音频文件复制归档
    let mut target_audio_path = None;
    if let Some(src_str) = source_audio_path {
        let src_path = std::path::Path::new(&src_str);
        if src_path.exists() {
            let audio_dir = state.db_dir.join("audios");
            let _ = std::fs::create_dir_all(&audio_dir);
            let ext = src_path.extension().and_then(|s| s.to_str()).unwrap_or("wav");
            let filename = format!("upload_{}_{}.{}", date.replace('-', ""), Uuid::new_v4().simple(), ext);
            let dest_path = audio_dir.join(filename);
            if std::fs::copy(src_path, &dest_path).is_ok() {
                target_audio_path = Some(dest_path.to_string_lossy().to_string());
            }
        }
    }

    let topic_name = if topic.trim().is_empty() { "专题方案沟通会".to_string() } else { topic.trim().to_string() };
    let summary = format!("【会议补录】{}\n会议时薪：{} 元/小时\n议题：{}", topic_name, actual_rate, topic_name);

    let entry = TimeEntry {
        id: Uuid::new_v4(),
        project_id: pid,
        start_time: start_local.with_timezone(&Utc),
        end_time: end_local.with_timezone(&Utc),
        raw_duration_secs: duration_secs,
        billable_hours,
        billable_days,
        rate_at_moment: actual_rate,
        total_amount,
        summary: Some(summary),
        audio_path: target_audio_path,
        is_invoiced: false,
        created_at: Utc::now(),
    };

    state.db.lock().unwrap().insert_time_entry(&entry).map_err(|e| e.to_string())?;
    Ok("会议工时与录音补录成功！".to_string())
}

#[tauri::command]
async fn transcribe_entry_audio(state: State<'_, AppState>, entry_id: String, audio_path: String) -> Result<String, String> {
    let eid = Uuid::parse_str(&entry_id).map_err(|e| e.to_string())?;
    let db = state.db.clone();
    let (provider, api_base, api_key, model) = {
        let guard = db.lock().unwrap();
        let provider = guard.get_setting("ai_provider").ok().flatten().unwrap_or_else(|| "agy".to_string());
        let api_base = guard.get_setting("ai_base").ok().flatten().unwrap_or_else(|| "https://api.deepseek.com/v1".to_string());
        let api_key = guard.get_setting("ai_key").ok().flatten().unwrap_or_default();
        let model = guard.get_setting("ai_model").ok().flatten().unwrap_or_else(|| "deepseek-chat".to_string());
        (provider, api_base, api_key, model)
    };

    let summarizer = AiSummarizer::new(AiConfig {
        provider,
        api_base,
        api_key,
        model,
    });

    let p = std::path::Path::new(&audio_path);
    let transcript = summarizer.transcribe_audio(p).await.map_err(|e| e.to_string())?;

    let mut guard = db.lock().unwrap();
    if let Ok(Some(mut entry)) = guard.get_time_entry(&eid) {
        let prev = entry.summary.unwrap_or_default();
        let new_summary = if prev.is_empty() {
            format!("【会议发言实录与摘要】\n{}", transcript)
        } else {
            format!("{}\n\n【会议转写实录】\n{}", prev, transcript)
        };
        entry.summary = Some(new_summary.clone());
        let _ = guard.insert_time_entry(&entry);
        return Ok(new_summary);
    }

    Ok(transcript)
}

#[tauri::command]
async fn generate_daily_markdown(state: State<'_, AppState>) -> Result<String, String> {
    let projs = state.db.lock().unwrap().list_projects().unwrap_or_default();
    let today_str = Local::now().format("%Y-%m-%d").to_string();
    let mut total_days = 0.0f64;
    let mut total_amount = 0.0f64;
    let mut proj_sections = Vec::new();

    for p in &projs {
        if let Ok(entries) = state.db.lock().unwrap().list_entries_for_project(&p.id) {
            let today_entries: Vec<_> = entries
                .into_iter()
                .filter(|e| e.start_time.with_timezone(&Local).format("%Y-%m-%d").to_string() == today_str)
                .collect();
            if !today_entries.is_empty() {
                let p_days: f64 = today_entries.iter().map(|e| e.billable_days).sum();
                let p_amount: f64 = today_entries.iter().map(|e| e.total_amount).sum();
                total_days += p_days;
                total_amount += p_amount;

                let mut lines = Vec::new();
                for e in today_entries {
                    let range = format!(
                        "{}-{}",
                        e.start_time.with_timezone(&Local).format("%H:%M"),
                        e.end_time.with_timezone(&Local).format("%H:%M")
                    );
                    let summ = e.summary.unwrap_or_else(|| "持续代码开发与测试交付".to_string());
                    lines.push(format!("- `{}` (投入: {}, 核算: {:.2}人天) : {}", range, format_duration(e.raw_duration_secs), e.billable_days, summ));
                }
                proj_sections.push(format!("### 📁 {}\n- **当日核算投入**: {:.2} 人天 ({:.2} {})\n{}", p.name, p_days, p_amount, p.currency, lines.join("\n")));
            }
        }
    }

    let report = format!(
        "# 📅 研发工时与交付成果日报 ({})\n\n**今日总投入**: {:.2} 人天 | **核算总价值**: ¥ {:.2}\n\n---\n\n{}",
        today_str,
        total_days,
        total_amount,
        if proj_sections.is_empty() { "今日暂无归档工时流水".to_string() } else { proj_sections.join("\n\n") }
    );
    Ok(report)
}

#[tauri::command]
fn toggle_entry_invoiced(state: State<AppState>, entry_id: String, is_invoiced: bool) -> Result<(), String> {
    let eid = Uuid::parse_str(&entry_id).map_err(|e| e.to_string())?;
    state.db.lock().unwrap().set_entry_invoiced(&eid, is_invoiced).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn batch_toggle_entries_invoiced(state: State<AppState>, entry_ids: Vec<String>, is_invoiced: bool) -> Result<usize, String> {
    let uids: Vec<Uuid> = entry_ids.iter().filter_map(|s| Uuid::parse_str(s).ok()).collect();
    let count = uids.len();
    state.db.lock().unwrap().batch_set_entries_invoiced(&uids, is_invoiced).map_err(|e| e.to_string())?;
    Ok(count)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SettlementReportResult {
    pub markdown: String,
    pub total_days: f64,
    pub total_amount: f64,
    pub settled_amount: f64,
    pub pending_amount: f64,
    pub matching_entry_ids: Vec<String>,
    pub entries: Vec<EntryDto>,
}

#[tauri::command]
async fn generate_settlement_report(
    state: State<'_, AppState>,
    project_ids: Option<Vec<String>>,
    months: Option<Vec<String>>,
    status_filter: Option<String>, // "all" | "pending" | "settled"
    enable_rebalance: Option<bool>, // 是否启用周一至周五 8h/天 削峰填谷 (兼容旧参数)
    settlement_strategy: Option<String>, // "rebalance" | "min8_carry" | "cap8_carry" | "exact"
) -> Result<SettlementReportResult, String> {
    use myth_core::billing::SettlementStrategy;

    let filter_pids: Option<std::collections::HashSet<String>> = project_ids.map(|list| list.into_iter().collect());
    let filter_months: Option<std::collections::HashSet<String>> = months.map(|list| list.into_iter().collect());
    let status_filter = status_filter.unwrap_or_else(|| "all".to_string());
    
    let strategy = match settlement_strategy.as_deref() {
        Some("min8_carry") => SettlementStrategy::Min8HoursCarryOver,
        Some("cap8_carry") => SettlementStrategy::Cap8HoursCarryOver,
        Some("exact") => SettlementStrategy::ExactActual,
        Some("rebalance") => SettlementStrategy::RebalanceWorkdays,
        _ => {
            if enable_rebalance.unwrap_or(false) {
                SettlementStrategy::RebalanceWorkdays
            } else {
                SettlementStrategy::ExactActual
            }
        }
    };

    let mut total_days = 0.0f64;
    let mut total_amount = 0.0f64;
    let mut settled_amount = 0.0f64;
    let mut pending_amount = 0.0f64;
    let mut matching_entry_ids = Vec::new();
    let mut all_entries = Vec::new();
    let mut proj_sections = Vec::new();

    // 格式化月份说明
    let months_display = if let Some(ref m_set) = filter_months {
        let mut sorted: Vec<_> = m_set.iter().cloned().collect();
        sorted.sort();
        if sorted.is_empty() { "全部历史跨度".to_string() } else { sorted.join(", ") }
    } else {
        "全部历史跨度".to_string()
    };

    let (projs, clients) = {
        let guard = state.db.lock().unwrap();
        let p_list = guard.list_projects().unwrap_or_default();
        let c_list = guard.list_clients().unwrap_or_default();
        (p_list, c_list)
    };

    let client_map: std::collections::HashMap<Uuid, String> = clients.into_iter().map(|c| (c.id, c.name)).collect();

    for p in &projs {
        let pid_str = p.id.to_string();
        if let Some(ref p_set) = filter_pids {
            if !p_set.is_empty() && !p_set.contains(&pid_str) {
                continue;
            }
        }

        let entries = state.db.lock().unwrap().list_entries_for_project(&p.id).unwrap_or_default();
        let filtered_entries: Vec<_> = entries
            .into_iter()
            .filter(|e| {
                let d_str = e.start_time.with_timezone(&Local).format("%Y-%m-%d").to_string();
                let m_str = &d_str[0..7]; // "YYYY-MM"
                if let Some(ref m_set) = filter_months {
                    if !m_set.is_empty() && !m_set.contains(m_str) {
                        return false;
                    }
                }
                match status_filter.as_str() {
                    "settled" => e.is_invoiced,
                    "pending" => !e.is_invoiced,
                    _ => true,
                }
            })
            .collect();

        if !filtered_entries.is_empty() {
            for e in &filtered_entries {
                matching_entry_ids.push(e.id.to_string());
            }

            // 根据选定策略生成该项目的单日核算工作日列表
            let transformed_days = BillingEngine::transform_project_workdays(
                filtered_entries.clone(),
                p.hours_per_day,
                p.daily_rate,
                &p.currency,
                strategy,
            );

            let p_days: f64 = transformed_days.iter().map(|d| d.days_val).sum();
            let p_amount: f64 = transformed_days.iter().map(|d| d.amount_val).sum();
            let p_settled: f64 = transformed_days.iter().filter(|d| d.is_invoiced).map(|d| d.amount_val).sum();
            let p_pending: f64 = transformed_days.iter().filter(|d| !d.is_invoiced).map(|d| d.amount_val).sum();

            total_days += p_days;
            total_amount += p_amount;
            settled_amount += p_settled;
            pending_amount += p_pending;

            let mut lines = Vec::new();
            for (idx, r) in transformed_days.into_iter().enumerate() {
                let virtual_id = format!("{}_workday_{}", p.id, idx);
                let status_badge = if r.is_invoiced { "✅ [已结算]" } else { "⏳ [待结算]" };
                lines.push(format!(
                    "- `{} 工作日` (核算: {}, 折算: {:.2}人天, 金额: {:.2} {}) {} : {}",
                    r.date_str,
                    r.duration_text,
                    r.days_val,
                    r.amount_val,
                    p.currency,
                    status_badge,
                    r.summary_text
                ));

                all_entries.push(EntryDto {
                    id: virtual_id,
                    project_id: p.id.to_string(),
                    project_name: p.name.clone(),
                    duration_text: r.duration_text,
                    days_text: r.days_text,
                    amount_text: r.amount_text,
                    summary_text: r.summary_text,
                    time_range: "全天标准".to_string(),
                    date_str: r.date_str,
                    has_audio: r.has_audio,
                    audio_path: "".to_string(),
                    raw_secs: (r.days_val * p.hours_per_day * 3600.0) as u64,
                    is_invoiced: r.is_invoiced,
                });
            }

            let client_name = p
                .client_id
                .and_then(|cid| client_map.get(&cid).cloned())
                .unwrap_or_else(|| "商业客户".to_string());

            let strategy_name = match strategy {
                SettlementStrategy::RebalanceWorkdays => "削峰填谷模式 (8h封顶·工作日平摊)",
                SettlementStrategy::Min8HoursCarryOver => "保底8小时模式 (不足8h按8h·超额顺延新天)",
                SettlementStrategy::Cap8HoursCarryOver => "封顶顺延模式 (单日封顶8h·超额顺延新天)",
                SettlementStrategy::ExactActual => "实际工时模式 (同日合并·实报实销)",
            };

            proj_sections.push(format!(
                "### 📁 {} (`{}`) — 🛡️ {}\n- **对账核算工时**: {:.2} 人天\n- **总计结算金额**: {:.2} {} (已结: {:.2} {} | 待结: {:.2} {})\n\n**合规工作日排期与交付明细**:\n{}",
                p.name, client_name, strategy_name, p_days, p_amount, p.currency, p_settled, p.currency, p_pending, p.currency, lines.join("\n")
            ));
        }
    }

    // 正序升序排列全部明细（按时间顺序从月初到月末排列）
    all_entries.sort_by(|a, b| a.date_str.cmp(&b.date_str));

    let status_desc = match status_filter.as_str() {
        "settled" => "已结算 (Settled)",
        "pending" => "待结算 / 待对账 (Pending)",
        _ => "全量工时 (全部状态)",
    };

    let report_content = format!(
        "# 📑 多项目人天工时结算对账报表\n\n- **核算周期范围**: {}\n- **对账状态筛选**: {}\n- **累计总核算投入**: **{:.2} 人天**\n- **总核算金额**: **¥ {:.2}** (已结算: `¥ {:.2}` | 待结算/待回款: `¥ {:.2}`)\n\n---\n\n{}",
        months_display,
        status_desc,
        total_days,
        total_amount,
        settled_amount,
        pending_amount,
        if proj_sections.is_empty() { "指定筛选条件下无相关工时明细流水。".to_string() } else { proj_sections.join("\n\n---\n\n") }
    );

    Ok(SettlementReportResult {
        markdown: report_content,
        total_days,
        total_amount,
        settled_amount,
        pending_amount,
        matching_entry_ids,
        entries: all_entries,
    })
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Result<serde_json::Value, String> {
    let provider = state.db.lock().unwrap().get_setting("ai_provider").ok().flatten().unwrap_or_else(|| "agy".to_string());
    let base = state.db.lock().unwrap().get_setting("ai_base").ok().flatten().unwrap_or_else(|| "https://api.deepseek.com/v1".to_string());
    let key = state.db.lock().unwrap().get_setting("ai_key").ok().flatten().unwrap_or_default();
    let model = state.db.lock().unwrap().get_setting("ai_model").ok().flatten().unwrap_or_else(|| "deepseek-chat".to_string());
    let hours = state.db.lock().unwrap().get_setting("hours_per_day").ok().flatten().unwrap_or_else(|| "8.0".to_string());
    let afk = state.db.lock().unwrap().get_setting("afk_minutes").ok().flatten().unwrap_or_else(|| "10".to_string());

    Ok(serde_json::json!({
        "ai_provider": provider,
        "ai_base": base,
        "ai_key": key,
        "ai_model": model,
        "hours_per_day": hours,
        "afk_minutes": afk
    }))
}

#[tauri::command]
fn save_settings(
    state: State<AppState>,
    ai_provider: String,
    ai_base: String,
    ai_key: String,
    ai_model: String,
    hours_per_day: String,
    afk_minutes: String,
) -> Result<(), String> {
    let _ = state.db.lock().unwrap().set_setting("ai_provider", &ai_provider);
    let _ = state.db.lock().unwrap().set_setting("ai_base", &ai_base);
    let _ = state.db.lock().unwrap().set_setting("ai_key", &ai_key);
    let _ = state.db.lock().unwrap().set_setting("ai_model", &ai_model);
    let _ = state.db.lock().unwrap().set_setting("hours_per_day", &hours_per_day);
    let _ = state.db.lock().unwrap().set_setting("afk_minutes", &afk_minutes);
    Ok(())
}

#[tauri::command]
fn save_and_open_excel(filename: String, base64_data: String) -> Result<String, String> {
    use std::io::Write;
    let home = std::env::var("HOME").map_err(|e| e.to_string())?;
    let downloads_dir = std::path::PathBuf::from(home).join("Downloads");
    let target_dir = if downloads_dir.exists() {
        downloads_dir
    } else {
        std::path::PathBuf::from("/tmp")
    };

    let target_path = target_dir.join(&filename);
    let decoded = base64_decode(&base64_data).map_err(|e| e.to_string())?;
    let mut file = std::fs::File::create(&target_path).map_err(|e| e.to_string())?;
    file.write_all(&decoded).map_err(|e| e.to_string())?;

    let path_str = target_path.to_string_lossy().to_string();

    // 尝试直接使用系统默认程序打开文件或者打开所在目录
    let _ = std::process::Command::new("xdg-open")
        .arg(&target_path)
        .spawn();

    Ok(path_str)
}

#[derive(Serialize, Deserialize)]
pub struct FilePayload {
    pub filename: String,
    pub base64_data: String,
}

#[tauri::command]
fn save_multiple_excel_files(files: Vec<FilePayload>) -> Result<Vec<String>, String> {
    use std::io::Write;
    let home = std::env::var("HOME").map_err(|e| e.to_string())?;
    let downloads_dir = std::path::PathBuf::from(home).join("Downloads");
    let target_dir = if downloads_dir.exists() {
        downloads_dir
    } else {
        std::path::PathBuf::from("/tmp")
    };

    let mut saved_paths = Vec::new();
    for item in files {
        let target_path = target_dir.join(&item.filename);
        let decoded = base64_decode(&item.base64_data).map_err(|e| e.to_string())?;
        let mut file = std::fs::File::create(&target_path).map_err(|e| e.to_string())?;
        file.write_all(&decoded).map_err(|e| e.to_string())?;
        saved_paths.push(target_path.to_string_lossy().to_string());
    }

    // 自动打开 Downloads 目录供用户查看生成的文件
    let _ = std::process::Command::new("xdg-open")
        .arg(&target_dir)
        .spawn();

    Ok(saved_paths)
}

#[tauri::command]
fn open_file_or_dir(path: String) -> Result<(), String> {
    let target_path = std::path::Path::new(&path);
    let _ = std::process::Command::new("xdg-open")
        .arg(target_path)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn base64_decode(data: &str) -> Result<Vec<u8>, String> {
    // 简易 base64 解码，支持标准 base64
    const B64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut map = [255u8; 256];
    for (i, &c) in B64_CHARS.iter().enumerate() {
        map[c as usize] = i as u8;
    }

    let clean = data.trim_end_matches('=');
    let mut result = Vec::with_capacity((clean.len() * 3) / 4);
    let mut buf = 0u32;
    let mut bits = 0;

    for &b in clean.as_bytes() {
        let val = map[b as usize];
        if val == 255 {
            continue; // 跳过非法字符或换行
        }
        buf = (buf << 6) | (val as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            result.push(((buf >> bits) & 0xff) as u8);
        }
    }
    Ok(result)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let db_dir = dirs_or_local_data_dir();
    std::fs::create_dir_all(&db_dir)?;

    let db_path = db_dir.join("mythmanday.db");
    let db = Arc::new(Mutex::new(Database::open(&db_path)?));

    let timer_manager = Arc::new(Mutex::new(MultiChannelManager::new()));
    let audio_recorder = Arc::new(Mutex::new(AudioRecorder::new()));
    let audio_player = Arc::new(Mutex::new(AudioPlayer::new()));
    let active_recording = Arc::new(Mutex::new(None));

    // 自动恢复崩溃重启前运行中的通道
    if let Ok(active_list) = db.lock().unwrap().list_active_timers() {
        let mut mgr = timer_manager.lock().unwrap();
        for rec in active_list {
            mgr.restore_channel(
                rec.project_id,
                rec.session_start_at,
                rec.current_started_at,
                rec.accumulated_secs,
                rec.is_paused,
            );
        }
    }

    let state = AppState {
        db,
        timer_manager,
        audio_recorder,
        audio_player,
        active_recording,
        db_dir,
    };

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_projects,
            get_daily_groups,
            get_global_stats,
            start_timer,
            pause_timer,
            resume_timer,
            stop_timer,
            start_quick_meeting,
            toggle_audio_pause,
            play_audio,
            stop_audio,
            create_project,
            update_project,
            add_project_note,
            list_project_notes,
            delete_note,
            log_manual_entry,
            log_manual_meeting,
            transcribe_entry_audio,
            generate_daily_markdown,
            toggle_entry_invoiced,
            batch_toggle_entries_invoiced,
            generate_settlement_report,
            get_settings,
            save_settings,
            save_and_open_excel,
            save_multiple_excel_files,
            open_file_or_dir
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    Ok(())
}
