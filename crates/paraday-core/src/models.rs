// Copyright (C) 2026 ParaDay Authors. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (ParaDay)
// Version: V1.0
// Description: Multi-channel concurrent time-tracking & man-day billing engine.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 计费模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillingType {
    /// 按人天计费 (Man-Day Billing)
    Daily,
    /// 按小时计费 (Hourly Billing)
    Hourly,
    /// 固定总包制 (Fixed Budget Cap)
    FixedCap,
    /// 不计费 (个人/自用/开源项目)
    NonBillable,
}

impl Default for BillingType {
    fn default() -> Self {
        Self::Daily
    }
}

/// 人天折算与取整策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DayCalcMode {
    /// 半天阶梯制（行业主流：<=4h 算 0.5 天，<=8h 算 1.0 天，>8h 算 1.0 + 超额比例）
    HalfStep,
    /// 严格浮点折算（比例制：实际工时 / 每日基准工时）
    Exact,
    /// 全天进位制（只要当天投入超过最小启动阈值，即算 1.0 天）
    FullDay,
}

impl Default for DayCalcMode {
    fn default() -> Self {
        Self::HalfStep
    }
}

/// 客户模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Client {
    pub id: Uuid,
    pub name: String,
    pub default_currency: String,
    pub created_at: DateTime<Utc>,
}

impl Client {
    pub fn new(name: impl Into<String>, default_currency: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            default_currency: default_currency.into(),
            created_at: Utc::now(),
        }
    }
}

/// 项目模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: Uuid,
    pub client_id: Option<Uuid>,
    pub name: String,
    /// 绑定的本地 Git 绝对路径（可选）
    pub git_repo_path: Option<String>,
    /// 计费模式（默认人天计费）
    pub billing_type: BillingType,
    /// 结算币种 (如 CNY, USD, EUR)
    pub currency: String,
    /// 单价：人天单价 (元/天)
    pub daily_rate: f64,
    /// 单价：时薪单价 (元/时，适用于 Hourly 模式)
    pub hourly_rate: f64,
    /// 每日基准工时（默认 8.0 小时 = 1 人天）
    pub hours_per_day: f64,
    /// 人天计算策略
    pub day_calc_mode: DayCalcMode,
    /// 最小计费分钟取整（如 1m, 15m, 30m）
    pub rounding_minutes: u32,
    /// 总预算工时上限（用于超期告警）
    pub budget_hours: f64,
    /// 是否激活
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

impl Project {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            client_id: None,
            name: name.into(),
            git_repo_path: None,
            billing_type: BillingType::Daily,
            currency: "CNY".to_string(),
            daily_rate: 0.0,
            hourly_rate: 0.0,
            hours_per_day: 8.0,
            day_calc_mode: DayCalcMode::HalfStep,
            rounding_minutes: 1,
            budget_hours: 0.0,
            is_active: true,
            created_at: Utc::now(),
        }
    }
}

/// 工时流水明细条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeEntry {
    pub id: Uuid,
    pub project_id: Uuid,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    /// 物理耗时（秒）
    pub raw_duration_secs: u64,
    /// 取整后的计费工时数（小时）
    pub billable_hours: f64,
    /// 换算后的计费人天数（人天，如 0.5, 1.0）
    pub billable_days: f64,
    /// 发生时的费率快照 (Snapshot)
    pub rate_at_moment: f64,
    /// 本次条目结算金额
    pub total_amount: f64,
    /// AI 自动提炼或手动备注的工作成果描述
    pub summary: Option<String>,
    /// 现场会议录音文件本地存储路径（若有录音凭证）
    pub audio_path: Option<String>,
    /// 是否已出账/已开具对账单
    pub is_invoiced: bool,
    pub created_at: DateTime<Utc>,
}

/// 闪念便签 / 零代码提交事件记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickNote {
    pub id: Uuid,
    pub project_id: Uuid,
    pub logged_at: DateTime<Utc>,
    pub content: String,
}

impl QuickNote {
    pub fn new(project_id: Uuid, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            project_id,
            logged_at: Utc::now(),
            content: content.into(),
        }
    }
}
