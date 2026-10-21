// Copyright (C) 2026 ljl8086 <ljl8086@gmail.com>. All Rights Reserved.
//
// Software Name: 平行人天多项目工时管理与交付结算软件 (MythManDay)
// Version: V1.0
// Description: Multi-channel concurrent time-tracking & man-day billing engine.

use myth_core::models::{MeetingMaterial, QuickNote};
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
pub struct AsrConfig {
    #[serde(default = "default_asr_mode")]
    pub mode: String, // "local" | "cloud" | "agent"
    #[serde(default)]
    pub api_base: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub local_model: Option<String>,
    #[serde(default)]
    pub local_bin_path: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
}

fn default_asr_mode() -> String {
    "local".to_string()
}

impl Default for AsrConfig {
    fn default() -> Self {
        Self {
            mode: "local".to_string(),
            api_base: None,
            api_key: None,
            model: Some("whisper-1".to_string()),
            local_model: Some("ggml-base.bin".to_string()),
            local_bin_path: None,
            language: Some("zh".to_string()),
            prompt: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub provider: String,
    pub api_base: String,
    pub api_key: String,
    pub model: String,
    #[serde(default)]
    pub proxy: Option<String>,
    #[serde(default)]
    pub asr: AsrConfig,
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
            model: std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string()),
            proxy: std::env::var("HTTP_PROXY")
                .or_else(|_| std::env::var("HTTPS_PROXY"))
                .or_else(|_| std::env::var("ALL_PROXY"))
                .ok(),
            asr: AsrConfig::default(),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiTimeSlotOption {
    pub label: String,
    pub start: String,
    pub end: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiWorkEvaluationRequest {
    pub project_name: String,
    pub client_name: String,
    pub date: String,
    pub commits: Vec<String>,
    pub diff_stat: Option<String>,
    pub earliest_commit_time: Option<String>,
    pub latest_commit_time: Option<String>,
    pub preferred_start_time: Option<String>,
    pub preferred_hours: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiBatchParsedItem {
    pub date: String,
    pub start_time: String,
    pub end_time: String,
    pub summary: String,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiWorkEvaluationResult {
    pub suggested_start_time: String,
    pub suggested_end_time: String,
    pub estimated_hours: f64,
    pub reason: String,
    pub deliverable_summary: String,
    pub slots: Vec<AiTimeSlotOption>,
}

#[derive(Serialize)]
pub struct ChatMessage {
    role: String,
    content: MessageContent,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

impl From<String> for MessageContent {
    fn from(s: String) -> Self {
        MessageContent::Text(s)
    }
}

impl From<&str> for MessageContent {
    fn from(s: &str) -> Self {
        MessageContent::Text(s.to_string())
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum ContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrlPayload },
}

#[derive(Serialize, Clone, Debug)]
pub struct ImageUrlPayload {
    pub url: String,
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
        let p = std::path::PathBuf::from(home)
            .join(".local")
            .join("bin")
            .join(name);
        if p.exists() {
            return p;
        }
    }
    std::path::PathBuf::from(name)
}

pub fn get_user_home_dir() -> Option<std::path::PathBuf> {
    if let Ok(home) = std::env::var("HOME") {
        if !home.trim().is_empty() {
            return Some(std::path::PathBuf::from(home));
        }
    }
    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        if !userprofile.trim().is_empty() {
            return Some(std::path::PathBuf::from(userprofile));
        }
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        if !appdata.trim().is_empty() {
            return Some(std::path::PathBuf::from(appdata));
        }
    }
    None
}

pub fn find_whisper_binary(custom_path: Option<&str>) -> Option<std::path::PathBuf> {
    if let Some(cp) = custom_path {
        let p = std::path::PathBuf::from(cp);
        if p.exists() && p.is_file() {
            return Some(p);
        }
    }
    let candidates = if cfg!(windows) {
        vec![
            "whisper-cli.exe",
            "whisper.exe",
            "whisper-cpp.exe",
            "main.exe",
            "whisper-cli",
            "whisper",
        ]
    } else {
        vec!["whisper-cli", "whisper", "whisper-cpp", "main"]
    };

    if let Some(home_p) = get_user_home_dir() {
        // 优先应用配置内置目录 ~/.config/mythmanday/bin/
        for name in &candidates {
            let p = home_p
                .join(".config")
                .join("mythmanday")
                .join("bin")
                .join(name);
            if p.exists() {
                return Some(p);
            }
        }
        // 其次 ~/.local/bin/
        for name in &candidates {
            let p = home_p.join(".local").join("bin").join(name);
            if p.exists() {
                return Some(p);
            }
        }
    }
    // 最后检测系统 PATH (跨平台适配：Windows 使用 where，Unix 使用 which)
    let lookup_cmd = if cfg!(windows) { "where" } else { "which" };
    for name in &candidates {
        if let Ok(out) = std::process::Command::new(lookup_cmd).arg(name).output() {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout);
                if let Some(first_line) = s.lines().map(|l| l.trim()).find(|l| !l.is_empty()) {
                    let pb = std::path::PathBuf::from(first_line);
                    if pb.exists() {
                        return Some(pb);
                    }
                }
            }
        }
    }
    None
}

pub fn find_whisper_model(custom_model: Option<&str>) -> Option<std::path::PathBuf> {
    let mut search_dirs = Vec::new();
    if let Some(home) = get_user_home_dir() {
        search_dirs.push(home.join(".config").join("mythmanday").join("models"));
        search_dirs.push(
            home.join(".local")
                .join("share")
                .join("mythmanday")
                .join("models"),
        );
    }
    search_dirs.push(std::path::PathBuf::from("./models"));

    if let Some(cm) = custom_model {
        let p = std::path::PathBuf::from(cm);
        if p.is_absolute() && p.exists() {
            return Some(p);
        }
        for dir in &search_dirs {
            let candidate = dir.join(cm);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    // 默认优先级查找模型
    let default_models = [
        "ggml-base.bin",
        "ggml-tiny.bin",
        "ggml-small.bin",
        "ggml-medium.bin",
        "ggml-large-v3.bin",
    ];
    for dir in &search_dirs {
        for m in &default_models {
            let p = dir.join(m);
            if p.exists() {
                return Some(p);
            }
        }
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("bin") {
                    return Some(path);
                }
            }
        }
    }

    None
}

impl AiSummarizer {
    pub fn new(config: AiConfig) -> Self {
        let mut builder = reqwest::Client::builder();
        if let Some(ref proxy_url) = config.proxy {
            let p_trim = proxy_url.trim();
            if !p_trim.is_empty() {
                if let Ok(p) = reqwest::Proxy::all(p_trim) {
                    builder = builder.proxy(p);
                } else {
                    eprintln!("无法解析指定的代理服务器地址: {}", p_trim);
                }
            }
        }
        let client = builder.build().unwrap_or_else(|_| reqwest::Client::new());
        Self { config, client }
    }

    /// 统一调用 AI 提供商（支持多模态参考图片输入）
    pub async fn execute_completion_multimodal(
        &self,
        system_prompt: &str,
        user_content: &str,
        image_paths: &[&std::path::Path],
    ) -> Result<String, AiError> {
        let provider = self.config.provider.trim().to_lowercase();
        let proxy_opt = self.config.proxy.clone().filter(|p| !p.trim().is_empty());
        if provider == "agy" {
            let mut full_prompt = format!("指令：{}\n\n内容：{}", system_prompt, user_content);
            if !image_paths.is_empty() {
                full_prompt.push_str(
                    "\n\n【关联图片资料本地存储绝对路径（请直接查看解析图纸/截图）】：\n",
                );
                for p in image_paths {
                    full_prompt.push_str(&format!("- {}\n", p.display()));
                }
            }
            let bin = find_binary("agy");
            let output = tokio::task::spawn_blocking(move || {
                let mut cmd = std::process::Command::new(bin);
                cmd.arg("-p")
                    .arg(&full_prompt)
                    .arg("--dangerously-skip-permissions");
                if let Some(ref px) = proxy_opt {
                    cmd.env("HTTP_PROXY", px)
                        .env("HTTPS_PROXY", px)
                        .env("ALL_PROXY", px)
                        .env("http_proxy", px)
                        .env("https_proxy", px)
                        .env("all_proxy", px);
                }
                cmd.output()
            })
            .await
            .map_err(|e| AiError::ParseError(format!("子线程调度失败: {}", e)))?
            .map_err(|e| AiError::ParseError(format!("本地启动 agy 失败: {}", e)))?;

            if !output.status.success() {
                let err = String::from_utf8_lossy(&output.stderr);
                return Err(AiError::ParseError(format!("agy 执行异常: {}", err)));
            }
            let res = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let cleaned = clean_repetitive_text(&res);
            if cleaned.is_empty() {
                return Err(AiError::ParseError("agy 返回结果为空".into()));
            }
            return Ok(cleaned);
        } else if provider == "codex" {
            let mut full_prompt = format!("指令：{}\n\n内容：{}", system_prompt, user_content);
            if !image_paths.is_empty() {
                full_prompt.push_str("\n\n【关联图片资料本地路径】：\n");
                for p in image_paths {
                    full_prompt.push_str(&format!("- {}\n", p.display()));
                }
            }
            let bin = find_binary("codex");
            let output = tokio::task::spawn_blocking(move || {
                let mut cmd = std::process::Command::new(bin);
                cmd.arg("exec").arg(&full_prompt);
                if let Some(ref px) = proxy_opt {
                    cmd.env("HTTP_PROXY", px)
                        .env("HTTPS_PROXY", px)
                        .env("ALL_PROXY", px)
                        .env("http_proxy", px)
                        .env("https_proxy", px)
                        .env("all_proxy", px);
                }
                cmd.output()
            })
            .await
            .map_err(|e| AiError::ParseError(format!("子线程调度失败: {}", e)))?
            .map_err(|e| AiError::ParseError(format!("本地启动 codex 失败: {}", e)))?;

            if !output.status.success() {
                let err = String::from_utf8_lossy(&output.stderr);
                return Err(AiError::ParseError(format!("codex 执行异常: {}", err)));
            }
            let res = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let cleaned = clean_repetitive_text(&res);
            if cleaned.is_empty() {
                return Err(AiError::ParseError("codex 返回结果为空".into()));
            }
            return Ok(cleaned);
        }

        // 默认走 OpenAI 兼容 HTTP API (原生支持 OpenAI gpt-4o / gpt-4o-mini / Vision)
        let api_key = if self.config.api_key.trim().is_empty() {
            std::env::var("OPENAI_API_KEY").unwrap_or_default()
        } else {
            self.config.api_key.clone()
        };
        let url = format!(
            "{}/chat/completions",
            self.config.api_base.trim_end_matches('/')
        );

        let user_content_payload = if image_paths.is_empty() {
            MessageContent::Text(user_content.to_string())
        } else {
            let mut parts = vec![ContentPart::Text {
                text: user_content.to_string(),
            }];
            for img_p in image_paths {
                if let Ok(bytes) = std::fs::read(img_p) {
                    if bytes.len() <= 10 * 1024 * 1024 {
                        use base64::Engine;
                        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                        let ext = img_p
                            .extension()
                            .and_then(|s| s.to_str())
                            .unwrap_or("png")
                            .to_lowercase();
                        let mime = match ext.as_str() {
                            "jpg" | "jpeg" => "image/jpeg",
                            "png" => "image/png",
                            "webp" => "image/webp",
                            "gif" => "image/gif",
                            _ => "image/png",
                        };
                        parts.push(ContentPart::ImageUrl {
                            image_url: ImageUrlPayload {
                                url: format!("data:{};base64,{}", mime, b64),
                            },
                        });
                    }
                }
            }
            MessageContent::Parts(parts)
        };

        let request_body = ChatRequest {
            model: self.config.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: MessageContent::Text(system_prompt.to_string()),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user_content_payload,
                },
            ],
            temperature: 0.3,
            max_tokens: 3000,
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
            .map(|c| clean_repetitive_text(&c.message.content))
            .unwrap_or_else(|| "完成相关模块开发与需求对接。".to_string());

        Ok(result)
    }

    /// 保持向前兼容的纯文本执行接口
    pub async fn execute_completion(
        &self,
        system_prompt: &str,
        user_content: &str,
    ) -> Result<String, AiError> {
        self.execute_completion_multimodal(system_prompt, user_content, &[])
            .await
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
            prompt_content.push_str(&format!(
                "【代码变更规模 (Diff Stat)】：\n{}\n\n",
                git_summary.diff_stat
            ));
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
        let prompt_content =
            Self::build_session_context(project_name, duration_hours, git_summary, notes);

        let system_prompt = "你是一名资深的技术架构师与软件工程专家。请根据开发者在本次工时区间内的真实代码变动（含 Git Commits 与代码 Diff 片段）及工作便签，撰写一段专业、严谨、有交付价值的工作成果摘要。要求：\n1. 语言干练体面，突出业务交付成果与技术严谨性（即使 commit 信息简略，也请从代码 Diff 中提炼出真实的业务/模块改动）；\n2. 涵盖代码开发、需求对齐或联调要点；\n3. 输出 1~3 句话（100字左右），不带任何客套寒暄。";

        self.execute_completion(system_prompt, &prompt_content)
            .await
    }

    /// 生成 OpenAI 结构化交付物审计报告 (Structured JSON Output)
    pub async fn summarize_session_structured(
        &self,
        project_name: &str,
        duration_hours: f64,
        git_summary: &GitActivitySummary,
        notes: &[QuickNote],
    ) -> Result<StructuredDeliverableReport, AiError> {
        let prompt_content =
            Self::build_session_context(project_name, duration_hours, git_summary, notes);
        let system_prompt = r#"You are a senior software engineering auditor. Analyze the developer's Git commits, code diff patches, and session notes, then output ONLY a valid JSON object matching this schema:
{
  "executive_summary": "Concise 1-2 sentence client-facing billing summary",
  "category": "feature_development" | "bugfix_and_optimization" | "architecture_and_review" | "client_alignment_and_meeting" | "testing_and_dev_ops",
  "deliverables": ["bullet 1", "bullet 2"],
  "technical_scope": "Affected crates/modules/components"
}"#;

        let raw = self
            .execute_completion(system_prompt, &prompt_content)
            .await?;
        let cleaned = raw
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        serde_json::from_str::<StructuredDeliverableReport>(cleaned).map_err(|e| {
            AiError::ParseError(format!(
                "Failed to parse structured deliverable JSON: {} (raw: {})",
                e, cleaned
            ))
        })
    }

    /// 评估 Git 活动与代码变更规模，智能推算研发时段（开始/结束时间）与商业交付说明
    pub async fn evaluate_work_timeslot(
        &self,
        req: &AiWorkEvaluationRequest,
    ) -> Result<AiWorkEvaluationResult, AiError> {
        let system_prompt = r#"你是一名资深的技术架构师与软件工程工时审计专家。开发者正在补录某天的研发工时，由于 Git 仅记录代码最终 Commit 提交时刻，而没有记录实际开始编写代码的时间。
请根据该开发者在当天的真实 Git 提交记录、提交时刻以及代码变更规模（Diff Stat），推理并评估其合理的研发时段（开始时间、结束时间）及商业交付成果说明。

【推理指引】：
1. 若当天有多个提交跨越上午和下午，或者提交涉及多个文件/数百行代码（核心业务拆分/重构），通常属于全天专注研发，开始时间对齐至上班时间（如 09:30 或 09:00），结束时间为最晚提交时间或下班时间（如 18:30）。
2. 若提交集中在下午（如 16:00~17:00 提交大功能），通常是白天连续开发后统一提交，建议全天时段（09:30~18:30）；若为轻量改动则建议下午半天（14:00~17:30）或提交前推 2 小时。
3. 若提交在上午，建议推算为上午半天时段（如 09:30~12:30）。

【交付内容说明撰写标准（必须丰富饱满、专业高端、具备高商业价值）】：
- 语调与格局：必须具备资深架构师/研发专家的高维视野，使用严谨、专业、规范的工程术语（如：底层架构解耦、领域模型沉淀、多维精算引擎、全链路闭环、鲁棒性校验、高内聚低耦合设计等）。
- 条目式结构化：采用清晰的分项序号（1. 2. 3. 4. ...），至少提炼出 3~5 项核心交付物，每项包含「行动动词 + 业务技术模块 + 核心机制/技术手段 + 带来的商业与质量价值」。
- 严禁空洞与单句：禁止输出一两句简单粗糙的话，要将代码变更展开为：
  ① 业务架构与领域模型实现（如核心业务逻辑、多维度计算引擎、状态机与链路闭环）；
  ② 性能优化与鲁棒性防护（如导入导出批量处理、高并发防护、多层边界条件校验、约束规则拦截）；
  ③ 交互体验与标准化工程落地（如标准化模板沉淀、接口规范统一、组件解耦、审计追踪）；
  ④ 质量验证与工程交付（如全场景回归测试、联调验证、数据一致性与合规性保障）。

必须且只能输出严格的 JSON 格式（不要包含任何 markdown 解释）：
{
  "suggested_start_time": "HH:MM",
  "suggested_end_time": "HH:MM",
  "estimated_hours": 8.0,
  "reason": "简短的 AI 评估依据（例如：检测到核心模块深度重构及导入模板改动，涉及多层代码影响，判定为全天专注研发投入，开始时间对齐至 09:30）",
  "deliverable_summary": "1. 【架构重构与领域设计】完成年度研发费用规划核心业务引擎开发，落地人工、设备、材料等多维度动态核算模型，实现业务领域深度解耦。\n2. 【核心服务与批量流转】研发预计月薪维护全量功能，支持基于流式处理的高性能 Excel 批量导入/导出体系，优化千万级数据吞吐性能。\n3. 【约束机制与风控引擎】构建预算编制的项目阶段资源配置动态约束机制，嵌入多维阈值校验与比例上限防护，杜绝越权与预算超支风险。\n4. 【健壮性保障与数据合规】搭建全链路输入校验与合规审查逻辑，完善异常回滚与审计溯源能力，确保财务预算核算的准确性与审计合规性。\n5. 【工程交付与联调验证】完成前后端关键交互联调及全场景边界回归测试，提升数据录入体验与系统整体工程交付质量。",
  "slots": [
    { "label": "全天研发", "start": "09:30", "end": "18:30" },
    { "label": "下午集中", "start": "13:30", "end": "18:00" },
    { "label": "提交前推", "start": "14:30", "end": "17:00" }
  ]
}"#;

        let mut user_content = format!(
            "【项目名称】：{} ({})\n【工作日期】：{}\n",
            req.project_name, req.client_name, req.date
        );
        if let Some(ref stat) = req.diff_stat {
            user_content.push_str(&format!("【代码变更规模】：{}\n", stat));
        }
        if let Some(ref e) = req.earliest_commit_time {
            user_content.push_str(&format!("【最早提交时间】：{}\n", e));
        }
        if let Some(ref l) = req.latest_commit_time {
            user_content.push_str(&format!("【最晚提交时间】：{}\n", l));
        }
        user_content.push_str("【Git 提交记录清单】：\n");
        for c in &req.commits {
            user_content.push_str(&format!("- {}\n", c));
        }

        let raw = self
            .execute_completion(system_prompt, &user_content)
            .await?;
        let cleaned = raw
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        let json_str = if let (Some(start), Some(end)) = (cleaned.find('{'), cleaned.rfind('}')) {
            if start < end {
                &cleaned[start..=end]
            } else {
                cleaned
            }
        } else {
            cleaned
        };

        serde_json::from_str::<AiWorkEvaluationResult>(json_str).map_err(|e| {
            AiError::ParseError(format!("解析 AI 响应失败: {} (raw: {})", e, json_str))
        })
    }

    /// 解析用户输入的第三方 AI 智能体总结、聊天纪要或一段时期的自由文本记录，
    /// 自动提取结构化的批量补录工时记录条目 (按日期、时段、高质量条目化交付物)
    pub async fn parse_custom_agent_summary(
        &self,
        project_name: &str,
        client_name: &str,
        raw_text: &str,
    ) -> Result<Vec<AiBatchParsedItem>, AiError> {
        let system_prompt = r#"你是一名专业的高级项目经理与软件工程工时审计员。用户向你提供了一段由第三方 AI 智能体生成的工作总结、日报周报、或者一段时间内的研发进度纪要文本。
请将该文本解析为多条按天划分的「结构化工时补录数据」。

【解析与提炼规则】：
1. 识别并归集每个日期的工作（日期格式必须统一标准化为 YYYY-MM-DD，若文本中只有“10月12日”或“周一”，根据上下文或当前年份 2026 年合理推算补全）。
2. 合理推断每个工作日的研发起止时间（若文本提及半天则如 14:00~18:30，全天则 09:30~18:30，格式为 HH:MM）。
3. 工作交付物（summary）必须严格遵循高大上的专业「条目式」清单（1. 2. 3. ...），语言专业、格局高远，体现架构重构、服务设计、约束风控、鲁棒性与工程交付等高维价值。

必须且只能输出严格的 JSON 数组（不要包含任何 markdown 解释）：
[
  {
    "date": "2026-10-12",
    "start_time": "09:30",
    "end_time": "18:30",
    "summary": "1. 【领域模型重构】完成设备与科研档案解耦\n2. 【批处理流转】落地批量数据模板导入与字段动态呈现\n3. 【工程交付】全链路交互联调及数据合规性校验",
    "reason": "AI 智能体总结解析（全天重点研发）"
  }
]"#;

        let user_content = format!(
            "【关联项目】：{} ({})\n【待解析的智能体工作总结/自由文本】：\n{}\n",
            project_name, client_name, raw_text
        );

        let raw = self
            .execute_completion(system_prompt, &user_content)
            .await?;
        let cleaned = raw
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        let json_str = if let (Some(start), Some(end)) = (cleaned.find('['), cleaned.rfind(']')) {
            if start < end {
                &cleaned[start..=end]
            } else {
                cleaned
            }
        } else {
            cleaned
        };

        serde_json::from_str::<Vec<AiBatchParsedItem>>(json_str).map_err(|e| {
            AiError::ParseError(format!(
                "解析批量智能总结 JSON 数组失败: {} (raw: {})",
                e, json_str
            ))
        })
    }

    /// 针对会议/技术对齐/客户沟通，结合会议议题、讨论要点、现场录音及客户参考资料（文档+图片），提炼真实客观的会议纪要
    pub async fn summarize_meeting(
        &self,
        meeting_topic: &str,
        duration_hours: f64,
        notes: &[QuickNote],
        materials: &[MeetingMaterial],
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

        let clean_transcript = audio_transcript
            .map(|t| clean_repetitive_text(t))
            .unwrap_or_default();

        let has_transcript = !clean_transcript.trim().is_empty();
        if has_transcript {
            prompt_content.push_str(&format!(
                "【现场录音文字转写】：\n{}\n\n",
                clean_transcript.trim()
            ));
        }

        if !notes.is_empty() {
            prompt_content.push_str("【会议要点记录/即时速记】：\n");
            for n in notes {
                prompt_content.push_str(&format!("- {}\n", n.content));
            }
            prompt_content.push('\n');
        }

        let mut image_paths: Vec<&std::path::Path> = Vec::new();
        if !materials.is_empty() {
            prompt_content.push_str("【客户提供的参考文档与图片资料】：\n");
            for m in materials {
                if m.file_type == "image" {
                    let p = std::path::Path::new(&m.file_path);
                    if p.exists() {
                        image_paths.push(p);
                    }
                    prompt_content.push_str(&format!(
                        "- 附图资料：《{}》（客户现场发送的图纸/截图/白板图）\n",
                        m.file_name
                    ));
                } else {
                    prompt_content.push_str(&format!("- 文档资料：《{}》\n", m.file_name));
                    if let Some(ref txt) = m.extracted_text {
                        if !txt.trim().is_empty() {
                            prompt_content
                                .push_str(&format!("  [提取内容摘要/正文]：\n{}\n\n", txt.trim()));
                        }
                    }
                }
            }
            prompt_content.push('\n');
        }

        if !has_transcript && notes.is_empty() && materials.is_empty() {
            return Ok(format!(
                "【会议纪要：{}】\n本次会议时长 {:.2} 小时。未检测到录音转写文稿、速记要点或参考资料，建议相关人员手动补充会议结论与待办事项。",
                meeting_topic, duration_hours
            ));
        }

        let system_prompt = "你是一名资深的技术架构师与项目管理专家。请根据本次会议的议题、沟通时长、速记要点、现场录音转写文稿，以及客户在会议期间发送的参考文档与图片资料，撰写一份条理清晰、真实严谨的高质量会议纪要。\n\
        严格遵循以下原则：\n\
        1. 结合参考资料与录音交叉核实：重点参考客户提供的文档规范、系统截图与图纸，对录音中提及的技术名词、模块代号、数据指标、业务流程进行精准校对与补全，纠正口语转写的同音错别字，消除口语指代模糊（如“看这个图”、“按刚才发的文件”）。\n\
        2. 真实客观第一原则：必须完全且仅基于实际会议讨论内容、录音文稿与客户参考资料进行提炼，严禁凭空捏造未在讨论或资料中出现的任何业务功能、架构决议或流程！\n\
        3. 智能术语与同音字纠偏：结合软件系统开发、数据管理、主从设备与业务功能上下文，自动纠正同音字与口语倒装，准确还原实际技术共识！\n\
        4. 若录音文稿较为零散或缺乏实质技术细节，请实事求是地归纳讨论触及的初步议题，并明确标注“待进一步明确”；\n\
        5. 结构采用条目式：\n\
           一、【核心议题与沟通背景】（结合议题与参考资料背景，真实归纳讨论的核心事项）\n\
           二、【达成共识与讨论要点】（逐项列出结合参考资料确认的业务/技术要点及决议）\n\
           三、【客户资料要点与关联分析】（归纳客户发来文档/图片的核心诉求与对应结论；若无资料则此项可略）\n\
           四、【后续推进与待办事项】（明确列出责任分工与下一步计划，若未提及则如实写“待后续对齐”）";

        self.execute_completion_multimodal(system_prompt, &prompt_content, &image_paths)
            .await
    }

    /// 结合会议议题、纪要内容与代码仓库深层目录树，调用 AI 进行真实语义归档分析
    pub async fn suggest_meeting_repo_dir(
        &self,
        topic: &str,
        summary_snippet: &str,
        directory_tree: &[String],
    ) -> Result<(String, String), AiError> {
        let system_prompt = "你是一名资深的软件架构师与工程文档管理专家。任务：根据本次会议的议题、纪要内容摘要，以及版本库现有目录结构，为会议纪要推荐一个最合适归档的父目录（相对路径）。\n\
        规则要求：\n\
        1. 必须返回合法的 JSON 对象，格式：{\"target_dir\": \"推荐的相对目录路径\", \"reason\": \"推荐理由简析（1句话说明与哪个模块/业务最匹配）\"}；\n\
        2. 优先在现有仓库目录中寻找与该会议内容业务高度契合的子目录（例如 docs/xxx 模块目录）；\n\
        3. 如果现有子目录都太偏底层代码（如 src, backend, frontend）而不适合存文档，推荐规范的文档子目录（如 docs/meetings 或 docs/对应业务模块）；\n\
        4. 严禁输出任何 markdown 格式标记、代码块标记或额外闲聊，只输出纯 JSON 字符串！";

        // 智能筛选并优先排列与文档、规范、核心模块相关的目录，同时容量扩大到 150 个目录，避免字典序简单截断
        let mut prioritized_dirs: Vec<&String> = directory_tree.iter().collect();
        prioritized_dirs.sort_by_key(|d| {
            let lower = d.to_lowercase();
            if lower.starts_with("docs")
                || lower.starts_with("doc")
                || lower.contains("document")
                || lower.contains("meeting")
                || lower.contains("spec")
                || lower.contains("wiki")
            {
                0
            } else if lower.starts_with("src")
                || lower.starts_with("app")
                || lower.starts_with("service")
                || lower.starts_with("packages")
                || lower.starts_with("crates")
            {
                1
            } else {
                2
            }
        });

        let dirs_formatted = prioritized_dirs
            .into_iter()
            .take(150)
            .map(|d| format!("- {}", d))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt_content = format!(
            "【会议议题】：{}\n\n【会议纪要摘要】：\n{}\n\n【当前代码仓库检测到的目录树】：\n{}",
            topic,
            summary_snippet.chars().take(800).collect::<String>(),
            dirs_formatted
        );

        let raw_res = self
            .execute_completion(system_prompt, &prompt_content)
            .await?;
        let clean_json = raw_res
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        #[derive(Deserialize)]
        struct AiDirResult {
            target_dir: String,
            reason: String,
        }

        if let Ok(parsed) = serde_json::from_str::<AiDirResult>(clean_json) {
            let clean_dir = parsed
                .target_dir
                .trim()
                .trim_start_matches('/')
                .trim_end_matches('/')
                .to_string();
            return Ok((clean_dir, parsed.reason.trim().to_string()));
        }

        // 兜底提取
        Ok((
            "docs/meetings".to_string(),
            "基于通用软件工程规范推荐".to_string(),
        ))
    }

    /// 语音转文字辅助函数：调用本地 CLI 智能体 (agy / codex)
    async fn transcribe_with_cli_agent(
        &self,
        actual_audio_path: &std::path::Path,
        provider: &str,
        proxy_opt: Option<String>,
    ) -> Result<String, AiError> {
        let audio_str = actual_audio_path.to_string_lossy().to_string();
        if provider == "agy" {
            let bin = find_binary("agy");
            let prompt = format!(
                "请直接读取并聆听该本地音频文件：{}\n请直接转写该录音中说话人讲出的所有真实文字内容。要求：只输出转写后的纯中文/对话文字正文，不要输出任何解释说明、格式前缀或客套话。",
                audio_str
            );
            let px = proxy_opt.clone();
            let output = tokio::task::spawn_blocking(move || {
                let mut cmd = std::process::Command::new(bin);
                cmd.arg("-p")
                    .arg(&prompt)
                    .arg("--dangerously-skip-permissions");
                if let Some(ref p) = px {
                    cmd.env("HTTP_PROXY", p)
                        .env("HTTPS_PROXY", p)
                        .env("ALL_PROXY", p)
                        .env("http_proxy", p)
                        .env("https_proxy", p)
                        .env("all_proxy", p);
                }
                cmd.output()
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
        } else {
            let bin = find_binary("codex");
            let prompt = format!(
                "请直接读取并聆听该本地音频文件：{}\n请直接转写该录音中的说话人实际语音内容，只输出文字文本：",
                audio_str
            );
            let px = proxy_opt.clone();
            let output = tokio::task::spawn_blocking(move || {
                let mut cmd = std::process::Command::new(bin);
                cmd.arg("exec").arg(&prompt);
                if let Some(ref p) = px {
                    cmd.env("HTTP_PROXY", p)
                        .env("HTTPS_PROXY", p)
                        .env("ALL_PROXY", p)
                        .env("http_proxy", p)
                        .env("https_proxy", p)
                        .env("all_proxy", p);
                }
                cmd.output()
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
    }

    /// 语音转文字处理（支持模块化多模路由：本地 whisper.cpp 极速推理 / 云端标准 Whisper API / CLI 智能体）
    pub async fn transcribe_audio(&self, input_path: &std::path::Path) -> Result<String, AiError> {
        self.transcribe_audio_with_prompt(input_path, None).await
    }

    /// 语音转文字处理（支持传入与会议资料相关的动态专有名词 Prompt，增强专有名词识别准确率）
    pub async fn transcribe_audio_with_prompt(
        &self,
        input_path: &std::path::Path,
        dynamic_prompt: Option<&str>,
    ) -> Result<String, AiError> {
        if !input_path.exists() {
            return Err(AiError::ParseError(format!(
                "音频文件不存在: {}",
                input_path.display()
            )));
        }

        let mode = self.config.asr.mode.trim().to_lowercase();
        let is_local_mode = mode == "local"
            || (mode.is_empty()
                && find_whisper_binary(self.config.asr.local_bin_path.as_deref()).is_some()
                && find_whisper_model(self.config.asr.local_model.as_deref()).is_some());

        // 原生 whisper.cpp 仅支持 16kHz PCM WAV 格式，切勿转为 MP3！
        // 仅在非本地模式（云端 Whisper API 或 CLI 智能体上传）时，才尝试将大体积 WAV 压缩转码为 MP3
        let actual_audio_path = if !is_local_mode
            && input_path
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("wav"))
                .unwrap_or(false)
        {
            let path_buf = input_path.to_path_buf();
            let converted =
                tokio::task::spawn_blocking(move || myth_audio::convert_wav_to_mp3(&path_buf))
                    .await;
            match converted {
                Ok(Ok(mp3_p)) => mp3_p,
                _ => input_path.to_path_buf(),
            }
        } else {
            input_path.to_path_buf()
        };

        let effective_prompt: Option<String> = match (&self.config.asr.prompt, dynamic_prompt) {
            (Some(p), Some(dp)) if !p.trim().is_empty() && !dp.trim().is_empty() => {
                Some(format!("{}, {}", p.trim(), dp.trim()))
            }
            (Some(p), _) if !p.trim().is_empty() => Some(p.trim().to_string()),
            (_, Some(dp)) if !dp.trim().is_empty() => Some(dp.trim().to_string()),
            _ => None,
        };

        // 1. 本地 whisper.cpp 极速推理模式 (默认优先)
        if is_local_mode {
            let whisper_bin = find_whisper_binary(self.config.asr.local_bin_path.as_deref());
            let whisper_model = find_whisper_model(self.config.asr.local_model.as_deref());

            match (whisper_bin, whisper_model) {
                (Some(bin), Some(model)) => {
                    // 硬件级参数兼容：确保音频文件符合 whisper.cpp 严格要求的 16kHz 单声道 16-bit PCM 格式
                    let compatible_wav = {
                        let path_clone = actual_audio_path.clone();
                        tokio::task::spawn_blocking(move || {
                            myth_audio::ensure_whisper_compatible_wav(&path_clone)
                        })
                        .await
                        .map_err(|e| AiError::ParseError(e.to_string()))?
                        .unwrap_or(actual_audio_path.clone())
                    };
                    let audio_str = compatible_wav.to_string_lossy().to_string();
                    let model_str = model.to_string_lossy().to_string();
                    let bin_str = bin.to_string_lossy().to_string();
                    let lang = self
                        .config
                        .asr
                        .language
                        .as_deref()
                        .unwrap_or("zh")
                        .to_string();
                    let prompt_opt = effective_prompt.clone();

                    let res = tokio::task::spawn_blocking(move || {
                        let mut cmd = std::process::Command::new(&bin_str);
                        cmd.arg("-m")
                            .arg(&model_str)
                            .arg("-f")
                            .arg(&audio_str)
                            .arg("-l")
                            .arg(&lang)
                            .arg("-nt")
                            .arg("--no-prints")
                            .arg("-t")
                            .arg("8")
                            .arg("-sns")
                            .arg("-et")
                            .arg("2.4")
                            .arg("-nth")
                            .arg("0.6")
                            .arg("-nf")
                            .arg("-mc")
                            .arg("64");
                        if let Some(ref p) = prompt_opt {
                            if !p.trim().is_empty() {
                                cmd.arg("--prompt").arg(p.trim());
                            }
                        }
                        cmd.output()
                    })
                    .await;

                    // 清理重采样生成的临时合规音频，防止磁盘空间翻倍占用
                    if compatible_wav != actual_audio_path {
                        let _ = std::fs::remove_file(&compatible_wav);
                    }

                    if let Ok(Ok(out)) = res {
                        if out.status.success() {
                            let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
                            let cleaned = clean_repetitive_text(&text);
                            if !cleaned.is_empty() {
                                return Ok(cleaned);
                            }
                        } else {
                            let err = String::from_utf8_lossy(&out.stderr);
                            if mode == "local" {
                                return Err(AiError::ParseError(format!(
                                    "本地 whisper-cli 执行失败: {}",
                                    err
                                )));
                            }
                        }
                    }
                }
                (None, _) if mode == "local" => {
                    return Err(AiError::ParseError("未检测到本地 whisper-cli 运行时程序。请在【系统设置 -> 录音转写引擎】中查看或切换为云端API。".into()));
                }
                (_, None) if mode == "local" => {
                    return Err(AiError::ParseError("未检测到本地 Whisper 模型文件（如 ggml-large-v3-turbo.bin）。请在【系统设置 -> 录音转写引擎】中一键下载推荐模型或切换为云端API。".into()));
                }
                _ => {}
            }
        }

        // 2. 智能体模式 (agy / codex CLI)
        if mode == "agent" {
            let provider = self.config.provider.trim().to_lowercase();
            let proxy_opt = self.config.proxy.clone().filter(|p| !p.trim().is_empty());
            let res = self
                .transcribe_with_cli_agent(&actual_audio_path, &provider, proxy_opt)
                .await?;
            return Ok(clean_repetitive_text(&res));
        }

        // 3. 云端兼容 Whisper 标准的 HTTP API (/audio/transcriptions)
        let asr_api_base = self
            .config
            .asr
            .api_base
            .as_ref()
            .filter(|s| !s.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| self.config.api_base.clone());

        let asr_api_key = self
            .config
            .asr
            .api_key
            .as_ref()
            .filter(|s| !s.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| {
                if self.config.api_key.trim().is_empty() {
                    std::env::var("OPENAI_API_KEY").unwrap_or_default()
                } else {
                    self.config.api_key.clone()
                }
            });

        let asr_model = self
            .config
            .asr
            .model
            .as_ref()
            .filter(|s| !s.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| "whisper-1".to_string());

        let url = format!(
            "{}/audio/transcriptions",
            asr_api_base.trim_end_matches('/')
        );

        // 检查音频文件大小。云端 Whisper 接口（Groq、OpenAI 等）普遍限制单个请求体 <= 25MB。
        // 若音频体积超限（例如超过 20MB），则自动进行分段切片（每段约 10 分钟）并顺序转写合并。
        const MAX_DIRECT_UPLOAD_BYTES: u64 = 20 * 1024 * 1024;
        let file_meta = tokio::fs::metadata(&actual_audio_path)
            .await
            .map_err(|e| AiError::ParseError(e.to_string()))?;

        if file_meta.len() > MAX_DIRECT_UPLOAD_BYTES {
            let temp_chunks_dir = std::env::temp_dir()
                .join(format!("myth_asr_chunks_{}", uuid::Uuid::new_v4().simple()));
            let audio_path_clone = actual_audio_path.clone();
            let chunks_dir_clone = temp_chunks_dir.clone();

            let chunk_paths_res = tokio::task::spawn_blocking(move || {
                // 每段 600 秒 (10 分钟)，64kbps MP3 约 4.8MB，远低于 25MB 限制
                myth_audio::split_audio_into_chunks(&audio_path_clone, 600, &chunks_dir_clone)
            })
            .await
            .map_err(|e| AiError::ParseError(e.to_string()))?;

            let chunk_paths = match chunk_paths_res {
                Ok(paths) => paths,
                Err(e) => {
                    let _ = std::fs::remove_dir_all(&temp_chunks_dir);
                    tracing::warn!("音频切片失败 ({})，回退尝试直接上传原音频", e);
                    // 切片工具不可用或切片失败时，优雅降级直接上传，避免直接阻断转写
                    let text = self
                        .transcribe_single_audio_file(
                            &url,
                            &asr_api_key,
                            &asr_model,
                            &actual_audio_path,
                            effective_prompt.as_deref(),
                        )
                        .await?;
                    return Ok(clean_repetitive_text(&text));
                }
            };

            let mut aggregated_transcript = String::new();
            let mut last_context_prompt = effective_prompt.clone();

            for chunk_file in &chunk_paths {
                let chunk_result = self
                    .transcribe_single_audio_file(
                        &url,
                        &asr_api_key,
                        &asr_model,
                        chunk_file,
                        last_context_prompt.as_deref(),
                    )
                    .await;

                match chunk_result {
                    Ok(chunk_text) => {
                        let cleaned = clean_repetitive_text(&chunk_text);
                        if !cleaned.is_empty() {
                            if !aggregated_transcript.is_empty() {
                                aggregated_transcript.push('\n');
                            }
                            aggregated_transcript.push_str(&cleaned);

                            // 取切片文末最近 100 个字符作为下一段转写的 prompt 上下文，增强长音频接缝识别的连贯性
                            let prompt_tail: String = cleaned
                                .chars()
                                .rev()
                                .take(100)
                                .collect::<Vec<_>>()
                                .into_iter()
                                .rev()
                                .collect();
                            last_context_prompt = match &effective_prompt {
                                Some(orig) => Some(format!("{}, {}", orig.trim(), prompt_tail)),
                                None => Some(prompt_tail),
                            };
                        }
                    }
                    Err(e) => {
                        let _ = std::fs::remove_dir_all(&temp_chunks_dir);
                        return Err(e);
                    }
                }
            }

            let _ = std::fs::remove_dir_all(&temp_chunks_dir);
            return Ok(clean_repetitive_text(&aggregated_transcript));
        }

        let text = self
            .transcribe_single_audio_file(
                &url,
                &asr_api_key,
                &asr_model,
                &actual_audio_path,
                effective_prompt.as_deref(),
            )
            .await?;
        Ok(clean_repetitive_text(&text))
    }

    /// 单个音频文件向云端 Whisper ASR 接口发起上传转写
    async fn transcribe_single_audio_file(
        &self,
        url: &str,
        asr_api_key: &str,
        asr_model: &str,
        audio_path: &std::path::Path,
        prompt: Option<&str>,
    ) -> Result<String, AiError> {
        let file_bytes = tokio::fs::read(audio_path)
            .await
            .map_err(|e| AiError::ParseError(e.to_string()))?;
        let filename = audio_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("audio.mp3")
            .to_string();

        let mime = if filename.ends_with(".mp3") {
            "audio/mpeg"
        } else {
            "audio/wav"
        };

        let part = reqwest::multipart::Part::bytes(file_bytes)
            .file_name(filename)
            .mime_str(mime)
            .map_err(|e| AiError::ParseError(e.to_string()))?;

        let mut form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model", asr_model.to_string())
            .text("response_format", "text");

        if let Some(ref lang) = self.config.asr.language {
            if !lang.trim().is_empty() && lang != "auto" {
                form = form.text("language", lang.clone());
            }
        }

        if let Some(p) = prompt {
            if !p.trim().is_empty() {
                form = form.text("prompt", p.to_string());
            }
        }

        let resp = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", asr_api_key))
            .multipart(form)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let err_text = resp.text().await.unwrap_or_default();
            return Err(AiError::ApiError(
                status,
                format!("云端 ASR 转写接口异常 ({url}): {err_text}"),
            ));
        }

        let text = resp.text().await.map_err(AiError::Reqwest)?;
        Ok(text)
    }
}

/// 对语音识别 (Whisper 等) 可能产生的恶性循环重复短语、静音幻觉文本进行清理与去重折叠
pub fn clean_repetitive_text(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // 1. 过滤常见的静音低信噪比固定幻觉短语及循环复读词
    let filter_hallucinations = [
        "( 字幕:J Chong )",
        "字幕:J Chong",
        "请不吝赐教",
        "感谢观看",
        "谢谢大家观看",
        "谢谢收看",
        "优酷网",
        "YouTube",
        "点赞关注",
    ];
    let mut text = trimmed.to_string();
    if text.starts_with("thought\n") {
        text = text["thought\n".len()..].trim().to_string();
    }
    if let Some(end_idx) = text.find("</thought>") {
        text = text[end_idx + "</thought>".len()..].trim().to_string();
    }
    if let Some(err_idx) = text.find("This request was blocked by Gemini's filters") {
        text = text[..err_idx].trim().to_string();
    }
    for h in filter_hallucinations {
        text = text.replace(h, "");
    }

    // 过滤 Whisper 中文死循环幻觉（如“你不认识我吗”循环）
    let loop_patterns = [
        "你不认识我吗",
        "我认识我吗",
        "你不认识我",
        "不认识我吗",
        "认识我吗",
    ];
    for lp in loop_patterns {
        if text.matches(lp).count() >= 2 {
            text = text.replace(lp, "");
        }
    }

    let text = text.trim();
    if text.is_empty() {
        return String::new();
    }

    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    if n < 4 {
        return text.to_string();
    }

    // 2. 连续重复子串折叠（支持短语长度从 30 字符到 1 字符）
    // 若某个长度为 L 的子串连续重复 >= 3 次，将其折叠为最多 1~2 次
    let mut result_chars: Vec<char> = Vec::with_capacity(n);
    let mut i = 0;
    while i < n {
        let mut matched = false;
        let max_plen = 30.min((n - i) / 3);
        for plen in (1..=max_plen).rev() {
            let pattern = &chars[i..i + plen];
            if pattern.iter().all(|c| c.is_whitespace()) {
                continue;
            }
            let mut repeat_count = 1;
            while i + (repeat_count + 1) * plen <= n
                && &chars[i + repeat_count * plen..i + (repeat_count + 1) * plen] == pattern
            {
                repeat_count += 1;
            }

            if repeat_count >= 3 {
                let keep_count = if plen <= 2 { 2 } else { 1 };
                for _ in 0..keep_count {
                    result_chars.extend_from_slice(pattern);
                }
                i += repeat_count * plen;
                matched = true;
                break;
            }
        }
        if !matched {
            result_chars.push(chars[i]);
            i += 1;
        }
    }

    let cleaned: String = result_chars.into_iter().collect();
    cleaned.trim().to_string()
}

use futures_util::StreamExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperModelInfo {
    pub name: String,
    pub path: String,
    pub size_mb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableModelInfo {
    pub name: String,
    pub size_label: String,
    pub desc: String,
    pub is_downloaded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrRuntimeStatus {
    pub whisper_bin_installed: bool,
    pub whisper_bin_path: Option<String>,
    pub installed_models: Vec<WhisperModelInfo>,
    pub available_models: Vec<AvailableModelInfo>,
    pub active_mode: String,
    pub active_model: String,
}

pub fn get_whisper_models_dir() -> std::path::PathBuf {
    if let Some(home) = get_user_home_dir() {
        home.join(".config").join("mythmanday").join("models")
    } else {
        std::path::PathBuf::from("./models")
    }
}

pub fn get_asr_runtime_status(cfg: &AiConfig) -> AsrRuntimeStatus {
    let bin_path = find_whisper_binary(cfg.asr.local_bin_path.as_deref());
    let models_dir = get_whisper_models_dir();
    let mut installed = Vec::new();

    if models_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&models_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().and_then(|s| s.to_str()) == Some("bin") {
                    let file_size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                    let size_mb = (file_size as f64) / (1024.0 * 1024.0);
                    let name = p
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string();
                    installed.push(WhisperModelInfo {
                        name,
                        path: p.to_string_lossy().to_string(),
                        size_mb: (size_mb * 10.0).round() / 10.0,
                    });
                }
            }
        }
    }

    let installed_names: std::collections::HashSet<String> =
        installed.iter().map(|m| m.name.clone()).collect();

    let preset_models = vec![
        (
            "ggml-large-v3-turbo.bin",
            "1.5 GB",
            "旗舰极速版 (强烈推荐)，中文识别精准度极高，无网络审查与拦截",
        ),
        (
            "ggml-tiny.bin",
            "75 MB",
            "极速轻量，内存开销极小，适合简单对话",
        ),
        (
            "ggml-base.bin",
            "141 MB",
            "均衡推荐 (官方推荐)，中文识别精准稳定",
        ),
        (
            "ggml-small.bin",
            "466 MB",
            "高精细度，适合专业术语多、发音嘈杂的场景",
        ),
    ];

    let available = preset_models
        .into_iter()
        .map(|(name, size, desc)| AvailableModelInfo {
            name: name.to_string(),
            size_label: size.to_string(),
            desc: desc.to_string(),
            is_downloaded: installed_names.contains(name),
        })
        .collect();

    AsrRuntimeStatus {
        whisper_bin_installed: bin_path.is_some(),
        whisper_bin_path: bin_path.map(|p| p.to_string_lossy().to_string()),
        installed_models: installed,
        available_models: available,
        active_mode: cfg.asr.mode.clone(),
        active_model: cfg
            .asr
            .local_model
            .clone()
            .unwrap_or_else(|| "ggml-large-v3-turbo.bin".to_string()),
    }
}

pub async fn download_whisper_model_file<F>(
    model_name: &str,
    proxy: Option<&str>,
    mut on_progress: F,
) -> Result<std::path::PathBuf, String>
where
    F: FnMut(f64, u64, u64) + Send + 'static,
{
    let target_dir = get_whisper_models_dir();
    tokio::fs::create_dir_all(&target_dir)
        .await
        .map_err(|e| e.to_string())?;
    let target_path = target_dir.join(model_name);

    let urls = [
        format!(
            "https://www.modelscope.cn/models/iceCream2025/whisper.cpp/resolve/master/{}",
            model_name
        ),
        format!(
            "https://hf-mirror.com/ggerganov/whisper.cpp/resolve/main/{}",
            model_name
        ),
        format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
            model_name
        ),
    ];

    let mut client_builder = reqwest::Client::builder();
    if let Some(p) = proxy {
        if !p.trim().is_empty() {
            if let Ok(proxy_obj) = reqwest::Proxy::all(p.trim()) {
                client_builder = client_builder.proxy(proxy_obj);
            }
        }
    }
    let client = client_builder.build().map_err(|e| e.to_string())?;

    let mut last_err = String::new();
    for url in &urls {
        match client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let total_bytes = resp.content_length().unwrap_or(0);
                let mut downloaded: u64 = 0;
                let mut stream = resp.bytes_stream();
                let tmp_path = target_dir.join(format!("{}.tmp", model_name));
                let mut file = match tokio::fs::File::create(&tmp_path).await {
                    Ok(f) => f,
                    Err(e) => {
                        last_err = e.to_string();
                        continue;
                    }
                };
                use tokio::io::AsyncWriteExt;

                let mut ok = true;
                while let Some(chunk_res) = stream.next().await {
                    match chunk_res {
                        Ok(chunk) => {
                            if let Err(e) = file.write_all(&chunk).await {
                                last_err = e.to_string();
                                ok = false;
                                break;
                            }
                            downloaded += chunk.len() as u64;
                            let pct = if total_bytes > 0 {
                                (downloaded as f64 / total_bytes as f64) * 100.0
                            } else {
                                0.0
                            };
                            on_progress(pct, downloaded, total_bytes);
                        }
                        Err(e) => {
                            last_err = e.to_string();
                            ok = false;
                            break;
                        }
                    }
                }
                if ok {
                    let _ = file.flush().await;
                    drop(file);
                    tokio::fs::rename(&tmp_path, &target_path)
                        .await
                        .map_err(|e| e.to_string())?;
                    return Ok(target_path);
                }
            }
            Ok(resp) => {
                last_err = format!("HTTP 状态码: {}", resp.status());
            }
            Err(e) => {
                last_err = e.to_string();
            }
        }
    }

    Err(format!("下载模型失败: {}", last_err))
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
        assert!(report
            .executive_summary
            .contains("OpenAI structured deliverable"));
    }

    #[test]
    fn test_asr_config_defaults() {
        let cfg = AiConfig::default();
        assert_eq!(cfg.asr.mode, "local");
        assert_eq!(cfg.asr.model, Some("whisper-1".to_string()));
        assert_eq!(cfg.asr.local_model, Some("ggml-base.bin".to_string()));
        assert_eq!(cfg.asr.language, Some("zh".to_string()));
    }

    #[test]
    fn test_find_whisper_helpers() {
        let _ = find_whisper_binary(None);
        let _ = find_whisper_model(None);
    }

    #[test]
    fn test_clean_repetitive_text() {
        let input = "喂大家上午好对你你你你你你你你你你不认识我吗你不认识我吗你不认识我吗你不认识我吗( 字幕:J Chong )";
        let cleaned = clean_repetitive_text(input);
        assert!(
            cleaned.starts_with("喂大家上午好对你你你不认识我吗")
                || cleaned.contains("喂大家上午好")
        );
        assert!(!cleaned.contains("( 字幕:J Chong )"));
        assert!(!cleaned.contains("你不认识我吗你不认识我吗你不认识我吗"));
    }
}
