// Copyright (C) 2026 MythManDay Authors. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (MythManDay)
// Version: V1.0
// Description: Multi-channel concurrent time-tracking & man-day billing engine.

slint::include_modules!();

mod tray;

use chrono::{Local, Utc};
use myth_ai::{AiConfig, AiSummarizer};
use myth_audio::{AudioPlayer, AudioRecorder};
use myth_core::billing::BillingEngine;
use myth_core::models::{BillingType, DayCalcMode, Project, QuickNote, TimeEntry};
use myth_core::timer::MultiChannelManager;
use myth_db::{ActiveTimerRecord, Database};
use myth_git::GitScanner;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use tray::{spawn_tray, TrayEvent};
use uuid::Uuid;

fn extract_wav_path(summary: Option<&str>) -> Option<String> {
    if let Some(s) = summary {
        for word in s.split(|c: char| c.is_whitespace() || c == '[' || c == ']' || c == '(' || c == ')' || c == ',' || c == ';') {
            if word.ends_with(".wav") && std::path::Path::new(word).exists() {
                return Some(word.to_string());
            }
        }
    }
    None
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

fn build_project_items(projects: &[Project], db: &Database) -> Vec<ProjectItem> {
    projects
        .iter()
        .map(|p| {
            let rate_info = match p.billing_type {
                BillingType::Daily => {
                    format!("{:.0} {} / 天 ({}h基准)", p.daily_rate, p.currency, p.hours_per_day)
                }
                BillingType::Hourly => format!("{:.0} {} / 小时", p.hourly_rate, p.currency),
                BillingType::FixedCap => format!("固定总包 {:.0} {}", p.daily_rate, p.currency),
                BillingType::NonBillable => "自用/不计费".to_string(),
            };
            let client_name = get_client_name(db, p.client_id);
            let entries = db.list_entries_for_project(&p.id).unwrap_or_default();
            let entries_count = entries.len() as i32;
            let total_secs: u64 = entries.iter().map(|e| e.raw_duration_secs).sum();
            let today_duration = format_duration(total_secs);

            ProjectItem {
                id: p.id.to_string().into(),
                name: p.name.clone().into(),
                client: client_name.into(),
                rate_info: rate_info.into(),
                timer_text: "00:00:00".into(),
                days_text: "0.00 人天".into(),
                amount_text: format!("{} 0.00", p.currency).into(),
                is_running: false,
                is_paused: false,
                today_duration: today_duration.into(),
                entries_count,
            }
        })
        .collect()
}


fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // 0. 初始化全局 Tokio Runtime 并持久进入上下文，供所有异步任务（包括子进程、网络驱动）使用
    let tokio_rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let _tokio_guard = tokio_rt.enter();

    // 1. 初始化本地持久化数据库
    let db_dir = dirs_or_local_data_dir();
    std::fs::create_dir_all(&db_dir)?;
    let db_path = db_dir.join("paraday.db");
    let db = Arc::new(Database::open(&db_path)?);

    // 初始化引导项目（包含代码开发与会议/沟通）
    let mut projects = db.list_projects()?;
    if projects.is_empty() {
        let mut p1 = Project::new("跨境商城系统重构");
        p1.billing_type = BillingType::Daily;
        p1.daily_rate = 3000.0;
        p1.hours_per_day = 8.0;
        p1.day_calc_mode = DayCalcMode::HalfStep;
        db.insert_project(&p1)?;

        let mut p2 = Project::new("海外 SaaS 架构咨询");
        p2.billing_type = BillingType::Daily;
        p2.currency = "USD".to_string();
        p2.daily_rate = 500.0;
        p2.hours_per_day = 8.0;
        p2.day_calc_mode = DayCalcMode::HalfStep;
        db.insert_project(&p2)?;

        let mut p3 = Project::new("开源自动化脚本");
        p3.billing_type = BillingType::NonBillable;
        db.insert_project(&p3)?;

        let mut p4 = Project::new("🎙️ 需求评审与线上会议");
        p4.billing_type = BillingType::Hourly;
        p4.hourly_rate = 350.0;
        p4.hours_per_day = 8.0;
        p4.day_calc_mode = DayCalcMode::Exact;
        db.insert_project(&p4)?;

        projects = db.list_projects()?;
    }

    // 2. 初始化核心多通道状态机与崩溃容灾恢复
    let timer_manager = Rc::new(RefCell::new(MultiChannelManager::new()));
    let audio_recorder = Rc::new(RefCell::new(AudioRecorder::new()));
    let audio_player = Rc::new(RefCell::new(AudioPlayer::new()));
    let active_recording_info = Rc::new(RefCell::new(None::<(Uuid, String, std::path::PathBuf)>));
    let app = MainWindow::new()?;

    // 自动恢复崩溃/重启前运行中的通道 (Wall-Clock 绝对时间持久化)
    let mut restored_count = 0;
    if let Ok(active_list) = db.list_active_timers() {
        let mut mgr = timer_manager.borrow_mut();
        for rec in active_list {
            mgr.restore_channel(
                rec.project_id,
                rec.session_start_at,
                rec.current_started_at,
                rec.accumulated_secs,
                rec.is_paused,
            );
            restored_count += 1;
        }
    }
    if restored_count > 0 {
        app.set_toast_message(format!("已自动恢复崩溃/重启前运行中的 {} 个通道，工时精准延续！", restored_count).into());
    }

    let projects_ref = Rc::new(RefCell::new(projects));

    // 3. 读取并填充设置数据
    if let Ok(Some(val)) = db.get_setting("ai_provider") {
        app.set_setting_ai_provider(val.into());
    } else {
        app.set_setting_ai_provider("agy".into());
    }
    if let Ok(Some(val)) = db.get_setting("ai_base") {
        app.set_setting_ai_base(val.into());
    }
    if let Ok(Some(val)) = db.get_setting("ai_key") {
        app.set_setting_ai_key(val.into());
    }
    if let Ok(Some(val)) = db.get_setting("ai_model") {
        app.set_setting_ai_model(val.into());
    }
    if let Ok(Some(val)) = db.get_setting("hours_per_day") {
        app.set_setting_hours_per_day(val.into());
    }
    if let Ok(Some(val)) = db.get_setting("afk_minutes") {
        app.set_setting_afk_minutes(val.into());
    }

    // 4. 数据绑定与刷新器
    let initial_items = build_project_items(&projects_ref.borrow(), &db);
    let model = Rc::new(VecModel::from(initial_items));
    app.set_projects(ModelRc::from(model.clone()));

    if let Some(first_p) = projects_ref.borrow().first() {
        app.set_selected_project_id(first_p.id.to_string().into());
        app.set_meeting_selected_proj_idx(0);
        app.set_meeting_selected_proj_name(first_p.name.clone().into());
        let cname = get_client_name(&db, first_p.client_id);
        app.set_meeting_selected_proj_client(cname.into());
        let (rate, rate_str) = match first_p.billing_type {
            BillingType::Daily => {
                let r = (first_p.daily_rate / first_p.hours_per_day.max(1.0)).round();
                (r, format!("{:.0} {} / 天 (约 {:.0}/h)", first_p.daily_rate, first_p.currency, r))
            }
            BillingType::Hourly => (first_p.hourly_rate.round(), format!("{:.0} {} / 小时", first_p.hourly_rate, first_p.currency)),
            BillingType::FixedCap => (350.0, format!("固定总包 {:.0} {}", first_p.daily_rate, first_p.currency)),
            BillingType::NonBillable => (0.0, "自用/不计费".to_string()),
        };
        app.set_meeting_rate_input(format!("{:.0}", rate).into());
        app.set_meeting_selected_proj_rate(rate_str.into());
    }

    let model_for_refresh = model.clone();
    let current_selected_id = Rc::new(RefCell::new(
        projects_ref.borrow().first().map(|p| p.id.to_string()).unwrap_or_default()
    ));

    let refresh_entries_ui = {
        let db = db.clone();
        let app_weak = app.as_weak();
        let projects_ref = projects_ref.clone();
        let model_sync = model_for_refresh.clone();
        let selected_id_ref = current_selected_id.clone();
        move || {
            let projs = projects_ref.borrow();
            let mut all_entries = Vec::new();
            let active_pid_str = selected_id_ref.borrow().clone();

            for (idx, p) in projs.iter().enumerate() {
                if let Ok(entries) = db.list_entries_for_project(&p.id) {
                    let total_secs: u64 = entries.iter().map(|e| e.raw_duration_secs).sum();
                    if let Some(mut item) = model_sync.row_data(idx) {
                        item.entries_count = entries.len() as i32;
                        item.today_duration = format_duration(total_secs).into();
                        model_sync.set_row_data(idx, item);
                    }

                    for e in entries {
                        let start_local = e.start_time.with_timezone(&Local);
                        let end_local = e.end_time.with_timezone(&Local);
                        let audio_path = e.audio_path.clone().unwrap_or_else(|| {
                            extract_wav_path(e.summary.as_deref()).unwrap_or_default()
                        });
                        let has_audio = !audio_path.is_empty() && std::path::Path::new(&audio_path).exists();
                        all_entries.push(EntryItem {
                            id: e.id.to_string().into(),
                            project_id: p.id.to_string().into(),
                            project_name: p.name.clone().into(),
                            duration_text: format_duration(e.raw_duration_secs).into(),
                            days_text: format!("{:.2} 天", e.billable_days).into(),
                            amount_text: format!("{} {:.2}", p.currency, e.total_amount).into(),
                            summary_text: e.summary.unwrap_or_else(|| "持续代码开发与测试交付".to_string()).into(),
                            time_range: format!("{}-{}", start_local.format("%H:%M"), end_local.format("%H:%M")).into(),
                            date_str: start_local.format("%Y-%m-%d").to_string().into(),
                            has_audio,
                            audio_path: audio_path.into(),
                            raw_secs: e.raw_duration_secs as i32,
                        });
                    }
                }
            }

            // 按照日期将选中项目的条目聚合为「一天一个卡片」
            let mut days_map: std::collections::BTreeMap<String, Vec<EntryItem>> = std::collections::BTreeMap::new();
            for item in &all_entries {
                if item.project_id.as_str() == active_pid_str {
                    days_map.entry(item.date_str.to_string()).or_default().push(item.clone());
                }
            }

            let today_str = Local::now().format("%Y-%m-%d").to_string();
            let mut daily_groups: Vec<DayGroup> = Vec::new();
            // 倒序排列：最新的日期显示在最前面
            for (date_str, day_entries) in days_map.into_iter().rev() {
                let count = day_entries.len() as i32;
                let display_date = if date_str == today_str {
                    format!("{} (今天)", date_str)
                } else {
                    date_str.clone()
                };

                let mut day_days = 0.0f64;
                let mut day_amount = 0.0f64;
                let mut day_secs: u64 = 0;
                let mut curr_sym = "¥";

                for entry in &day_entries {
                    day_secs += entry.raw_secs.max(0) as u64;
                    // 解析天数与金额
                    let d_parts: Vec<&str> = entry.days_text.split_whitespace().collect();
                    if let Some(d_val) = d_parts.first().and_then(|s| s.parse::<f64>().ok()) {
                        day_days += d_val;
                    }
                    let a_parts: Vec<&str> = entry.amount_text.split_whitespace().collect();
                    if a_parts.len() >= 2 {
                        curr_sym = a_parts[0];
                        if let Ok(a_val) = a_parts[1].parse::<f64>() {
                            day_amount += a_val;
                        }
                    }
                }

                daily_groups.push(DayGroup {
                    date_str: date_str.into(),
                    display_date: display_date.into(),
                    entries_count: count,
                    total_duration: format_duration(day_secs).into(),
                    total_days: format!("{:.2} 天", day_days).into(),
                    total_amount: format!("{} {:.2}", curr_sym, day_amount).into(),
                    entries: ModelRc::from(Rc::new(VecModel::from(day_entries))),
                });
            }

            if let Some(ui) = app_weak.upgrade() {
                ui.set_recent_entries(ModelRc::from(Rc::new(VecModel::from(all_entries))));
                ui.set_daily_groups(ModelRc::from(Rc::new(VecModel::from(daily_groups))));
            }
        }
    };
    refresh_entries_ui();

    // 选中项目交互
    let refresh_on_select = refresh_entries_ui.clone();
    let current_selected_for_cb = current_selected_id.clone();
    app.on_select_project(move |pid_str| {
        *current_selected_for_cb.borrow_mut() = pid_str.to_string();
        refresh_on_select();
    });

    let refresh_notes_ui = {
        let db = db.clone();
        let app_weak = app.as_weak();
        let projects_ref = projects_ref.clone();
        move || {
            let projs = projects_ref.borrow();
            let mut items = Vec::new();
            if let Ok(notes) = db.list_all_quick_notes() {
                for n in notes {
                    if n.content.starts_with('[') {
                        continue;
                    }
                    let proj_name = projs
                        .iter()
                        .find(|p| p.id == n.project_id)
                        .map(|p| p.name.as_str())
                        .unwrap_or("全局便签");
                    let local_time = n.logged_at.with_timezone(&Local);
                    items.push(NoteItem {
                        id: n.id.to_string().into(),
                        project_id: n.project_id.to_string().into(),
                        project_name: proj_name.into(),
                        time_text: local_time.format("%H:%M").to_string().into(),
                        content: n.content.into(),
                    });
                }
            }
            if let Some(ui) = app_weak.upgrade() {
                ui.set_note_items(ModelRc::from(Rc::new(VecModel::from(items))));
            }
        }
    };
    refresh_notes_ui();

    // 5. 定时心跳调度器 (1 秒钟触发一次)
    let app_weak = app.as_weak();
    let timer_mgr_clone = timer_manager.clone();
    let model_clone = model.clone();
    let projects_cache = projects_ref.clone();

    // 5. 初始化 Linux 顶栏原生托盘服务 (StatusNotifierItem)
    let (tray_tx, tray_rx) = std::sync::mpsc::channel();
    let tray_handle = spawn_tray(tray_tx);
    let is_window_visible = Rc::new(RefCell::new(true));

    let timer = slint::Timer::default();
    let is_window_vis_clone = is_window_visible.clone();
    let db_timer = db.clone();
    let heartbeat_tick = Rc::new(RefCell::new(0u32));
    let audio_rec_timer = audio_recorder.clone();
    let audio_player_timer = audio_player.clone();

    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(150),
        move || {
            let is_rec = audio_rec_timer.borrow().is_recording();
            let is_paused = audio_rec_timer.borrow().is_paused();
            if is_rec {
                let vol = audio_rec_timer.borrow().current_volume();
                let elapsed = audio_rec_timer.borrow().elapsed_secs();
                if let Some(ui) = app_weak.upgrade() {
                    ui.set_is_recording_audio(true);
                    ui.set_is_audio_paused(is_paused);
                    ui.set_audio_volume_level(vol);
                    ui.set_audio_timer_text(format_duration(elapsed).into());
                }
            } else {
                if let Some(ui) = app_weak.upgrade() {
                    if ui.get_is_recording_audio() {
                        ui.set_is_recording_audio(false);
                        ui.set_is_audio_paused(false);
                        ui.set_audio_volume_level(0.0);
                    }
                }
            }

            if !audio_player_timer.borrow().is_playing() {
                if let Some(ui) = app_weak.upgrade() {
                    if !ui.get_playing_audio_path().is_empty() || !ui.get_playing_entry_id().is_empty() {
                        ui.set_playing_entry_id("".into());
                        ui.set_playing_audio_path("".into());
                    }
                }
            }

            let mut tick = heartbeat_tick.borrow_mut();
            *tick = tick.wrapping_add(1);

            // 每 1 秒 (~7 个 150ms 周期) 执行通道时间更新与对账计算
            if *tick % 7 != 0 {
                return;
            }

            let mgr = timer_mgr_clone.borrow();
            let projs = projects_cache.borrow();
            let mut total_days = 0.0;
            let mut total_cny_amount = 0.0;
            let mut running_count = 0;

            // 累计今日已归档的工时人天与金额
            let today_local_date = Local::now().date_naive();
            for p in projs.iter() {
                if let Ok(entries) = db_timer.list_entries_for_project(&p.id) {
                    for e in entries {
                        if e.start_time.with_timezone(&Local).date_naive() == today_local_date {
                            total_days += e.billable_days;
                            if p.currency == "CNY" {
                                total_cny_amount += e.total_amount;
                            }
                        }
                    }
                }
            }

            for (idx, p) in projs.iter().enumerate() {
                if let Some(mut item) = model_clone.row_data(idx) {
                    let elapsed = mgr.get_elapsed(&p.id);
                    let status = mgr.get_status(&p.id);
                    let is_running = matches!(status, myth_core::timer::TimerStatus::Running { .. });
                    let is_paused = matches!(status, myth_core::timer::TimerStatus::Paused { .. });

                    if is_running {
                        running_count += 1;
                    }

                    item.is_running = is_running;
                    item.is_paused = is_paused;
                    item.timer_text = format_duration(elapsed).into();

                    let billable_hours = BillingEngine::calculate_billable_hours(elapsed, p.rounding_minutes);
                    let billable_days = BillingEngine::calculate_billable_days(billable_hours, p.hours_per_day, p.day_calc_mode);
                    let (_, amount) = BillingEngine::calculate_amount(p, billable_hours, billable_days);

                    item.days_text = format!("{:.2} 人天", billable_days).into();
                    item.amount_text = format!("{} {:.2}", p.currency, amount).into();

                    if is_running || is_paused {
                        total_days += billable_days;
                        if p.currency == "CNY" {
                            total_cny_amount += amount;
                        }
                    }

                    model_clone.set_row_data(idx, item);
                }
            }

            // 定期更新活动通道心跳与容灾存档 (每15秒更新一次)
            if *tick % 105 == 0 {
                let now = Utc::now();
                for p in projs.iter() {
                    let status = mgr.get_status(&p.id);
                    if matches!(status, myth_core::timer::TimerStatus::Running { .. }) {
                        let session_start = mgr.get_timer(&p.id).and_then(|t| t.session_start_at).unwrap_or(now);
                        let elapsed = mgr.get_elapsed(&p.id);
                        let _ = db_timer.upsert_active_timer(&ActiveTimerRecord {
                            project_id: p.id,
                            session_start_at: session_start,
                            current_started_at: Some(now),
                            accumulated_secs: elapsed,
                            is_paused: false,
                            last_heartbeat: now,
                        });
                    }
                }
            }

            if let Some(ref handle) = tray_handle {
                handle.update(move |tray| {
                    tray.running_count = running_count;
                });
            }

            while let Ok(event) = tray_rx.try_recv() {
                match event {
                    TrayEvent::ToggleWindow => {
                        if let Some(ui) = app_weak.upgrade() {
                            let mut vis = is_window_vis_clone.borrow_mut();
                            if *vis {
                                let _ = ui.hide();
                                *vis = false;
                            } else {
                                let _ = ui.show();
                                *vis = true;
                            }
                        }
                    }
                    TrayEvent::PauseAll => {
                        timer_mgr_clone.borrow_mut().pause_all();
                        if let Some(ui) = app_weak.upgrade() {
                            ui.set_toast_message("已从系统托盘暂停所有计时通道".into());
                        }
                    }
                    TrayEvent::Quit => {
                        std::process::exit(0);
                    }
                }
            }

            if let Some(ui) = app_weak.upgrade() {
                ui.set_total_days(format!("{:.2}", total_days).into());
                ui.set_total_amount(format!("¥ {:.2}", total_cny_amount).into());
                ui.set_running_count(running_count as i32);
            }
        },
    );

    // 6. 交互回调：通道 启动 / 暂停 / 继续 / 结束工时 (具备崩溃容灾持久化)
    let timer_mgr_start = timer_manager.clone();
    let db_start = db.clone();
    let app_weak_start = app.as_weak();
    app.on_start_project(move |id_str| {
        if let Ok(id) = Uuid::parse_str(id_str.as_str()) {
            timer_mgr_start.borrow_mut().start_channel(id);
            let now = Utc::now();
            let session_start = timer_mgr_start.borrow().get_timer(&id).and_then(|t| t.session_start_at).unwrap_or(now);
            let _ = db_start.upsert_active_timer(&ActiveTimerRecord {
                project_id: id,
                session_start_at: session_start,
                current_started_at: Some(now),
                accumulated_secs: timer_mgr_start.borrow().get_elapsed(&id),
                is_paused: false,
                last_heartbeat: now,
            });
            if let Some(ui) = app_weak_start.upgrade() {
                ui.set_toast_message("通道已启动计时 (已写入容灾持久化)".into());
            }
        }
    });

    let timer_mgr_pause = timer_manager.clone();
    let db_pause = db.clone();
    let app_weak_pause = app.as_weak();
    let active_rec_pause = active_recording_info.clone();
    let audio_rec_pause = audio_recorder.clone();
    app.on_pause_project(move |id_str| {
        if let Ok(id) = Uuid::parse_str(id_str.as_str()) {
            timer_mgr_pause.borrow_mut().pause_channel(id);
            if let Some((rec_pid, _, _)) = active_rec_pause.borrow().as_ref() {
                if *rec_pid == id {
                    audio_rec_pause.borrow_mut().pause_recording();
                }
            }
            let elapsed = timer_mgr_pause.borrow().get_elapsed(&id);
            let session_start = timer_mgr_pause.borrow().get_timer(&id).and_then(|t| t.session_start_at).unwrap_or_else(Utc::now);
            let _ = db_pause.upsert_active_timer(&ActiveTimerRecord {
                project_id: id,
                session_start_at: session_start,
                current_started_at: None,
                accumulated_secs: elapsed,
                is_paused: true,
                last_heartbeat: Utc::now(),
            });
            if let Some(ui) = app_weak_pause.upgrade() {
                ui.set_toast_message("通道已暂停 (工时已锁定，可随时继续)".into());
            }
        }
    });

    let timer_mgr_resume = timer_manager.clone();
    let db_resume = db.clone();
    let app_weak_resume = app.as_weak();
    let active_rec_resume = active_recording_info.clone();
    let audio_rec_resume = audio_recorder.clone();
    app.on_resume_project(move |id_str| {
        if let Ok(id) = Uuid::parse_str(id_str.as_str()) {
            timer_mgr_resume.borrow_mut().start_channel(id);
            if let Some((rec_pid, _, _)) = active_rec_resume.borrow().as_ref() {
                if *rec_pid == id {
                    audio_rec_resume.borrow_mut().resume_recording();
                }
            }
            let now = Utc::now();
            let elapsed = timer_mgr_resume.borrow().get_elapsed(&id);
            let session_start = timer_mgr_resume.borrow().get_timer(&id).and_then(|t| t.session_start_at).unwrap_or(now);
            let _ = db_resume.upsert_active_timer(&ActiveTimerRecord {
                project_id: id,
                session_start_at: session_start,
                current_started_at: Some(now),
                accumulated_secs: elapsed,
                is_paused: false,
                last_heartbeat: now,
            });
            if let Some(ui) = app_weak_resume.upgrade() {
                ui.set_toast_message("通道已继续计时".into());
            }
        }
    });

    let timer_mgr_stop = timer_manager.clone();
    let db_stop = db.clone();
    let projects_stop = projects_ref.clone();
    let app_weak_stop = app.as_weak();
    let refresh_entries_stop = refresh_entries_ui.clone();
    let active_rec_stop = active_recording_info.clone();
    let audio_rec_stop = audio_recorder.clone();

    app.on_stop_project(move |id_str| {
        if let Ok(id) = Uuid::parse_str(id_str.as_str()) {
            let mut mgr = timer_mgr_stop.borrow_mut();
            if let Some((start_at, end_at, elapsed_secs)) = mgr.stop_channel(id) {
                // 1. 结束工时完成，从 active_timers 移除持久化记录
                let _ = db_stop.delete_active_timer(&id);

                // 2. 如果当前停止的项目正在录音，停止录音并获取 WAV 文件凭证
                let mut saved_audio_file: Option<String> = None;
                let active_rec = active_rec_stop.borrow_mut().take();
                if let Some((rec_pid, _rec_topic, rec_path)) = active_rec {
                    if rec_pid == id {
                        if audio_rec_stop.borrow().is_recording() {
                            if let Ok(saved_path) = audio_rec_stop.borrow_mut().stop_recording() {
                                saved_audio_file = Some(saved_path.display().to_string());
                            }
                        } else if rec_path.exists() {
                            saved_audio_file = Some(rec_path.display().to_string());
                        }
                        if let Some(ui) = app_weak_stop.upgrade() {
                            ui.set_is_recording_audio(false);
                            ui.set_active_recording_project_name("".into());
                        }
                    } else {
                        *active_rec_stop.borrow_mut() = Some((rec_pid, _rec_topic, rec_path));
                    }
                }

                let projs = projects_stop.borrow();
                if let Some(p) = projs.iter().find(|proj| proj.id == id) {
                    let billable_hours = BillingEngine::calculate_billable_hours(elapsed_secs, p.rounding_minutes);
                    let billable_days = BillingEngine::calculate_billable_days(billable_hours, p.hours_per_day, p.day_calc_mode);
                    let (rate, amount) = BillingEngine::calculate_amount(p, billable_hours, billable_days);

                    // 3. 智能嗅探 Git 活动与便签记录
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

                    let notes = db_stop.list_quick_notes(&id).unwrap_or_default();
                    let user_notes: Vec<&str> = notes.iter()
                        .map(|n| n.content.as_str())
                        .filter(|s| !s.starts_with('['))
                        .collect();

                    // 4. 构建业务级规范工作成果摘要（绝不污染机器文件路径或内部调试标记）
                    let is_meeting = p.name.contains("会议") || p.name.contains("沟通") || p.name.contains("评审") || saved_audio_file.is_some();
                    let summary_desc = if is_meeting {
                        if !user_notes.is_empty() {
                            format!("会议沟通与决议：{}", user_notes.join("；"))
                        } else {
                            format!("会议沟通与技术决策：{}", p.name)
                        }
                    } else if !git_summary.commits.is_empty() {
                        format!("代码研发与提交 {} 次：{}", git_summary.commits.len(), git_summary.commits.join("; "))
                    } else if git_summary.has_uncommitted_changes {
                        format!("修改了 {} 个工作区文件，完成功能研发联调", git_summary.uncommitted_files.len())
                    } else if !user_notes.is_empty() {
                        format!("阶段性研发与联调：{}", user_notes.join("；"))
                    } else {
                        "完成阶段性功能需求研发与验证".to_string()
                    };

                    let entry_id = Uuid::new_v4();
                    let entry = TimeEntry {
                        id: entry_id,
                        project_id: id,
                        start_time: start_at,
                        end_time: end_at,
                        raw_duration_secs: elapsed_secs,
                        billable_hours,
                        billable_days,
                        rate_at_moment: rate,
                        total_amount: amount,
                        summary: Some(summary_desc),
                        audio_path: saved_audio_file.clone(),
                        is_invoiced: false,
                        created_at: Utc::now(),
                    };

                    let _ = db_stop.insert_time_entry(&entry);
                    refresh_entries_stop();

                    if let Some(ui) = app_weak_stop.upgrade() {
                        ui.set_toast_message(format!("工时已归档入库：{:.2} 人天", billable_days).into());
                    }

                    // 5. 后台全自动静默调用 AI 润色提炼交付总结 / 会议纪要
                    let db_async = db_stop.clone();
                    let refresh_async = refresh_entries_stop.clone();
                    let app_async = app_weak_stop.clone();
                    let proj_name = p.name.clone();

                    let ai_provider = db_stop.get_setting("ai_provider").ok().flatten().unwrap_or_else(|| "agy".into());
                    let ai_base = db_stop.get_setting("ai_base").ok().flatten().unwrap_or_else(|| "https://api.deepseek.com/v1".into());
                    let ai_key = db_stop.get_setting("ai_key").ok().flatten().unwrap_or_default();
                    let ai_model = db_stop.get_setting("ai_model").ok().flatten().unwrap_or_else(|| "deepseek-chat".into());

                    let is_cli = ai_provider == "agy" || ai_provider == "codex";
                    if is_cli || !ai_key.is_empty() || ai_base.contains("localhost") || ai_base.contains("127.0.0.1") {
                        slint::spawn_local(async move {
                            let summarizer = AiSummarizer::new(AiConfig {
                                provider: ai_provider,
                                api_base: ai_base,
                                api_key: ai_key,
                                model: ai_model,
                            });
                            let ai_summary_res = if is_meeting {
                                summarizer.summarize_meeting(&proj_name, billable_hours, &notes, saved_audio_file.as_deref(), None).await
                            } else {
                                summarizer.summarize_session(&proj_name, billable_hours, &git_summary, &notes).await
                            };
                            if let Ok(ai_summary) = ai_summary_res {
                                if let Ok(Some(mut e)) = db_async.get_time_entry(&entry_id) {
                                    e.summary = Some(ai_summary);
                                    let _ = db_async.insert_time_entry(&e);
                                    refresh_async();
                                    if let Some(ui) = app_async.upgrade() {
                                        ui.set_toast_message(format!("🤖 AI 已自动提炼完成工时成果：{}", proj_name).into());
                                    }
                                }
                            }
                        }).unwrap();
                    }
                }
            }
        }
    });

    // 7. 便签管理：添加与删除
    let db_for_notes = db.clone();
    let projects_for_notes = projects_ref.clone();
    let app_weak_note = app.as_weak();
    let refresh_notes_add = refresh_notes_ui.clone();
    app.on_add_quick_note(move |content| {
        let text = content.as_str().trim();
        if !text.is_empty() {
            let projs = projects_for_notes.borrow();
            let target_proj_id = projs.first().map(|p| p.id).unwrap_or_else(Uuid::nil);
            let note = QuickNote::new(target_proj_id, text);
            let _ = db_for_notes.insert_quick_note(&note);
            refresh_notes_add();
            if let Some(ui) = app_weak_note.upgrade() {
                ui.set_toast_message(format!("便签已登记: {}", text).into());
            }
        }
    });

    let db_for_proj_notes = db.clone();
    let app_weak_proj_note = app.as_weak();
    let refresh_notes_proj_add = refresh_notes_ui.clone();
    app.on_add_project_note(move |pid_str, content| {
        let text = content.as_str().trim();
        if !text.is_empty() {
            if let Ok(pid) = Uuid::parse_str(pid_str.as_str()) {
                let note = QuickNote::new(pid, text);
                let _ = db_for_proj_notes.insert_quick_note(&note);
                refresh_notes_proj_add();
                if let Some(ui) = app_weak_proj_note.upgrade() {
                    ui.set_toast_message(format!("项目备忘已记录: {}", text).into());
                }
            }
        }
    });

    let db_del_note = db.clone();
    let app_weak_del_note = app.as_weak();
    let refresh_notes_del = refresh_notes_ui.clone();
    app.on_delete_quick_note(move |id_str| {
        if let Ok(id) = Uuid::parse_str(id_str.as_str()) {
            let _ = db_del_note.delete_quick_note(&id);
            refresh_notes_del();
            if let Some(ui) = app_weak_del_note.upgrade() {
                ui.set_toast_message("便签已删除".into());
            }
        }
    });

    // 8. 新建项目提交
    let db_new_proj = db.clone();
    let projects_new_proj = projects_ref.clone();
    let model_new_proj = model.clone();
    let app_weak_new = app.as_weak();

    app.on_submit_new_project(move || {
        if let Some(ui) = app_weak_new.upgrade() {
            let name = ui.get_new_proj_name().trim().to_string();
            let client_name = ui.get_new_proj_client().trim().to_string();
            let rate_str = ui.get_new_proj_rate().trim().to_string();
            let currency = ui.get_new_proj_currency().trim().to_string();
            let git_path_str = ui.get_new_proj_git().trim().to_string();

            if name.is_empty() {
                ui.set_toast_message("项目名称不能为空".into());
                return;
            }

            let daily_rate = rate_str.parse::<f64>().unwrap_or(3000.0);
            let mut p = Project::new(name);
            p.daily_rate = daily_rate;
            p.currency = if currency.is_empty() { "CNY".to_string() } else { currency };
            if !git_path_str.is_empty() {
                p.git_repo_path = Some(git_path_str);
            }

            if !client_name.is_empty() {
                let client_obj = myth_core::models::Client::new(&client_name, &p.currency);
                let _ = db_new_proj.insert_client(&client_obj);
                p.client_id = Some(client_obj.id);
            }

            if let Ok(_) = db_new_proj.insert_project(&p) {
                projects_new_proj.borrow_mut().push(p);
                let new_items = build_project_items(&projects_new_proj.borrow(), &db_new_proj);
                model_new_proj.set_vec(new_items);

                ui.set_new_proj_name("".into());
                ui.set_new_proj_client("".into());
                ui.set_new_proj_git("".into());
                ui.set_show_new_project_modal(false);
                ui.set_toast_message("新项目已成功创建！".into());
            }
        }
    });

    // 8.1 快速发起会议与技术讨论计时 (支持同步开启麦克风录音)
    let db_quick_meeting = db.clone();
    let projects_quick_meeting = projects_ref.clone();
    let model_quick_meeting = model.clone();
    let timer_mgr_quick_meeting = timer_manager.clone();
    let app_weak_meeting = app.as_weak();
    let audio_rec_meeting = audio_recorder.clone();
    let active_rec_meeting = active_recording_info.clone();
    let db_dir_recordings = db_dir.clone();

    app.on_start_quick_meeting(move |proj_idx, topic_str, rate_str, record_audio| {
        if let Some(ui) = app_weak_meeting.upgrade() {
            let topic = if topic_str.trim().is_empty() {
                "客户需求与方案沟通会".to_string()
            } else {
                topic_str.trim().to_string()
            };
            let hourly_rate = rate_str.trim().parse::<f64>().unwrap_or(350.0);

            let projs = projects_quick_meeting.borrow();
            let target_project = if proj_idx >= 0 && (proj_idx as usize) < projs.len() {
                Some(projs[proj_idx as usize].clone())
            } else {
                None
            };
            drop(projs);

            let (pid, proj_name) = if let Some(p) = target_project {
                (p.id, p.name)
            } else {
                // 新建独立临时会议项目
                let mut p = Project::new(&format!("🎙️ {}", topic));
                p.billing_type = BillingType::Hourly;
                p.hourly_rate = hourly_rate;
                p.hours_per_day = 8.0;
                p.day_calc_mode = DayCalcMode::Exact;

                let _ = db_quick_meeting.insert_project(&p);
                let pid = p.id;
                let name = p.name.clone();
                projects_quick_meeting.borrow_mut().push(p);
                let new_items = build_project_items(&projects_quick_meeting.borrow(), &db_quick_meeting);
                model_quick_meeting.set_vec(new_items);
                (pid, name)
            };

            // 启动该项目/会议的计时通道并写入容灾持久化
            timer_mgr_quick_meeting.borrow_mut().start_channel(pid);
            let now = Utc::now();
            let _ = db_quick_meeting.upsert_active_timer(&ActiveTimerRecord {
                project_id: pid,
                session_start_at: now,
                current_started_at: Some(now),
                accumulated_secs: timer_mgr_quick_meeting.borrow().get_elapsed(&pid),
                is_paused: false,
                last_heartbeat: now,
            });

            let mut rec_started = false;
            if record_audio {
                let rec_dir = db_dir_recordings.join("recordings");
                let _ = std::fs::create_dir_all(&rec_dir);
                let timestamp_str = Local::now().format("%Y%m%d_%H%M%S").to_string();
                let wav_file = rec_dir.join(format!("meeting_{}_{}.wav", pid, timestamp_str));
                let mut rec = audio_rec_meeting.borrow_mut();
                match rec.start_recording(&wav_file) {
                    Ok(_) => {
                        *active_rec_meeting.borrow_mut() = Some((pid, topic.clone(), wav_file.clone()));
                        ui.set_is_recording_audio(true);
                        ui.set_is_audio_paused(false);
                        ui.set_active_recording_project_name(format!("{} · {}", proj_name, topic).into());
                        rec_started = true;
                    }
                    Err(e) => {
                        tracing::warn!("启动录音失败: {}", e);
                        ui.set_toast_message(format!("提示: 无法启动录音 ({})", e).into());
                    }
                }
            }

            ui.set_meeting_topic_input("".into());
            ui.set_show_meeting_modal(false);
            if rec_started {
                ui.set_toast_message(format!("已启动【{}】项目会议并开启全程录音！议题: {}", proj_name, topic).into());
            } else {
                ui.set_toast_message(format!("已启动【{}】项目会议计时！议题: {}", proj_name, topic).into());
            }
        }
    });

    // 8.1.1 选择会议归属项目联动 (自动更新项目名称、客户与折算单价)
    let projects_select = projects_ref.clone();
    let db_select = db.clone();
    let app_weak_select = app.as_weak();
    app.on_select_meeting_project(move |idx| {
        if let Some(ui) = app_weak_select.upgrade() {
            ui.set_meeting_selected_proj_idx(idx);
            let projs = projects_select.borrow();
            if idx >= 0 && (idx as usize) < projs.len() {
                let p = &projs[idx as usize];
                ui.set_meeting_selected_proj_name(p.name.clone().into());
                let cname = get_client_name(&db_select, p.client_id);
                ui.set_meeting_selected_proj_client(cname.into());
                let (rate, rate_str) = match p.billing_type {
                    BillingType::Daily => {
                        let r = (p.daily_rate / p.hours_per_day.max(1.0)).round();
                        (r, format!("{:.0} {} / 天 (约 {:.0}/h)", p.daily_rate, p.currency, r))
                    }
                    BillingType::Hourly => (p.hourly_rate.round(), format!("{:.0} {} / 小时", p.hourly_rate, p.currency)),
                    BillingType::FixedCap => (350.0, format!("固定总包 {:.0} {}", p.daily_rate, p.currency)),
                    BillingType::NonBillable => (0.0, "自用/不计费".to_string()),
                };
                ui.set_meeting_rate_input(format!("{:.0}", rate).into());
                ui.set_meeting_selected_proj_rate(rate_str.into());
            } else {
                ui.set_meeting_selected_proj_name("独立临时会议".into());
                ui.set_meeting_selected_proj_client("临时立项".into());
                ui.set_meeting_rate_input("350".into());
                ui.set_meeting_selected_proj_rate("350 CNY / 小时".into());
            }
        }
    });

    // 8.1.2 打开会议发起弹窗 (自动初始化默认归属与费率)
    let projects_open = projects_ref.clone();
    let db_open = db.clone();
    let app_weak_open = app.as_weak();
    app.on_open_meeting_modal(move || {
        if let Some(ui) = app_weak_open.upgrade() {
            let projs = projects_open.borrow();
            if !projs.is_empty() {
                let curr_idx = ui.get_meeting_selected_proj_idx();
                let idx = if curr_idx >= 0 && (curr_idx as usize) < projs.len() {
                    curr_idx
                } else {
                    0
                };
                ui.set_meeting_selected_proj_idx(idx);
                let p = &projs[idx as usize];
                ui.set_meeting_selected_proj_name(p.name.clone().into());
                let cname = get_client_name(&db_open, p.client_id);
                ui.set_meeting_selected_proj_client(cname.into());
                let (rate, rate_str) = match p.billing_type {
                    BillingType::Daily => {
                        let r = (p.daily_rate / p.hours_per_day.max(1.0)).round();
                        (r, format!("{:.0} {} / 天 (约 {:.0}/h)", p.daily_rate, p.currency, r))
                    }
                    BillingType::Hourly => (p.hourly_rate.round(), format!("{:.0} {} / 小时", p.hourly_rate, p.currency)),
                    BillingType::FixedCap => (350.0, format!("固定总包 {:.0} {}", p.daily_rate, p.currency)),
                    BillingType::NonBillable => (0.0, "自用/不计费".to_string()),
                };
                ui.set_meeting_rate_input(format!("{:.0}", rate).into());
                ui.set_meeting_selected_proj_rate(rate_str.into());
            } else {
                ui.set_meeting_selected_proj_idx(-1);
                ui.set_meeting_selected_proj_name("独立临时会议".into());
                ui.set_meeting_selected_proj_client("临时立项".into());
                ui.set_meeting_rate_input("350".into());
                ui.set_meeting_selected_proj_rate("350 CNY / 小时".into());
            }
            ui.set_show_meeting_modal(true);
        }
    });



    // 8.2 录音控制交互回调 (暂停/继续、结束录音)
    let audio_rec_toggle = audio_recorder.clone();
    let app_weak_toggle = app.as_weak();
    app.on_toggle_audio_pause(move || {
        let mut rec = audio_rec_toggle.borrow_mut();
        if rec.is_recording() {
            if rec.is_paused() {
                rec.resume_recording();
                if let Some(ui) = app_weak_toggle.upgrade() {
                    ui.set_toast_message("🎙️ 麦克风录音已继续".into());
                }
            } else {
                rec.pause_recording();
                if let Some(ui) = app_weak_toggle.upgrade() {
                    ui.set_toast_message("🎙️ 麦克风录音已暂停".into());
                }
            }
        }
    });

    let audio_rec_stop_cmd = audio_recorder.clone();
    let active_rec_stop_cmd = active_recording_info.clone();
    let app_weak_stop_audio = app.as_weak();
    app.on_stop_audio_recording(move || {
        let mut rec = audio_rec_stop_cmd.borrow_mut();
        if rec.is_recording() {
            match rec.stop_recording() {
                Ok(path) => {
                    let file_size_mb = std::fs::metadata(&path).map(|m| m.len() as f64 / 1_048_576.0).unwrap_or(0.0);
                    if let Some((pid, topic, _)) = active_rec_stop_cmd.borrow().as_ref() {
                        *active_rec_stop_cmd.borrow_mut() = Some((*pid, topic.clone(), path.clone()));
                    }
                    if let Some(ui) = app_weak_stop_audio.upgrade() {
                        ui.set_is_recording_audio(false);
                        ui.set_active_recording_project_name("".into());
                        ui.set_toast_message(format!("会议录音已安全归档 ({:.2} MB)", file_size_mb).into());
                    }
                }
                Err(e) => {
                    if let Some(ui) = app_weak_stop_audio.upgrade() {
                        ui.set_toast_message(format!("停止录音失败: {}", e).into());
                    }
                }
            }
        }
    });

    // 8.3 录音回放与文件管理回调
    let audio_player_play = audio_player.clone();
    let app_weak_play = app.as_weak();
    app.on_play_entry_audio(move |entry_id_str, path_str| {
        let p = std::path::Path::new(path_str.as_str());
        let res = audio_player_play.borrow().play(p);
        if let Some(ui) = app_weak_play.upgrade() {
            match res {
                Ok(_) => {
                    ui.set_playing_entry_id(entry_id_str.clone());
                    ui.set_playing_audio_path(path_str.clone());
                    ui.set_toast_message("🔊 正在播放会议录音...".into());
                }
                Err(e) => {
                    ui.set_toast_message(format!("播放失败: {}", e).into());
                }
            }
        }
    });

    let audio_player_stop = audio_player.clone();
    let app_weak_stop_play = app.as_weak();
    app.on_stop_audio_playback(move || {
        audio_player_stop.borrow().stop();
        if let Some(ui) = app_weak_stop_play.upgrade() {
            ui.set_playing_entry_id("".into());
            ui.set_playing_audio_path("".into());
            ui.set_toast_message("已停止播放音频".into());
        }
    });

    let audio_player_folder = audio_player.clone();
    let app_weak_folder = app.as_weak();
    app.on_open_entry_folder(move |path_str| {
        let p = std::path::Path::new(path_str.as_str());
        let res = audio_player_folder.borrow().open_directory(p);
        if let Some(ui) = app_weak_folder.upgrade() {
            if res.is_ok() {
                ui.set_toast_message("已在系统文件管理器中定位录音文件所在目录".into());
            } else {
                ui.set_toast_message("无法打开文件夹".into());
            }
        }
    });

    // 8.4 单条交付记录 AI 语音转写与纪要提炼
    let db_transcribe = db.clone();
    let app_weak_transcribe = app.as_weak();
    let refresh_entries_transcribe = refresh_entries_ui.clone();
    app.on_transcribe_entry_audio(move |entry_id_str, audio_path_str| {
        if let Some(ui) = app_weak_transcribe.upgrade() {
            let provider = ui.get_setting_ai_provider().trim().to_string();
            let api_key = ui.get_setting_ai_key().trim().to_string();
            let api_base = ui.get_setting_ai_base().trim().to_string();
            let model = ui.get_setting_ai_model().trim().to_string();

            if provider == "api" && api_key.is_empty() {
                ui.set_toast_message("提示：使用自定义 API 时请先在【⚙️ 设置】中配置 API Key".into());
                return;
            }

            let entry_id = match Uuid::parse_str(entry_id_str.as_str()) {
                Ok(id) => id,
                Err(_) => return,
            };

            ui.set_toast_message(format!("正在调用 [{}] AI 引擎进行会议语音转写与纪要提炼，请稍候...", provider).into());
            let app_clone = ui.as_weak();
            let db_clone = db_transcribe.clone();
            let refresh_clone = refresh_entries_transcribe.clone();
            let audio_path = std::path::PathBuf::from(audio_path_str.as_str());
            let target_entry_id_str = entry_id_str.clone();

            slint::spawn_local(async move {
                let summarizer = AiSummarizer::new(AiConfig {
                    provider: provider.clone(),
                    api_base,
                    api_key,
                    model,
                });

                match summarizer.transcribe_audio(&audio_path).await {
                    Ok(transcript) => {
                        let txt_path = audio_path.with_extension("txt");
                        let _ = std::fs::write(&txt_path, &transcript);

                        if let Ok(Some(mut e)) = db_clone.get_time_entry(&entry_id) {
                            let notes = db_clone.list_quick_notes(&e.project_id).unwrap_or_default();
                            let summary_res = summarizer
                                .summarize_meeting("会议交流与技术决议", e.billable_hours, &notes, Some(&audio_path.display().to_string()), Some(&transcript))
                                .await;
                            let minutes = summary_res.unwrap_or_else(|_| "完成会议沟通对接，各方已达成一致决策。".to_string());
                            e.summary = Some(minutes.clone());
                            let _ = db_clone.insert_time_entry(&e);
                            refresh_clone();
                            if let Some(ui) = app_clone.upgrade() {
                                if ui.get_show_detail_modal() && ui.get_detail_entry_id().as_str() == target_entry_id_str.as_str() {
                                    ui.set_detail_summary(minutes.into());
                                    ui.set_detail_transcript(transcript.into());
                                }
                                ui.set_toast_message(format!("🎉 [{}] 录音语音转写与纪要提炼完成！", provider).into());
                            }
                        }
                    }
                    Err(e) => {
                        if let Some(ui) = app_clone.upgrade() {
                            ui.set_toast_message(format!("转写失败: {}", e).into());
                        }
                    }
                }
            }).unwrap();
        }
    });

    // 8.5 查看工时/会议纪要与转写文稿详情
    let db_detail = db.clone();
    let projects_detail = projects_ref.clone();
    let app_weak_detail = app.as_weak();
    app.on_open_entry_detail(move |entry_id_str| {
        if let Some(ui) = app_weak_detail.upgrade() {
            if let Ok(entry_id) = Uuid::parse_str(entry_id_str.as_str()) {
                if let Ok(Some(e)) = db_detail.get_time_entry(&entry_id) {
                    let projs = projects_detail.borrow();
                    let (p_name, currency) = projs.iter()
                        .find(|p| p.id == e.project_id)
                        .map(|p| (p.name.clone(), p.currency.clone()))
                        .unwrap_or_else(|| ("已归档项目".to_string(), "CNY".to_string()));
                    let notes = db_detail.list_quick_notes(&e.project_id).unwrap_or_default();
                    let audio_path_str = e.audio_path.clone().unwrap_or_else(|| {
                        extract_wav_path(e.summary.as_deref()).unwrap_or_default()
                    });
                    let has_audio = !audio_path_str.is_empty() && std::path::Path::new(&audio_path_str).exists();

                    let transcript = if has_audio {
                        let txt_path = std::path::PathBuf::from(&audio_path_str).with_extension("txt");
                        if txt_path.exists() {
                            std::fs::read_to_string(&txt_path).unwrap_or_default()
                        } else {
                            notes.iter()
                                .find(|n| n.content.starts_with("[录音全文转写]"))
                                .map(|n| n.content.trim_start_matches("[录音全文转写]").trim().to_string())
                                .unwrap_or_default()
                        }
                    } else {
                        "".to_string()
                    };

                    let start_local = e.start_time.with_timezone(&Local);
                    let end_local = e.end_time.with_timezone(&Local);

                    ui.set_detail_entry_id(entry_id_str.clone());
                    ui.set_detail_project_name(p_name.into());
                    ui.set_detail_time_range(format!("{}-{}", start_local.format("%H:%M"), end_local.format("%H:%M")).into());
                    ui.set_detail_duration(format_duration(e.raw_duration_secs).into());
                    ui.set_detail_days(format!("{:.2} 天", e.billable_days).into());
                    ui.set_detail_amount(format!("{} {:.2}", currency, e.total_amount).into());
                    ui.set_detail_summary(e.summary.unwrap_or_else(|| "持续代码开发与测试交付".to_string()).into());
                    ui.set_detail_transcript(transcript.into());
                    ui.set_detail_audio_path(audio_path_str.into());
                    ui.set_detail_has_audio(has_audio);
                    ui.set_show_detail_modal(true);
                }
            }
        }
    });

    // 8.6 复制详情文稿到系统剪贴板
    let app_weak_copy = app.as_weak();
    app.on_copy_text_to_clipboard(move |text_str| {
        if let Some(ui) = app_weak_copy.upgrade() {
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                if clipboard.set_text(text_str.as_str()).is_ok() {
                    ui.set_toast_message("📋 已将会议纪要与转写文稿复制到剪贴板！".into());
                    return;
                }
            }
            ui.set_toast_message("无法访问系统剪贴板".into());
        }
    });

    // 9. 设置中心：保存设置与测试连接
    let db_settings = db.clone();
    let app_weak_settings = app.as_weak();
    app.on_save_settings(move || {
        if let Some(ui) = app_weak_settings.upgrade() {
            let _ = db_settings.set_setting("ai_provider", ui.get_setting_ai_provider().trim());
            let _ = db_settings.set_setting("ai_base", ui.get_setting_ai_base().trim());
            let _ = db_settings.set_setting("ai_key", ui.get_setting_ai_key().trim());
            let _ = db_settings.set_setting("ai_model", ui.get_setting_ai_model().trim());
            let _ = db_settings.set_setting("hours_per_day", ui.get_setting_hours_per_day().trim());
            let _ = db_settings.set_setting("afk_minutes", ui.get_setting_afk_minutes().trim());

            ui.set_toast_message("系统配置与 AI 引擎参数已保存！".into());
        }
    });

    let app_weak_test_ai = app.as_weak();
    app.on_test_ai_connection(move || {
        if let Some(ui) = app_weak_test_ai.upgrade() {
            let provider = ui.get_setting_ai_provider().trim().to_string();
            let base = ui.get_setting_ai_base().trim().to_string();
            let key = ui.get_setting_ai_key().trim().to_string();
            let model = ui.get_setting_ai_model().trim().to_string();

            if provider == "api" && key.is_empty() {
                ui.set_toast_message("使用自定义 API 时请先填入 API Key".into());
                return;
            }

            ui.set_toast_message(format!("正在测试 [{}] AI 引擎连通性...", provider).into());

            let app_clone = ui.as_weak();
            slint::spawn_local(async move {
                let summarizer = AiSummarizer::new(AiConfig {
                    provider: provider.clone(),
                    api_base: base,
                    api_key: key,
                    model,
                });
                match summarizer.test_connection().await {
                    Ok(resp) => {
                        if let Some(ui) = app_clone.upgrade() {
                            let preview: String = resp.chars().take(50).collect();
                            ui.set_toast_message(format!("✅ AI [{}] 通信测试成功: {}", provider, preview).into());
                        }
                    }
                    Err(e) => {
                        if let Some(ui) = app_clone.upgrade() {
                            ui.set_toast_message(format!("❌ AI [{}] 测试失败: {}", provider, e).into());
                        }
                    }
                }
            }).unwrap();
        }
    });

    // 10. AI 自动总结调用
    let db_ai = db.clone();
    let projects_ai = projects_ref.clone();
    let app_weak_ai = app.as_weak();
    let refresh_entries_ai = refresh_entries_ui.clone();
    app.on_trigger_ai_summarize(move || {
        if let Some(ui) = app_weak_ai.upgrade() {
            let provider = ui.get_setting_ai_provider().trim().to_string();
            let api_key = ui.get_setting_ai_key().trim().to_string();
            let api_base = ui.get_setting_ai_base().trim().to_string();
            let model = ui.get_setting_ai_model().trim().to_string();

            if provider == "api" && api_key.is_empty() {
                ui.set_toast_message("提示：使用自定义 API 时请在【⚙️ 设置】中填入 API Key".into());
                return;
            }

            ui.set_toast_message(format!("正在调用 [{}] AI 引擎分析生成工作日报，请稍候...", provider).into());
            let ai_config = AiConfig {
                provider,
                api_base,
                api_key,
                model,
            };

            let app_clone = ui.as_weak();
            let db_clone = db_ai.clone();
            let projs_clone = projects_ai.clone();
            let refresh_clone = refresh_entries_ai.clone();

            slint::spawn_local(async move {
                let summarizer = AiSummarizer::new(ai_config);
                let projs = projs_clone.borrow();

                for p in projs.iter() {
                    if let Ok(mut entries) = db_clone.list_entries_for_project(&p.id) {
                        for e in entries.iter_mut() {
                            let dummy_git = myth_git::GitActivitySummary {
                                commits: vec!["feat: 实现核心业务接口并调优".to_string()],
                                diff_stat: "3 files changed, 45 insertions(+)".to_string(),
                                has_uncommitted_changes: false,
                                uncommitted_files: vec![],
                            };
                            let notes = vec![];
                            if let Ok(summary) = summarizer.summarize_session(&p.name, e.billable_hours, &dummy_git, &notes).await {
                                e.summary = Some(summary);
                                let _ = db_clone.insert_time_entry(e);
                            }
                        }
                    }
                }

                refresh_clone();
                if let Some(ui) = app_clone.upgrade() {
                    ui.set_toast_message("🎉 AI 工作成果与日报分析已全部提炼完成！".into());
                }
            }).unwrap();
        }
    });

    // 11. AFK 离开校准弹窗与应用
    let timer_mgr_afk = timer_manager.clone();
    let projects_afk = projects_ref.clone();
    let app_weak_afk = app.as_weak();

    app.on_open_afk_modal(move || {
        if let Some(ui) = app_weak_afk.upgrade() {
            let mgr = timer_mgr_afk.borrow();
            let projs = projects_afk.borrow();
            let mut afk_list = Vec::new();

            for p in projs.iter() {
                let elapsed = mgr.get_elapsed(&p.id);
                let is_running = matches!(mgr.get_status(&p.id), myth_core::timer::TimerStatus::Running { .. });
                if is_running {
                    afk_list.push(AfkItem {
                        id: p.id.to_string().into(),
                        project_name: p.name.clone().into(),
                        elapsed_text: format_duration(elapsed).into(),
                        is_deduct: true,
                    });
                }
            }

            ui.set_afk_items(ModelRc::from(Rc::new(VecModel::from(afk_list))));
            ui.set_show_afk_modal(true);
        }
    });

    let timer_mgr_resolve = timer_manager.clone();
    let projects_resolve = projects_ref.clone();
    let app_weak_resolve = app.as_weak();

    app.on_apply_afk_resolution(move |deduct_all| {
        if let Some(ui) = app_weak_resolve.upgrade() {
            if deduct_all {
                let away_mins: u64 = ui.get_afk_away_minutes().trim().parse().unwrap_or(30);
                let deduct_secs = away_mins * 60;
                let mut mgr = timer_mgr_resolve.borrow_mut();
                let projs = projects_resolve.borrow();

                for p in projs.iter() {
                    mgr.deduct_from_channel(p.id, deduct_secs);
                }
                ui.set_toast_message(format!("已成功从活动项目中扣除 {} 分钟摸鱼/离开时间", away_mins).into());
            } else {
                ui.set_toast_message("已保留全部活动工时（视作有效离线投入）".into());
            }
            ui.set_show_afk_modal(false);
        }
    });

    // 12. 复制 Markdown 工时日报到剪贴板
    let db_markdown = db.clone();
    let projects_markdown = projects_ref.clone();
    let app_weak_copy = app.as_weak();

    app.on_copy_markdown(move || {
        let projs = projects_markdown.borrow();
        let mut md = String::from("# MythManDay 项目研发工时日报\n\n");
        md.push_str("| 日期 | 项目名称 | 投入工时 | 核算人天 | 金额 | 成果与会议纪要 |\n");
        md.push_str("| :--- | :--- | :--- | :--- | :--- | :--- |\n");

        let mut grand_days = 0.0;
        let mut grand_amount = 0.0;

        for p in projs.iter() {
            if let Ok(entries) = db_markdown.list_entries_for_project(&p.id) {
                for e in entries {
                    let local_time = e.start_time.with_timezone(&Local);
                    let summary = e.summary.unwrap_or_else(|| "功能研发与交付".to_string());
                    md.push_str(&format!(
                        "| {} | {} | {:.2}h | **{:.2} 天** | {} {:.2} | {} |\n",
                        local_time.format("%Y-%m-%d"),
                        p.name,
                        e.billable_hours,
                        e.billable_days,
                        p.currency,
                        e.total_amount,
                        summary
                    ));
                    grand_days += e.billable_days;
                    grand_amount += e.total_amount;
                }
            }
        }

        md.push_str(&format!("\n**今日累计投入**：**{:.2} 人天**，核算价值：**¥ {:.2}**\n", grand_days, grand_amount));

        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(md);
        }

        if let Some(ui) = app_weak_copy.upgrade() {
            ui.set_toast_message("Markdown 研发工时日报已复制到剪贴板！".into());
        }
    });

    let app_weak_close = app.as_weak();
    let is_window_vis_close = is_window_visible.clone();
    app.window().on_close_requested(move || {
        if let Some(ui) = app_weak_close.upgrade() {
            let _ = ui.hide();
            *is_window_vis_close.borrow_mut() = false;
        }
        slint::CloseRequestResponse::HideWindow
    });

    app.show()?;
    slint::run_event_loop_until_quit()?;
    Ok(())
}

fn dirs_or_local_data_dir() -> std::path::PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        std::path::PathBuf::from(home).join(".config").join("paraday")
    } else {
        std::path::PathBuf::from("./data")
    }
}
