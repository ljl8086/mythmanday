// Copyright (C) 2026 ljl8086 <ljl8086@gmail.com>. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (MythManDay)
// Version: V1.0
// Description: Multi-channel concurrent time-tracking & man-day billing engine.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AudioError {
    #[error("未检测到可用的麦克风音频输入设备")]
    NoInputDevice,
    #[error("无法获取麦克风默认配置: {0}")]
    DefaultConfigError(String),
    #[error("构建音频录制流失败: {0}")]
    BuildStreamError(String),
    #[error("启动音频流失败: {0}")]
    PlayStreamError(String),
    #[error("WAV 音频文件创建失败: {0}")]
    WavError(#[from] hound::Error),
    #[error("当前未在录音中")]
    NotRecording,
    #[error("录音已在进行中")]
    AlreadyRecording,
}

pub struct AudioRecorder {
    is_recording: Arc<AtomicBool>,
    is_paused: Arc<AtomicBool>,
    current_volume: Arc<Mutex<f32>>,
    current_output_path: Option<PathBuf>,
    stream: Option<Stream>,
    writer_handle: Option<JoinHandle<Result<(), hound::Error>>>,
    sample_tx: Option<Sender<Vec<i16>>>,
    start_time: Option<std::time::Instant>,
}

unsafe impl Send for AudioRecorder {}
unsafe impl Sync for AudioRecorder {}

impl Default for AudioRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            is_recording: Arc::new(AtomicBool::new(false)),
            is_paused: Arc::new(AtomicBool::new(false)),
            current_volume: Arc::new(Mutex::new(0.0)),
            current_output_path: None,
            stream: None,
            writer_handle: None,
            sample_tx: None,
            start_time: None,
        }
    }

    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }

    pub fn is_paused(&self) -> bool {
        self.is_paused.load(Ordering::SeqCst)
    }

    /// 暂停当前录音流（停止写入磁盘并清空音量波动）
    pub fn pause_recording(&mut self) {
        if self.is_recording() {
            self.is_paused.store(true, Ordering::SeqCst);
            if let Ok(mut vol) = self.current_volume.lock() {
                *vol = 0.0;
            }
        }
    }

    /// 继续当前录音流
    pub fn resume_recording(&mut self) {
        if self.is_recording() {
            self.is_paused.store(false, Ordering::SeqCst);
        }
    }

    /// 获取当前麦克风实时音量等级 (0.0 ~ 1.0)，用于前端绘制跳动声波
    pub fn current_volume(&self) -> f32 {
        *self.current_volume.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 获取本次录音累计秒数
    pub fn elapsed_secs(&self) -> u64 {
        if let Some(st) = self.start_time {
            st.elapsed().as_secs()
        } else {
            0
        }
    }

    /// 开始录制麦克风音频流至指定的 .wav 文件路径
    pub fn start_recording(&mut self, output_path: &Path) -> Result<(), AudioError> {
        if self.is_recording() {
            return Err(AudioError::AlreadyRecording);
        }

        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or(AudioError::NoInputDevice)?;

        let default_config = device
            .default_input_config()
            .map_err(|e| AudioError::DefaultConfigError(e.to_string()))?;

        let sample_rate = default_config.sample_rate().0;
        let channels = default_config.channels();
        let sample_format = default_config.sample_format();

        let wav_spec = hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let (tx, rx): (Sender<Vec<i16>>, Receiver<Vec<i16>>) = channel();
        let path_clone = output_path.to_path_buf();

        // 独立文件写入线程，实时追加写入磁盘，避免内存堆积与断电丢失
        let writer_handle = thread::spawn(move || -> Result<(), hound::Error> {
            let file = File::create(&path_clone)?;
            let buf_writer = BufWriter::new(file);
            let mut writer = hound::WavWriter::new(buf_writer, wav_spec)?;

            while let Ok(samples) = rx.recv() {
                for sample in samples {
                    writer.write_sample(sample)?;
                }
            }

            writer.finalize()?;
            Ok(())
        });

        let volume_arc = self.current_volume.clone();
        let is_paused_flag = self.is_paused.clone();
        self.is_paused.store(false, Ordering::SeqCst);
        let err_fn = |err| {
            tracing::error!("Audio recording stream error: {}", err);
        };

        let stream = match sample_format {
            SampleFormat::F32 => {
                let tx_clone = tx.clone();
                let is_paused_f = is_paused_flag.clone();
                let vol_arc = volume_arc.clone();
                device
                    .build_input_stream(
                        &default_config.into(),
                        move |data: &[f32], _| {
                            if is_paused_f.load(Ordering::SeqCst) {
                                if let Ok(mut vol) = vol_arc.lock() {
                                    *vol = 0.0;
                                }
                                return;
                            }
                            let mut sum_sq = 0.0;
                            let mut converted = Vec::with_capacity(data.len());
                            for &s in data {
                                sum_sq += s * s;
                                let sample_i16 = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
                                converted.push(sample_i16);
                            }
                            let rms = (sum_sq / data.len().max(1) as f32).sqrt();
                            if let Ok(mut vol) = vol_arc.lock() {
                                *vol = (rms * 4.0).clamp(0.0, 1.0);
                            }
                            let _ = tx_clone.send(converted);
                        },
                        err_fn,
                        None,
                    )
                    .map_err(|e| AudioError::BuildStreamError(e.to_string()))?
            }
            SampleFormat::I16 => {
                let tx_clone = tx.clone();
                let is_paused_f = is_paused_flag.clone();
                let vol_arc = volume_arc.clone();
                device
                    .build_input_stream(
                        &default_config.into(),
                        move |data: &[i16], _| {
                            if is_paused_f.load(Ordering::SeqCst) {
                                if let Ok(mut vol) = vol_arc.lock() {
                                    *vol = 0.0;
                                }
                                return;
                            }
                            let mut sum_sq = 0.0;
                            let mut converted = Vec::with_capacity(data.len());
                            for &s in data {
                                let norm = s as f32 / 32768.0;
                                sum_sq += norm * norm;
                                converted.push(s);
                            }
                            let rms = (sum_sq / data.len().max(1) as f32).sqrt();
                            if let Ok(mut vol) = vol_arc.lock() {
                                *vol = (rms * 4.0).clamp(0.0, 1.0);
                            }
                            let _ = tx_clone.send(converted);
                        },
                        err_fn,
                        None,
                    )
                    .map_err(|e| AudioError::BuildStreamError(e.to_string()))?
            }
            SampleFormat::U16 => {
                let tx_clone = tx.clone();
                let is_paused_f = is_paused_flag.clone();
                let vol_arc = volume_arc.clone();
                device
                    .build_input_stream(
                        &default_config.into(),
                        move |data: &[u16], _| {
                            if is_paused_f.load(Ordering::SeqCst) {
                                if let Ok(mut vol) = vol_arc.lock() {
                                    *vol = 0.0;
                                }
                                return;
                            }
                            let mut sum_sq = 0.0;
                            let mut converted = Vec::with_capacity(data.len());
                            for &s in data {
                                let signed = (s as i32 - 32768) as i16;
                                let norm = signed as f32 / 32768.0;
                                sum_sq += norm * norm;
                                converted.push(signed);
                            }
                            let rms = (sum_sq / data.len().max(1) as f32).sqrt();
                            if let Ok(mut vol) = vol_arc.lock() {
                                *vol = (rms * 4.0).clamp(0.0, 1.0);
                            }
                            let _ = tx_clone.send(converted);
                        },
                        err_fn,
                        None,
                    )
                    .map_err(|e| AudioError::BuildStreamError(e.to_string()))?
            }
            _ => {
                return Err(AudioError::BuildStreamError(
                    "Unsupported audio sample format".into(),
                ))
            }
        };

        stream
            .play()
            .map_err(|e| AudioError::PlayStreamError(e.to_string()))?;

        self.stream = Some(stream);
        self.sample_tx = Some(tx);
        self.writer_handle = Some(writer_handle);
        self.current_output_path = Some(output_path.to_path_buf());
        self.is_recording.store(true, Ordering::SeqCst);
        self.start_time = Some(std::time::Instant::now());

        Ok(())
    }

    /// 停止录制并保存 WAV 音频文件，返回输出文件路径
    pub fn stop_recording(&mut self) -> Result<PathBuf, AudioError> {
        if !self.is_recording() {
            return Err(AudioError::NotRecording);
        }

        // 停止音频流
        self.stream = None;
        self.is_recording.store(false, Ordering::SeqCst);
        self.start_time = None;
        if let Ok(mut vol) = self.current_volume.lock() {
            *vol = 0.0;
        }

        // 关闭发送端通道，让写入线程安全写完尾帧并 finalize WAV 标头
        drop(self.sample_tx.take());

        if let Some(handle) = self.writer_handle.take() {
            if let Ok(res) = handle.join() {
                res.map_err(AudioError::WavError)?;
            }
        }

        let out = self
            .current_output_path
            .take()
            .ok_or(AudioError::NotRecording)?;
        Ok(out)
    }
}
