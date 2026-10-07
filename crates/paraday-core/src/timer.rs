// Copyright (C) 2026 ParaDay Authors. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (ParaDay)
// Version: V1.0
// Description: Multi-channel concurrent time-tracking & man-day billing engine.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// 单个通道的计时状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TimerStatus {
    /// 停止/未启动
    Stopped,
    /// 运行中 (包含启动时间戳及历史累计秒数)
    Running {
        current_started_at: DateTime<Utc>,
        accumulated_secs: u64,
    },
    /// 已暂停 (挂起中)
    Paused {
        accumulated_secs: u64,
    },
}

/// 独立的通道计时器 (Channel Timer)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelTimer {
    pub project_id: Uuid,
    pub session_start_at: Option<DateTime<Utc>>,
    pub status: TimerStatus,
}

impl ChannelTimer {
    pub fn new(project_id: Uuid) -> Self {
        Self {
            project_id,
            session_start_at: None,
            status: TimerStatus::Stopped,
        }
    }

    /// 启动计时通道
    pub fn start(&mut self) {
        let now = Utc::now();
        match self.status {
            TimerStatus::Stopped => {
                self.session_start_at = Some(now);
                self.status = TimerStatus::Running {
                    current_started_at: now,
                    accumulated_secs: 0,
                };
            }
            TimerStatus::Paused { accumulated_secs } => {
                self.status = TimerStatus::Running {
                    current_started_at: now,
                    accumulated_secs,
                };
            }
            TimerStatus::Running { .. } => {
                // 已经在运行中，保持
            }
        }
    }

    /// 暂停当前通道
    pub fn pause(&mut self) {
        if let TimerStatus::Running {
            current_started_at,
            accumulated_secs,
        } = self.status
        {
            let now = Utc::now();
            let delta = now
                .signed_duration_since(current_started_at)
                .num_seconds()
                .max(0) as u64;
            self.status = TimerStatus::Paused {
                accumulated_secs: accumulated_secs + delta,
            };
        }
    }

    /// 停止通道并返回完整的本次区间与总耗时 (开始时间, 结束时间, 总有效秒数)
    pub fn stop(&mut self) -> Option<(DateTime<Utc>, DateTime<Utc>, u64)> {
        let now = Utc::now();
        let total_secs = self.elapsed_secs();
        let start_at = self.session_start_at.unwrap_or(now);

        if total_secs == 0 && self.status == TimerStatus::Stopped {
            return None;
        }

        self.status = TimerStatus::Stopped;
        self.session_start_at = None;

        Some((start_at, now, total_secs))
    }

    /// 获取当前通道累计耗时（秒）
    pub fn elapsed_secs(&self) -> u64 {
        match self.status {
            TimerStatus::Stopped => 0,
            TimerStatus::Paused { accumulated_secs } => accumulated_secs,
            TimerStatus::Running {
                current_started_at,
                accumulated_secs,
            } => {
                let now = Utc::now();
                let delta = now
                    .signed_duration_since(current_started_at)
                    .num_seconds()
                    .max(0) as u64;
                accumulated_secs + delta
            }
        }
    }

    /// 扣除/校准秒数（例如 AFK 离开处置时扣除摸鱼时间）
    pub fn deduct_secs(&mut self, secs: u64) {
        match &mut self.status {
            TimerStatus::Stopped => {}
            TimerStatus::Paused { accumulated_secs } => {
                *accumulated_secs = accumulated_secs.saturating_sub(secs);
            }
            TimerStatus::Running {
                accumulated_secs, ..
            } => {
                *accumulated_secs = accumulated_secs.saturating_sub(secs);
            }
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(self.status, TimerStatus::Running { .. })
    }
}

/// 多通道并发计时引擎管理器 (Multi-Channel Manager)
#[derive(Debug, Default)]
pub struct MultiChannelManager {
    channels: HashMap<Uuid, ChannelTimer>,
}

impl MultiChannelManager {
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
        }
    }

    /// 启动指定项目的计时通道
    pub fn start_channel(&mut self, project_id: Uuid) {
        let timer = self
            .channels
            .entry(project_id)
            .or_insert_with(|| ChannelTimer::new(project_id));
        timer.start();
    }

    /// 暂停指定项目
    pub fn pause_channel(&mut self, project_id: Uuid) {
        if let Some(timer) = self.channels.get_mut(&project_id) {
            timer.pause();
        }
    }

    /// 停止指定项目通道并返回结算数据
    pub fn stop_channel(&mut self, project_id: Uuid) -> Option<(DateTime<Utc>, DateTime<Utc>, u64)> {
        if let Some(timer) = self.channels.get_mut(&project_id) {
            timer.stop()
        } else {
            None
        }
    }

    /// 全局 AFK 保护：批量挂起所有正在运行的通道
    pub fn pause_all(&mut self) {
        for timer in self.channels.values_mut() {
            if timer.is_running() {
                timer.pause();
            }
        }
    }

    /// 批量扣除空闲时间（AFK 校准）
    pub fn deduct_from_channel(&mut self, project_id: Uuid, secs: u64) {
        if let Some(timer) = self.channels.get_mut(&project_id) {
            timer.deduct_secs(secs);
        }
    }

    /// 获取所有正在运行的项目 ID 列表
    pub fn active_channel_ids(&self) -> Vec<Uuid> {
        self.channels
            .iter()
            .filter(|(_, timer)| timer.is_running())
            .map(|(id, _)| *id)
            .collect()
    }

    /// 获取指定通道累计耗时
    pub fn get_elapsed(&self, project_id: &Uuid) -> u64 {
        self.channels
            .get(project_id)
            .map(|t| t.elapsed_secs())
            .unwrap_or(0)
    }

    /// 获取通道当前状态
    pub fn get_status(&self, project_id: &Uuid) -> TimerStatus {
        self.channels
            .get(project_id)
            .map(|t| t.status.clone())
            .unwrap_or(TimerStatus::Stopped)
    }

    /// 获取通道详情
    pub fn get_timer(&self, project_id: &Uuid) -> Option<&ChannelTimer> {
        self.channels.get(project_id)
    }

    /// 从持久化存储恢复通道（崩溃与断电容灾）
    pub fn restore_channel(
        &mut self,
        project_id: Uuid,
        session_start: DateTime<Utc>,
        current_started: Option<DateTime<Utc>>,
        accumulated_secs: u64,
        is_paused: bool,
    ) {
        let status = if is_paused {
            TimerStatus::Paused { accumulated_secs }
        } else {
            TimerStatus::Running {
                current_started_at: current_started.unwrap_or(session_start),
                accumulated_secs,
            }
        };
        let timer = ChannelTimer {
            project_id,
            session_start_at: Some(session_start),
            status,
        };
        self.channels.insert(project_id, timer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_channel_parallel_runs() {
        let mut manager = MultiChannelManager::new();
        let proj_a = Uuid::new_v4();
        let proj_b = Uuid::new_v4();
        let proj_c = Uuid::new_v4();

        // 同时启动 A 和 B
        manager.start_channel(proj_a);
        manager.start_channel(proj_b);

        let active = manager.active_channel_ids();
        assert_eq!(active.len(), 2);
        assert!(active.contains(&proj_a));
        assert!(active.contains(&proj_b));
        assert!(!active.contains(&proj_c));

        // 暂停 A，B 继续跑
        manager.pause_channel(proj_a);
        assert_eq!(manager.active_channel_ids().len(), 1);
        assert_eq!(manager.active_channel_ids()[0], proj_b);

        // 停止 B
        let result_b = manager.stop_channel(proj_b);
        assert!(result_b.is_some());
        assert_eq!(manager.active_channel_ids().len(), 0);
    }
}
