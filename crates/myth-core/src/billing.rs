// Copyright (C) 2026 ljl8086 <ljl8086@gmail.com>. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (MythManDay)
// Version: V1.0
// Description: Multi-channel concurrent time-tracking & man-day billing engine.

use crate::models::{BillingType, DayCalcMode, Project, RoundingMode};

/// 结算核算策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SettlementStrategy {
    /// 策略 1: 削峰填谷 (工作日封顶 8h，超额回填历史谷值或顺延未来工作日，周末不排)
    RebalanceWorkdays,
    /// 策略 2: 保底 8 小时 (当天只要有投入不足 8h 按 8h 计算，超过 8h 的溢出工时结转到新的一天继续按 8h 排期)
    Min8HoursCarryOver,
    /// 策略 3: 单日封顶 8 小时，多出的工时直接顺延到新的一天 (不回填历史，仅向后追加)
    Cap8HoursCarryOver,
    /// 策略 4: 实际工时 (同项目同一天合并为一行，按实际投入比例核算，不平摊不顺延)
    ExactActual,
}

#[derive(Debug, Clone)]
pub struct RawDayHelper {
    pub date: chrono::NaiveDate,
    pub total_secs: u64,
    pub summaries: Vec<String>,
    pub is_all_invoiced: bool,
    pub has_invoiced: bool,
    pub any_audio: bool,
}

/// 计费计算引擎
pub struct BillingEngine;

impl BillingEngine {
    /// 通用数值阶梯舍入（支持四舍五入 HalfUp、进位 Ceil、舍去 Floor，支持自定义最小刻度）
    pub fn round_to_granularity(val: f64, granularity: f64, mode: RoundingMode) -> f64 {
        if val <= 0.0 {
            return 0.0;
        }
        let step = if granularity > 0.0 { granularity } else { 0.5 };
        let ratio = val / step;
        let rounded_units = match mode {
            RoundingMode::Ceil => ratio.ceil(),
            RoundingMode::HalfUp => ratio.round(),
            RoundingMode::Floor => ratio.floor(),
        };
        (rounded_units * step * 1000.0).round() / 1000.0
    }

    /// 计算取整后的计费小时数 (小时)
    pub fn calculate_billable_hours(duration_secs: u64, rounding_minutes: u32) -> f64 {
        if duration_secs == 0 {
            return 0.0;
        }

        let raw_minutes = duration_secs as f64 / 60.0;
        let step = rounding_minutes.max(1) as f64;

        // 向上取整到指定的分钟步长 (如 1m, 15m, 30m)
        let rounded_minutes = (raw_minutes / step).ceil() * step;
        rounded_minutes / 60.0
    }

    /// 将小时数按最小刻度 0.5 阶梯向上取整并格式化为纯数字字符串 (如 "8.0", "4.5", "0.5")
    /// 规则：以小时为单位，最小刻度为 0.5，不足 0.5 按 0.5，超过按 1 (即向上取整到 0.5 的整数倍)，单位统一在列头展示
    pub fn format_step_half_hours(hours: f64) -> (f64, String) {
        if hours <= 0.0 {
            return (0.0, "0.0".to_string());
        }
        // 向上取整到 0.5 刻度：ceil(hours * 2.0) / 2.0
        let stepped_hours = (hours * 2.0).ceil() / 2.0;
        let text = format!("{:.1}", stepped_hours);
        (stepped_hours, text)
    }

    /// 将计费工时换算为人天 (Man-Days)
    pub fn calculate_billable_days(billable_hours: f64, hours_per_day: f64, mode: DayCalcMode) -> f64 {
        if billable_hours <= 0.0 || hours_per_day <= 0.0 {
            return 0.0;
        }

        match mode {
            // 阶梯半人天制：<=半天算0.5天，<=1天算1.0天，超出部分按半天递进
            DayCalcMode::HalfStep => {
                let half_day = hours_per_day * 0.5;
                if billable_hours <= half_day {
                    0.5
                } else if billable_hours <= hours_per_day {
                    1.0
                } else {
                    // 超额部分按 0.5 天向上步进
                    let overtime = billable_hours - hours_per_day;
                    let extra_steps = (overtime / half_day).ceil();
                    1.0 + (extra_steps * 0.5)
                }
            }

            // 严格浮点比例制
            DayCalcMode::Exact => {
                let days = billable_hours / hours_per_day;
                // 保留两位小数
                (days * 100.0).round() / 100.0
            }

            // 全天制：只要有效介入（哪怕 1 小时），即记 1.0 天起步
            DayCalcMode::FullDay => {
                let full_days = (billable_hours / hours_per_day).ceil();
                full_days.max(1.0)
            }
        }
    }

    /// 结算金额计算
    pub fn calculate_amount(
        project: &Project,
        billable_hours: f64,
        billable_days: f64,
    ) -> (f64, f64) {
        match project.billing_type {
            BillingType::Daily => {
                let rate = project.daily_rate;
                let amount = billable_days * rate;
                (rate, (amount * 100.0).round() / 100.0)
            }
            BillingType::Hourly => {
                let rate = project.hourly_rate;
                let amount = billable_hours * rate;
                (rate, (amount * 100.0).round() / 100.0)
            }
            BillingType::FixedCap => {
                // 固定总包按基准日薪或时薪折算等效价值
                let rate = if project.daily_rate > 0.0 {
                    project.daily_rate
                } else {
                    project.hourly_rate
                };
                let amount = if project.daily_rate > 0.0 {
                    billable_days * project.daily_rate
                } else {
                    billable_hours * project.hourly_rate
                };
                (rate, (amount * 100.0).round() / 100.0)
            }
            BillingType::NonBillable => (0.0, 0.0),
        }
    }

    /// 根据指定策略转换单项目工时流水为单日核算工作日列表
    pub fn transform_project_workdays(
        daily_entries: Vec<crate::models::TimeEntry>,
        hours_per_day: f64,
        daily_rate: f64,
        currency: &str,
        strategy: SettlementStrategy,
    ) -> Vec<RebalancedWorkday> {
        use chrono::{Datelike, Local, NaiveDate, Weekday};
        use std::collections::BTreeMap;

        if daily_entries.is_empty() {
            return Vec::new();
        }

        let cap_hours = if hours_per_day > 0.0 { hours_per_day } else { 8.0 };

        let mut days_map: BTreeMap<NaiveDate, RawDayHelper> = BTreeMap::new();
        for e in daily_entries {
            let local_start = e.start_time.with_timezone(&Local);
            let date = local_start.date_naive();
            let entry_secs = e.raw_duration_secs;
            let summary = e.summary.unwrap_or_else(|| "持续代码开发与测试交付".to_string());
            let has_audio = e.audio_path.as_ref().map(|p| !p.is_empty()).unwrap_or(false);

            let day = days_map.entry(date).or_insert_with(|| RawDayHelper {
                date,
                total_secs: 0,
                summaries: Vec::new(),
                is_all_invoiced: true,
                has_invoiced: false,
                any_audio: false,
            });

            day.total_secs += entry_secs;
            if !day.summaries.contains(&summary) {
                day.summaries.push(summary);
            }
            if !e.is_invoiced {
                day.is_all_invoiced = false;
            } else {
                day.has_invoiced = true;
            }
            if has_audio {
                day.any_audio = true;
            }
        }

        let is_workday = |d: NaiveDate| -> bool {
            let wd = d.weekday();
            wd != Weekday::Sat && wd != Weekday::Sun
        };

        struct TargetDay {
            #[allow(dead_code)]
            date: NaiveDate,
            hours: f64,
            summaries: Vec<String>,
            is_invoiced: bool,
            has_audio: bool,
        }

        let mut target_days: BTreeMap<NaiveDate, TargetDay> = BTreeMap::new();

        match strategy {
            SettlementStrategy::ExactActual => {
                // 策略 4: 实际工时合并 (当天只合并为一行，实报实销)
                for (date, raw) in days_map {
                    let actual_hours = raw.total_secs as f64 / 3600.0;
                    target_days.insert(date, TargetDay {
                        date,
                        hours: actual_hours,
                        summaries: raw.summaries,
                        is_invoiced: raw.has_invoiced,
                        has_audio: raw.any_audio,
                    });
                }
            }

            SettlementStrategy::RebalanceWorkdays => {
                // 策略 1: 削峰填谷模式
                return Self::rebalance_project_workdays_internal(days_map, cap_hours, daily_rate, currency);
            }

            SettlementStrategy::Min8HoursCarryOver => {
                // 策略 2: 不足 8 小时按 8 小时 (1.0 天)，多出的溢出顺延到新的一天继续按 8h/天 计算
                let mut sorted_dates: Vec<NaiveDate> = days_map.keys().cloned().collect();
                sorted_dates.sort();

                let mut overflow_hours = 0.0f64;
                let mut overflow_summaries: Vec<String> = Vec::new();

                for d in sorted_dates {
                    let raw = days_map.remove(&d).unwrap();
                    let raw_hours = raw.total_secs as f64 / 3600.0;

                    // 商业保底逻辑：只要当天有工时投入，基准即为 8 小时
                    let effective_day_hours = if raw_hours <= cap_hours {
                        cap_hours
                    } else {
                        let extra = raw_hours - cap_hours;
                        overflow_hours += extra;
                        for s in &raw.summaries {
                            if !overflow_summaries.contains(s) {
                                overflow_summaries.push(s.clone());
                            }
                        }
                        cap_hours
                    };

                    target_days.insert(d, TargetDay {
                        date: d,
                        hours: effective_day_hours,
                        summaries: raw.summaries,
                        is_invoiced: raw.has_invoiced,
                        has_audio: raw.any_audio,
                    });
                }

                // 将多余的工时追加顺延到新工作日
                let last_date = target_days.keys().last().cloned().unwrap_or_else(|| chrono::Local::now().date_naive());
                let mut curr_date = last_date.succ_opt().unwrap_or(last_date);

                while overflow_hours > 0.001 {
                    if is_workday(curr_date) {
                        let take = overflow_hours.min(cap_hours);
                        overflow_hours -= take;

                        let mut carry_summaries = Vec::new();
                        for os in &overflow_summaries {
                            if !carry_summaries.contains(os) {
                                carry_summaries.push(os.clone());
                            }
                        }
                        if carry_summaries.is_empty() {
                            carry_summaries.push("超额工时顺延核算与成果交付".to_string());
                        }

                        // 新顺延出的一天同样按 8 小时核算 (保底)
                        target_days.insert(curr_date, TargetDay {
                            date: curr_date,
                            hours: cap_hours,
                            summaries: carry_summaries,
                            is_invoiced: false,
                            has_audio: false,
                        });
                    }
                    curr_date = curr_date.succ_opt().unwrap_or(curr_date);
                }
            }

            SettlementStrategy::Cap8HoursCarryOver => {
                // 策略 3: 每日封顶 8 小时，多出的工时顺延到新的一天 (不强行把不足 8h 的拔高到 8h)
                let mut sorted_dates: Vec<NaiveDate> = days_map.keys().cloned().collect();
                sorted_dates.sort();

                let mut overflow_hours = 0.0f64;
                let mut overflow_summaries: Vec<String> = Vec::new();

                for d in sorted_dates {
                    let raw = days_map.remove(&d).unwrap();
                    let raw_hours = raw.total_secs as f64 / 3600.0;

                    if raw_hours > cap_hours {
                        let extra = raw_hours - cap_hours;
                        overflow_hours += extra;
                        for s in &raw.summaries {
                            if !overflow_summaries.contains(s) {
                                overflow_summaries.push(s.clone());
                            }
                        }
                        target_days.insert(d, TargetDay {
                            date: d,
                            hours: cap_hours,
                            summaries: raw.summaries,
                            is_invoiced: raw.has_invoiced,
                            has_audio: raw.any_audio,
                        });
                    } else {
                        target_days.insert(d, TargetDay {
                            date: d,
                            hours: raw_hours,
                            summaries: raw.summaries,
                            is_invoiced: raw.has_invoiced,
                            has_audio: raw.any_audio,
                        });
                    }
                }

                // 将多余的工时追加顺延到新工作日
                let last_date = target_days.keys().last().cloned().unwrap_or_else(|| chrono::Local::now().date_naive());
                let mut curr_date = last_date.succ_opt().unwrap_or(last_date);

                while overflow_hours > 0.001 {
                    if is_workday(curr_date) {
                        let take = overflow_hours.min(cap_hours);
                        overflow_hours -= take;

                        let mut carry_summaries = Vec::new();
                        for os in &overflow_summaries {
                            if !carry_summaries.contains(os) {
                                carry_summaries.push(os.clone());
                            }
                        }
                        if carry_summaries.is_empty() {
                            carry_summaries.push("阶段性工时顺延交付与核算".to_string());
                        }

                        target_days.insert(curr_date, TargetDay {
                            date: curr_date,
                            hours: take,
                            summaries: carry_summaries,
                            is_invoiced: false,
                            has_audio: false,
                        });
                    }
                    curr_date = curr_date.succ_opt().unwrap_or(curr_date);
                }
            }
        }

        // 格式化输出
        let mut results = Vec::new();
        for (date, td) in target_days {
            let days_val = (td.hours / cap_hours * 100.0).round() / 100.0;
            let amount = (days_val * daily_rate * 100.0).round() / 100.0;

            let (_, duration_text) = Self::format_step_half_hours(td.hours);

            results.push(RebalancedWorkday {
                date_str: date.format("%Y-%m-%d").to_string(),
                duration_text,
                days_text: format!("{:.2} 天", days_val),
                days_val,
                amount_text: format!("{} {:.2}", currency, amount),
                amount_val: amount,
                summary_text: td.summaries.join("；"),
                is_invoiced: td.is_invoiced,
                has_audio: td.has_audio,
            });
        }
        results
    }

    /// 削峰填谷核心内部实现
    fn rebalance_project_workdays_internal(
        mut days_map: std::collections::BTreeMap<chrono::NaiveDate, RawDayHelper>,
        cap_hours: f64,
        daily_rate: f64,
        currency: &str,
    ) -> Vec<RebalancedWorkday> {
        use chrono::{Datelike, NaiveDate, Weekday};
        use std::collections::BTreeMap;

        let is_workday = |d: NaiveDate| -> bool {
            let wd = d.weekday();
            wd != Weekday::Sat && wd != Weekday::Sun
        };

        let mut sorted_dates: Vec<NaiveDate> = days_map.keys().cloned().collect();
        sorted_dates.sort();

        struct AdjustedDay {
            #[allow(dead_code)]
            date: NaiveDate,
            hours: f64,
            summaries: Vec<String>,
            is_invoiced: bool,
            has_audio: bool,
        }

        let mut adjusted_days: BTreeMap<NaiveDate, AdjustedDay> = BTreeMap::new();
        let mut overflow_hours = 0.0f64;
        let mut overflow_summaries: Vec<String> = Vec::new();

        for d in sorted_dates {
            let raw = days_map.remove(&d).unwrap();
            let raw_hours = raw.total_secs as f64 / 3600.0;

            if !is_workday(d) {
                overflow_hours += raw_hours;
                for s in raw.summaries {
                    if !overflow_summaries.contains(&s) {
                        overflow_summaries.push(s);
                    }
                }
                continue;
            }

            if raw_hours > cap_hours {
                let extra = raw_hours - cap_hours;
                overflow_hours += extra;
                for s in &raw.summaries {
                    if !overflow_summaries.contains(s) {
                        overflow_summaries.push(s.clone());
                    }
                }

                adjusted_days.insert(d, AdjustedDay {
                    date: d,
                    hours: cap_hours,
                    summaries: raw.summaries,
                    is_invoiced: raw.has_invoiced,
                    has_audio: raw.any_audio,
                });
            } else {
                let can_absorb = cap_hours - raw_hours;
                let take = overflow_hours.min(can_absorb);
                let final_hours = raw_hours + take;
                overflow_hours -= take;

                let mut combined_summaries = raw.summaries;
                if take > 0.0 && !overflow_summaries.is_empty() {
                    for os in &overflow_summaries {
                        if !combined_summaries.contains(os) {
                            combined_summaries.push(os.clone());
                        }
                    }
                }

                adjusted_days.insert(d, AdjustedDay {
                    date: d,
                    hours: final_hours,
                    summaries: combined_summaries,
                    is_invoiced: raw.has_invoiced,
                    has_audio: raw.any_audio,
                });
            }
        }

        if overflow_hours > 0.001 {
            let dates_asc: Vec<NaiveDate> = adjusted_days.keys().cloned().collect();
            for d in dates_asc {
                if overflow_hours <= 0.001 {
                    break;
                }
                if let Some(adj) = adjusted_days.get_mut(&d) {
                    if adj.hours < cap_hours {
                        let can_take = (cap_hours - adj.hours).min(overflow_hours);
                        adj.hours += can_take;
                        overflow_hours -= can_take;
                        for os in &overflow_summaries {
                            if !adj.summaries.contains(os) {
                                adj.summaries.push(os.clone());
                            }
                        }
                    }
                }
            }
        }

        if overflow_hours > 0.001 {
            let last_date = adjusted_days.keys().last().cloned().unwrap_or_else(|| {
                chrono::Local::now().date_naive()
            });
            let mut curr_date = last_date.succ_opt().unwrap_or(last_date);

            while overflow_hours > 0.001 {
                if is_workday(curr_date) {
                    let take = overflow_hours.min(cap_hours);
                    overflow_hours -= take;

                    let mut carry_summaries = Vec::new();
                    for os in &overflow_summaries {
                        if !carry_summaries.contains(os) {
                            carry_summaries.push(os.clone());
                        }
                    }
                    if carry_summaries.is_empty() {
                        carry_summaries.push("阶段性研发与交付成果顺延对账".to_string());
                    }

                    adjusted_days.insert(curr_date, AdjustedDay {
                        date: curr_date,
                        hours: take,
                        summaries: carry_summaries,
                        is_invoiced: false,
                        has_audio: false,
                    });
                }
                curr_date = curr_date.succ_opt().unwrap_or(curr_date);
            }
        }

        let mut results = Vec::new();
        for (date, adj) in adjusted_days {
            let days_val = (adj.hours / cap_hours * 100.0).round() / 100.0;
            let amount = (days_val * daily_rate * 100.0).round() / 100.0;

            let (_, duration_text) = Self::format_step_half_hours(adj.hours);

            results.push(RebalancedWorkday {
                date_str: date.format("%Y-%m-%d").to_string(),
                duration_text,
                days_text: format!("{:.2} 天", days_val),
                days_val,
                amount_text: format!("{} {:.2}", currency, amount),
                amount_val: amount,
                summary_text: adj.summaries.join("；"),
                is_invoiced: adj.is_invoiced,
                has_audio: adj.has_audio,
            });
        }
        results
    }

    /// 保持兼容的削峰填谷入口
    pub fn rebalance_project_workdays(
        daily_entries: Vec<crate::models::TimeEntry>,
        hours_per_day: f64,
        daily_rate: f64,
        currency: &str,
    ) -> Vec<RebalancedWorkday> {
        Self::transform_project_workdays(
            daily_entries,
            hours_per_day,
            daily_rate,
            currency,
            SettlementStrategy::RebalanceWorkdays,
        )
    }
}


/// 平摊重排后的单日合规工作日结构
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RebalancedWorkday {
    pub date_str: String,
    pub duration_text: String,
    pub days_text: String,
    pub days_val: f64,
    pub amount_text: String,
    pub amount_val: f64,
    pub summary_text: String,
    pub is_invoiced: bool,
    pub has_audio: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{BillingType, DayCalcMode, Project};

    #[test]
    fn test_billable_hours_rounding() {
        // 32 分钟，15 分钟步长 -> 向上取整为 45 分钟 = 0.75 小时
        let hours = BillingEngine::calculate_billable_hours(32 * 60, 15);
        assert_eq!(hours, 0.75);

        // 30 分钟整，15 分钟步长 -> 0.5 小时
        let hours = BillingEngine::calculate_billable_hours(30 * 60, 15);
        assert_eq!(hours, 0.5);

        // 1 分钟步长
        let hours = BillingEngine::calculate_billable_hours(60, 1);
        assert_eq!(hours, 1.0 / 60.0);
    }

    #[test]
    fn test_half_step_day_calculation() {
        let hpd = 8.0;

        // 2 小时 -> 0.5 天
        assert_eq!(
            BillingEngine::calculate_billable_days(2.0, hpd, DayCalcMode::HalfStep),
            0.5
        );

        // 4 小时整 -> 0.5 天
        assert_eq!(
            BillingEngine::calculate_billable_days(4.0, hpd, DayCalcMode::HalfStep),
            0.5
        );

        // 5 小时 -> 1.0 天
        assert_eq!(
            BillingEngine::calculate_billable_days(5.0, hpd, DayCalcMode::HalfStep),
            1.0
        );

        // 8 小时整 -> 1.0 天
        assert_eq!(
            BillingEngine::calculate_billable_days(8.0, hpd, DayCalcMode::HalfStep),
            1.0
        );

        // 9.5 小时 -> 1.5 天
        assert_eq!(
            BillingEngine::calculate_billable_days(9.5, hpd, DayCalcMode::HalfStep),
            1.5
        );
    }

    #[test]
    fn test_daily_amount_calculation() {
        let mut project = Project::new("商业客户电商项目");
        project.billing_type = BillingType::Daily;
        project.daily_rate = 3000.0;
        project.hours_per_day = 8.0;
        project.day_calc_mode = DayCalcMode::HalfStep;

        // 投入 3.5 小时 -> 0.5 天 -> ¥1500
        let hours = 3.5;
        let days = BillingEngine::calculate_billable_days(hours, project.hours_per_day, project.day_calc_mode);
        let (rate, amount) = BillingEngine::calculate_amount(&project, hours, days);

        assert_eq!(days, 0.5);
        assert_eq!(rate, 3000.0);
        assert_eq!(amount, 1500.0);
    }

    #[test]
    fn test_format_step_half_hours() {
        assert_eq!(BillingEngine::format_step_half_hours(0.0).1, "0.0");
        assert_eq!(BillingEngine::format_step_half_hours(0.2).1, "0.5");
        assert_eq!(BillingEngine::format_step_half_hours(0.5).1, "0.5");
        assert_eq!(BillingEngine::format_step_half_hours(0.6).1, "1.0");
        assert_eq!(BillingEngine::format_step_half_hours(4.0).1, "4.0");
        assert_eq!(BillingEngine::format_step_half_hours(4.15).1, "4.5");
        assert_eq!(BillingEngine::format_step_half_hours(4.5).1, "4.5");
        assert_eq!(BillingEngine::format_step_half_hours(4.7).1, "5.0");
        assert_eq!(BillingEngine::format_step_half_hours(8.0).1, "8.0");
    }
}
