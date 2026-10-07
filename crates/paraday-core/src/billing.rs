// Copyright (C) 2026 ParaDay Authors. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (ParaDay)
// Version: V1.0
// Description: Multi-channel concurrent time-tracking & man-day billing engine.

use crate::models::{BillingType, DayCalcMode, Project};

/// 计费计算引擎
pub struct BillingEngine;

impl BillingEngine {
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
}
