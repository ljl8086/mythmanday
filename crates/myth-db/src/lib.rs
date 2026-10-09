// Copyright (C) 2026 MythManDay Authors. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (MythManDay)
// Version: V1.0
// Description: Multi-channel concurrent time-tracking & man-day billing engine.

pub mod repo;
pub mod schema;

pub use repo::{ActiveTimerRecord, Database};
