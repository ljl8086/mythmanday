// Copyright (C) 2026 ljl8086 <ljl8086@gmail.com>. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (MythManDay)
// Version: V1.0
// Description: Multi-channel concurrent time-tracking & man-day billing engine.

pub mod client;
pub mod doc_parser;

pub use client::{
    download_whisper_model_file, find_whisper_binary, find_whisper_model, get_asr_runtime_status,
    get_whisper_models_dir, AiBatchParsedItem, AiConfig, AiError, AiSummarizer, AiTimeSlotOption,
    AiWorkEvaluationRequest, AiWorkEvaluationResult, AsrConfig, AsrRuntimeStatus,
    AvailableModelInfo, StructuredDeliverableReport, WhisperModelInfo, WorkCategory,
};
pub use doc_parser::{
    extract_document_text, extract_glossary_for_asr, is_document_file, is_image_file,
};
