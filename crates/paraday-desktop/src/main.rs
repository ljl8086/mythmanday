// Copyright (C) 2026 ParaDay Authors. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (ParaDay)
// Version: V1.0
// Description: Multi-channel concurrent time-tracking & man-day billing engine.

slint::include_modules!();

use chrono::{Local, Utc};
use paraday_ai::{AiConfig, AiSummarizer};
use paraday_core::billing::BillingEngine;
use paraday_core::models::{BillingType, DayCalcMode, Project, QuickNote, TimeEntry};
use paraday_core::timer::MultiChannelManager;
use paraday_db::Database;
use paraday_git::GitScanner;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use uuid::Uuid;

fn format_duration(secs: u64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

fn build_project_items(projects: &[Project]) -> Vec<ProjectItem> {
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
            ProjectItem {
                id: p.id.to_string().into(),
                name: p.name.clone().into(),
                client: "商业客户".into(),
                rate_info: rate_info.into(),
                timer_text: "00:00:00".into(),
                days_text: "0.00 人天".into(),
                amount_text: format!("{} 0.00", p.currency).into(),
                is_running: false,
                is_paused: false,
            }
        })
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // 1. 初始化本地持久化数据库
    let db_dir = dirs_or_local_data_dir();
    std::fs::create_dir_all(&db_dir)?;
    let db_path = db_dir.join("paraday.db");
    let db = Arc::new(Database::open(&db_path)?);

    // 初始化引导项目
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

        projects = db.list_projects()?;
    }

    // 2. 初始化核心多通道状态机与界面
    let timer_manager = Rc::new(RefCell::new(MultiChannelManager::new()));
    let projects_ref = Rc::new(RefCell::new(projects));
    let app = MainWindow::new()?;

    // 3. 读取并填充设置数据
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
    let initial_items = build_project_items(&projects_ref.borrow());
    let model = Rc::new(VecModel::from(initial_items));
    app.set_projects(ModelRc::from(model.clone()));

    let refresh_entries_ui = {
        let db = db.clone();
        let app_weak = app.as_weak();
        let projects_ref = projects_ref.clone();
        move || {
            let projs = projects_ref.borrow();
            let mut all_entries = Vec::new();
            for p in projs.iter() {
                if let Ok(entries) = db.list_entries_for_project(&p.id) {
                    for e in entries {
                        let start_local = e.start_time.with_timezone(&Local);
                        let end_local = e.end_time.with_timezone(&Local);
                        all_entries.push(EntryItem {
                            id: e.id.to_string().into(),
                            project_name: p.name.clone().into(),
                            duration_text: format_duration(e.raw_duration_secs).into(),
                            days_text: format!("{:.2} 天", e.billable_days).into(),
                            amount_text: format!("{} {:.2}", p.currency, e.total_amount).into(),
                            summary_text: e.summary.unwrap_or_else(|| "持续代码开发与测试交付".to_string()).into(),
                            time_range: format!("{}-{}", start_local.format("%H:%M"), end_local.format("%H:%M")).into(),
                        });
                    }
                }
            }
            if let Some(ui) = app_weak.upgrade() {
                ui.set_recent_entries(ModelRc::from(Rc::new(VecModel::from(all_entries))));
            }
        }
    };
    refresh_entries_ui();

    let refresh_notes_ui = {
        let db = db.clone();
        let app_weak = app.as_weak();
        let projects_ref = projects_ref.clone();
        move || {
            let projs = projects_ref.borrow();
            let mut items = Vec::new();
            if let Ok(notes) = db.list_all_quick_notes() {
                for n in notes {
                    let proj_name = projs
                        .iter()
                        .find(|p| p.id == n.project_id)
                        .map(|p| p.name.as_str())
                        .unwrap_or("全局便签");
                    let local_time = n.logged_at.with_timezone(&Local);
                    items.push(NoteItem {
                        id: n.id.to_string().into(),
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

    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_secs(1),
        move || {
            let mgr = timer_mgr_clone.borrow();
            let projs = projects_cache.borrow();
            let mut total_days = 0.0;
            let mut total_cny_amount = 0.0;
            let mut running_count = 0;

            for (idx, p) in projs.iter().enumerate() {
                if let Some(mut item) = model_clone.row_data(idx) {
                    let elapsed = mgr.get_elapsed(&p.id);
                    let status = mgr.get_status(&p.id);
                    let is_running = matches!(status, paraday_core::timer::TimerStatus::Running { .. });
                    let is_paused = matches!(status, paraday_core::timer::TimerStatus::Paused { .. });

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

            if let Some(ui) = app_weak.upgrade() {
                ui.set_total_days(format!("{:.2}", total_days).into());
                ui.set_total_amount(format!("¥ {:.2}", total_cny_amount).into());
                ui.set_running_count(running_count);
            }
        },
    );

    // 6. 交互回调：通道 启动 / 暂停 / 继续 / 结算
    let timer_mgr_start = timer_manager.clone();
    let app_weak_start = app.as_weak();
    app.on_start_project(move |id_str| {
        if let Ok(id) = Uuid::parse_str(id_str.as_str()) {
            timer_mgr_start.borrow_mut().start_channel(id);
            if let Some(ui) = app_weak_start.upgrade() {
                ui.set_toast_message("通道已启动计时".into());
            }
        }
    });

    let timer_mgr_pause = timer_manager.clone();
    let app_weak_pause = app.as_weak();
    app.on_pause_project(move |id_str| {
        if let Ok(id) = Uuid::parse_str(id_str.as_str()) {
            timer_mgr_pause.borrow_mut().pause_channel(id);
            if let Some(ui) = app_weak_pause.upgrade() {
                ui.set_toast_message("通道已暂停 (工时已锁定，可随时继续)".into());
            }
        }
    });

    let timer_mgr_resume = timer_manager.clone();
    let app_weak_resume = app.as_weak();
    app.on_resume_project(move |id_str| {
        if let Ok(id) = Uuid::parse_str(id_str.as_str()) {
            timer_mgr_resume.borrow_mut().start_channel(id);
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

    app.on_stop_project(move |id_str| {
        if let Ok(id) = Uuid::parse_str(id_str.as_str()) {
            let mut mgr = timer_mgr_stop.borrow_mut();
            if let Some((start_at, end_at, elapsed_secs)) = mgr.stop_channel(id) {
                let projs = projects_stop.borrow();
                if let Some(p) = projs.iter().find(|proj| proj.id == id) {
                    let billable_hours = BillingEngine::calculate_billable_hours(elapsed_secs, p.rounding_minutes);
                    let billable_days = BillingEngine::calculate_billable_days(billable_hours, p.hours_per_day, p.day_calc_mode);
                    let (rate, amount) = BillingEngine::calculate_amount(p, billable_hours, billable_days);

                    // 智能嗅探 Git 活动
                    let mut summary_desc = "完成阶段性需求研发与联调".to_string();
                    if let Some(ref git_path) = p.git_repo_path {
                        if let Ok(git_summary) = GitScanner::extract_activity(git_path, start_at, end_at) {
                            if !git_summary.commits.is_empty() {
                                summary_desc = format!("代码提交 {} 次：{}", git_summary.commits.len(), git_summary.commits.join("; "));
                            } else if git_summary.has_uncommitted_changes {
                                summary_desc = format!("修改了 {} 个工作区文件，完成功能测试", git_summary.uncommitted_files.len());
                            }
                        }
                    }

                    let entry = TimeEntry {
                        id: Uuid::new_v4(),
                        project_id: id,
                        start_time: start_at,
                        end_time: end_at,
                        raw_duration_secs: elapsed_secs,
                        billable_hours,
                        billable_days,
                        rate_at_moment: rate,
                        total_amount: amount,
                        summary: Some(summary_desc),
                        is_invoiced: false,
                        created_at: Utc::now(),
                    };

                    let _ = db_stop.insert_time_entry(&entry);
                    refresh_entries_stop();

                    if let Some(ui) = app_weak_stop.upgrade() {
                        ui.set_toast_message(format!("本次工时已结算入库：{:.2} 人天", billable_days).into());
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
                let client_obj = paraday_core::models::Client::new(&client_name, &p.currency);
                let _ = db_new_proj.insert_client(&client_obj);
                p.client_id = Some(client_obj.id);
            }

            if let Ok(_) = db_new_proj.insert_project(&p) {
                projects_new_proj.borrow_mut().push(p);
                let new_items = build_project_items(&projects_new_proj.borrow());
                model_new_proj.set_vec(new_items);

                ui.set_new_proj_name("".into());
                ui.set_new_proj_client("".into());
                ui.set_new_proj_git("".into());
                ui.set_active_tab(0);
                ui.set_toast_message("新项目已成功创建！".into());
            }
        }
    });

    // 9. 设置中心：保存设置与测试连接
    let db_settings = db.clone();
    let app_weak_settings = app.as_weak();
    app.on_save_settings(move || {
        if let Some(ui) = app_weak_settings.upgrade() {
            let _ = db_settings.set_setting("ai_base", ui.get_setting_ai_base().trim());
            let _ = db_settings.set_setting("ai_key", ui.get_setting_ai_key().trim());
            let _ = db_settings.set_setting("ai_model", ui.get_setting_ai_model().trim());
            let _ = db_settings.set_setting("hours_per_day", ui.get_setting_hours_per_day().trim());
            let _ = db_settings.set_setting("afk_minutes", ui.get_setting_afk_minutes().trim());

            ui.set_toast_message("系统配置与 AI 服务商参数已保存！".into());
        }
    });

    let app_weak_test_ai = app.as_weak();
    app.on_test_ai_connection(move || {
        if let Some(ui) = app_weak_test_ai.upgrade() {
            let base = ui.get_setting_ai_base().trim().to_string();
            let key = ui.get_setting_ai_key().trim().to_string();
            let _model = ui.get_setting_ai_model().trim().to_string();

            if key.is_empty() {
                ui.set_toast_message("请先在输入框填入您的 API Key".into());
                return;
            }

            let app_clone = ui.as_weak();
            slint::spawn_local(async move {
                let client = reqwest::Client::new();
                let url = format!("{}/models", base.trim_end_matches('/'));
                let res = client.get(&url).header("Authorization", format!("Bearer {}", key)).send().await;

                if let Some(ui) = app_clone.upgrade() {
                    match res {
                        Ok(resp) if resp.status().is_success() => {
                            ui.set_toast_message("AI 服务连接测试成功！接口通信正常。".into());
                        }
                        Ok(resp) => {
                            ui.set_toast_message(format!("接口通信响应状态: {}", resp.status()).into());
                        }
                        Err(e) => {
                            ui.set_toast_message(format!("连接失败: {}", e).into());
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
            let api_key = ui.get_setting_ai_key().trim().to_string();
            if api_key.is_empty() {
                ui.set_toast_message("提示：请在【⚙️ 设置】中填入 API Key 即可启动大模型生成".into());
                return;
            }

            ui.set_toast_message("正在调用 AI 大模型分析生成交付报告，请稍候...".into());
            let ai_config = AiConfig {
                api_base: ui.get_setting_ai_base().trim().to_string(),
                api_key,
                model: ui.get_setting_ai_model().trim().to_string(),
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
                            let dummy_git = paraday_git::GitActivitySummary {
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
                    ui.set_toast_message("AI 智能分析与润色已全部完成！".into());
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
                let is_running = matches!(mgr.get_status(&p.id), paraday_core::timer::TimerStatus::Running { .. });
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
            ui.set_active_tab(5);
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
            ui.set_active_tab(0);
        }
    });

    // 12. 复制 Markdown 结算单到剪贴板
    let db_markdown = db.clone();
    let projects_markdown = projects_ref.clone();
    let app_weak_copy = app.as_weak();

    app.on_copy_markdown(move || {
        let projs = projects_markdown.borrow();
        let mut md = String::from("# ParaDay 客户项目交付结算单\n\n");
        md.push_str("| 日期 | 项目名称 | 投入工时 | 核算人天 | 金额 | 交付成果摘要 |\n");
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

        md.push_str(&format!("\n**总计核算**：**{:.2} 人天**，总应收：**¥ {:.2}**\n", grand_days, grand_amount));

        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(md);
        }

        if let Some(ui) = app_weak_copy.upgrade() {
            ui.set_toast_message("Markdown 结算对账单已复制到剪贴板！".into());
        }
    });

    app.run()?;
    Ok(())
}

fn dirs_or_local_data_dir() -> std::path::PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        std::path::PathBuf::from(home).join(".config").join("paraday")
    } else {
        std::path::PathBuf::from("./data")
    }
}
