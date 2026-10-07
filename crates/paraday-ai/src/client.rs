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
}
