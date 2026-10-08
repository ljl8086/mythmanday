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
    pub provider: String,
    pub api_base: String,
    pub api_key: String,
    pub model: String,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: "agy".to_string(),
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

fn find_binary(name: &str) -> std::path::PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        let p = std::path::PathBuf::from(home).join(".local").join("bin").join(name);
        if p.exists() {
            return p;
        }
    }
    std::path::PathBuf::from(name)
}

fn find_whisper_binary() -> Option<std::path::PathBuf> {
    let candidates = ["whisper", "whisper-cli", "whisper-cpp", "main"];
    for name in &candidates {
        if let Ok(home) = std::env::var("HOME") {
            let p = std::path::PathBuf::from(&home).join(".local").join("bin").join(name);
            if p.exists() {
                return Some(p);
            }
        }
        if let Ok(out) = std::process::Command::new("which").arg(name).output() {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !s.is_empty() {
                    return Some(std::path::PathBuf::from(s));
                }
            }
        }
    }
    None
}

impl AiSummarizer {
    pub fn new(config: AiConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// 统一调用 AI 提供商 (支持 agy CLI / codex CLI / 兼容 HTTP API)
    pub async fn execute_completion(&self, system_prompt: &str, user_content: &str) -> Result<String, AiError> {
        let provider = self.config.provider.trim().to_lowercase();
        if provider == "agy" {
            let full_prompt = format!("指令：{}\n\n内容：{}", system_prompt, user_content);
            let bin = find_binary("agy");
            let output = tokio::task::spawn_blocking(move || {
                std::process::Command::new(bin)
                    .arg("-p")
                    .arg(&full_prompt)
                    .output()
            })
            .await
            .map_err(|e| AiError::ParseError(format!("子线程调度失败: {}", e)))?
            .map_err(|e| AiError::ParseError(format!("本地启动 agy 失败: {}", e)))?;

            if !output.status.success() {
                let err = String::from_utf8_lossy(&output.stderr);
                return Err(AiError::ParseError(format!("agy 执行异常: {}", err)));
            }
            let res = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if res.is_empty() {
                return Err(AiError::ParseError("agy 返回结果为空".into()));
            }
            return Ok(res);
        } else if provider == "codex" {
            let full_prompt = format!("指令：{}\n\n内容：{}", system_prompt, user_content);
            let bin = find_binary("codex");
            let output = tokio::task::spawn_blocking(move || {
                std::process::Command::new(bin)
                    .arg("exec")
                    .arg(&full_prompt)
                    .output()
            })
            .await
            .map_err(|e| AiError::ParseError(format!("子线程调度失败: {}", e)))?
            .map_err(|e| AiError::ParseError(format!("本地启动 codex 失败: {}", e)))?;

            if !output.status.success() {
                let err = String::from_utf8_lossy(&output.stderr);
                return Err(AiError::ParseError(format!("codex 执行异常: {}", err)));
            }
            let res = String::from_utf8_lossy(&output.stdout).trim().to_string();
            return Ok(res);
        }

        // 默认走 HTTP API (兼容 OpenAI / DeepSeek / Ollama)
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
                    content: user_content.to_string(),
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
            .unwrap_or_else(|| "完成相关模块开发与需求对接。".to_string());

        Ok(result)
    }

    /// 测试 AI 连通性
    pub async fn test_connection(&self) -> Result<String, AiError> {
        self.execute_completion(
            "你是一个AI服务测试助手",
            "请用一句话回答：AI 服务通信测试成功，已准备就绪！",
        )
        .await
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

        let system_prompt = "你是一名资深的技术架构师与软件工程专家。请根据开发者在本次工时区间内的真实代码变动及工作便签，撰写一段专业、严谨、有交付价值的工作成果摘要。要求：\n1. 语言干练体面，突出业务交付成果与技术严谨性；\n2. 涵盖代码开发、需求对齐或联调要点；\n3. 输出 1~3 句话（100字左右），不带任何客套寒暄。";

        self.execute_completion(system_prompt, &prompt_content).await
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

        self.execute_completion(system_prompt, &prompt_content).await
    }

    /// 语音转文字处理（支持本地 Whisper CLI / agy CLI / codex CLI / 兼容 HTTP Whisper 接口）
    pub async fn transcribe_audio(&self, wav_path: &std::path::Path) -> Result<String, AiError> {
        if !wav_path.exists() {
            return Err(AiError::ParseError(format!("音频文件不存在: {}", wav_path.display())));
        }

        // 1. 如果本地系统安装了 whisper 命令行工具，优先尝试执行本地 whisper 提取
        if let Some(whisper_bin) = find_whisper_binary() {
            let wav_str = wav_path.to_string_lossy().to_string();
            let parent_dir = wav_path
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_string_lossy()
                .to_string();
            let res = tokio::task::spawn_blocking(move || {
                std::process::Command::new(&whisper_bin)
                    .arg(&wav_str)
                    .arg("--output_format")
                    .arg("txt")
                    .arg("--output_dir")
                    .arg(&parent_dir)
                    .output()
            })
            .await;
            if let Ok(Ok(out)) = res {
                if out.status.success() {
                    let txt_path = wav_path.with_extension("txt");
                    if let Ok(text) = std::fs::read_to_string(&txt_path) {
                        if !text.trim().is_empty() {
                            return Ok(text.trim().to_string());
                        }
                    }
                }
            }
        }

        let provider = self.config.provider.trim().to_lowercase();
        // 2. 如果使用本地 AI 引擎 (agy)，agy 原生具备多模态音频理解与语音转文字能力
        if provider == "agy" {
            let wav_str = wav_path.to_string_lossy().to_string();
            let bin = find_binary("agy");
            let prompt = format!(
                "请直接读取并聆听该本地音频文件：{}\n请直接转写该录音中说话人讲出的所有真实文字内容。要求：只输出转写后的纯中文/对话文字正文，不要输出任何解释说明、格式前缀或客套话。",
                wav_str
            );
            let output = tokio::task::spawn_blocking(move || {
                std::process::Command::new(bin)
                    .arg("-p")
                    .arg(&prompt)
                    .output()
            })
            .await
            .map_err(|e| AiError::ParseError(format!("子线程调度失败: {}", e)))?
            .map_err(|e| AiError::ParseError(format!("本地启动 agy 失败: {}", e)))?;

            if !output.status.success() {
                let err = String::from_utf8_lossy(&output.stderr);
                return Err(AiError::ParseError(format!("agy 转写异常: {}", err)));
            }
            let res = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if res.is_empty() {
                return Err(AiError::ParseError("agy 音频识别结果为空".into()));
            }
            return Ok(res);
        } else if provider == "codex" {
            let wav_str = wav_path.to_string_lossy().to_string();
            let bin = find_binary("codex");
            let prompt = format!(
                "请直接读取并聆听该本地音频文件：{}\n请直接转写该录音中的说话人实际语音内容，只输出文字文本：",
                wav_str
            );
            let output = tokio::task::spawn_blocking(move || {
                std::process::Command::new(bin)
                    .arg("exec")
                    .arg(&prompt)
                    .output()
            })
            .await
            .map_err(|e| AiError::ParseError(format!("子线程调度失败: {}", e)))?
            .map_err(|e| AiError::ParseError(format!("本地启动 codex 失败: {}", e)))?;

            if !output.status.success() {
                let err = String::from_utf8_lossy(&output.stderr);
                return Err(AiError::ParseError(format!("codex 执行异常: {}", err)));
            }
            let res = String::from_utf8_lossy(&output.stdout).trim().to_string();
            return Ok(res);
        }

        // 3. 走云端兼容 Whisper 标准的 HTTP API (/audio/transcriptions)
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
