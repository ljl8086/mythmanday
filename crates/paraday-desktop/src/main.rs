// Copyright (C) 2026 ParaDay Authors. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (ParaDay)
// Version: V1.0
// Description: Multi-channel concurrent time-tracking & man-day billing engine.

slint::include_modules!();

use paraday_core::billing::BillingEngine;
use paraday_core::models::{BillingType, DayCalcMode, Project, QuickNote};
use paraday_core::timer::MultiChannelManager;
use paraday_db::Database;
use slint::{Model, ModelRc, VecModel};
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // 1. 初始化数据库目录与连接
    let db_dir = dirs_or_local_data_dir();
    std::fs::create_dir_all(&db_dir)?;
    let db_path = db_dir.join("paraday.db");
    let db = Arc::new(Database::open(&db_path)?);

    // 若数据库为空，初始化引导项目
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

    // 2. 初始化核心多通道引擎
    let timer_manager = Rc::new(RefCell::new(MultiChannelManager::new()));
    let app = MainWindow::new()?;

    // 3. 构造界面数据模型
    let items: Vec<ProjectItem> = projects
        .iter()
        .map(|p| {
            let rate_info = match p.billing_type {
                BillingType::Daily => format!("{:.0} {} / 天 ({}h基准)", p.daily_rate, p.currency, p.hours_per_day),
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
            }
        })
        .collect();

    let model = Rc::new(VecModel::from(items));
    app.set_projects(ModelRc::from(model.clone()));

    // 4. 定时心跳刷新器 (1 秒钟触发一次)
    let app_weak = app.as_weak();
    let timer_mgr_clone = timer_manager.clone();
    let model_clone = model.clone();
    let projects_cache = projects.clone();

    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_secs(1),
        move || {
            let mgr = timer_mgr_clone.borrow();
            let mut total_days = 0.0;
            let mut total_cny_amount = 0.0;
            let mut running_count = 0;

            for (idx, p) in projects_cache.iter().enumerate() {
                if let Some(mut item) = model_clone.row_data(idx) {
                    let elapsed = mgr.get_elapsed(&p.id);
                    let is_running = mgr.get_status(&p.id) != paraday_core::timer::TimerStatus::Stopped;

                    if is_running {
                        running_count += 1;
                    }

                    item.is_running = is_running;
                    item.timer_text = format_duration(elapsed).into();

                    let billable_hours = BillingEngine::calculate_billable_hours(elapsed, p.rounding_minutes);
                    let billable_days = BillingEngine::calculate_billable_days(billable_hours, p.hours_per_day, p.day_calc_mode);
                    let (_, amount) = BillingEngine::calculate_amount(p, billable_hours, billable_days);

                    item.days_text = format!("{:.2} 人天", billable_days).into();
                    item.amount_text = format!("{} {:.2}", p.currency, amount).into();

                    total_days += billable_days;
                    if p.currency == "CNY" {
                        total_cny_amount += amount;
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

    // 5. 绑定交互回调
    let timer_mgr_toggle = timer_manager.clone();
    app.on_toggle_project(move |id_str| {
        if let Ok(id) = Uuid::parse_str(id_str.as_str()) {
            let mut mgr = timer_mgr_toggle.borrow_mut();
            if mgr.get_status(&id) == paraday_core::timer::TimerStatus::Stopped {
                mgr.start_channel(id);
            } else {
                mgr.stop_channel(id);
            }
        }
    });

    let db_for_notes = db.clone();
    app.on_add_quick_note(move |content| {
        let text = content.as_str().trim();
        if !text.is_empty() {
            let dummy_proj_id = Uuid::nil();
            let note = QuickNote::new(dummy_proj_id, text);
            let _ = db_for_notes.insert_quick_note(&note);
            println!("已登记闪念便签: {}", text);
        }
    });

    app.on_open_export(move || {
        println!("触发生成对账单与导出流程...");
    });

    app.on_open_add_project(move || {
        println!("触发添加新项目窗口...");
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
