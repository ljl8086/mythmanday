// Copyright (C) 2026 ljl8086 <ljl8086@gmail.com>. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (MythManDay)
// Version: V1.0
// Description: Native microphone audio recording & acoustic processing engine.

pub mod player;
pub mod recorder;

pub use player::AudioPlayer;
pub use recorder::{AudioError, AudioRecorder};

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
}
