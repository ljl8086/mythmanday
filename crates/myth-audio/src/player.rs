// Copyright (C) 2026 MythManDay Authors. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (MythManDay)
// Version: V1.0
// Description: Native audio player & system media file launcher.

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AudioPlayer {
    current_process: Arc<Mutex<Option<Child>>>,
    current_file: Arc<Mutex<Option<PathBuf>>>,
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioPlayer {
    pub fn new() -> Self {
        Self {
            current_process: Arc::new(Mutex::new(None)),
            current_file: Arc::new(Mutex::new(None)),
        }
    }

    /// 播放指定的 WAV 音频文件 (优先原生 paplay -> aplay -> xdg-open)
    pub fn play(&self, path: &Path) -> Result<(), String> {
        self.stop();

        if !path.exists() {
            return Err(format!("音频文件不存在: {}", path.display()));
        }

        let child = Command::new("paplay")
            .arg(path)
            .spawn()
            .or_else(|_| Command::new("aplay").arg(path).spawn())
            .or_else(|_| Command::new("xdg-open").arg(path).spawn())
            .map_err(|e| format!("启动音频播放进程失败: {}", e))?;

        if let Ok(mut proc) = self.current_process.lock() {
            *proc = Some(child);
        }
        if let Ok(mut f) = self.current_file.lock() {
            *f = Some(path.to_path_buf());
        }

        Ok(())
    }

    /// 停止当前播放中的音频
    pub fn stop(&self) {
        if let Ok(mut proc) = self.current_process.lock() {
            if let Some(mut child) = proc.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        if let Ok(mut f) = self.current_file.lock() {
            *f = None;
        }
    }

    /// 检查是否正在播放音频
    pub fn is_playing(&self) -> bool {
        if let Ok(mut proc) = self.current_process.lock() {
            if let Some(ref mut child) = *proc {
                match child.try_wait() {
                    Ok(Some(_)) => {
                        *proc = None;
                        false
                    }
                    Ok(None) => true,
                    Err(_) => false,
                }
            } else {
                false
            }
        } else {
            false
        }
    }

    /// 获取当前正在播放的文件路径
    pub fn current_file(&self) -> Option<PathBuf> {
        self.current_file.lock().ok().and_then(|f| f.clone())
    }

    /// 在系统原生文件管理器中高亮定位打开音频所在目录
    pub fn open_directory(&self, path: &Path) -> Result<(), String> {
        let target_dir = if path.is_file() {
            path.parent().unwrap_or(path)
        } else {
            path
        };
        Command::new("xdg-open")
            .arg(target_dir)
            .spawn()
            .map_err(|e| format!("打开文件夹失败: {}", e))?;
        Ok(())
    }
}
