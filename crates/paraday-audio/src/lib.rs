// Copyright (C) 2026 ParaDay Authors. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (ParaDay)
// Version: V1.0
// Description: Native microphone audio recording & acoustic processing engine.

pub mod recorder;

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
