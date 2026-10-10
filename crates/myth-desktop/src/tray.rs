// Copyright (C) 2026 MythManDay Authors. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (MythManDay)
// Version: V1.0
// Description: Linux StatusNotifierItem (AppIndicator) 顶栏系统托盘服务

use ksni::{blocking::TrayMethods, menu::StandardItem, MenuItem, Tray};
use std::sync::mpsc::Sender;

pub enum TrayEvent {
    ToggleWindow,
    PauseAll,
    Quit,
}

pub struct MythManDayTray {
    pub tx: Sender<TrayEvent>,
    pub running_count: usize,
}

impl Tray for MythManDayTray {
    fn id(&self) -> String {
        "mythmanday-time-tracker".into()
    }

    fn title(&self) -> String {
        if self.running_count > 0 {
            format!("MythManDay ({} 运行中)", self.running_count)
        } else {
            "MythManDay 平行人天".into()
        }
    }

    fn icon_name(&self) -> String {
        "mythmanday".into()
    }

    fn icon_theme_path(&self) -> String {
        if let Ok(home) = std::env::var("HOME") {
            format!("{}/.local/share/icons", home)
        } else {
            "".into()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.tx.send(TrayEvent::ToggleWindow);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: "显示 / 隐藏主控制台".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayEvent::ToggleWindow);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "⏸ 暂停全部活动计时".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayEvent::PauseAll);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "退出 MythManDay".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayEvent::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub fn spawn_tray(tx: Sender<TrayEvent>) -> Option<ksni::blocking::Handle<MythManDayTray>> {
    let tray = MythManDayTray {
        tx,
        running_count: 0,
    };
    tray.spawn().ok()
}
