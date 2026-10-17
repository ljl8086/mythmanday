// Copyright (C) 2026 ljl8086 <ljl8086@gmail.com>. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (MythManDay - Tauri Host)
// Version: V1.0

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::{Local, TimeZone, Utc};
use myth_ai::{
    download_whisper_model_file, get_asr_runtime_status, AiConfig, AiSummarizer,
    AiWorkEvaluationRequest, AsrRuntimeStatus,
};
use myth_audio::{AudioPlayer, AudioRecorder};
use myth_core::billing::BillingEngine;
use myth_core::models::{
    BillingType, DayCalcMode, Meeting, MeetingAudioClip, MeetingMaterial, Project, QuickNote, TimeEntry, WorkSpan,
};
use myth_core::timer::MultiChannelManager;
use myth_db::{ActiveTimerRecord, Database};
use myth_git::GitScanner;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

/// 从数据库统一加载 AI 与 ASR 完整多模配置
fn load_ai_config(guard: &Database) -> AiConfig {
    let provider = guard
        .get_setting("ai_provider")
        .ok()
        .flatten()
        .unwrap_or_else(|| "codex".to_string());
    let api_base = guard
        .get_setting("ai_base")
        .ok()
        .flatten()
        .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
    let api_key = guard
        .get_setting("ai_key")
        .ok()
        .flatten()
        .unwrap_or_default();
    let model = guard
        .get_setting("ai_model")
        .ok()
        .flatten()
        .unwrap_or_else(|| "gpt-4o-mini".to_string());
    let proxy = guard
        .get_setting("ai_proxy")
        .ok()
        .flatten()
        .filter(|s| !s.trim().is_empty());

    let asr_mode = guard
        .get_setting("asr_mode")
        .ok()
        .flatten()
        .unwrap_or_else(|| "local".to_string());
    let asr_api_base = guard
        .get_setting("asr_api_base")
        .ok()
        .flatten()
        .filter(|s| !s.trim().is_empty());
    let asr_api_key = guard
        .get_setting("asr_api_key")
        .ok()
        .flatten()
        .filter(|s| !s.trim().is_empty());
    let asr_model = guard
        .get_setting("asr_model")
        .ok()
        .flatten()
        .filter(|s| !s.trim().is_empty());
    let asr_local_model = guard
        .get_setting("asr_local_model")
        .ok()
        .flatten()
        .filter(|s| !s.trim().is_empty());
    let asr_local_bin_path = guard
        .get_setting("asr_local_bin_path")
        .ok()
        .flatten()
        .filter(|s| !s.trim().is_empty());

    AiConfig {
        provider,
        api_base,
        api_key,
        model,
        proxy,
        asr: myth_ai::AsrConfig {
            mode: asr_mode,
            api_base: asr_api_base,
            api_key: asr_api_key,
            model: asr_model.or_else(|| Some("whisper-1".to_string())),
            local_model: asr_local_model.or_else(|| Some("ggml-base.bin".to_string())),
            local_bin_path: asr_local_bin_path,
            language: Some("zh".to_string()),
            prompt: None,
        },
    }
}

/// 活跃录音上下文 (项目ID, 会议ID, 会议主题, 音频文件路径, 该片段录音开始时刻)
pub type ActiveRecordingState = Option<(
    Uuid,
    Uuid,
    String,
    std::path::PathBuf,
    chrono::DateTime<Utc>,
)>;

pub struct AppState {
    pub db: Arc<Mutex<Database>>,
    pub timer_manager: Arc<Mutex<MultiChannelManager>>,
    pub audio_recorder: Arc<Mutex<AudioRecorder>>,
    pub audio_player: Arc<Mutex<AudioPlayer>>,
    pub active_recording: Arc<Mutex<ActiveRecordingState>>,
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
    pub git_repo_path: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AiTimeSlotDto {
    pub label: String,
    pub start: String,
    pub end: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GitSmartFillDto {
    pub success: bool,
    pub repo_path: String,
    pub summary_text: String,
    pub commits_count: usize,
    pub commits: Vec<String>,
    pub suggested_start_time: Option<String>,
    pub suggested_end_time: Option<String>,
    pub diff_stat: Option<String>,
    pub is_fallback_to_day: bool,
    pub ai_reason: Option<String>,
    pub time_slots: Vec<AiTimeSlotDto>,
    pub message: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BatchSmartEntryDto {
    pub temp_id: String,
    pub date: String,
    pub start_time: String,
    pub end_time: String,
    pub summary: String,
    pub reason: String,
    pub commits_count: usize,
    pub is_selected: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BatchSmartFillResultDto {
    pub success: bool,
    pub total_days: usize,
    pub total_commits: usize,
    pub entries: Vec<BatchSmartEntryDto>,
    pub message: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AudioClipDto {
    pub id: String,
    pub clip_order: u32,
    pub audio_path: String,
    pub duration_secs: u64,
    pub duration_text: String,
    pub transcript: Option<String>,
    pub status: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MeetingMaterialDto {
    pub id: String,
    pub meeting_id: String,
    pub file_name: String,
    pub file_type: String,
    pub file_size: u64,
    pub file_size_text: String,
    pub is_image: bool,
    pub has_text: bool,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MeetingDetailDto {
    pub id: String,
    pub project_id: String,
    pub topic: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub duration_text: String,
    pub status: String,
    pub summary: Option<String>,
    pub quick_notes: Option<String>,
    pub clips: Vec<AudioClipDto>,
    #[serde(default)]
    pub materials: Vec<MeetingMaterialDto>,
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
    pub status: String,
    #[serde(default)]
    pub meetings: Vec<MeetingDetailDto>,
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
    pub active_recording_project_id: Option<String>,
    pub active_recording_project_name: String,
    #[serde(default)]
    pub active_meeting_id: Option<String>,
    pub is_audio_paused: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StopMeetingResultDto {
    pub success: bool,
    #[serde(default)]
    pub meeting_id: Option<String>,
    pub project_id: String,
    pub topic: String,
    pub audio_path: String,
    pub transcript: String,
    pub summary: String,
    pub message: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MeetingSummaryReadyDto {
    pub meeting_id: Option<String>,
    pub project_id: String,
    pub entry_id: Option<String>,
    pub topic: String,
    pub audio_path: String,
    pub transcript: String,
    pub summary: String,
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

fn user_home_dir() -> Option<std::path::PathBuf> {
    dirs::home_dir().or_else(|| {
        std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .ok()
            .map(std::path::PathBuf::from)
    })
}

fn user_download_dir() -> std::path::PathBuf {
    if let Some(d) = dirs::download_dir() {
        if d.exists() {
            return d;
        }
    }
    if let Some(home) = user_home_dir() {
        let candidates = [
            home.join("Downloads"),
            home.join("下载"),
            home.join("Desktop"),
            home.join("桌面"),
        ];
        for c in candidates {
            if c.exists() && c.is_dir() {
                return c;
            }
        }
        return home;
    }
    std::env::temp_dir()
}

fn system_open_path(path: &std::path::Path) -> Result<std::process::Child, std::io::Error> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer").arg(path).spawn()
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(path).spawn()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(path).spawn()
    }
}

fn dirs_or_local_data_dir() -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    if let Ok(appdata) = std::env::var("APPDATA") {
        return std::path::PathBuf::from(appdata).join("mythmanday");
    }
    #[cfg(target_os = "macos")]
    if let Some(home) = user_home_dir() {
        return home
            .join("Library")
            .join("Application Support")
            .join("mythmanday");
    }
    if let Some(home) = user_home_dir() {
        home.join(".config").join("mythmanday")
    } else {
        std::path::PathBuf::from("./data")
    }
}

fn get_materials_dir(meeting_id: &Uuid) -> std::path::PathBuf {
    dirs_or_local_data_dir().join("materials").join(meeting_id.to_string())
}

fn map_material_to_dto(mat: &MeetingMaterial) -> MeetingMaterialDto {
    let size_text = if mat.file_size < 1024 {
        format!("{} B", mat.file_size)
    } else if mat.file_size < 1024 * 1024 {
        format!("{:.1} KB", mat.file_size as f64 / 1024.0)
    } else {
        format!("{:.1} MB", mat.file_size as f64 / (1024.0 * 1024.0))
    };
    MeetingMaterialDto {
        id: mat.id.to_string(),
        meeting_id: mat.meeting_id.to_string(),
        file_name: mat.file_name.clone(),
        file_type: mat.file_type.clone(),
        file_size: mat.file_size,
        file_size_text: size_text,
        is_image: mat.file_type == "image",
        has_text: mat.extracted_text.as_ref().map(|t| !t.trim().is_empty()).unwrap_or(false),
        created_at: mat.created_at.with_timezone(&Local).format("%H:%M").to_string(),
    }
}

// ======================= Tauri Commands =======================

#[tauri::command]
fn get_projects(state: State<AppState>) -> Result<Vec<ProjectDto>, String> {
    let projects = state
        .db
        .lock()
        .unwrap()
        .list_projects()
        .map_err(|e| e.to_string())?;
    let mgr = state.timer_manager.lock().unwrap();
    let mut list = Vec::new();

    for p in projects {
        let status = mgr.get_status(&p.id);
        let is_running = matches!(status, myth_core::timer::TimerStatus::Running { .. });
        let is_paused = matches!(status, myth_core::timer::TimerStatus::Paused { .. });
        let elapsed = mgr.get_elapsed(&p.id);

        let entries = state
            .db
            .lock()
            .unwrap()
            .list_entries_for_project(&p.id)
            .unwrap_or_default();
        let today_str = Local::now().format("%Y-%m-%d").to_string();
        let today_entries: Vec<_> = entries
            .iter()
            .filter(|e| {
                e.start_time
                    .with_timezone(&Local)
                    .format("%Y-%m-%d")
                    .to_string()
                    == today_str
            })
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
            git_repo_path: p.git_repo_path,
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
    let entries = state
        .db
        .lock()
        .unwrap()
        .list_entries_for_project(&pid)
        .unwrap_or_default();
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

        let legacy_audio_path = e.audio_path.clone().unwrap_or_default();
        let legacy_has_audio =
            !legacy_audio_path.is_empty() && std::path::Path::new(&legacy_audio_path).exists();

        let status = if e
            .summary
            .as_ref()
            .map(|s| s.contains("【AI提炼中】"))
            .unwrap_or(false)
        {
            "summarizing".to_string()
        } else {
            "normal".to_string()
        };

        let meetings = state
            .db
            .lock()
            .unwrap()
            .list_meetings_for_entry(&e.id)
            .unwrap_or_default();
        let meeting_dtos: Vec<MeetingDetailDto> = meetings
            .into_iter()
            .map(|m| {
                let clips = state
                    .db
                    .lock()
                    .unwrap()
                    .list_audio_clips_for_meeting(&m.id)
                    .unwrap_or_default();
                let clip_dtos: Vec<AudioClipDto> = clips
                    .into_iter()
                    .map(|c| AudioClipDto {
                        id: c.id.to_string(),
                        clip_order: c.clip_order,
                        audio_path: c.audio_path,
                        duration_secs: c.duration_secs,
                        duration_text: format_duration(c.duration_secs),
                        transcript: c.transcript,
                        status: c.status,
                    })
                    .collect();

                let materials = state
                    .db
                    .lock()
                    .unwrap()
                    .list_materials_for_meeting(&m.id)
                    .unwrap_or_default();
                let material_dtos: Vec<MeetingMaterialDto> = materials
                    .iter()
                    .map(map_material_to_dto)
                    .collect();

                let m_start_local = m.start_at.with_timezone(&Local);
                let m_end_local = m.end_at.map(|d| d.with_timezone(&Local));
                MeetingDetailDto {
                    id: m.id.to_string(),
                    project_id: m.project_id.to_string(),
                    topic: m.topic,
                    start_time: m_start_local.format("%H:%M").to_string(),
                    end_time: m_end_local.map(|d| d.format("%H:%M").to_string()),
                    duration_text: format_duration(m.duration_secs),
                    status: m.status,
                    summary: m.meeting_summary,
                    quick_notes: m.quick_notes,
                    clips: clip_dtos,
                    materials: material_dtos,
                }
            })
            .collect();

        let has_clip_audio = meeting_dtos
            .iter()
            .any(|m| m.clips.iter().any(|c| !c.audio_path.is_empty()));
        let has_audio = legacy_has_audio || has_clip_audio;
        let audio_path = if !legacy_audio_path.is_empty() {
            legacy_audio_path
        } else {
            meeting_dtos
                .iter()
                .flat_map(|m| m.clips.iter())
                .find(|c| !c.audio_path.is_empty())
                .map(|c| c.audio_path.clone())
                .unwrap_or_default()
        };

        let item = EntryDto {
            id: e.id.to_string(),
            project_id: project_id.clone(),
            project_name: proj.map(|p| p.name.clone()).unwrap_or_default(),
            duration_text: format_duration(e.raw_duration_secs),
            days_text: format!("{:.2} 天", e.billable_days),
            amount_text: format!("{} {:.2}", currency, e.total_amount),
            summary_text: e.summary.unwrap_or_default(),
            time_range: format!(
                "{}-{}",
                start_local.format("%H:%M"),
                end_local.format("%H:%M")
            ),
            date_str: date_str.clone(),
            has_audio,
            audio_path,
            raw_secs: e.raw_duration_secs,
            is_invoiced: e.is_invoiced,
            status,
            meetings: meeting_dtos,
        };

        days_map.entry(date_str).or_default().push(item);
    }

    let today_str = Local::now().format("%Y-%m-%d").to_string();

    // 检查是否有正在进行的会议
    let active_rec = state.active_recording.lock().unwrap().clone();
    let is_recording_for_proj = active_rec
        .as_ref()
        .map(|(rec_pid, _, _, _, _)| *rec_pid == pid)
        .unwrap_or(false);

    // 如果今天该项目列表里已经有主卡片，将“会议中”作为其子会议展示；若今天尚未有条目且正在开会，则动态注入一条
    if is_recording_for_proj && !days_map.contains_key(&today_str) {
        let should_show_for_month = match month {
            Some(ref m) => m.is_empty() || today_str.starts_with(m),
            None => true,
        };

        if should_show_for_month {
            let active_topic = active_rec
                .as_ref()
                .map(|(_, _, top, _, _)| top.clone())
                .unwrap_or_else(|| "现场会议沟通与决议".to_string());
            let audio_path = active_rec
                .as_ref()
                .map(|(_, _, _, p, _)| p.display().to_string())
                .unwrap_or_default();
            let has_audio = !audio_path.is_empty() && std::path::Path::new(&audio_path).exists();
            let mgr = state.timer_manager.lock().unwrap();
            let elapsed_secs = mgr.get_elapsed(&pid);
            let now_local = Local::now();
            let start_time_local = now_local - chrono::Duration::seconds(elapsed_secs as i64);

            let (_billable_hours, billable_days, amount) = if let Some(p) = proj {
                let bh = BillingEngine::calculate_billable_hours(elapsed_secs, p.rounding_minutes);
                let bd =
                    BillingEngine::calculate_billable_days(bh, p.hours_per_day, p.day_calc_mode);
                let (_, amt) = BillingEngine::calculate_amount(p, bh, bd);
                (bh, bd, amt)
            } else {
                (0.0, 0.0, 0.0)
            };

            let live_entry = EntryDto {
                id: format!("active-meeting-{}", pid),
                project_id: project_id.clone(),
                project_name: proj.map(|p| p.name.clone()).unwrap_or_default(),
                duration_text: format_duration(elapsed_secs),
                days_text: format!("{:.2} 天", billable_days),
                amount_text: format!("{} {:.2}", currency, amount),
                summary_text: format!("【会议进行中 🎙️】{}", active_topic),
                time_range: format!("{}-进行中", start_time_local.format("%H:%M")),
                date_str: today_str.clone(),
                has_audio,
                audio_path,
                raw_secs: elapsed_secs,
                is_invoiced: false,
                status: "recording".to_string(),
                meetings: vec![],
            };

            days_map
                .entry(today_str.clone())
                .or_default()
                .insert(0, live_entry);
        }
    }

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
                if e.start_time
                    .with_timezone(&Local)
                    .format("%Y-%m-%d")
                    .to_string()
                    == today_str
                {
                    total_days += e.billable_days;
                    total_amount += e.total_amount;
                }
            }
        }
    }

    let mgr = state.timer_manager.lock().unwrap();
    let running_count = mgr.active_channel_ids().len();
    let audio_rec = state.audio_recorder.lock().unwrap();
    let active_rec = state.active_recording.lock().unwrap();
    let has_active_rec = active_rec.is_some();
    let is_recording_audio = audio_rec.is_recording() || has_active_rec;
    let is_audio_paused = audio_rec.is_paused();
    let (active_recording_project_id, active_meeting_id, active_recording_project_name) = active_rec
        .as_ref()
        .map(|(pid, mid, topic, _, _)| (Some(pid.to_string()), Some(mid.to_string()), topic.clone()))
        .unwrap_or((None, None, String::new()));

    Ok(GlobalStatsDto {
        total_days: format!("{:.2} 人天", total_days),
        total_amount: format!("¥ {:.2}", total_amount),
        running_count,
        is_recording_audio,
        active_recording_project_id,
        active_recording_project_name,
        active_meeting_id,
        is_audio_paused,
    })
}

#[tauri::command]
fn start_timer(state: State<AppState>, project_id: String) -> Result<(), String> {
    let pid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let today_str = Local::now().format("%Y-%m-%d").to_string();

    // 检查今天是否已有已结算/已结束的该项目工时记录（实现一日一条主工时，支持恢复计时）
    let existing_today = state
        .db
        .lock()
        .unwrap()
        .get_entry_for_project_and_date(&pid, &today_str)
        .unwrap_or(None);
    let base_secs = existing_today
        .as_ref()
        .map(|e| e.raw_duration_secs)
        .unwrap_or(0);

    let mut mgr = state.timer_manager.lock().unwrap();
    mgr.start_channel_with_base(pid, base_secs);
    let now = Utc::now();
    let session_start = mgr
        .get_timer(&pid)
        .and_then(|t| t.session_start_at)
        .unwrap_or(now);
    let _ = state
        .db
        .lock()
        .unwrap()
        .upsert_active_timer(&ActiveTimerRecord {
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
    let session_start = mgr
        .get_timer(&pid)
        .and_then(|t| t.session_start_at)
        .unwrap_or_else(Utc::now);
    let _ = state
        .db
        .lock()
        .unwrap()
        .upsert_active_timer(&ActiveTimerRecord {
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
    let session_start = mgr
        .get_timer(&pid)
        .and_then(|t| t.session_start_at)
        .unwrap_or(now);
    let _ = state
        .db
        .lock()
        .unwrap()
        .upsert_active_timer(&ActiveTimerRecord {
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

        let projs = state
            .db
            .lock()
            .unwrap()
            .list_projects()
            .map_err(|e| e.to_string())?;
        if let Some(p) = projs.iter().find(|proj| proj.id == pid) {
            let billable_hours =
                BillingEngine::calculate_billable_hours(elapsed_secs, p.rounding_minutes);
            let billable_days = BillingEngine::calculate_billable_days(
                billable_hours,
                p.hours_per_day,
                p.day_calc_mode,
            );
            let (rate, amount) = BillingEngine::calculate_amount(p, billable_hours, billable_days);

            let today_str = Local::now().format("%Y-%m-%d").to_string();
            let mut git_summary = myth_git::GitActivitySummary::default();
            if let Some(ref git_path) = p.git_repo_path {
                if let Ok(scanned) = GitScanner::extract_activity(git_path, start_at, end_at) {
                    git_summary = scanned;
                }
            }

            let notes = state
                .db
                .lock()
                .unwrap()
                .list_quick_notes(&pid)
                .unwrap_or_default();
            let user_notes: Vec<&str> = notes
                .iter()
                .map(|n| n.content.as_str())
                .filter(|s| !s.starts_with('['))
                .collect();

            let summary_desc = if !git_summary.commits.is_empty() {
                format!(
                    "代码研发与提交 {} 次：{}",
                    git_summary.commits.len(),
                    git_summary.commits.join("; ")
                )
            } else if !user_notes.is_empty() {
                format!("阶段性研发与联调：{}", user_notes.join("；"))
            } else {
                String::new()
            };

            let db = state.db.clone();
            let (_entry_id, _final_summary) = {
                let guard = db.lock().unwrap();
                let existing_opt = guard
                    .get_entry_for_project_and_date(&pid, &today_str)
                    .unwrap_or(None);
                if let Some(mut existing) = existing_opt {
                    let existing_id = existing.id;
                    existing.end_time = end_at;
                    existing.raw_duration_secs = elapsed_secs;
                    existing.billable_hours = billable_hours;
                    existing.billable_days = billable_days;
                    existing.rate_at_moment = rate;
                    existing.total_amount = amount;

                    // 若已有用户填写的 summary，保持已有内容；若为空，才填充自动抓取的描述
                    let combined_sum = match existing.summary.as_ref() {
                        Some(prev) if !prev.trim().is_empty() => prev.clone(),
                        _ => summary_desc.clone(),
                    };
                    existing.summary = if combined_sum.trim().is_empty() {
                        None
                    } else {
                        Some(combined_sum.clone())
                    };
                    let _ = guard.insert_time_entry(&existing);

                    // 记录打卡时段 WorkSpan
                    let spans = guard
                        .list_work_spans_for_entry(&existing_id)
                        .unwrap_or_default();
                    let span_order = (spans.len() + 1) as u32;
                    let span = WorkSpan::new(
                        existing_id,
                        span_order,
                        start_at,
                        end_at,
                        end_at.signed_duration_since(start_at).num_seconds().max(0) as u64,
                        "timer",
                    );
                    let _ = guard.insert_work_span(&span);

                    (existing_id, combined_sum)
                } else {
                    let new_id = Uuid::new_v4();
                    let new_entry = TimeEntry {
                        id: new_id,
                        project_id: pid,
                        start_time: start_at,
                        end_time: end_at,
                        raw_duration_secs: elapsed_secs,
                        billable_hours,
                        billable_days,
                        rate_at_moment: rate,
                        total_amount: amount,
                        summary: if summary_desc.trim().is_empty() {
                            None
                        } else {
                            Some(summary_desc.clone())
                        },
                        audio_path: None,
                        is_invoiced: false,
                        created_at: Utc::now(),
                    };
                    let _ = guard.insert_time_entry(&new_entry);

                    // 记录首次打卡时段 WorkSpan
                    let span = WorkSpan::new(new_id, 1, start_at, end_at, elapsed_secs, "timer");
                    let _ = guard.insert_work_span(&span);

                    (new_id, summary_desc)
                }
            };

            return Ok(format!(
                "今日工时已归档：{:.2} 人天 ({})",
                billable_days,
                format_duration(elapsed_secs)
            ));
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
            state
                .db
                .lock()
                .unwrap()
                .insert_project(&p)
                .map_err(|e| e.to_string())?;
            (p.id, p.name)
        } else {
            let p_uuid = Uuid::parse_str(&pid_str).map_err(|e| e.to_string())?;
            let projs = state.db.lock().unwrap().list_projects().unwrap_or_default();
            let name = projs
                .iter()
                .find(|p| p.id == p_uuid)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| topic.clone());
            (p_uuid, name)
        }
    } else {
        let mut p = Project::new(&format!("🎙️ {}", topic.trim()));
        p.billing_type = BillingType::Hourly;
        p.hourly_rate = 350.0;
        p.hours_per_day = 8.0;
        p.day_calc_mode = DayCalcMode::Exact;
        state
            .db
            .lock()
            .unwrap()
            .insert_project(&p)
            .map_err(|e| e.to_string())?;
        (p.id, p.name)
    };

    let today_str = Local::now().format("%Y-%m-%d").to_string();
    let now = Utc::now();

    // 1. 确保当天有该项目的主工时记录（实现一日一记录，作为所有工时与会议的聚合根）
    let entry_id = {
        let guard = state.db.lock().unwrap();
        let existing = guard
            .get_entry_for_project_and_date(&pid, &today_str)
            .unwrap_or(None);
        if let Some(e) = existing {
            e.id
        } else {
            let new_id = Uuid::new_v4();
            let new_entry = TimeEntry {
                id: new_id,
                project_id: pid,
                start_time: now,
                end_time: now,
                raw_duration_secs: 0,
                billable_hours: 0.0,
                billable_days: 0.0,
                rate_at_moment: 350.0,
                total_amount: 0.0,
                summary: Some(format!("【会议进行中 🎙️】{}", topic)),
                audio_path: None,
                is_invoiced: false,
                created_at: now,
            };
            let _ = guard.insert_time_entry(&new_entry);
            new_id
        }
    };

    // 2. 创建 Meeting 实体入库校验（提前执行，失败时立刻终止退出，避免对计时器状态产生任何副作用）
    let meeting = Meeting::new(entry_id, pid, topic.clone());
    let meeting_id = meeting.id;
    {
        let guard = state.db.lock().unwrap();
        guard
            .insert_meeting(&meeting)
            .map_err(|e| format!("写入会议数据失败: {}", e))?;
        let _ = guard.set_setting("active_meeting_id", &meeting_id.to_string());
        let _ = guard.set_setting("active_meeting_pid", &pid.to_string());
        let _ = guard.set_setting("active_meeting_topic", &topic);
    }

    // 3. 自动启动主工时通道（会议入库成功后启动；跨天暂停计时器以今日工时基底重置）
    {
        let existing_today = state
            .db
            .lock()
            .unwrap()
            .get_entry_for_project_and_date(&pid, &today_str)
            .unwrap_or(None);
        let base_secs = existing_today
            .as_ref()
            .map(|e| e.raw_duration_secs)
            .unwrap_or(0);

        let mut mgr = state.timer_manager.lock().unwrap();
        let status = mgr.get_status(&pid);
        match status {
            myth_core::timer::TimerStatus::Stopped => {
                mgr.start_channel_with_base(pid, base_secs);
            }
            myth_core::timer::TimerStatus::Paused { .. } => {
                let session_start = mgr
                    .get_timer(&pid)
                    .and_then(|t| t.session_start_at)
                    .unwrap_or(now);
                let session_date = session_start
                    .with_timezone(&Local)
                    .format("%Y-%m-%d")
                    .to_string();
                if session_date != today_str {
                    // 跨天暂停：重置并以今日已结工时为基底开启新通道，防止前日工时漂移到今日
                    mgr.start_channel_with_base(pid, base_secs);
                } else {
                    mgr.start_channel(pid);
                }
            }
            myth_core::timer::TimerStatus::Running { .. } => {}
        }

        let session_start = mgr
            .get_timer(&pid)
            .and_then(|t| t.session_start_at)
            .unwrap_or(now);
        let _ = state
            .db
            .lock()
            .unwrap()
            .upsert_active_timer(&ActiveTimerRecord {
                project_id: pid,
                session_start_at: session_start,
                current_started_at: Some(now),
                accumulated_secs: mgr.get_elapsed(&pid),
                is_paused: false,
                last_heartbeat: now,
            });
    }

    // 4. 若勾选录音，开启首个录音片段
    if record_audio {
        let rec_dir = state.db_dir.join("recordings");
        let _ = std::fs::create_dir_all(&rec_dir);
        let timestamp_str = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let wav_file = rec_dir.join(format!(
            "meeting_{}_clip1_{}.wav",
            meeting_id, timestamp_str
        ));
        let mut recorder = state.audio_recorder.lock().unwrap();
        if let Ok(_) = recorder.start_recording(&wav_file) {
            *state.active_recording.lock().unwrap() =
                Some((pid, meeting_id, topic.clone(), wav_file.clone(), Utc::now()));
            let guard = state.db.lock().unwrap();
            let _ = guard.set_setting("active_meeting_path", &wav_file.display().to_string());
        }
    } else {
        *state.active_recording.lock().unwrap() = None;
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

/// 在当前进行中的会议中启动新的录音片段（支持一场会议多次录音）
#[tauri::command]
fn start_recording_for_meeting(
    state: State<AppState>,
    meeting_id: String,
) -> Result<String, String> {
    let m_uuid = Uuid::parse_str(&meeting_id).map_err(|e| e.to_string())?;
    let db = state.db.lock().unwrap();
    let meeting = db
        .get_meeting(&m_uuid)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "未找到指定会议".to_string())?;
    let pid = meeting.project_id;
    let topic = meeting.topic;

    let clips = db.list_audio_clips_for_meeting(&m_uuid).unwrap_or_default();
    let clip_order = (clips.len() + 1) as u32;

    let rec_dir = state.db_dir.join("recordings");
    let _ = std::fs::create_dir_all(&rec_dir);
    let timestamp_str = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let wav_file = rec_dir.join(format!(
        "meeting_{}_clip{}_{}.wav",
        m_uuid, clip_order, timestamp_str
    ));

    let mut recorder = state.audio_recorder.lock().unwrap();
    if recorder.is_recording() {
        return Err("当前已有录音正在进行中".to_string());
    }

    recorder
        .start_recording(&wav_file)
        .map_err(|e| e.to_string())?;
    *state.active_recording.lock().unwrap() =
        Some((pid, m_uuid, topic, wav_file.clone(), Utc::now()));
    let _ = db.set_setting("active_meeting_path", &wav_file.display().to_string());

    Ok("已开启新录音片段".to_string())
}

/// 停止当前正在录制的单条录音片段（录音附属于会议，单次停止仅做音频转写，绝不生成纪要、不关会议）
#[tauri::command]
async fn stop_recording_clip(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    let active = state.active_recording.lock().unwrap().take();
    let (_pid, meeting_id, _topic, wav_path, start_time) =
        active.ok_or_else(|| "当前没有正在进行的录音片段".to_string())?;

    let saved_audio_path = {
        let mut recorder = state.audio_recorder.lock().unwrap();
        if recorder.is_recording() {
            recorder.stop_recording().ok()
        } else if wav_path.exists() {
            Some(wav_path.clone())
        } else {
            None
        }
    };

    let audio_file = saved_audio_path.unwrap_or(wav_path);
    let audio_str = audio_file.display().to_string();
    let duration_secs = Utc::now()
        .signed_duration_since(start_time)
        .num_seconds()
        .max(0) as u64;

    // 清空活跃录音临时配置
    {
        let db = state.db.lock().unwrap();
        let _ = db.set_setting("active_meeting_path", "");
    }

    // 录音片段入库
    let clip_id = Uuid::new_v4();
    let clip_order = {
        let db = state.db.lock().unwrap();
        let clips = db
            .list_audio_clips_for_meeting(&meeting_id)
            .unwrap_or_default();
        (clips.len() + 1) as u32
    };

    let mut clip = MeetingAudioClip::new(meeting_id, clip_order, audio_str.clone(), duration_secs);
    clip.id = clip_id;
    {
        let db = state.db.lock().unwrap();
        let _ = db.insert_audio_clip(&clip);
    }

    // 后台异步进行 Whisper 音频转写并沉淀为该片段的 transcript
    let db_for_async = state.db.clone();
    let audio_file_clone = audio_file.clone();
    let app_handle = app.clone();

    tokio::spawn(async move {
        let ai_config = {
            let guard = db_for_async.lock().unwrap();
            load_ai_config(&guard)
        };

        let dynamic_prompt = {
            let guard = db_for_async.lock().unwrap();
            let mats = guard.list_materials_for_meeting(&meeting_id).unwrap_or_default();
            if mats.is_empty() {
                None
            } else {
                let file_names: Vec<&str> = mats.iter().map(|m| m.file_name.as_str()).collect();
                let doc_texts: Vec<&str> = mats
                    .iter()
                    .filter_map(|m| m.extracted_text.as_deref())
                    .collect();
                let gloss = myth_ai::extract_glossary_for_asr(&file_names, &doc_texts);
                if gloss.is_empty() { None } else { Some(gloss) }
            }
        };

        let summarizer = AiSummarizer::new(ai_config);
        if audio_file_clone.exists() {
            let res = tokio::time::timeout(
                std::time::Duration::from_secs(600),
                summarizer.transcribe_audio_with_prompt(&audio_file_clone, dynamic_prompt.as_deref()),
            )
            .await;
            match res {
                Ok(Ok(transcript_text)) if !transcript_text.trim().is_empty() => {
                    let guard = db_for_async.lock().unwrap();
                    if let Ok(Some(mut target)) = guard.get_audio_clip(&clip_id) {
                        target.transcript = Some(transcript_text);
                        target.status = "transcribed".to_string();
                        let _ = guard.insert_audio_clip(&target);
                    }
                }
                _ => {
                    let guard = db_for_async.lock().unwrap();
                    if let Ok(Some(mut target)) = guard.get_audio_clip(&clip_id) {
                        target.status = "failed".to_string();
                        let _ = guard.insert_audio_clip(&target);
                    }
                }
            }
        } else {
            let guard = db_for_async.lock().unwrap();
            if let Ok(Some(mut target)) = guard.get_audio_clip(&clip_id) {
                target.status = "failed".to_string();
                let _ = guard.insert_audio_clip(&target);
            }
        }

        // 录音转写完成后，自动将临时 WAV 压缩转码为 MP3 归档，彻底释放磁盘空间并丢弃原 WAV
        if audio_file_clone
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("wav"))
            .unwrap_or(false)
        {
            let path_to_convert = audio_file_clone.clone();
            let converted_mp3 = tokio::task::spawn_blocking(move || {
                myth_audio::convert_wav_to_mp3(&path_to_convert)
            })
            .await;

            if let Ok(Ok(mp3_path)) = converted_mp3 {
                if mp3_path.exists() {
                    let _ = std::fs::remove_file(&audio_file_clone);
                    let guard = db_for_async.lock().unwrap();
                    if let Ok(Some(mut target)) = guard.get_audio_clip(&clip_id) {
                        target.audio_path = mp3_path.display().to_string();
                        let _ = guard.insert_audio_clip(&target);
                    }
                }
            }
        }

        let _ = app_handle.emit("meeting_clip_ready", meeting_id.to_string());
    });

    Ok(format!(
        "录音片段 {} 已保存，正在后台转写原始文本...",
        clip_order
    ))
}

/// 对转写失败或需要重新识别的单条录音片段进行重新转写
#[tauri::command]
async fn retry_transcribe_audio_clip(
    app: AppHandle,
    state: State<'_, AppState>,
    clip_id: String,
) -> Result<String, String> {
    let cid = Uuid::parse_str(&clip_id).map_err(|e| e.to_string())?;
    let db = state.db.clone();
    let (meeting_id, audio_path, clip_order) = {
        let guard = db.lock().unwrap();
        let mut clip = guard
            .get_audio_clip(&cid)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "未找到指定录音片段".to_string())?;
        clip.status = "pending".to_string();
        let _ = guard.insert_audio_clip(&clip);
        (clip.meeting_id, clip.audio_path.clone(), clip.clip_order)
    };

    let _ = app.emit("meeting_clip_ready", meeting_id.to_string());

    let db_for_async = db.clone();
    let app_handle = app.clone();
    let audio_file = std::path::PathBuf::from(&audio_path);

    tokio::spawn(async move {
        let ai_config = {
            let guard = db_for_async.lock().unwrap();
            load_ai_config(&guard)
        };

        let dynamic_prompt = {
            let guard = db_for_async.lock().unwrap();
            let mats = guard.list_materials_for_meeting(&meeting_id).unwrap_or_default();
            if mats.is_empty() {
                None
            } else {
                let file_names: Vec<&str> = mats.iter().map(|m| m.file_name.as_str()).collect();
                let doc_texts: Vec<&str> = mats
                    .iter()
                    .filter_map(|m| m.extracted_text.as_deref())
                    .collect();
                let gloss = myth_ai::extract_glossary_for_asr(&file_names, &doc_texts);
                if gloss.is_empty() { None } else { Some(gloss) }
            }
        };

        let summarizer = AiSummarizer::new(ai_config);
        if audio_file.exists() {
            let res = tokio::time::timeout(
                std::time::Duration::from_secs(600),
                summarizer.transcribe_audio_with_prompt(&audio_file, dynamic_prompt.as_deref()),
            )
            .await;
            match res {
                Ok(Ok(transcript_text)) if !transcript_text.trim().is_empty() => {
                    let guard = db_for_async.lock().unwrap();
                    if let Ok(Some(mut target)) = guard.get_audio_clip(&cid) {
                        target.transcript = Some(transcript_text);
                        target.status = "transcribed".to_string();
                        let _ = guard.insert_audio_clip(&target);
                    }
                }
                _ => {
                    let guard = db_for_async.lock().unwrap();
                    if let Ok(Some(mut target)) = guard.get_audio_clip(&cid) {
                        target.status = "failed".to_string();
                        let _ = guard.insert_audio_clip(&target);
                    }
                }
            }
        } else {
            let guard = db_for_async.lock().unwrap();
            if let Ok(Some(mut target)) = guard.get_audio_clip(&cid) {
                target.status = "failed".to_string();
                let _ = guard.insert_audio_clip(&target);
            }
        }

        // 若历史片段仍为临时 WAV，转写完成后自动转为 MP3 归档并清理原 WAV
        if audio_file
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("wav"))
            .unwrap_or(false)
        {
            let path_to_convert = audio_file.clone();
            let converted_mp3 = tokio::task::spawn_blocking(move || {
                myth_audio::convert_wav_to_mp3(&path_to_convert)
            })
            .await;

            if let Ok(Ok(mp3_path)) = converted_mp3 {
                if mp3_path.exists() {
                    let _ = std::fs::remove_file(&audio_file);
                    let guard = db_for_async.lock().unwrap();
                    if let Ok(Some(mut target)) = guard.get_audio_clip(&cid) {
                        target.audio_path = mp3_path.display().to_string();
                        let _ = guard.insert_audio_clip(&target);
                    }
                }
            }
        }

        let _ = app_handle.emit("meeting_clip_ready", meeting_id.to_string());
    });

    Ok(format!("已启动片段 {} 的重试转写任务", clip_order))
}

/// 结束整场会议（核心业务：汇聚名下所有录音片段文字实录统一生成会议纪要；提供双选项【继续工时 / 停止今日工时】）
#[tauri::command]
async fn finish_meeting(
    app: AppHandle,
    state: State<'_, AppState>,
    meeting_id: Option<String>,
    stop_project_timer: bool,
) -> Result<StopMeetingResultDto, String> {
    // 1. 如果当前还有正在录制的片段，先停止它
    if state.active_recording.lock().unwrap().is_some() {
        let _ = stop_recording_clip(app.clone(), state.clone()).await;
    }

    // 2. 找到目标会议
    let db = state.db.clone();
    let meeting_opt = {
        let guard = db.lock().unwrap();
        let mid = if let Some(ref m_str) = meeting_id {
            Uuid::parse_str(m_str).ok()
        } else {
            guard
                .get_setting("active_meeting_id")
                .ok()
                .flatten()
                .and_then(|s| Uuid::parse_str(&s).ok())
        };
        mid.and_then(|id| guard.get_meeting(&id).ok().flatten())
    };

    let mut meeting = meeting_opt.ok_or_else(|| "未找到活跃会议".to_string())?;
    let m_id = meeting.id;
    let pid = meeting.project_id;
    let entry_id = meeting.entry_id;
    let topic = meeting.topic.clone();

    // 标记会议结束时间与状态
    let now = Utc::now();
    meeting.end_at = Some(now);
    let duration_secs = now
        .signed_duration_since(meeting.start_at)
        .num_seconds()
        .max(0) as u64;
    meeting.duration_secs = duration_secs;
    let duration_hours = (duration_secs as f64) / 3600.0;
    meeting.status = "summarizing".to_string();
    {
        let guard = db.lock().unwrap();
        let _ = guard.insert_meeting(&meeting);
        let _ = guard.set_setting("active_meeting_id", "");
        let _ = guard.set_setting("active_meeting_pid", "");
        let _ = guard.set_setting("active_meeting_topic", "");
        let _ = guard.set_setting("active_meeting_path", "");
    }

    // 3. 处理是否同时停止工时 (方案2：双选项)
    if stop_project_timer {
        let _ = stop_timer(state.clone(), pid.to_string());
    }

    // 4. 汇集所有录音片段文本进行综合纪要提炼
    let db_for_async = db.clone();
    let app_handle = app.clone();
    let topic_clone = topic.clone();

    tokio::spawn(async move {
        let ai_config = {
            let guard = db_for_async.lock().unwrap();
            load_ai_config(&guard)
        };

        let summarizer = AiSummarizer::new(ai_config);

        // 1. 等待名下所有音频片段转录完成 (最长等待 120 秒，避免并行 race condition)
        let wait_start = std::time::Instant::now();
        loop {
            let pending_count = {
                let guard = db_for_async.lock().unwrap();
                let clips = guard
                    .list_audio_clips_for_meeting(&m_id)
                    .unwrap_or_default();
                clips
                    .iter()
                    .filter(|c| c.status == "pending" && c.transcript.is_none())
                    .count()
            };
            if pending_count == 0 || wait_start.elapsed() > std::time::Duration::from_secs(120) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        let clips = db_for_async
            .lock()
            .unwrap()
            .list_audio_clips_for_meeting(&m_id)
            .unwrap_or_default();
        let notes = db_for_async
            .lock()
            .unwrap()
            .list_quick_notes(&pid)
            .unwrap_or_default();

        // 时序合并所有录音片段的原始实录
        let mut combined_transcript = String::new();
        for c in &clips {
            if let Some(ref txt) = c.transcript {
                if !txt.trim().is_empty() {
                    combined_transcript.push_str(&format!(
                        "【录音片段 {}】：\n{}\n\n",
                        c.clip_order,
                        txt.trim()
                    ));
                }
            }
        }

        let materials = db_for_async
            .lock()
            .unwrap()
            .list_materials_for_meeting(&m_id)
            .unwrap_or_default();

        let summary = match summarizer.summarize_meeting(&topic_clone, duration_hours, &notes, &materials, None, Some(&combined_transcript)).await {
            Ok(sum) => sum,
            Err(_) => format!("1. 【核心议题】{}\n2. 【达成共识】已完成会议沟通与各录音片段归档。\n3. 【后续待办】待人工补充纪要要点。", topic_clone),
        };

        // 更新会议记录的 meeting_summary，并清理每日工时中的【会议进行中 🎙️】临时占位符
        {
            let guard = db_for_async.lock().unwrap();
            if let Ok(Some(mut m)) = guard.get_meeting(&m_id) {
                m.meeting_summary = Some(summary.clone());
                m.status = "completed".to_string();
                let _ = guard.insert_meeting(&m);
            }
            if let Ok(Some(mut e)) = guard.get_time_entry(&entry_id) {
                if let Some(ref s) = e.summary {
                    if s.contains("【会议进行中 🎙️】") {
                        e.summary = None;
                        let _ = guard.insert_time_entry(&e);
                    }
                }
            }
        }

        let payload = MeetingSummaryReadyDto {
            meeting_id: Some(m_id.to_string()),
            project_id: pid.to_string(),
            entry_id: Some(entry_id.to_string()),
            topic: topic_clone,
            audio_path: String::new(),
            transcript: combined_transcript,
            summary,
        };
        let _ = app_handle.emit("meeting_summary_ready", payload);
    });

    Ok(StopMeetingResultDto {
        success: true,
        meeting_id: Some(m_id.to_string()),
        project_id: pid.to_string(),
        topic: topic.clone(),
        audio_path: String::new(),
        transcript: String::new(),
        summary: format!("【会议进行中/提炼中】{}", topic),
        message: if stop_project_timer {
            "会议与今日工时均已结束，AI 正在后台提炼综合纪要！".to_string()
        } else {
            "会议已结束，项目主工时继续计时，AI 正在后台提炼纪要！".to_string()
        },
    })
}

/// 兼容老接口
#[tauri::command]
async fn stop_current_meeting(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<StopMeetingResultDto, String> {
    finish_meeting(app, state, None, false).await
}

#[tauri::command]
fn save_meeting_summary_to_project(
    state: State<AppState>,
    project_id: String,
    topic: String,
    summary: String,
    audio_path: Option<String>,
) -> Result<String, String> {
    let pid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let db = state.db.lock().unwrap();

    // 存为项目便签记录，作为交付与讨论凭据
    let audio_tag = if let Some(ref a) = audio_path {
        format!("\n[录音凭据: {}]", a)
    } else {
        String::new()
    };
    let content = format!("【会议纪要：{}】\n{}{}", topic, summary, audio_tag);
    let note = QuickNote::new(pid, content.trim());
    db.insert_quick_note(&note).map_err(|e| e.to_string())?;

    Ok("会议纪要已成功保存至项目备忘与交付档案中！".to_string())
}

#[tauri::command]
async fn upload_meeting_material(
    state: State<'_, AppState>,
    meeting_id: String,
    file_name: String,
    base64_data: String,
) -> Result<MeetingMaterialDto, String> {
    let mid = Uuid::parse_str(&meeting_id).map_err(|e| format!("会议ID格式错误: {}", e))?;
    let raw_b64 = if let Some(idx) = base64_data.find(";base64,") {
        &base64_data[idx + 8..]
    } else {
        &base64_data
    };
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(raw_b64.trim())
        .map_err(|e| format!("Base64 解码失败: {}", e))?;

    let target_dir = get_materials_dir(&mid);
    tokio::fs::create_dir_all(&target_dir)
        .await
        .map_err(|e| format!("创建参考资料目录失败: {}", e))?;

    // 防止同名文件覆盖：若存在同名文件，追加短 uuid
    let mut safe_filename = file_name.trim().to_string();
    if safe_filename.is_empty() {
        safe_filename = format!("material_{}.png", &Uuid::new_v4().to_string()[..8]);
    }
    let mut target_path = target_dir.join(&safe_filename);
    if target_path.exists() {
        let stem = std::path::Path::new(&safe_filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file");
        let ext = std::path::Path::new(&safe_filename)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let new_name = if ext.is_empty() {
            format!("{}_{}", stem, &Uuid::new_v4().to_string()[..6])
        } else {
            format!("{}_{}.{}", stem, &Uuid::new_v4().to_string()[..6], ext)
        };
        safe_filename = new_name;
        target_path = target_dir.join(&safe_filename);
    }

    tokio::fs::write(&target_path, &bytes)
        .await
        .map_err(|e| format!("保存文件失败: {}", e))?;

    let is_img = myth_ai::is_image_file(&target_path);
    let extracted_text = if !is_img && myth_ai::is_document_file(&target_path) {
        let p_clone = target_path.clone();
        tokio::task::spawn_blocking(move || myth_ai::extract_document_text(&p_clone))
            .await
            .unwrap_or(None)
    } else {
        None
    };

    let _ext = target_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let file_type = if is_img {
        "image".to_string()
    } else {
        "document".to_string()
    };

    let material = MeetingMaterial {
        id: Uuid::new_v4(),
        meeting_id: mid,
        file_name: safe_filename,
        file_path: target_path.to_string_lossy().to_string(),
        file_type,
        file_size: bytes.len() as u64,
        extracted_text,
        created_at: Utc::now(),
    };

    {
        let db = state.db.lock().unwrap();
        db.insert_material(&material)
            .map_err(|e| format!("数据库保存参考资料失败: {}", e))?;
    }

    Ok(map_material_to_dto(&material))
}

#[tauri::command]
fn delete_meeting_material(
    state: State<'_, AppState>,
    material_id: String,
) -> Result<(), String> {
    let mat_id = Uuid::parse_str(&material_id).map_err(|e| e.to_string())?;
    let db = state.db.lock().unwrap();
    if let Ok(Some(mat)) = db.get_material(&mat_id) {
        let path = std::path::PathBuf::from(&mat.file_path);
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
        db.delete_material(&mat_id).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn list_meeting_materials(
    state: State<'_, AppState>,
    meeting_id: String,
) -> Result<Vec<MeetingMaterialDto>, String> {
    let mid = Uuid::parse_str(&meeting_id).map_err(|e| e.to_string())?;
    let db = state.db.lock().unwrap();
    let mats = db.list_materials_for_meeting(&mid).map_err(|e| e.to_string())?;
    Ok(mats.iter().map(map_material_to_dto).collect())
}

#[tauri::command]
async fn get_material_data_url(
    state: State<'_, AppState>,
    material_id: String,
) -> Result<String, String> {
    let mat_id = Uuid::parse_str(&material_id).map_err(|e| e.to_string())?;
    let mat = {
        let db = state.db.lock().unwrap();
        db.get_material(&mat_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "未找到参考资料".to_string())?
    };
    let path = std::path::PathBuf::from(&mat.file_path);
    if !path.exists() {
        return Err("资料文件在磁盘上不存在".to_string());
    }
    let bytes = tokio::fs::read(&path).await.map_err(|e| e.to_string())?;
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("png")
        .to_lowercase();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    };
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{};base64,{}", mime, b64))
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MeetingRepoTargetInfo {
    pub repo_root: String,
    pub parent_dir: String,
    pub is_remembered: bool,
    pub folder_name: String,
    pub material_count: usize,
    pub audio_count: usize,
    pub available_dirs: Vec<String>,
    pub ai_reason: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WriteMeetingToRepoResult {
    pub success: bool,
    pub full_path: String,
    pub relative_path: String,
    pub commit_hash_or_msg: Option<String>,
}

fn sanitize_folder_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | ' ' => '_',
            other => other,
        })
        .collect();
    let trimmed = sanitized.trim_matches('_');
    if trimmed.is_empty() {
        "会议纪要".to_string()
    } else {
        trimmed.to_string()
    }
}

/// 遍历仓库最多 3 层深度的所有有效目录（排除 .git、node_modules、target、dist 等）
fn scan_repository_directory_tree(repo_root: &std::path::Path) -> Vec<String> {
    let mut results = Vec::new();

    fn walk(root: &std::path::Path, current: &std::path::Path, depth: usize, out: &mut Vec<String>) {
        if depth > 3 {
            return;
        }
        if let Ok(entries) = std::fs::read_dir(current) {
            for entry in entries.flatten() {
                if let Ok(ft) = entry.file_type() {
                    if ft.is_dir() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.starts_with('.') || name == "target" || name == "node_modules" || name == "dist" || name == "vendor" {
                            continue;
                        }
                        let path = entry.path();
                        if let Ok(rel) = path.strip_prefix(root) {
                            let rel_str = rel.to_string_lossy().to_string();
                            out.push(rel_str);
                        }
                        walk(root, &path, depth + 1, out);
                    }
                }
            }
        }
    }

    walk(repo_root, repo_root, 1, &mut results);
    results.sort();
    results
}

#[tauri::command]
async fn get_meeting_repo_target_info(
    state: State<'_, AppState>,
    project_id: String,
    meeting_id: Option<String>,
    topic: Option<String>,
    summary: Option<String>,
) -> Result<MeetingRepoTargetInfo, String> {
    let pid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let (project, remembered_dir, summary_snippet, ai_config) = {
        let db = state.db.lock().unwrap();

        let project = db
            .get_project(&pid)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "未找到项目".to_string())?;

        let memory_key = format!("meeting_repo_parent_dir_{}", pid);
        let remembered = db.get_setting(&memory_key).ok().flatten();

        // 优先使用前端传入的最新 summary，若未传或为空才查数据库
        let mut sum_text = summary.unwrap_or_default();
        if sum_text.trim().is_empty() {
            if let Some(ref mid_str) = meeting_id {
                if let Ok(mid) = Uuid::parse_str(mid_str) {
                    if let Ok(Some(m)) = db.get_meeting(&mid) {
                        if let Some(ref s) = m.meeting_summary {
                            sum_text = s.clone();
                        }
                    }
                }
            }
        }

        let ai_conf = load_ai_config(&db);

        (project, remembered, sum_text, ai_conf)
    };

    let git_repo_path_str = project
        .git_repo_path
        .ok_or_else(|| "该项目尚未关联本地 Git 仓库路径，请先在项目设置中配置仓库路径！".to_string())?;

    let repo_root = std::path::PathBuf::from(&git_repo_path_str);
    if !repo_root.is_dir() {
        return Err(format!("项目配置的 Git 路径不存在或不是文件夹: {}", git_repo_path_str));
    }

    // 会议详情、材料与录音数量
    let mut mat_count = 0;
    let mut audio_count = 0;
    // 优先采用调用方传入的 topic（用户可能在弹窗中修改过），未传时才取数据库
    let mut m_topic = topic.unwrap_or_default();
    let mut meeting_time = Local::now();

    if let Some(ref mid_str) = meeting_id {
        if let Ok(mid) = Uuid::parse_str(mid_str) {
            let db = state.db.lock().unwrap();
            if let Ok(Some(m)) = db.get_meeting(&mid) {
                if m_topic.trim().is_empty() {
                    m_topic = m.topic;
                }
                meeting_time = m.start_at.with_timezone(&Local);
            }
            if let Ok(mats) = db.list_materials_for_meeting(&mid) {
                mat_count = mats.len();
            }
            if let Ok(clips) = db.list_audio_clips_for_meeting(&mid) {
                audio_count = clips.iter().filter(|c| !c.audio_path.is_empty()).count();
            }
        }
    }

    if m_topic.trim().is_empty() {
        m_topic = "会议沟通".to_string();
    }

    // 编号格式: YYYYMMDDHHmm
    let time_code = meeting_time.format("%Y%m%d%H%M").to_string();
    let folder_name = format!("{}-{}", time_code, sanitize_folder_name(&m_topic));

    // 全量深度扫描实际仓库目录树
    let all_dirs = scan_repository_directory_tree(&repo_root);

    // 筛选适合在 UI 快捷展示的常见文档与模块目录
    let mut available_dirs: Vec<String> = all_dirs
        .iter()
        .filter(|d| d.starts_with("doc") || d.starts_with("wiki") || d.contains("meeting") || d.contains("spec") || d.contains("proposal") || d.contains("design"))
        .cloned()
        .collect();
    if available_dirs.is_empty() {
        available_dirs = all_dirs.iter().take(12).cloned().collect();
    }

    let (parent_dir, is_remembered, ai_reason) = if let Some(mem) = remembered_dir {
        (mem, true, None)
    } else {
        // 未曾选择过：真实调用 AI 智能分析该会议与仓库哪个目录最匹配！
        let summarizer = myth_ai::AiSummarizer::new(ai_config);
        match summarizer.suggest_meeting_repo_dir(&m_topic, &summary_snippet, &all_dirs).await {
            Ok((ai_dir, reason)) => {
                let final_dir = if ai_dir.is_empty() {
                    "docs/meetings".to_string()
                } else {
                    ai_dir
                };
                (final_dir, false, Some(reason))
            }
            Err(e) => {
                tracing::warn!("调用大模型推荐目录失败: {}", e);
                // 失败回退方案
                let fallback = available_dirs.first().cloned().unwrap_or_else(|| "docs/meetings".to_string());
                (fallback, false, Some(format!("AI 分析暂时不可用，默认定位到通用文档目录 ({})", e)))
            }
        }
    };

    Ok(MeetingRepoTargetInfo {
        repo_root: git_repo_path_str,
        parent_dir,
        is_remembered,
        folder_name,
        material_count: mat_count,
        audio_count,
        available_dirs,
        ai_reason,
    })
}

#[tauri::command]
async fn pick_repo_parent_folder(
    app: tauri::AppHandle,
    repo_root: String,
    current_parent: Option<String>,
) -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(move || {
        let root_path = std::path::PathBuf::from(&repo_root);
        let init_dir = if let Some(ref cur) = current_parent {
            let p = root_path.join(cur.trim_start_matches('/'));
            if p.is_dir() {
                p
            } else {
                root_path.clone()
            }
        } else {
            root_path.clone()
        };

        // 1. Linux 下优先尝试 zenity 系统图形目录选择器
        if let Ok(output) = std::process::Command::new("zenity")
            .arg("--file-selection")
            .arg("--directory")
            .arg("--title=选择保存会议纪要的父目录")
            .arg(format!("--filename={}/", init_dir.display()))
            .output()
        {
            if output.status.success() {
                let picked_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !picked_str.is_empty() {
                    let picked_path = std::path::PathBuf::from(&picked_str);
                    // 计算相对于仓库根目录的相对路径
                    if let Ok(rel) = picked_path.strip_prefix(&root_path) {
                        return Ok(Some(rel.to_string_lossy().to_string()));
                    } else {
                        // 如果选到了仓库外，则仍返回该选中的绝对路径
                        return Ok(Some(picked_str));
                    }
                }
            }
        }

        // 2. Linux KDE 下尝试 kdialog (KDE 桌面环境备选)
        if let Ok(output) = std::process::Command::new("kdialog")
            .arg("--getexistingdirectory")
            .arg(init_dir.display().to_string())
            .output()
        {
            if output.status.success() {
                let picked_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !picked_str.is_empty() {
                    let picked_path = std::path::PathBuf::from(&picked_str);
                    if let Ok(rel) = picked_path.strip_prefix(&root_path) {
                        return Ok(Some(rel.to_string_lossy().to_string()));
                    } else {
                        return Ok(Some(picked_str));
                    }
                }
            }
        }

        // 3. 跨平台兜底 (Windows / macOS / 无上述工具的 Linux)：使用 tauri-plugin-dialog
        use tauri_plugin_dialog::DialogExt;
        let picked = app.dialog().file().set_directory(&init_dir).set_title("选择保存会议纪要的父目录").blocking_pick_folder();
        if let Some(folder_path) = picked {
            let picked_path = folder_path.into_path().map_err(|e| e.to_string())?;
            if let Ok(rel) = picked_path.strip_prefix(&root_path) {
                return Ok(Some(rel.to_string_lossy().to_string()));
            } else {
                return Ok(Some(picked_path.to_string_lossy().to_string()));
            }
        }

        Ok(None)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn write_meeting_to_git_repo(
    state: State<'_, AppState>,
    project_id: String,
    meeting_id: Option<String>,
    parent_dir: String,
    folder_name: Option<String>,
    topic: String,
    summary: String,
    auto_commit: bool,
) -> Result<WriteMeetingToRepoResult, String> {
    let pid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let (project, materials_to_copy, clips_to_copy) = {
        let db = state.db.lock().unwrap();

        let project = db
            .get_project(&pid)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "未找到项目".to_string())?;

        let mut mats = Vec::new();
        let mut clips = Vec::new();

        if let Some(ref mid_str) = meeting_id {
            if let Ok(mid) = Uuid::parse_str(mid_str) {
                if let Ok(m_list) = db.list_materials_for_meeting(&mid) {
                    mats = m_list;
                }
                if let Ok(c_list) = db.list_audio_clips_for_meeting(&mid) {
                    clips = c_list;
                }
            }
        }

        (project, mats, clips)
    };

    let git_repo_path_str = project
        .git_repo_path
        .ok_or_else(|| "未配置 Git 仓库路径".to_string())?;

    let repo_root = std::path::PathBuf::from(&git_repo_path_str);
    if !repo_root.is_dir() {
        return Err(format!("Git 仓库路径不存在: {}", git_repo_path_str));
    }

    // 确定文件夹名称（时间戳编号）
    let clean_folder_name = folder_name
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            let time_code = Local::now().format("%Y%m%d%H%M").to_string();
            format!("{}-{}", time_code, sanitize_folder_name(&topic))
        });

    let rel_parent = parent_dir.trim().trim_start_matches('/').trim_end_matches('/');

    // 严格安全检查：严禁将目标目录指向 .git 元数据目录或其子目录
    let normalized_parent = rel_parent.replace('\\', "/");
    let normalized_folder = clean_folder_name.replace('\\', "/");
    if normalized_parent == ".git"
        || normalized_parent.starts_with(".git/")
        || normalized_folder == ".git"
        || normalized_folder.starts_with(".git/")
        || normalized_parent.contains("/.git/")
        || normalized_parent.ends_with("/.git")
    {
        return Err("安全拦截：严禁将归档目标目录设置为 Git 元数据目录 (.git)！".to_string());
    }

    let target_meeting_dir = if rel_parent.is_empty() {
        repo_root.join(&clean_folder_name)
    } else {
        repo_root.join(rel_parent).join(&clean_folder_name)
    };

    // 确保目标路径位于 repo_root 内部，防止路径遍历 (Path traversal)
    let canonical_root = repo_root.canonicalize().unwrap_or_else(|_| repo_root.clone());
    let target_parent_to_check = target_meeting_dir.parent().unwrap_or(&repo_root);
    if let Ok(canon_parent) = target_parent_to_check.canonicalize() {
        if !canon_parent.starts_with(&canonical_root) {
            return Err("安全拦截：目标归档目录超出了指定的 Git 仓库根目录！".to_string());
        }
    }

    let assets_dir = target_meeting_dir.join("assets");
    tokio::fs::create_dir_all(&assets_dir)
        .await
        .map_err(|e| format!("创建目标目录失败: {}", e))?;

    // 复制材料与音频切片
    let mut materials_copied: Vec<(String, String, String)> = Vec::new(); // (orig_name, rel_path, file_type)
    let mut clips_copied: Vec<(u32, String, String)> = Vec::new(); // (clip_order, rel_path, duration_text)

    for mat in materials_to_copy {
        let src_path = std::path::PathBuf::from(&mat.file_path);
        if src_path.is_file() {
            let dest_filename = &mat.file_name;
            let dest_path = assets_dir.join(dest_filename);
            if let Err(e) = tokio::fs::copy(&src_path, &dest_path).await {
                tracing::warn!("复制会议附件失败 {}: {}", src_path.display(), e);
            } else {
                materials_copied.push((
                    mat.file_name.clone(),
                    format!("./assets/{}", dest_filename),
                    mat.file_type.clone(),
                ));
            }
        }
    }

    for clip in clips_to_copy {
        let src_path = std::path::PathBuf::from(&clip.audio_path);
        if src_path.is_file() {
            let ext = src_path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("wav");
            let dest_filename = format!("clip_{}_{}s.{}", clip.clip_order, clip.duration_secs, ext);
            let dest_path = assets_dir.join(&dest_filename);
            if let Err(e) = tokio::fs::copy(&src_path, &dest_path).await {
                tracing::warn!("复制录音切片失败 {}: {}", src_path.display(), e);
            } else {
                clips_copied.push((
                    clip.clip_order,
                    format!("./assets/{}", dest_filename),
                    format_duration(clip.duration_secs),
                ));
            }
        }
    }

    // 生成 Markdown 纪要
    let mut md_content = format!("# 会议纪要：{}\n\n", topic);
    md_content.push_str(&format!("- **归档时间**：{}\n", Local::now().format("%Y-%m-%d %H:%M:%S")));
    md_content.push_str(&format!("- **关联项目**：{}\n\n", project.name));
    md_content.push_str("## 📝 纪要正文与核心决议\n\n");
    md_content.push_str(summary.trim());
    md_content.push_str("\n\n---\n\n");

    // 附录：材料与录音
    if !materials_copied.is_empty() {
        md_content.push_str("## 📎 关联会议材料与图纸\n\n");
        for (name, rel, ftype) in &materials_copied {
            if ftype == "image" {
                md_content.push_str(&format!("### {}\n![{}]({})\n\n", name, name, rel));
            } else {
                md_content.push_str(&format!("- [{}]({})\n", name, rel));
            }
        }
        md_content.push('\n');
    }

    if !clips_copied.is_empty() {
        md_content.push_str("## 🎙️ 录音原始切片凭据\n\n");
        for (order, rel, dur) in &clips_copied {
            md_content.push_str(&format!("- **录音片段 {}** (时长: {}): [点击试听/查看]({})\n", order, dur, rel));
        }
        md_content.push('\n');
    }

    // 写入 README.md
    let md_file_path = target_meeting_dir.join("README.md");
    tokio::fs::write(&md_file_path, md_content.as_bytes())
        .await
        .map_err(|e| format!("写入纪要 Markdown 文件失败: {}", e))?;

    // 保存父目录到记忆配置
    {
        let db = state.db.lock().unwrap();
        let memory_key = format!("meeting_repo_parent_dir_{}", pid);
        let _ = db.set_setting(&memory_key, rel_parent);
    }

    // 计算相对路径
    let relative_target_str = match target_meeting_dir.strip_prefix(&repo_root) {
        Ok(p) => p.to_string_lossy().to_string(),
        Err(_) => clean_folder_name.clone(),
    };

    // 可选：执行 Git Add & Commit
    let mut commit_msg_res = None;
    if auto_commit {
        let commit_title = format!("docs(meeting): archive meeting minutes for {}", topic);
        match myth_git::GitScanner::commit_files(&repo_root, &relative_target_str, &commit_title) {
            Ok(msg) => commit_msg_res = Some(msg),
            Err(e) => tracing::warn!("Git 自动提交失败: {}", e),
        }
    }

    Ok(WriteMeetingToRepoResult {
        success: true,
        full_path: target_meeting_dir.to_string_lossy().to_string(),
        relative_path: relative_target_str,
        commit_hash_or_msg: commit_msg_res,
    })
}

#[derive(serde::Serialize)]
struct SystemClipboardDto {
    content_type: String, // "image" or "text" or "empty"
    data: String,         // base64 data URL for image or plain text
    file_name: String,
}

#[tauri::command]
async fn get_system_clipboard() -> Result<SystemClipboardDto, String> {
    tokio::task::spawn_blocking(|| {
        let date_str = chrono::Local::now().format("%Y%m%d%H%M%S").to_string();

        // 1. 尝试 Wayland wl-paste
        if let Ok(output) = std::process::Command::new("wl-paste")
            .arg("--list-types")
            .output()
        {
            if output.status.success() {
                let types_str = String::from_utf8_lossy(&output.stdout);
                let types: Vec<&str> = types_str.lines().map(|s| s.trim()).collect();
                
                // 优先检查图片
                for t in &types {
                    if t.starts_with("image/") {
                        if let Ok(img_output) = std::process::Command::new("wl-paste")
                            .arg("--type")
                            .arg(t)
                            .output()
                        {
                            if img_output.status.success() && !img_output.stdout.is_empty() {
                                use base64::Engine;
                                let b64 = base64::engine::general_purpose::STANDARD.encode(&img_output.stdout);
                                let ext = t.split('/').nth(1).unwrap_or("png");
                                return Ok(SystemClipboardDto {
                                    content_type: "image".to_string(),
                                    data: format!("data:{};base64,{}", t, b64),
                                    file_name: format!("clipboard_{}.{}", date_str, ext),
                                });
                            }
                        }
                    }
                }

                // 其次检查文本
                if let Ok(text_output) = std::process::Command::new("wl-paste")
                    .arg("--no-newline")
                    .output()
                {
                    if text_output.status.success() && !text_output.stdout.is_empty() {
                        let text = String::from_utf8_lossy(&text_output.stdout).to_string();
                        if !text.trim().is_empty() {
                            return Ok(SystemClipboardDto {
                                content_type: "text".to_string(),
                                data: text,
                                file_name: format!("clipboard_text_{}.txt", date_str),
                            });
                        }
                    }
                }
            }
        }

        // 2. 尝试 X11 xclip
        if let Ok(output) = std::process::Command::new("xclip")
            .args(&["-selection", "clipboard", "-t", "TARGETS", "-o"])
            .output()
        {
            if output.status.success() {
                let targets = String::from_utf8_lossy(&output.stdout);
                if targets.contains("image/png") {
                    if let Ok(img) = std::process::Command::new("xclip")
                        .args(&["-selection", "clipboard", "-t", "image/png", "-o"])
                        .output()
                    {
                        if img.status.success() && !img.stdout.is_empty() {
                            use base64::Engine;
                            let b64 = base64::engine::general_purpose::STANDARD.encode(&img.stdout);
                            return Ok(SystemClipboardDto {
                                content_type: "image".to_string(),
                                data: format!("data:image/png;base64,{}", b64),
                                file_name: format!("clipboard_{}.png", date_str),
                            });
                        }
                    }
                }

                if let Ok(txt) = std::process::Command::new("xclip")
                    .args(&["-selection", "clipboard", "-o"])
                    .output()
                {
                    if txt.status.success() && !txt.stdout.is_empty() {
                        let text = String::from_utf8_lossy(&txt.stdout).to_string();
                        if !text.trim().is_empty() {
                            return Ok(SystemClipboardDto {
                                content_type: "text".to_string(),
                                data: text,
                                file_name: format!("clipboard_text_{}.txt", date_str),
                            });
                        }
                    }
                }
            }
        }

        Ok(SystemClipboardDto {
            content_type: "empty".to_string(),
            data: "".to_string(),
            file_name: "".to_string(),
        })
    })
    .await
    .map_err(|e| format!("读取剪贴板线程执行失败: {}", e))?
}

#[tauri::command]
fn open_material_file(
    state: State<'_, AppState>,
    material_id: String,
) -> Result<(), String> {
    let mat_id = Uuid::parse_str(&material_id).map_err(|e| e.to_string())?;
    let mat = {
        let db = state.db.lock().unwrap();
        db.get_material(&mat_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "未找到参考资料".to_string())?
    };
    let path = std::path::PathBuf::from(&mat.file_path);
    if !path.exists() {
        return Err("资料文件在磁盘上不存在".to_string());
    }
    let _ = system_open_path(&path).map_err(|e| format!("打开文件失败: {}", e))?;
    Ok(())
}

#[tauri::command]
fn play_audio(state: State<AppState>, audio_path: String) -> Result<(), String> {
    let p = std::path::Path::new(&audio_path);
    state
        .audio_player
        .lock()
        .unwrap()
        .play(p)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn stop_audio(state: State<AppState>) -> Result<(), String> {
    state.audio_player.lock().unwrap().stop();
    Ok(())
}

#[tauri::command]
fn get_audio_data_url(audio_path: String) -> Result<String, String> {
    use std::io::Read;
    let p = std::path::Path::new(&audio_path);
    if !p.exists() {
        return Err(format!("音频文件不存在: {}", audio_path));
    }
    let ext = p
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("mp3")
        .to_lowercase();
    let mime = match ext.as_str() {
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "m4a" => "audio/mp4",
        "aac" => "audio/aac",
        _ => "audio/mpeg",
    };
    let mut file = std::fs::File::open(p).map_err(|e| format!("打开音频文件失败: {}", e))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|e| format!("读取音频文件失败: {}", e))?;

    // 转换为 base64
    const B64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity((buffer.len() * 4) / 3 + 4);
    for chunk in buffer.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        encoded.push(B64_CHARS[((triple >> 18) & 0x3f) as usize] as char);
        encoded.push(B64_CHARS[((triple >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            encoded.push(B64_CHARS[((triple >> 6) & 0x3f) as usize] as char);
        } else {
            encoded.push('=');
        }
        if chunk.len() > 2 {
            encoded.push(B64_CHARS[(triple & 0x3f) as usize] as char);
        } else {
            encoded.push('=');
        }
    }

    Ok(format!("data:{};base64,{}", mime, encoded))
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

    p.currency = if currency.trim().is_empty() {
        "CNY".to_string()
    } else {
        currency
    };
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

    state
        .db
        .lock()
        .unwrap()
        .insert_project(&p)
        .map_err(|e| e.to_string())?;
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
    let guard = state.db.lock().unwrap();
    let mut p = guard
        .get_project(&pid)
        .map_err(|e| e.to_string())?
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
fn delete_project(state: State<AppState>, id: String) -> Result<(), String> {
    let pid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;

    // 1. 如果该项目正在录音，停止并清理录音
    {
        let mut active_rec = state.active_recording.lock().unwrap();
        if let Some((rec_pid, _, _, _, _)) = &*active_rec {
            if *rec_pid == pid {
                let mut recorder = state.audio_recorder.lock().unwrap();
                if recorder.is_recording() {
                    let _ = recorder.stop_recording();
                }
                *active_rec = None;
            }
        }
    }

    // 2. 如果通道在运行/暂停，停止并移除通道
    {
        let mut mgr = state.timer_manager.lock().unwrap();
        let _ = mgr.stop_channel(pid);
        mgr.remove_channel(&pid);
    }

    // 3. 彻底删除项目数据与级联记录
    let db = state.db.lock().unwrap();
    let _ = db.delete_active_timer(&pid);
    db.delete_project(&pid).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
fn add_project_note(
    state: State<AppState>,
    project_id: String,
    content: String,
) -> Result<(), String> {
    let pid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let note = QuickNote::new(pid, content.trim());
    state
        .db
        .lock()
        .unwrap()
        .insert_quick_note(&note)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn list_project_notes(state: State<AppState>, project_id: String) -> Result<Vec<NoteDto>, String> {
    let pid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let notes = state
        .db
        .lock()
        .unwrap()
        .list_quick_notes(&pid)
        .unwrap_or_default();
    Ok(notes
        .into_iter()
        .map(|n| NoteDto {
            id: n.id.to_string(),
            project_id: n.project_id.to_string(),
            content: n.content,
            time_text: n
                .logged_at
                .with_timezone(&Local)
                .format("%H:%M")
                .to_string(),
        })
        .collect())
}

#[tauri::command]
fn delete_note(state: State<AppState>, note_id: String) -> Result<(), String> {
    let nid = Uuid::parse_str(&note_id).map_err(|e| e.to_string())?;
    state
        .db
        .lock()
        .unwrap()
        .delete_quick_note(&nid)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn fetch_git_work_summary(
    state: State<'_, AppState>,
    project_id: String,
    date: String,
    start_time: Option<String>,
    end_time: Option<String>,
    custom_repo_path: Option<String>,
) -> Result<GitSmartFillDto, String> {
    let pid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let db = state.db.clone();

    let (mut proj, client_name, ai_config, hours_per_day) = {
        let guard = db.lock().unwrap();
        let p = guard
            .list_projects()
            .unwrap_or_default()
            .into_iter()
            .find(|p| p.id == pid)
            .ok_or_else(|| "项目不存在".to_string())?;

        let c_name = get_client_name(&guard, p.client_id);
        let ai_config = load_ai_config(&guard);
        let hours = p.hours_per_day;

        (p, c_name, ai_config, hours)
    };

    // 确定仓库路径 (优先使用自定义传入路径，否则使用项目已绑定路径)
    let repo_path = if let Some(ref custom) = custom_repo_path {
        let p_trim = custom.trim().to_string();
        if !p_trim.is_empty() {
            if proj.git_repo_path.as_deref() != Some(&p_trim) {
                proj.git_repo_path = Some(p_trim.clone());
                let _ = db.lock().unwrap().update_project(&proj);
            }
            p_trim
        } else {
            proj.git_repo_path.clone().unwrap_or_default()
        }
    } else {
        proj.git_repo_path.clone().unwrap_or_default()
    };

    if repo_path.trim().is_empty() {
        return Ok(GitSmartFillDto {
            success: false,
            repo_path: "".to_string(),
            summary_text: "".to_string(),
            commits_count: 0,
            commits: vec![],
            suggested_start_time: None,
            suggested_end_time: None,
            diff_stat: None,
            is_fallback_to_day: false,
            ai_reason: None,
            time_slots: vec![],
            message: "该项目尚未绑定本地 Git 仓库路径。请在输入框中填入本地 Git 仓库路径。"
                .to_string(),
        });
    }

    if !GitScanner::is_git_repo(&repo_path) {
        return Ok(GitSmartFillDto {
            success: false,
            repo_path: repo_path.clone(),
            summary_text: "".to_string(),
            commits_count: 0,
            commits: vec![],
            suggested_start_time: None,
            suggested_end_time: None,
            diff_stat: None,
            is_fallback_to_day: false,
            ai_reason: None,
            time_slots: vec![],
            message: format!("指定的路径不是有效的 Git 仓库：{}", repo_path),
        });
    }

    let s_time = start_time
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("08:30");
    let e_time = end_time
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("19:00");

    let start_str = format!("{} {}:00", date.trim(), s_time.trim());
    let end_str = format!("{} {}:00", date.trim(), e_time.trim());
    let day_start_str = format!("{} 00:00:00", date.trim());
    let day_end_str = format!("{} 23:59:59", date.trim());

    let (since_utc, until_utc) = match (
        chrono::NaiveDateTime::parse_from_str(&start_str, "%Y-%m-%d %H:%M:%S"),
        chrono::NaiveDateTime::parse_from_str(&end_str, "%Y-%m-%d %H:%M:%S"),
    ) {
        (Ok(s), Ok(e)) => {
            let s_local = Local.from_local_datetime(&s).unwrap();
            let e_local = Local.from_local_datetime(&e).unwrap();
            (s_local.with_timezone(&Utc), e_local.with_timezone(&Utc))
        }
        _ => {
            return Err("开始/结束时间格式错误，需为 YYYY-MM-DD 与 HH:MM".to_string());
        }
    };

    let (day_since_utc, day_until_utc) = match (
        chrono::NaiveDateTime::parse_from_str(&day_start_str, "%Y-%m-%d %H:%M:%S"),
        chrono::NaiveDateTime::parse_from_str(&day_end_str, "%Y-%m-%d %H:%M:%S"),
    ) {
        (Ok(s), Ok(e)) => {
            let s_local = Local.from_local_datetime(&s).unwrap();
            let e_local = Local.from_local_datetime(&e).unwrap();
            (s_local.with_timezone(&Utc), e_local.with_timezone(&Utc))
        }
        _ => {
            return Err("日期格式解析错误".to_string());
        }
    };

    let work_summary = GitScanner::fetch_smart_work_summary(
        &repo_path,
        since_utc,
        until_utc,
        day_since_utc,
        day_until_utc,
    )
    .map_err(|e| e.to_string())?;

    if work_summary.commits.is_empty() {
        return Ok(GitSmartFillDto {
            success: false,
            repo_path,
            summary_text: "".to_string(),
            commits_count: 0,
            commits: vec![],
            suggested_start_time: None,
            suggested_end_time: None,
            diff_stat: None,
            is_fallback_to_day: false,
            ai_reason: None,
            time_slots: vec![],
            message: format!(
                "在 {} 全天范围内未发现任何代码提交记录（Git Log 为空）。",
                date.trim()
            ),
        });
    }

    let commits_len = work_summary.commits.len();
    let commits_desc: Vec<String> = work_summary
        .commits
        .iter()
        .map(|c| format!("{} [{}] ({})", c.message, c.hash, c.time_str))
        .collect();

    // 默认本地启发式推荐时间与候选槽
    let local_start = work_summary
        .earliest_time
        .clone()
        .unwrap_or_else(|| "09:30".to_string());
    let local_end = work_summary
        .latest_time
        .clone()
        .unwrap_or_else(|| "18:30".to_string());

    let mut final_start = local_start.clone();
    let mut final_end = local_end.clone();
    let mut final_summary = work_summary.summary_text.clone();
    let mut ai_reason: Option<String> = None;
    let mut candidate_slots: Vec<AiTimeSlotDto> = Vec::new();

    // 构建默认启发式候选时段
    candidate_slots.push(AiTimeSlotDto {
        label: "全天研发".to_string(),
        start: "09:30".to_string(),
        end: local_end.clone(),
    });
    candidate_slots.push(AiTimeSlotDto {
        label: "下午集中".to_string(),
        start: "13:30".to_string(),
        end: local_end.clone(),
    });
    candidate_slots.push(AiTimeSlotDto {
        label: "提交对齐".to_string(),
        start: local_start.clone(),
        end: local_end.clone(),
    });

    // 尝试调用 AI 模块 (agy / codex / openai)
    let ai_eval_req = AiWorkEvaluationRequest {
        project_name: proj.name.clone(),
        client_name: client_name.clone(),
        date: date.clone(),
        commits: commits_desc.clone(),
        diff_stat: work_summary.diff_stat.clone(),
        earliest_commit_time: work_summary.earliest_time.clone(),
        latest_commit_time: work_summary.latest_time.clone(),
        preferred_start_time: Some("09:30".to_string()),
        preferred_hours: hours_per_day,
    };

    let summarizer = AiSummarizer::new(ai_config.clone());
    match summarizer.evaluate_work_timeslot(&ai_eval_req).await {
        Ok(ai_res) => {
            if !ai_res.suggested_start_time.trim().is_empty() {
                final_start = ai_res.suggested_start_time;
            }
            if !ai_res.suggested_end_time.trim().is_empty() {
                final_end = ai_res.suggested_end_time;
            }
            if !ai_res.deliverable_summary.trim().is_empty() {
                // 确保条目化格式：若 AI 返回的大长句使用分号/句号连接而未换行，自动整理为序号条目
                let raw_sum = ai_res.deliverable_summary.trim();
                if raw_sum.contains('\n') {
                    final_summary = raw_sum.to_string();
                } else if raw_sum.contains('；') || raw_sum.contains(';') || raw_sum.contains('。')
                {
                    let parts: Vec<&str> = raw_sum
                        .split(|c| c == '；' || c == ';' || c == '。')
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .collect();
                    if parts.len() > 1 {
                        final_summary = parts
                            .iter()
                            .enumerate()
                            .map(|(i, p)| {
                                let clean_p = p
                                    .trim_start_matches(|c: char| {
                                        c.is_numeric()
                                            || c == '.'
                                            || c == '、'
                                            || c == '-'
                                            || c == '•'
                                    })
                                    .trim();
                                format!("{}. {}", i + 1, clean_p)
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                    } else {
                        final_summary = raw_sum.to_string();
                    }
                } else {
                    final_summary = raw_sum.to_string();
                }
            }
            if !ai_res.reason.trim().is_empty() {
                ai_reason = Some(format!(
                    "AI 评估 ({}): {}",
                    ai_config.provider, ai_res.reason
                ));
            }
            if !ai_res.slots.is_empty() {
                candidate_slots = ai_res
                    .slots
                    .into_iter()
                    .map(|s| AiTimeSlotDto {
                        label: s.label,
                        start: s.start,
                        end: s.end,
                    })
                    .collect();
            }
        }
        Err(e) => {
            eprintln!("AI 评估推算未启用或失败，无缝回退到本地启发式分析: {}", e);
            ai_reason = Some("本地启发式推算（AI未响应或已降级）".to_string());
        }
    }

    let message = if work_summary.is_fallback_to_day {
        format!("在指定时段未发现提交，已智能抓取该日期（{}）全天的 {} 条 Git 提交记录并由 AI 完成工时推算", date.trim(), commits_len)
    } else {
        format!(
            "成功提取 {} 条 Git 提交记录，AI 已智能推算研发时段与交付成果",
            commits_len
        )
    };

    Ok(GitSmartFillDto {
        success: true,
        repo_path,
        summary_text: final_summary,
        commits_count: commits_len,
        commits: commits_desc,
        suggested_start_time: Some(final_start),
        suggested_end_time: Some(final_end),
        diff_stat: work_summary.diff_stat,
        is_fallback_to_day: work_summary.is_fallback_to_day,
        ai_reason,
        time_slots: candidate_slots,
        message,
    })
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

    let billable_hours =
        BillingEngine::calculate_billable_hours(duration_secs, proj.rounding_minutes);
    let billable_days = BillingEngine::calculate_billable_days(
        billable_hours,
        proj.hours_per_day,
        proj.day_calc_mode,
    );
    let (rate_snapshot, total_amount) =
        BillingEngine::calculate_amount(&proj, billable_hours, billable_days);

    let entry = TimeEntry {
        id: Uuid::new_v4(),
        project_id: pid,
        start_time: start_local.with_timezone(&Utc),
        end_time: end_local.with_timezone(&Utc),
        raw_duration_secs: duration_secs,
        billable_hours,
        billable_days,
        rate_at_moment: rate_snapshot,
        total_amount,
        summary: Some(if summary.trim().is_empty() {
            "补录工时投入与研发交付".to_string()
        } else {
            summary.trim().to_string()
        }),
        audio_path: None,
        is_invoiced: false,
        created_at: Utc::now(),
    };

    state
        .db
        .lock()
        .unwrap()
        .insert_time_entry(&entry)
        .map_err(|e| e.to_string())?;
    Ok("工时补录成功！".to_string())
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BatchSaveItemDto {
    pub date: String,
    pub start_time: String,
    pub end_time: String,
    pub summary: String,
}

#[tauri::command]
async fn fetch_batch_git_work_summary(
    state: State<'_, AppState>,
    project_id: String,
    start_date: String,
    end_date: String,
    custom_repo_path: Option<String>,
) -> Result<BatchSmartFillResultDto, String> {
    let pid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let db = state.db.clone();

    let (mut proj, client_name, ai_config, hours_per_day) = {
        let guard = db.lock().unwrap();
        let p = guard
            .list_projects()
            .unwrap_or_default()
            .into_iter()
            .find(|p| p.id == pid)
            .ok_or_else(|| "项目不存在".to_string())?;

        let c_name = get_client_name(&guard, p.client_id);
        let ai_config = load_ai_config(&guard);
        let hours = p.hours_per_day;

        (p, c_name, ai_config, hours)
    };

    let repo_path = if let Some(ref custom) = custom_repo_path {
        let p_trim = custom.trim().to_string();
        if !p_trim.is_empty() {
            if proj.git_repo_path.as_deref() != Some(&p_trim) {
                proj.git_repo_path = Some(p_trim.clone());
                let _ = db.lock().unwrap().update_project(&proj);
            }
            p_trim
        } else {
            proj.git_repo_path.clone().unwrap_or_default()
        }
    } else {
        proj.git_repo_path.clone().unwrap_or_default()
    };

    if repo_path.trim().is_empty() {
        return Ok(BatchSmartFillResultDto {
            success: false,
            total_days: 0,
            total_commits: 0,
            entries: vec![],
            message: "该项目尚未绑定本地 Git 仓库路径。请在输入框中填入本地 Git 仓库路径。"
                .to_string(),
        });
    }

    if !GitScanner::is_git_repo(&repo_path) {
        return Ok(BatchSmartFillResultDto {
            success: false,
            total_days: 0,
            total_commits: 0,
            entries: vec![],
            message: format!("指定的路径不是有效的 Git 仓库：{}", repo_path),
        });
    }

    let start_str = format!("{} 00:00:00", start_date.trim());
    let end_str = format!("{} 23:59:59", end_date.trim());

    let (since_utc, until_utc) = match (
        chrono::NaiveDateTime::parse_from_str(&start_str, "%Y-%m-%d %H:%M:%S"),
        chrono::NaiveDateTime::parse_from_str(&end_str, "%Y-%m-%d %H:%M:%S"),
    ) {
        (Ok(s), Ok(e)) => {
            let s_local = Local.from_local_datetime(&s).unwrap();
            let e_local = Local.from_local_datetime(&e).unwrap();
            (s_local.with_timezone(&Utc), e_local.with_timezone(&Utc))
        }
        _ => {
            return Err("起止日期格式错误，需为 YYYY-MM-DD".to_string());
        }
    };

    let grouped_commits =
        GitScanner::fetch_grouped_commits_by_date(&repo_path, since_utc, until_utc)
            .map_err(|e| e.to_string())?;

    if grouped_commits.is_empty() {
        return Ok(BatchSmartFillResultDto {
            success: false,
            total_days: 0,
            total_commits: 0,
            entries: vec![],
            message: format!(
                "在 {} 至 {} 区间内未发现任何代码提交记录（Git Log 为空）。",
                start_date.trim(),
                end_date.trim()
            ),
        });
    }

    let mut total_commits_count = 0;
    let mut candidate_entries = Vec::new();
    let summarizer = AiSummarizer::new(ai_config.clone());

    for (date_str, commits) in grouped_commits {
        total_commits_count += commits.len();
        let commits_len = commits.len();
        let earliest_time = commits.first().map(|c| c.time_str.clone());
        let latest_time = commits.last().map(|c| c.time_str.clone());
        let commits_desc: Vec<String> = commits
            .iter()
            .map(|c| format!("{} [{}] ({})", c.message, c.hash, c.time_str))
            .collect();

        // 默认本地启发式推算
        let local_start = earliest_time.clone().unwrap_or_else(|| "09:30".to_string());
        let local_end = latest_time.clone().unwrap_or_else(|| "18:30".to_string());

        let mut final_start = local_start.clone();
        let mut final_end = local_end.clone();
        let mut final_summary = format!("1. 完成代码研发与提交（共 {} 次提交）\n", commits_len);
        for (i, c) in commits.iter().enumerate() {
            final_summary.push_str(&format!("{}. {} [{}]\n", i + 1, c.message, c.hash));
        }
        let mut final_reason = format!("本地启发式提取：{} 条提交记录", commits_len);

        // 调用 AI 提炼每天的高规格交付物
        let ai_eval_req = AiWorkEvaluationRequest {
            project_name: proj.name.clone(),
            client_name: client_name.clone(),
            date: date_str.clone(),
            commits: commits_desc,
            diff_stat: None,
            earliest_commit_time: earliest_time,
            latest_commit_time: latest_time,
            preferred_start_time: Some("09:30".to_string()),
            preferred_hours: hours_per_day,
        };

        if let Ok(ai_res) = summarizer.evaluate_work_timeslot(&ai_eval_req).await {
            if !ai_res.suggested_start_time.trim().is_empty() {
                final_start = ai_res.suggested_start_time;
            }
            if !ai_res.suggested_end_time.trim().is_empty() {
                final_end = ai_res.suggested_end_time;
            }
            if !ai_res.deliverable_summary.trim().is_empty() {
                final_summary = ai_res.deliverable_summary;
            }
            if !ai_res.reason.trim().is_empty() {
                final_reason = format!("AI ({}) 推理: {}", ai_config.provider, ai_res.reason);
            }
        }

        candidate_entries.push(BatchSmartEntryDto {
            temp_id: Uuid::new_v4().to_string(),
            date: date_str,
            start_time: final_start,
            end_time: final_end,
            summary: final_summary.trim().to_string(),
            reason: final_reason,
            commits_count: commits_len,
            is_selected: true,
        });
    }

    let days_count = candidate_entries.len();
    let message = format!(
        "成功检索到 {} 天、共 {} 条 Git 提交记录，AI 已智能推算各工作日时段与交付纪要",
        days_count, total_commits_count
    );

    Ok(BatchSmartFillResultDto {
        success: true,
        total_days: days_count,
        total_commits: total_commits_count,
        entries: candidate_entries,
        message,
    })
}

#[tauri::command]
async fn parse_custom_batch_summary(
    state: State<'_, AppState>,
    project_id: String,
    raw_text: String,
) -> Result<BatchSmartFillResultDto, String> {
    let pid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let db = state.db.clone();

    let (proj, client_name, ai_config) = {
        let guard = db.lock().unwrap();
        let p = guard
            .list_projects()
            .unwrap_or_default()
            .into_iter()
            .find(|p| p.id == pid)
            .ok_or_else(|| "项目不存在".to_string())?;

        let c_name = get_client_name(&guard, p.client_id);
        let ai_config = load_ai_config(&guard);

        (p, c_name, ai_config)
    };

    let text_trim = raw_text.trim();
    if text_trim.is_empty() {
        return Err("待解析的总结文本不能为空".to_string());
    }

    let summarizer = AiSummarizer::new(ai_config.clone());
    let parsed_items = summarizer
        .parse_custom_agent_summary(&proj.name, &client_name, text_trim)
        .await
        .map_err(|e| format!("AI 智能体总结解析失败: {}", e))?;

    if parsed_items.is_empty() {
        return Ok(BatchSmartFillResultDto {
            success: false,
            total_days: 0,
            total_commits: 0,
            entries: vec![],
            message: "未在文本中识别出任何有效的日期与工时信息。".to_string(),
        });
    }

    let mut candidate_entries = Vec::new();
    for item in parsed_items {
        candidate_entries.push(BatchSmartEntryDto {
            temp_id: Uuid::new_v4().to_string(),
            date: item.date,
            start_time: if item.start_time.is_empty() {
                "09:30".to_string()
            } else {
                item.start_time
            },
            end_time: if item.end_time.is_empty() {
                "18:30".to_string()
            } else {
                item.end_time
            },
            summary: item.summary,
            reason: if item.reason.is_empty() {
                "由智能体纪要文本解析得出".to_string()
            } else {
                item.reason
            },
            commits_count: 0,
            is_selected: true,
        });
    }

    let days_count = candidate_entries.len();
    Ok(BatchSmartFillResultDto {
        success: true,
        total_days: days_count,
        total_commits: 0,
        entries: candidate_entries,
        message: format!(
            "AI 智能体总结解析成功，已自动提炼出 {} 天的结构化工时记录",
            days_count
        ),
    })
}

#[tauri::command]
fn log_batch_manual_entries(
    state: State<AppState>,
    project_id: String,
    entries: Vec<BatchSaveItemDto>,
) -> Result<String, String> {
    let pid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let db = state.db.lock().unwrap();
    let proj = db
        .list_projects()
        .unwrap_or_default()
        .into_iter()
        .find(|p| p.id == pid)
        .ok_or_else(|| "项目不存在".to_string())?;

    let mut count = 0;
    for item in entries {
        let start_str = format!("{} {}:00", item.date.trim(), item.start_time.trim());
        let end_str = format!("{} {}:00", item.date.trim(), item.end_time.trim());

        let start_naive =
            match chrono::NaiveDateTime::parse_from_str(&start_str, "%Y-%m-%d %H:%M:%S") {
                Ok(dt) => dt,
                Err(_) => continue,
            };
        let end_naive = match chrono::NaiveDateTime::parse_from_str(&end_str, "%Y-%m-%d %H:%M:%S") {
            Ok(dt) => dt,
            Err(_) => continue,
        };

        let start_local = match Local.from_local_datetime(&start_naive) {
            chrono::LocalResult::Single(dt) => dt,
            chrono::LocalResult::Ambiguous(dt, _) => dt,
            chrono::LocalResult::None => continue,
        };
        let end_local = match Local.from_local_datetime(&end_naive) {
            chrono::LocalResult::Single(dt) => dt,
            chrono::LocalResult::Ambiguous(dt, _) => dt,
            chrono::LocalResult::None => continue,
        };

        if end_local <= start_local {
            continue;
        }

        let diff = end_local.signed_duration_since(start_local);
        let duration_secs = diff.num_seconds().max(0) as u64;

        let billable_hours =
            BillingEngine::calculate_billable_hours(duration_secs, proj.rounding_minutes);
        let billable_days = BillingEngine::calculate_billable_days(
            billable_hours,
            proj.hours_per_day,
            proj.day_calc_mode,
        );
        let (rate_snapshot, total_amount) =
            BillingEngine::calculate_amount(&proj, billable_hours, billable_days);

        let entry = TimeEntry {
            id: Uuid::new_v4(),
            project_id: pid,
            start_time: start_local.with_timezone(&Utc),
            end_time: end_local.with_timezone(&Utc),
            raw_duration_secs: duration_secs,
            billable_hours,
            billable_days,
            rate_at_moment: rate_snapshot,
            total_amount,
            summary: Some(if item.summary.trim().is_empty() {
                "批量补录工时投入与研发交付".to_string()
            } else {
                item.summary.trim().to_string()
            }),
            audio_path: None,
            is_invoiced: false,
            created_at: Utc::now(),
        };

        let _ = db.insert_time_entry(&entry);
        count += 1;
    }

    Ok(format!("成功批量补录 {} 天的工时投入！", count))
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
            state
                .db
                .lock()
                .unwrap()
                .insert_project(&p)
                .map_err(|e| e.to_string())?;
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
        state
            .db
            .lock()
            .unwrap()
            .insert_project(&p)
            .map_err(|e| e.to_string())?;
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
            let ext = src_path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("wav");
            let filename = format!(
                "upload_{}_{}.{}",
                date.replace('-', ""),
                Uuid::new_v4().simple(),
                ext
            );
            let dest_path = audio_dir.join(filename);
            if std::fs::copy(src_path, &dest_path).is_ok() {
                target_audio_path = Some(dest_path.to_string_lossy().to_string());
            }
        }
    }

    let topic_name = if topic.trim().is_empty() {
        "专题方案沟通会".to_string()
    } else {
        topic.trim().to_string()
    };
    let summary = format!(
        "【会议补录】{}\n会议时薪：{} 元/小时\n议题：{}",
        topic_name, actual_rate, topic_name
    );

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

    state
        .db
        .lock()
        .unwrap()
        .insert_time_entry(&entry)
        .map_err(|e| e.to_string())?;
    Ok("会议工时与录音补录成功！".to_string())
}

#[tauri::command]
async fn transcribe_entry_audio(
    state: State<'_, AppState>,
    entry_id: String,
    audio_path: String,
) -> Result<String, String> {
    let eid = Uuid::parse_str(&entry_id).map_err(|e| e.to_string())?;
    let db = state.db.clone();
    let ai_config = {
        let guard = db.lock().unwrap();
        load_ai_config(&guard)
    };

    let summarizer = AiSummarizer::new(ai_config);

    let p = std::path::Path::new(&audio_path);
    let transcript = summarizer
        .transcribe_audio(p)
        .await
        .map_err(|e| e.to_string())?;

    // AI 结构化精炼提取（基于转写文本提取核心议题、技术决议与交付待办）
    let ai_summary = match summarizer.summarize_meeting("会议交流与技术讨论", 1.0, &[], &[], Some(&audio_path), Some(&transcript)).await {
        Ok(sum) if !sum.trim().is_empty() => sum,
        _ => format!("1. 【核心议题】会议研讨与技术方案沟通\n2. 【讨论要点】已转写录音内容。\n3. 【后续待办】落实会议共识。"),
    };

    let guard = db.lock().unwrap();
    if let Ok(Some(mut entry)) = guard.get_time_entry(&eid) {
        let prev = entry.summary.unwrap_or_default();
        let new_summary = if prev.is_empty() {
            format!("{}\n\n【转写全文实录】\n{}", ai_summary, transcript)
        } else {
            format!(
                "{}\n\n【AI 提炼纪要】\n{}\n\n【转写全文实录】\n{}",
                prev, ai_summary, transcript
            )
        };
        entry.summary = Some(new_summary.clone());
        let _ = guard.insert_time_entry(&entry);
        return Ok(new_summary);
    }

    Ok(ai_summary)
}

#[tauri::command]
async fn resummarize_meeting_entry(
    state: State<'_, AppState>,
    entry_id: String,
) -> Result<String, String> {
    let eid = Uuid::parse_str(&entry_id).map_err(|e| e.to_string())?;
    let db = state.db.clone();
    let ai_config = {
        let guard = db.lock().unwrap();
        load_ai_config(&guard)
    };
    let summarizer = AiSummarizer::new(ai_config);

    // 1. 查找此 entry 下的结构化会议记录
    let (meetings, project_id, entry_audio_path) = {
        let guard = db.lock().unwrap();
        let ms = guard.list_meetings_for_entry(&eid).unwrap_or_default();
        let entry_opt = guard.get_time_entry(&eid).ok().flatten();
        let pid = entry_opt.as_ref().map(|e| e.project_id).unwrap_or_default();
        let a_path = entry_opt
            .as_ref()
            .and_then(|e| e.audio_path.clone())
            .unwrap_or_default();
        (ms, pid, a_path)
    };

    let notes = db
        .lock()
        .unwrap()
        .list_quick_notes(&project_id)
        .unwrap_or_default();

    if !meetings.is_empty() {
        let mut final_meeting_summary = String::new();
        let mut all_combined_transcripts = String::new();

        for m in meetings {
            let clips = db
                .lock()
                .unwrap()
                .list_audio_clips_for_meeting(&m.id)
                .unwrap_or_default();
            let mut combined_transcript = String::new();
            for c in &clips {
                if let Some(ref txt) = c.transcript {
                    if !txt.trim().is_empty() {
                        combined_transcript.push_str(&format!(
                            "【录音片段 {}】：\n{}\n\n",
                            c.clip_order,
                            txt.trim()
                        ));
                    }
                }
            }

            let topic_for_sum = m.topic.clone();
            let duration_hours = if m.duration_secs > 0 {
                (m.duration_secs as f64) / 3600.0
            } else {
                0.5
            };
            let materials = db
                .lock()
                .unwrap()
                .list_materials_for_meeting(&m.id)
                .unwrap_or_default();
            let summary = match summarizer.summarize_meeting(&topic_for_sum, duration_hours, &notes, &materials, None, Some(&combined_transcript)).await {
                Ok(sum) if !sum.trim().is_empty() => sum,
                _ => format!("1. 【核心议题】{}\n2. 【达成共识】已完成会议沟通与各录音片段归档。\n3. 【后续待办】待人工补充纪要要点。", topic_for_sum),
            };

            // 重新从数据库获取最新 meeting 实例，防止覆盖 AI 请求期间由 finish_meeting 更新的最新状态
            {
                let guard = db.lock().unwrap();
                if let Ok(Some(mut fresh_m)) = guard.get_meeting(&m.id) {
                    fresh_m.meeting_summary = Some(summary.clone());
                    if fresh_m.status != "in_progress" {
                        fresh_m.status = "completed".to_string();
                    }
                    let _ = guard.insert_meeting(&fresh_m);
                }
            }

            final_meeting_summary = summary;
            if !combined_transcript.trim().is_empty() {
                all_combined_transcripts.push_str(&combined_transcript);
            }
        }

        return Ok(final_meeting_summary);
    } else {
        let p = std::path::Path::new(&entry_audio_path);
        if !p.exists() {
            return Err(format!("录音文件不存在: {}", entry_audio_path));
        }
        let transcript = summarizer
            .transcribe_audio(p)
            .await
            .map_err(|e| format!("语音转写失败: {}", e))?;

        let ai_summary = match summarizer.summarize_meeting("会议交流与技术讨论", 1.0, &notes, &[], Some(&entry_audio_path), Some(&transcript)).await {
            Ok(sum) if !sum.trim().is_empty() => sum,
            _ => format!("1. 【核心议题】会议研讨与技术方案沟通\n2. 【讨论要点】已转写录音内容。\n3. 【后续待办】落实会议共识。"),
        };

        let new_summary = if transcript.is_empty() {
            ai_summary
        } else {
            format!("{}\n\n【转写全文实录】\n{}", ai_summary, transcript)
        };

        let guard = db.lock().unwrap();
        if let Ok(Some(mut entry)) = guard.get_time_entry(&eid) {
            entry.summary = Some(new_summary.clone());
            let _ = guard.insert_time_entry(&entry);
            return Ok(new_summary);
        }

        Ok(new_summary)
    }
}

#[tauri::command]
fn update_entry_summary(
    state: State<AppState>,
    entry_id: String,
    summary: String,
) -> Result<String, String> {
    let eid = Uuid::parse_str(&entry_id).map_err(|e| e.to_string())?;
    let db = state.db.lock().unwrap();
    if let Ok(Some(mut entry)) = db.get_time_entry(&eid) {
        entry.summary = Some(summary);
        db.insert_time_entry(&entry).map_err(|e| e.to_string())?;
        return Ok("会议纪要与交付说明已成功保存更新！".to_string());
    }
    Err("未找到对应的工时记录".to_string())
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExtractGitResultDto {
    pub success: bool,
    pub commits_count: usize,
    pub commits: Vec<String>,
    pub diff_stat: String,
    pub extracted_text: String,
    pub message: String,
}

#[tauri::command]
async fn extract_entry_git_summary(
    state: State<'_, AppState>,
    entry_id: String,
    custom_repo_path: Option<String>,
) -> Result<ExtractGitResultDto, String> {
    let eid = Uuid::parse_str(&entry_id).map_err(|e| e.to_string())?;
    let db = state.db.clone();

    let (entry, proj) = {
        let guard = db.lock().unwrap();
        let e = guard
            .get_time_entry(&eid)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "未找到对应的工时记录".to_string())?;
        let projs = guard.list_projects().map_err(|e| e.to_string())?;
        let p = projs
            .into_iter()
            .find(|p| p.id == e.project_id)
            .ok_or_else(|| "未找到所属项目信息".to_string())?;
        (e, p)
    };

    let repo_path = custom_repo_path
        .filter(|s| !s.trim().is_empty())
        .or(proj.git_repo_path)
        .ok_or_else(|| {
            "当前项目未关联本地 Git 仓库路径，请先在项目设置中配置本地代码路径".to_string()
        })?;

    if !GitScanner::is_git_repo(&repo_path) {
        return Err(format!("路径「{}」不是有效的 Git 仓库", repo_path));
    }

    // 扫描该工时记录完整时间窗口（start_time 至 end_time）
    let git_activity = GitScanner::extract_activity(&repo_path, entry.start_time, entry.end_time)
        .map_err(|e| format!("读取 Git 活动失败: {}", e))?;

    let commits_len = git_activity.commits.len();
    let commits_desc = git_activity.commits.clone();

    // 格式化提炼文本
    let extracted_text = if !git_activity.commits.is_empty() {
        let commit_items = git_activity
            .commits
            .iter()
            .map(|c| format!("- {}", c))
            .collect::<Vec<_>>()
            .join("\n");
        if !git_activity.diff_stat.is_empty() {
            format!(
                "代码研发与提交 {} 次：\n{}\n\n**代码变动统计**：\n```text\n{}\n```",
                commits_len, commit_items, git_activity.diff_stat
            )
        } else {
            format!("代码研发与提交 {} 次：\n{}", commits_len, commit_items)
        }
    } else if git_activity.has_uncommitted_changes {
        let files = git_activity
            .uncommitted_files
            .iter()
            .map(|f| format!("- {}", f))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "本地未提交工作区变更（共 {} 个文件）：\n{}\n\n```text\n{}\n```",
            git_activity.uncommitted_files.len(),
            files,
            git_activity.diff_stat
        )
    } else {
        "在当前工时时段内未检测到新的 Git 提交记录。".to_string()
    };

    let message = if commits_len > 0 {
        format!("成功从 Git 仓库抽取到 {} 条提交记录！", commits_len)
    } else if git_activity.has_uncommitted_changes {
        format!(
            "检测到 {} 个本地工作区改动文件！",
            git_activity.uncommitted_files.len()
        )
    } else {
        "该时间段内暂无新的 Git 提交记录。".to_string()
    };

    Ok(ExtractGitResultDto {
        success: true,
        commits_count: commits_len,
        commits: commits_desc,
        diff_stat: git_activity.diff_stat,
        extracted_text,
        message,
    })
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
                .filter(|e| {
                    e.start_time
                        .with_timezone(&Local)
                        .format("%Y-%m-%d")
                        .to_string()
                        == today_str
                })
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
                    let summ = e
                        .summary
                        .unwrap_or_else(|| "持续代码开发与测试交付".to_string());
                    lines.push(format!(
                        "- `{}` (投入: {}, 核算: {:.2}人天) : {}",
                        range,
                        format_duration(e.raw_duration_secs),
                        e.billable_days,
                        summ
                    ));
                }
                proj_sections.push(format!(
                    "### 📁 {}\n- **当日核算投入**: {:.2} 人天 ({:.2} {})\n{}",
                    p.name,
                    p_days,
                    p_amount,
                    p.currency,
                    lines.join("\n")
                ));
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
fn toggle_entry_invoiced(
    state: State<AppState>,
    entry_id: String,
    is_invoiced: bool,
) -> Result<(), String> {
    let eid = Uuid::parse_str(&entry_id).map_err(|e| e.to_string())?;
    state
        .db
        .lock()
        .unwrap()
        .set_entry_invoiced(&eid, is_invoiced)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn batch_toggle_entries_invoiced(
    state: State<AppState>,
    entry_ids: Vec<String>,
    is_invoiced: bool,
) -> Result<usize, String> {
    let uids: Vec<Uuid> = entry_ids
        .iter()
        .filter_map(|s| Uuid::parse_str(s).ok())
        .collect();
    let count = uids.len();
    state
        .db
        .lock()
        .unwrap()
        .batch_set_entries_invoiced(&uids, is_invoiced)
        .map_err(|e| e.to_string())?;
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
    status_filter: Option<String>,  // "all" | "pending" | "settled"
    enable_rebalance: Option<bool>, // 是否启用周一至周五 8h/天 削峰填谷 (兼容旧参数)
    settlement_strategy: Option<String>, // "rebalance" | "min8_carry" | "cap8_carry" | "exact"
) -> Result<SettlementReportResult, String> {
    use myth_core::billing::SettlementStrategy;

    let filter_pids: Option<std::collections::HashSet<String>> =
        project_ids.map(|list| list.into_iter().collect());
    let filter_months: Option<std::collections::HashSet<String>> =
        months.map(|list| list.into_iter().collect());
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
        if sorted.is_empty() {
            "全部历史跨度".to_string()
        } else {
            sorted.join(", ")
        }
    } else {
        "全部历史跨度".to_string()
    };

    let (projs, clients) = {
        let guard = state.db.lock().unwrap();
        let p_list = guard.list_projects().unwrap_or_default();
        let c_list = guard.list_clients().unwrap_or_default();
        (p_list, c_list)
    };

    let client_map: std::collections::HashMap<Uuid, String> =
        clients.into_iter().map(|c| (c.id, c.name)).collect();

    for p in &projs {
        let pid_str = p.id.to_string();
        if let Some(ref p_set) = filter_pids {
            if !p_set.is_empty() && !p_set.contains(&pid_str) {
                continue;
            }
        }

        let entries = state
            .db
            .lock()
            .unwrap()
            .list_entries_for_project(&p.id)
            .unwrap_or_default();
        let filtered_entries: Vec<_> = entries
            .into_iter()
            .filter(|e| {
                let d_str = e
                    .start_time
                    .with_timezone(&Local)
                    .format("%Y-%m-%d")
                    .to_string();
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
            let p_settled: f64 = transformed_days
                .iter()
                .filter(|d| d.is_invoiced)
                .map(|d| d.amount_val)
                .sum();
            let p_pending: f64 = transformed_days
                .iter()
                .filter(|d| !d.is_invoiced)
                .map(|d| d.amount_val)
                .sum();

            total_days += p_days;
            total_amount += p_amount;
            settled_amount += p_settled;
            pending_amount += p_pending;

            let mut lines = Vec::new();
            for (idx, r) in transformed_days.into_iter().enumerate() {
                let virtual_id = format!("{}_workday_{}", p.id, idx);
                let status_badge = if r.is_invoiced {
                    "✅ [已结算]"
                } else {
                    "⏳ [待结算]"
                };
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
                    status: "normal".to_string(),
                    meetings: vec![],
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
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let provider = guard
        .get_setting("ai_provider")
        .ok()
        .flatten()
        .unwrap_or_else(|| "codex".to_string());
    let base = guard
        .get_setting("ai_base")
        .ok()
        .flatten()
        .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
    let key = guard
        .get_setting("ai_key")
        .ok()
        .flatten()
        .unwrap_or_default();
    let model = guard
        .get_setting("ai_model")
        .ok()
        .flatten()
        .unwrap_or_else(|| "gpt-4o-mini".to_string());
    let proxy = guard
        .get_setting("ai_proxy")
        .ok()
        .flatten()
        .unwrap_or_default();
    let hours = guard
        .get_setting("hours_per_day")
        .ok()
        .flatten()
        .unwrap_or_else(|| "8.0".to_string());
    let afk = guard
        .get_setting("afk_minutes")
        .ok()
        .flatten()
        .unwrap_or_else(|| "10".to_string());

    let asr_mode = guard
        .get_setting("asr_mode")
        .ok()
        .flatten()
        .unwrap_or_else(|| "local".to_string());
    let asr_api_base = guard
        .get_setting("asr_api_base")
        .ok()
        .flatten()
        .unwrap_or_default();
    let asr_api_key = guard
        .get_setting("asr_api_key")
        .ok()
        .flatten()
        .unwrap_or_default();
    let asr_model = guard
        .get_setting("asr_model")
        .ok()
        .flatten()
        .unwrap_or_else(|| "whisper-1".to_string());
    let asr_local_model = guard
        .get_setting("asr_local_model")
        .ok()
        .flatten()
        .unwrap_or_else(|| "ggml-base.bin".to_string());

    Ok(serde_json::json!({
        "ai_provider": provider,
        "ai_base": base,
        "ai_key": key,
        "ai_model": model,
        "ai_proxy": proxy,
        "hours_per_day": hours,
        "afk_minutes": afk,
        "asr_mode": asr_mode,
        "asr_api_base": asr_api_base,
        "asr_api_key": asr_api_key,
        "asr_model": asr_model,
        "asr_local_model": asr_local_model
    }))
}

#[tauri::command]
fn save_settings(
    state: State<AppState>,
    ai_provider: String,
    ai_base: String,
    ai_key: String,
    ai_model: String,
    ai_proxy: Option<String>,
    hours_per_day: String,
    afk_minutes: String,
    asr_mode: Option<String>,
    asr_api_base: Option<String>,
    asr_api_key: Option<String>,
    asr_model: Option<String>,
    asr_local_model: Option<String>,
) -> Result<(), String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let _ = guard.set_setting("ai_provider", &ai_provider);
    let _ = guard.set_setting("ai_base", &ai_base);
    let _ = guard.set_setting("ai_key", &ai_key);
    let _ = guard.set_setting("ai_model", &ai_model);
    if let Some(ref p) = ai_proxy {
        let _ = guard.set_setting("ai_proxy", p.trim());
    }
    let _ = guard.set_setting("hours_per_day", &hours_per_day);
    let _ = guard.set_setting("afk_minutes", &afk_minutes);

    if let Some(ref m) = asr_mode {
        let _ = guard.set_setting("asr_mode", m.trim());
    }
    if let Some(ref b) = asr_api_base {
        let _ = guard.set_setting("asr_api_base", b.trim());
    }
    if let Some(ref k) = asr_api_key {
        let _ = guard.set_setting("asr_api_key", k.trim());
    }
    if let Some(ref am) = asr_model {
        let _ = guard.set_setting("asr_model", am.trim());
    }
    if let Some(ref lm) = asr_local_model {
        let _ = guard.set_setting("asr_local_model", lm.trim());
    }
    Ok(())
}

#[tauri::command]
fn get_asr_status(state: State<AppState>) -> Result<AsrRuntimeStatus, String> {
    let guard = state.db.lock().map_err(|e| e.to_string())?;
    let ai_cfg = load_ai_config(&guard);
    Ok(get_asr_runtime_status(&ai_cfg))
}

#[tauri::command]
async fn download_whisper_model(
    app: AppHandle,
    state: State<'_, AppState>,
    model_name: String,
) -> Result<String, String> {
    let proxy = {
        let guard = state.db.lock().map_err(|e| e.to_string())?;
        guard
            .get_setting("ai_proxy")
            .ok()
            .flatten()
            .filter(|s| !s.trim().is_empty())
    };

    let app_clone = app.clone();
    let m_name = model_name.clone();
    let res = download_whisper_model_file(
        &model_name,
        proxy.as_deref(),
        move |pct, downloaded, total| {
            let _ = app_clone.emit(
                "whisper_download_progress",
                serde_json::json!({
                    "model": m_name,
                    "percentage": (pct * 10.0).round() / 10.0,
                    "downloaded": downloaded,
                    "total": total,
                    "status": "downloading"
                }),
            );
        },
    )
    .await;

    match res {
        Ok(path) => {
            let _ = app.emit(
                "whisper_download_progress",
                serde_json::json!({
                    "model": model_name,
                    "percentage": 100.0,
                    "status": "completed"
                }),
            );
            Ok(path.to_string_lossy().to_string())
        }
        Err(e) => {
            let _ = app.emit(
                "whisper_download_progress",
                serde_json::json!({
                    "model": model_name,
                    "percentage": 0.0,
                    "status": "failed",
                    "error": e
                }),
            );
            Err(e)
        }
    }
}

#[tauri::command]
fn save_and_open_excel(filename: String, base64_data: String) -> Result<String, String> {
    use std::io::Write;
    let target_dir = user_download_dir();
    let target_path = target_dir.join(&filename);
    let decoded = base64_decode(&base64_data).map_err(|e| e.to_string())?;
    let mut file = std::fs::File::create(&target_path).map_err(|e| e.to_string())?;
    file.write_all(&decoded).map_err(|e| e.to_string())?;

    let path_str = target_path.to_string_lossy().to_string();

    // 尝试直接使用系统默认程序打开文件或者打开所在目录
    let _ = system_open_path(&target_path);

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
    let target_dir = user_download_dir();

    let mut saved_paths = Vec::new();
    for item in files {
        let target_path = target_dir.join(&item.filename);
        let decoded = base64_decode(&item.base64_data).map_err(|e| e.to_string())?;
        let mut file = std::fs::File::create(&target_path).map_err(|e| e.to_string())?;
        file.write_all(&decoded).map_err(|e| e.to_string())?;
        saved_paths.push(target_path.to_string_lossy().to_string());
    }

    // 自动打开 Downloads 目录供用户查看生成的文件
    let _ = system_open_path(&target_dir);

    Ok(saved_paths)
}

#[tauri::command]
fn open_file_or_dir(path: String) -> Result<(), String> {
    let target_path = std::path::Path::new(&path);
    let _ = system_open_path(target_path).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn export_meeting_pdf(
    filename: String,
    title: String,
    project_name: String,
    date_str: String,
    html_content: String,
) -> Result<String, String> {
    use std::io::Write;
    let target_dir = user_download_dir();
    let target_pdf = target_dir.join(&filename);
    let temp_html = std::env::temp_dir().join(format!("meeting_{}.html", uuid::Uuid::new_v4()));

    // 格式化输出高端专业的企业交付级会议纪要 HTML
    let styled_doc = format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<title>{}</title>
<style>
  @page {{
    size: A4 portrait;
    margin: 22mm 18mm 20mm 18mm;
  }}
  * {{ box-sizing: border-box; }}
  body {{
    font-family: -apple-system, BlinkMacSystemFont, "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", "WenQuanYi Micro Hei", sans-serif;
    color: #1e293b;
    line-height: 1.7;
    margin: 0;
    padding: 0;
    background: #ffffff;
    font-size: 13.5px;
    -webkit-print-color-adjust: exact;
    print-color-adjust: exact;
  }}
  .header-card {{
    border-bottom: 2px solid #4f46e5;
    padding-bottom: 16px;
    margin-bottom: 24px;
  }}
  .badge {{
    display: inline-block;
    padding: 3px 10px;
    background: #eef2ff;
    color: #4338ca;
    border: 1px solid #c7d2fe;
    border-radius: 9999px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.5px;
    margin-bottom: 8px;
  }}
  .doc-title {{
    font-size: 22px;
    font-weight: 800;
    color: #0f172a;
    margin: 0 0 10px 0;
    letter-spacing: -0.3px;
  }}
  .meta-grid {{
    display: table;
    width: 100%;
    margin-top: 10px;
    background: #f8fafc;
    border: 1px solid #e2e8f0;
    border-radius: 8px;
    padding: 10px 14px;
    font-size: 12px;
  }}
  .meta-item {{
    display: inline-block;
    margin-right: 24px;
    color: #475569;
  }}
  .meta-item strong {{
    color: #0f172a;
  }}
  .content-area {{
    margin-top: 20px;
  }}
  .content-area h1 {{
    font-size: 18px;
    font-weight: 800;
    color: #0f172a;
    border-left: 4px solid #4f46e5;
    padding-left: 10px;
    margin: 22px 0 10px 0;
  }}
  .content-area h2 {{
    font-size: 15px;
    font-weight: 700;
    color: #1e293b;
    border-left: 3px solid #6366f1;
    padding-left: 8px;
    margin: 18px 0 8px 0;
  }}
  .content-area h3 {{
    font-size: 14px;
    font-weight: 600;
    color: #334155;
    margin: 14px 0 6px 0;
  }}
  .content-area p {{
    margin: 8px 0;
    color: #334155;
  }}
  .content-area ul, .content-area ol {{
    padding-left: 22px;
    margin: 8px 0;
  }}
  .content-area li {{
    margin-bottom: 6px;
    color: #334155;
  }}
  .content-area blockquote {{
    margin: 12px 0;
    padding: 8px 14px;
    background: #f8fafc;
    border-left: 3px solid #cbd5e1;
    color: #64748b;
    font-size: 12.5px;
  }}
  .content-area hr {{
    border: none;
    border-top: 1px solid #e2e8f0;
    margin: 20px 0;
  }}
  .content-area code {{
    background: #f1f5f9;
    padding: 2px 5px;
    border-radius: 4px;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    font-size: 12px;
    color: #0f172a;
  }}
  .content-area pre {{
    background: #0f172a;
    color: #f8fafc;
    padding: 12px 14px;
    border-radius: 8px;
    overflow-x: auto;
    font-size: 11.5px;
    line-height: 1.5;
  }}
  .content-area pre code {{
    background: transparent;
    color: inherit;
    padding: 0;
  }}
  .content-area table {{
    width: 100%;
    border-collapse: collapse;
    margin: 16px 0;
    font-size: 11.5px;
    line-height: 1.5;
    page-break-inside: auto;
  }}
  .content-area tr {{
    page-break-inside: avoid;
    page-break-after: auto;
  }}
  .content-area th, .content-area td {{
    border: 1px solid #cbd5e1;
    padding: 7px 10px;
    text-align: left;
    vertical-align: top;
  }}
  .content-area th {{
    background-color: #f1f5f9;
    color: #0f172a;
    font-weight: 700;
  }}
  .content-area tr:nth-child(even) td {{
    background-color: #f8fafc;
  }}
  .footer-bar {{
    margin-top: 40px;
    padding-top: 14px;
    border-top: 1px solid #e2e8f0;
    display: flex;
    justify-content: space-between;
    font-size: 11px;
    color: #94a3b8;
  }}
  .sign-area {{
    margin-top: 36px;
    display: flex;
    justify-content: space-between;
    font-size: 12px;
    color: #64748b;
    padding: 16px;
    background: #f8fafc;
    border: 1px dashed #cbd5e1;
    border-radius: 8px;
  }}
</style>
</head>
<body>
  <div class="header-card">
    <div class="badge">人天神话 · 商业技术交付档案</div>
    <h1 class="doc-title">{}</h1>
    <div class="meta-grid">
      <div class="meta-item">所属项目：<strong>{}</strong></div>
      <div class="meta-item">会议/记录日期：<strong>{}</strong></div>
      <div class="meta-item">文档性质：<strong>决议与交付确认单</strong></div>
    </div>
  </div>

  <div class="content-area">
    {}
  </div>

  <div class="sign-area">
    <div><strong>承接方确认：</strong>已核对技术方案与实施路径</div>
    <div><strong>客户方签署/查阅：</strong>____________________</div>
  </div>

  <div class="footer-bar">
    <div>生成系统：人天神话 (MythManDay) 数字化人天工时核算系统</div>
    <div>本文件为双方技术沟通与商务实施备忘凭证</div>
  </div>
</body>
</html>"#,
        title, title, project_name, date_str, html_content
    );

    let mut f = std::fs::File::create(&temp_html).map_err(|e| e.to_string())?;
    f.write_all(styled_doc.as_bytes())
        .map_err(|e| e.to_string())?;

    // 寻找 Chromium/Google Chrome 或其它 headless 渲染引擎
    let chrome_bin = which_browser_binary();

    if let Some(bin) = chrome_bin {
        let output = std::process::Command::new(bin)
            .arg("--headless")
            .arg("--disable-gpu")
            .arg("--no-pdf-header-footer")
            .arg(format!("--print-to-pdf={}", target_pdf.to_string_lossy()))
            .arg(&temp_html)
            .output()
            .map_err(|e| format!("执行 PDF 导出命令失败: {}", e))?;

        let _ = std::fs::remove_file(&temp_html);

        if !output.status.success() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            return Err(format!("PDF 导出失败: {}", err_msg));
        }
    } else {
        let _ = std::fs::remove_file(&temp_html);
        return Err(
            "系统未检测到 Google Chrome 或 Chromium 浏览器环境，无法完成自动化 PDF 打印导出。"
                .to_string(),
        );
    }

    let path_str = target_pdf.to_string_lossy().to_string();
    let _ = system_open_path(&target_pdf);

    Ok(path_str)
}

fn which_browser_binary() -> Option<std::path::PathBuf> {
    let candidates = [
        "/usr/bin/google-chrome",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
        "/opt/google/chrome/google-chrome",
        "/snap/bin/chromium",
    ];
    for c in &candidates {
        let p = std::path::PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
    }
    None
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

    // 自动恢复未完成的会议录音状态（防止退出或重启后丢失当前会议）
    {
        let guard = db.lock().unwrap();
        let pid_opt = guard.get_setting("active_meeting_pid").ok().flatten();
        let mid_opt = guard.get_setting("active_meeting_id").ok().flatten();
        let topic_opt = guard.get_setting("active_meeting_topic").ok().flatten();
        let path_opt = guard.get_setting("active_meeting_path").ok().flatten();

        if let (Some(pid_str), Some(topic), Some(path_str)) = (pid_opt, topic_opt, path_opt) {
            if let Ok(pid) = Uuid::parse_str(&pid_str) {
                let mid = mid_opt
                    .and_then(|s| Uuid::parse_str(&s).ok())
                    .unwrap_or_else(Uuid::new_v4);
                let p = std::path::PathBuf::from(&path_str);
                if p.exists() {
                    *active_recording.lock().unwrap() = Some((pid, mid, topic, p, Utc::now()));
                }
            }
        }
    }

    // 自动恢复由于崩溃或异常退出遗留的悬挂 pending 转写状态
    if let Ok(guard) = db.lock() {
        let _ = guard.recover_dangling_audio_clips();
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
        .plugin(tauri_plugin_dialog::init())
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
            start_recording_for_meeting,
            stop_recording_clip,
            retry_transcribe_audio_clip,
            finish_meeting,
            stop_current_meeting,
            save_meeting_summary_to_project,
            play_audio,
            stop_audio,
            get_audio_data_url,
            create_project,
            update_project,
            delete_project,
            add_project_note,
            list_project_notes,
            delete_note,
            fetch_git_work_summary,
            fetch_batch_git_work_summary,
            parse_custom_batch_summary,
            log_batch_manual_entries,
            log_manual_entry,
            log_manual_meeting,
            transcribe_entry_audio,
            resummarize_meeting_entry,
            update_entry_summary,
            extract_entry_git_summary,
            generate_daily_markdown,
            toggle_entry_invoiced,
            batch_toggle_entries_invoiced,
            generate_settlement_report,
            get_settings,
            save_settings,
            get_asr_status,
            download_whisper_model,
            save_and_open_excel,
            save_multiple_excel_files,
            open_file_or_dir,
            export_meeting_pdf,
            upload_meeting_material,
            delete_meeting_material,
            list_meeting_materials,
            get_material_data_url,
            open_material_file,
            get_system_clipboard,
            get_meeting_repo_target_info,
            pick_repo_parent_folder,
            write_meeting_to_git_repo
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    Ok(())
}
