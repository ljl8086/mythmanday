// Copyright (C) 2026 ljl8086 <ljl8086@gmail.com>. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (MythManDay)
// Version: V1.0
// Description: Multi-channel concurrent time-tracking & man-day billing engine.

pub mod billing;
pub mod models;
pub mod timer;

pub use billing::BillingEngine;
pub use models::{BillingType, Client, DayCalcMode, Project, QuickNote, RoundingMode, TimeEntry};
pub use timer::{ChannelTimer, MultiChannelManager, TimerStatus};
