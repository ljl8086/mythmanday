// Copyright (C) 2026 ljl8086 <ljl8086@gmail.com>. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (MythManDay)
// Version: V1.0
// Description: Native microphone audio recording & acoustic processing engine.

pub mod player;
pub mod recorder;

pub use player::AudioPlayer;
pub use recorder::{convert_wav_to_mp3, ensure_whisper_compatible_wav, AudioError, AudioRecorder};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recorder_initial_state() {
        let rec = AudioRecorder::new();
        assert!(!rec.is_recording());
        assert_eq!(rec.current_volume(), 0.0);
        assert_eq!(rec.elapsed_secs(), 0);
    }

    #[test]
    fn test_ensure_whisper_compatible_wav_conversion() {
        let temp_dir = std::env::temp_dir();
        let input_wav = temp_dir.join(format!("test_48k_stereo_{}.wav", uuid::Uuid::new_v4()));

        // 构造一个 48kHz 双声道 16-bit 测试音频
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        {
            let mut writer = hound::WavWriter::create(&input_wav, spec).expect("create test wav");
            for i in 0..4800 {
                let sample = ((i as f32 / 100.0).sin() * 10000.0) as i16;
                writer.write_sample(sample).unwrap(); // L
                writer.write_sample(sample).unwrap(); // R
            }
            writer.finalize().unwrap();
        }

        // 转换重采样
        let converted = ensure_whisper_compatible_wav(&input_wav).expect("conversion should succeed");
        assert!(converted.exists());

        // 验证转换后的 WAV 规格必须严格是 16kHz 单声道 16-bit
        let reader = hound::WavReader::open(&converted).expect("open converted wav");
        let out_spec = reader.spec();
        assert_eq!(out_spec.channels, 1, "Must be Mono (1 channel)");
        assert_eq!(out_spec.sample_rate, 16000, "Must be 16000 Hz");
        assert_eq!(out_spec.bits_per_sample, 16, "Must be 16-bit");

        // 清理临时测试文件
        let _ = std::fs::remove_file(&input_wav);
        let _ = std::fs::remove_file(&converted);
    }
}

