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
        *self
            .current_volume
            .lock()
            .unwrap_or_else(|e| e.into_inner())
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

/// 尝试使用本地 ffmpeg 将高保真 WAV 音频转码为高度压缩的 MP3 文件 (64k-128kbps)，大幅减轻发送给 AI 的体积并防超时
pub fn convert_wav_to_mp3(wav_path: &Path) -> Result<PathBuf, String> {
    if !wav_path.exists() {
        return Err(format!("WAV 文件不存在: {}", wav_path.display()));
    }

    let mp3_path = wav_path.with_extension("mp3");

    // 检查是否具备 ffmpeg
    let output = std::process::Command::new("ffmpeg")
        .arg("-y") // 覆盖已有文件
        .arg("-i")
        .arg(wav_path)
        .arg("-vn") // 禁用视频流
        .arg("-ar")
        .arg("16000") // 语音识别最佳采样率 16kHz
        .arg("-ac")
        .arg("1") // 单声道
        .arg("-b:a")
        .arg("64k") // 64kbps 高度压缩且人声清晰
        .arg(&mp3_path)
        .output()
        .map_err(|e| format!("启动 ffmpeg 压缩失败: {}", e))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffmpeg 压缩转码异常: {}", err_msg));
    }

    if mp3_path.exists() {
        Ok(mp3_path)
    } else {
        Err("转码后未生成 MP3 文件".to_string())
    }
}

/// 将过大的音频文件切分为适合云端 API (通常限制 25MB) 转写的分段切片。
/// 返回切片后的文件绝对路径列表，按时间顺序排列。
pub fn split_audio_into_chunks(
    audio_path: &Path,
    segment_duration_secs: u32,
    output_dir: &Path,
) -> Result<Vec<PathBuf>, String> {
    if !audio_path.exists() {
        return Err(format!("音频文件不存在: {}", audio_path.display()));
    }

    std::fs::create_dir_all(output_dir).map_err(|e| format!("创建切片临时目录失败: {}", e))?;

    let ext = audio_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("mp3");

    let pattern = output_dir.join(format!("chunk_%04d.{}", ext));
    let pattern_str = pattern.to_string_lossy().to_string();

    let output = std::process::Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(audio_path)
        .arg("-f")
        .arg("segment")
        .arg("-segment_time")
        .arg(segment_duration_secs.to_string())
        .arg("-c")
        .arg("copy")
        .arg(&pattern_str)
        .output()
        .map_err(|e| format!("启动 ffmpeg 切分音频失败: {}", e))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffmpeg 切片异常: {}", err_msg));
    }

    let mut chunk_paths = Vec::new();
    if let Ok(entries) = std::fs::read_dir(output_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    if file_name.starts_with("chunk_") && file_name.ends_with(ext) {
                        chunk_paths.push(path);
                    }
                }
            }
        }
    }

    chunk_paths.sort();
    if chunk_paths.is_empty() {
        return Err("音频切片后未发现任何有效切片文件".to_string());
    }

    Ok(chunk_paths)
}

/// 确保音频为 whisper.cpp 严格要求的 16kHz 单声道 (Mono) 16-bit PCM WAV 格式。
/// 若当前音频已符合标准则直接返回；若非 16kHz 或非单声道（如硬件声卡默认 44.1k/48k 双声道），
/// 则优先调用本地 ffmpeg 转码，或使用内建轻量重采样与声道合并算法生成合规 WAV。
pub fn ensure_whisper_compatible_wav(input_path: &Path) -> Result<PathBuf, String> {
    if !input_path.exists() {
        return Err(format!("输入音频文件不存在: {}", input_path.display()));
    }

    // 1. 尝试以 hound 读取 spec 检查是否已经合规
    if let Ok(reader) = hound::WavReader::open(input_path) {
        let spec = reader.spec();
        if spec.channels == 1
            && spec.sample_rate == 16000
            && spec.bits_per_sample == 16
            && spec.sample_format == hound::SampleFormat::Int
        {
            return Ok(input_path.to_path_buf());
        }
    }

    let target_path = input_path.with_extension("whisper_16k_mono.wav");

    // 2. 优先尝试本地 ffmpeg 进行专业高质量重采样与单声道混音
    let ffmpeg_res = std::process::Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(input_path)
        .arg("-vn")
        .arg("-ar")
        .arg("16000")
        .arg("-ac")
        .arg("1")
        .arg("-c:a")
        .arg("pcm_s16le")
        .arg(&target_path)
        .output();

    if let Ok(output) = ffmpeg_res {
        if output.status.success() && target_path.exists() {
            return Ok(target_path);
        }
    }

    // 3. 无 ffmpeg 时，利用内建纯 Rust 流式算法进行多声道降混与线性插值重采样 (内存恒定 O(1)，无惧大音频)
    let reader =
        hound::WavReader::open(input_path).map_err(|e| format!("读取原始音频失败: {}", e))?;
    let in_spec = reader.spec();
    let in_channels = in_spec.channels as usize;
    let in_rate = in_spec.sample_rate as f64;
    let out_rate = 16000.0f64;

    if in_channels == 0 || in_rate <= 0.0 {
        return Err("非法音频规格参数".to_string());
    }

    let out_spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let file =
        std::fs::File::create(&target_path).map_err(|e| format!("创建合规 WAV 文件失败: {}", e))?;
    let mut writer = hound::WavWriter::new(std::io::BufWriter::new(file), out_spec)
        .map_err(|e| format!("初始化 WAV 写入器失败: {}", e))?;

    let max_int_val = match in_spec.bits_per_sample {
        8 => 128.0,
        16 => 32768.0,
        24 => 8388608.0,
        32 => 2147483648.0,
        _ => 32768.0,
    };

    let mut float_samples: Box<dyn Iterator<Item = f32>> = match in_spec.sample_format {
        hound::SampleFormat::Float => Box::new(reader.into_samples::<f32>().filter_map(|s| s.ok())),
        hound::SampleFormat::Int => Box::new(
            reader
                .into_samples::<i32>()
                .filter_map(|s| s.ok())
                .map(move |s| (s as f64 / max_int_val) as f32),
        ),
    };

    // 单声道流式生成器
    let mut mono_stream = std::iter::from_fn(move || {
        let mut sum = 0.0f32;
        let mut count = 0usize;
        for _ in 0..in_channels {
            if let Some(s) = float_samples.next() {
                sum += s;
                count += 1;
            } else {
                break;
            }
        }
        if count == 0 {
            None
        } else {
            Some(sum / (count as f32))
        }
    });

    let mut s0 = match mono_stream.next() {
        Some(s) => s,
        None => return Err("音频数据为空".to_string()),
    };
    let mut s1 = mono_stream.next().unwrap_or(s0);

    let mut in_frame: usize = 0;
    let mut out_frame: usize = 0;

    loop {
        let target_in = (out_frame as f64) * in_rate / out_rate;
        let target_floor = target_in.floor() as usize;

        while in_frame < target_floor {
            s0 = s1;
            if let Some(next_s) = mono_stream.next() {
                s1 = next_s;
                in_frame += 1;
            } else {
                break;
            }
        }

        if in_frame < target_floor {
            break;
        }

        let frac = (target_in - (target_floor as f64)) as f32;
        let val = (s0 + (s1 - s0) * frac).clamp(-1.0, 1.0);
        let sample_i16 = (val * 32767.0).round() as i16;
        writer
            .write_sample(sample_i16)
            .map_err(|e| format!("写入样本失败: {}", e))?;

        out_frame += 1;
    }

    writer
        .finalize()
        .map_err(|e| format!("写入完成失败: {}", e))?;

    Ok(target_path)
}
