// Copyright (C) 2026 ParaDay Authors. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (ParaDay)
// Version: V1.0
// Description: Multi-channel concurrent time-tracking & man-day billing engine.

use paraday_core::models::QuickNote;
use paraday_git::GitActivitySummary;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AiError {
    #[error("Network error calling AI service: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("API returned error: status {0}, message {1}")]
    ApiError(u16, String),
    #[error("Failed to parse response: {0}")]
    ParseError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub api_base: String,
    pub api_key: String,
    pub model: String,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            api_base: "https://api.deepseek.com/v1".to_string(),
            api_key: "".to_string(),
            model: "deepseek-chat".to_string(),
        }
    }
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
}

#[derive(Deserialize)]
struct ChatResponseMessage {
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

pub struct AiSummarizer {
    config: AiConfig,
    client: reqwest::Client,
}

impl AiSummarizer {
    pub fn new(config: AiConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// 结合 Git 活动与便签，提炼生成商业客户交付摘要
    pub async fn summarize_session(
        &self,
        project_name: &str,
        duration_hours: f64,
        git_summary: &GitActivitySummary,
        notes: &[QuickNote],
    ) -> Result<String, AiError> {
        let mut prompt_content = format!(
            "项目名称：{}\n投入工时：{:.2} 小时\n\n",
            project_name, duration_hours
        );

        if !git_summary.commits.is_empty() {
            prompt_content.push_str("【代码提交记录】：\n");
            for c in &git_summary.commits {
                prompt_content.push_str(&format!("- {}\n", c));
            }
            prompt_content.push('\n');
        }

        if !git_summary.diff_stat.is_empty() {
            prompt_content.push_str(&format!("【代码变更规模】：\n{}\n\n", git_summary.diff_stat));
        }

        if !notes.is_empty() {
            prompt_content.push_str("【离线沟通/工作便签】：\n");
            for n in notes {
                prompt_content.push_str(&format!("- {}\n", n.content));
            }
            prompt_content.push('\n');
        }

        let system_prompt = "你是一名资深的技术架构师与软件工程专家。请根据开发者在本次工时区间内的真实代码变动及工作便签，为甲方客户撰写一段专业、严谨、有交付价值的工作成果摘要（用于对账单与周报）。要求：\n1. 语言干练体面，突出业务交付成果与技术严谨性；\n2. 涵盖代码开发、需求对齐或联调要点；\n3. 输出 1~3 句话（100字左右），不带任何客套寒暄。";

        let url = format!("{}/chat/completions", self.config.api_base.trim_end_matches('/'));
        let request_body = ChatRequest {
            model: self.config.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: prompt_content,
                },
            ],
            temperature: 0.3,
            max_tokens: 300,
        };

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&request_body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let err_text = resp.text().await.unwrap_or_default();
            return Err(AiError::ApiError(status, err_text));
        }

        let parsed: ChatResponse = resp.json().await?;
        let result = parsed
            .choices
            .first()
            .map(|c| c.message.content.trim().to_string())
            .unwrap_or_else(|| "完成相关模块开发与需求对接。".to_string());

        Ok(result)
    }

    /// 针对会议/技术对齐/客户沟通，结合会议议题、讨论要点及录音留存信息，提炼商业级会议纪要
    pub async fn summarize_meeting(
        &self,
        meeting_topic: &str,
        duration_hours: f64,
        notes: &[QuickNote],
        audio_file_info: Option<&str>,
        audio_transcript: Option<&str>,
    ) -> Result<String, AiError> {
        let mut prompt_content = format!(
            "会议议题：{}\n会议时长：{:.2} 小时\n\n",
            meeting_topic, duration_hours
        );

        if let Some(audio_info) = audio_file_info {
            prompt_content.push_str(&format!("【音频留存凭证】：{}\n\n", audio_info));
        }

        if let Some(transcript) = audio_transcript {
            if !transcript.trim().is_empty() {
                prompt_content.push_str(&format!("【会议录音全量转写文稿】：\n{}\n\n", transcript));
            }
        }

        if !notes.is_empty() {
            prompt_content.push_str("【会议要点记录/即时速记】：\n");
            for n in notes {
                prompt_content.push_str(&format!("- {}\n", n.content));
            }
            prompt_content.push('\n');
        }

        let system_prompt = "你是一名资深的技术架构师与项目管理专家。请根据本次会议的议题、沟通时长、速记要点及现场录音转写文稿，撰写一份条理清晰、专业体面的会议交付纪要。要求：\n1. 结构包含：【核心议题】、【达成共识与技术决议】、【后续跟进待办】；\n2. 语言干练严谨，适合呈报给客户或项目团队作为工时交付凭证；\n3. 控制在 180 字以内，不带任何客套寒暄。";

        let url = format!("{}/chat/completions", self.config.api_base.trim_end_matches('/'));
        let request_body = ChatRequest {
            model: self.config.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: prompt_content,
                },
            ],
            temperature: 0.3,
            max_tokens: 400,
        };

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&request_body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let err_text = resp.text().await.unwrap_or_default();
            return Err(AiError::ApiError(status, err_text));
        }

        let parsed: ChatResponse = resp.json().await?;
        let result = parsed
            .choices
            .first()
            .map(|c| c.message.content.trim().to_string())
            .unwrap_or_else(|| format!("完成【{}】沟通对接，各方已达成一致决策。", meeting_topic));

        Ok(result)
    }

    /// 调用兼容 Whisper 标准规范的语音转文字接口 (/audio/transcriptions)
    pub async fn transcribe_audio(&self, wav_path: &std::path::Path) -> Result<String, AiError> {
        if !wav_path.exists() {
            return Err(AiError::ParseError(format!("音频文件不存在: {}", wav_path.display())));
        }

        let file_bytes = tokio::fs::read(wav_path)
            .await
            .map_err(|e| AiError::ParseError(e.to_string()))?;
        let filename = wav_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("audio.wav")
            .to_string();

        let part = reqwest::multipart::Part::bytes(file_bytes)
            .file_name(filename)
            .mime_str("audio/wav")
            .map_err(|e| AiError::ParseError(e.to_string()))?;

        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model", "whisper-1")
            .text("response_format", "text");

        let url = format!("{}/audio/transcriptions", self.config.api_base.trim_end_matches('/'));

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .multipart(form)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let err_text = resp.text().await.unwrap_or_default();
            return Err(AiError::ApiError(status, err_text));
        }

        let text = resp.text().await.map_err(AiError::Reqwest)?;
        Ok(text.trim().to_string())
    }
}
