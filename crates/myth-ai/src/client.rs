// Copyright (C) 2026 ljl8086 <ljl8086@gmail.com>. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (MythManDay)
// Version: V1.0
// Description: Multi-channel concurrent time-tracking & man-day billing engine.

use myth_core::models::QuickNote;
use myth_git::GitActivitySummary;
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
        let env_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
        let default_provider = if !env_key.is_empty() {
            "openai"
        } else {
            "codex"
        };
        Self {
            provider: default_provider.to_string(),
            api_base: std::env::var("OPENAI_API_BASE")
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
            api_key: env_key,
            model: std::env::var("OPENAI_MODEL")
                .unwrap_or_else(|_| "gpt-4o-mini".to_string()),
        }
    }
}

/// 标准化工作交付分类（用于商业对账单与 OpenAI 结构化审计输出）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkCategory {
    FeatureDevelopment,
    BugfixAndOptimization,
    ArchitectureAndReview,
    ClientAlignmentAndMeeting,
    TestingAndDevOps,
}

/// OpenAI 结构化交付物审计报告 (Structured Deliverable Report)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredDeliverableReport {
    /// 简洁的商业交付摘要（适合直接写入 Excel / Invoice 账单明细）
    pub executive_summary: String,
    /// 工作性质分类
    pub category: WorkCategory,
    /// 提炼出的关键交付物条目（Deliverable Bullet Points）
    pub deliverables: Vec<String>,
    /// 技术变更影响面评估（例如涉及的模块或架构层）
    pub technical_scope: String,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    format_type: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
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

        // 默认走 OpenAI 兼容 HTTP API (原生支持 OpenAI gpt-4o / gpt-4o-mini / codex，并兼容自定义端点)
        let api_key = if self.config.api_key.trim().is_empty() {
            std::env::var("OPENAI_API_KEY").unwrap_or_default()
        } else {
            self.config.api_key.clone()
        };
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
            max_tokens: 500,
            response_format: None,
        };

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
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

    /// 构建用于 AI 分析的项目活动上下文文本（含 Commit 列表、Diff 统计、语义级代码 Patch 与便签）
    pub fn build_session_context(
        project_name: &str,
        duration_hours: f64,
        git_summary: &GitActivitySummary,
        notes: &[QuickNote],
    ) -> String {
        let mut prompt_content = format!(
            "项目名称：{}\n投入工时：{:.2} 小时\n\n",
            project_name, duration_hours
        );

        if !git_summary.commits.is_empty() {
            prompt_content.push_str("【代码提交记录 (Git Commits)】：\n");
            for c in &git_summary.commits {
                prompt_content.push_str(&format!("- {}\n", c));
            }
            prompt_content.push('\n');
        }

        if !git_summary.diff_stat.is_empty() {
            prompt_content.push_str(&format!("【代码变更规模 (Diff Stat)】：\n{}\n\n", git_summary.diff_stat));
        }

        if !git_summary.diff_snippet.is_empty() {
            prompt_content.push_str(&format!(
                "【语义级代码变更片段 (Git Diff Patch)】：\n```diff\n{}\n```\n\n",
                git_summary.diff_snippet
            ));
        }

        if !notes.is_empty() {
            prompt_content.push_str("【离线沟通/工作便签】：\n");
            for n in notes {
                prompt_content.push_str(&format!("- {}\n", n.content));
            }
            prompt_content.push('\n');
        }

        prompt_content
    }

    /// 结合 Git 活动（含语义级代码 Diff）与便签，提炼生成商业客户交付摘要
    pub async fn summarize_session(
        &self,
        project_name: &str,
        duration_hours: f64,
        git_summary: &GitActivitySummary,
        notes: &[QuickNote],
    ) -> Result<String, AiError> {
        let prompt_content = Self::build_session_context(project_name, duration_hours, git_summary, notes);

        let system_prompt = "你是一名资深的技术架构师与软件工程专家。请根据开发者在本次工时区间内的真实代码变动（含 Git Commits 与代码 Diff 片段）及工作便签，撰写一段专业、严谨、有交付价值的工作成果摘要。要求：\n1. 语言干练体面，突出业务交付成果与技术严谨性（即使 commit 信息简略，也请从代码 Diff 中提炼出真实的业务/模块改动）；\n2. 涵盖代码开发、需求对齐或联调要点；\n3. 输出 1~3 句话（100字左右），不带任何客套寒暄。";

        self.execute_completion(system_prompt, &prompt_content).await
    }

    /// 生成 OpenAI 结构化交付物审计报告 (Structured JSON Output)
    pub async fn summarize_session_structured(
        &self,
        project_name: &str,
        duration_hours: f64,
        git_summary: &GitActivitySummary,
        notes: &[QuickNote],
    ) -> Result<StructuredDeliverableReport, AiError> {
        let prompt_content = Self::build_session_context(project_name, duration_hours, git_summary, notes);
        let system_prompt = r#"You are a senior software engineering auditor. Analyze the developer's Git commits, code diff patches, and session notes, then output ONLY a valid JSON object matching this schema:
{
  "executive_summary": "Concise 1-2 sentence client-facing billing summary",
  "category": "feature_development" | "bugfix_and_optimization" | "architecture_and_review" | "client_alignment_and_meeting" | "testing_and_dev_ops",
  "deliverables": ["bullet 1", "bullet 2"],
  "technical_scope": "Affected crates/modules/components"
}"#;

        let raw = self.execute_completion(system_prompt, &prompt_content).await?;
        let cleaned = raw
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        serde_json::from_str::<StructuredDeliverableReport>(cleaned)
            .map_err(|e| AiError::ParseError(format!("Failed to parse structured deliverable JSON: {} (raw: {})", e, cleaned)))
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

        let api_key = if self.config.api_key.trim().is_empty() {
            std::env::var("OPENAI_API_KEY").unwrap_or_default()
        } else {
            self.config.api_key.clone()
        };
        let url = format!("{}/audio/transcriptions", self.config.api_base.trim_end_matches('/'));

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
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

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_default_ai_config_uses_openai_standard() {
        let cfg = AiConfig::default();
        assert_eq!(cfg.api_base, "https://api.openai.com/v1");
        assert_eq!(cfg.model, "gpt-4o-mini");
    }

    #[test]
    fn test_build_session_context_includes_diff_snippet() {
        let git_summary = GitActivitySummary {
            commits: vec!["feat: implement multi-channel billing [a1b2c3d]".to_string()],
            diff_stat: "2 files changed, 45 insertions(+)".to_string(),
            has_uncommitted_changes: true,
            uncommitted_files: vec!["M crates/myth-ai/src/client.rs".to_string()],
            diff_snippet: "+ pub struct StructuredDeliverableReport".to_string(),
        };
        let notes = vec![QuickNote::new(
            Uuid::new_v4(),
            "Aligned API schema with client",
        )];

        let ctx = AiSummarizer::build_session_context("MythManDay Core", 2.5, &git_summary, &notes);
        assert!(ctx.contains("MythManDay Core"));
        assert!(ctx.contains("feat: implement multi-channel billing"));
        assert!(ctx.contains("+ pub struct StructuredDeliverableReport"));
        assert!(ctx.contains("Aligned API schema with client"));
    }

    #[test]
    fn test_structured_deliverable_report_deserialization() {
        let sample_json = r#"{
            "executive_summary": "Implemented OpenAI structured deliverable pipeline and semantic Git diff extraction.",
            "category": "feature_development",
            "deliverables": [
                "Added semantic diff patch extraction in myth-git",
                "Integrated OpenAI gpt-4o-mini structured JSON auditing in myth-ai"
            ],
            "technical_scope": "crates/myth-ai, crates/myth-git"
        }"#;

        let report: StructuredDeliverableReport = serde_json::from_str(sample_json).unwrap();
        assert_eq!(report.category, WorkCategory::FeatureDevelopment);
        assert_eq!(report.deliverables.len(), 2);
        assert!(report.executive_summary.contains("OpenAI structured deliverable"));
    }
}

