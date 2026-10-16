// Copyright (C) 2026 ljl8086 <ljl8086@gmail.com>. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (MythManDay)
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

/// 舍入方式策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoundingMode {
    /// 向上取整 / 进位 (例如：不足 0.5 按 0.5，超过按 1)
    Ceil,
    /// 四舍五入 (例如：标准 Math.round 步进)
    HalfUp,
    /// 舍去 / 向下截断 (例如：Math.floor 步进)
    Floor,
}

impl Default for RoundingMode {
    fn default() -> Self {
        Self::Ceil
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
    /// 计费模式（按天 Daily / 按小时 Hourly / 固定总包 FixedCap）
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
    /// 舍入方式（向上进位 Ceil / 四舍五入 HalfUp / 截断舍去 Floor）
    pub rounding_mode: RoundingMode,
    /// 最小核算刻度（如按天时 0.5 天或 1.0 天；按小时时 0.5 小时或 1.0 小时）
    pub min_granularity: f64,
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
            rounding_mode: RoundingMode::Ceil,
            min_granularity: 0.5,
            rounding_minutes: 1,
            budget_hours: 0.0,
            is_active: true,
            created_at: Utc::now(),
        }
    }
}

/// 工时流水明细条目 (按天聚合主记录，一日一条)
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
    /// AI 自动提炼或手动备注的工作成果描述（汇总研发交付与会议纪要）
    pub summary: Option<String>,
    /// 现场会议录音文件本地存储路径（历史兼容或单录音凭证）
    pub audio_path: Option<String>,
    /// 是否已出账/已开具对账单
    pub is_invoiced: bool,
    pub created_at: DateTime<Utc>,
}

/// 每日工时内的打卡时段切片（支持单日内多次启停、恢复计时）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkSpan {
    pub id: Uuid,
    pub entry_id: Uuid,
    pub span_order: u32,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
    pub duration_secs: u64,
    /// 类型: timer(工时打卡) / meeting(会议时段) / manual(手动补录)
    pub span_type: String,
    pub created_at: DateTime<Utc>,
}

impl WorkSpan {
    pub fn new(
        entry_id: Uuid,
        span_order: u32,
        start_at: DateTime<Utc>,
        end_at: DateTime<Utc>,
        duration_secs: u64,
        span_type: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            entry_id,
            span_order,
            start_at,
            end_at,
            duration_secs,
            span_type: span_type.into(),
            created_at: Utc::now(),
        }
    }
}

/// 结构化会议记录实体（工时记录的子项，一天可有多场会议）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meeting {
    pub id: Uuid,
    pub entry_id: Uuid,
    pub project_id: Uuid,
    pub topic: String,
    pub start_at: DateTime<Utc>,
    pub end_at: Option<DateTime<Utc>>,
    pub duration_secs: u64,
    /// 状态: in_progress / summarizing / completed
    pub status: String,
    /// 该场会议的结构化 AI 提炼纪要
    pub meeting_summary: Option<String>,
    /// 会议便签速记
    pub quick_notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl Meeting {
    pub fn new(entry_id: Uuid, project_id: Uuid, topic: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            entry_id,
            project_id,
            topic: topic.into(),
            start_at: now,
            end_at: None,
            duration_secs: 0,
            status: "in_progress".to_string(),
            meeting_summary: None,
            quick_notes: None,
            created_at: now,
        }
    }
}

/// 会议附属录音片段实体（1 场会议可有多段录音，只记录音频和转写原始文字）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingAudioClip {
    pub id: Uuid,
    pub meeting_id: Uuid,
    pub clip_order: u32,
    pub audio_path: String,
    pub duration_secs: u64,
    /// 纯语音转写原始实录（不含总结纪要）
    pub transcript: Option<String>,
    /// 转写状态: pending / transcribed / failed
    pub status: String,
    pub created_at: DateTime<Utc>,
}

impl MeetingAudioClip {
    pub fn new(
        meeting_id: Uuid,
        clip_order: u32,
        audio_path: impl Into<String>,
        duration_secs: u64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            meeting_id,
            clip_order,
            audio_path: audio_path.into(),
            duration_secs,
            transcript: None,
            status: "pending".to_string(),
            created_at: Utc::now(),
        }
    }
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

/// 会议关联参考资料实体（支持客户发送的文档与图片）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingMaterial {
    pub id: Uuid,
    pub meeting_id: Uuid,
    pub file_name: String,
    /// 资料类型: "image" | "document"
    pub file_type: String,
    /// 本地存储绝对路径
    pub file_path: String,
    /// 文件字节大小
    pub file_size: u64,
    /// 从文档解析出的文本摘要或提取内容
    pub extracted_text: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl MeetingMaterial {
    pub fn new(
        meeting_id: Uuid,
        file_name: impl Into<String>,
        file_type: impl Into<String>,
        file_path: impl Into<String>,
        file_size: u64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            meeting_id,
            file_name: file_name.into(),
            file_type: file_type.into(),
            file_path: file_path.into(),
            file_size,
            extracted_text: None,
            created_at: Utc::now(),
        }
    }
}

