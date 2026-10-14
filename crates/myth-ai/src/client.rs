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
    #[serde(default)]
    pub proxy: Option<String>,
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
            proxy: std::env::var("HTTP_PROXY").or_else(|_| std::env::var("HTTPS_PROXY")).or_else(|_| std::env::var("ALL_PROXY")).ok(),
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
        Self {
            config,
            client,
        }
    }

    /// 统一调用 AI 提供商 (支持 agy CLI / codex CLI / 兼容 HTTP API)
    pub async fn execute_completion(&self, system_prompt: &str, user_content: &str) -> Result<String, AiError> {
        let provider = self.config.provider.trim().to_lowercase();
        let proxy_opt = self.config.proxy.clone().filter(|p| !p.trim().is_empty());
        if provider == "agy" {
            let full_prompt = format!("指令：{}\n\n内容：{}", system_prompt, user_content);
            let bin = find_binary("agy");
            let output = tokio::task::spawn_blocking(move || {
                let mut cmd = std::process::Command::new(bin);
                cmd.arg("-p").arg(&full_prompt).arg("--dangerously-skip-permissions");
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
            if res.is_empty() {
                return Err(AiError::ParseError("agy 返回结果为空".into()));
            }
            return Ok(res);
        } else if provider == "codex" {
            let full_prompt = format!("指令：{}\n\n内容：{}", system_prompt, user_content);
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

        let raw = self.execute_completion(system_prompt, &user_content).await?;
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

        serde_json::from_str::<AiWorkEvaluationResult>(json_str)
            .map_err(|e| AiError::ParseError(format!("解析 AI 响应失败: {} (raw: {})", e, json_str)))
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

        let raw = self.execute_completion(system_prompt, &user_content).await?;
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

        serde_json::from_str::<Vec<AiBatchParsedItem>>(json_str)
            .map_err(|e| AiError::ParseError(format!("解析批量智能总结 JSON 数组失败: {} (raw: {})", e, json_str)))
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

        let system_prompt = "你是一名资深的技术架构师与项目管理专家。请根据本次会议的议题、沟通时长、速记要点及现场录音转写文稿，撰写一份条理清晰、内容详实、格局高远的商业交付级会议纪要。\n要求：\n1. 结构严格采用分层条目式：\n   一、【核心议题与沟通背景】（提炼本次会议讨论的核心背景与功能诉求）\n   二、【达成共识与技术决议】（条目式列出每一条落地的业务/技术架构决议，如菜单设计、表单归属、线下签字闭环、数据隔离等）\n   三、【后续推进与工程待办】（条目式列出具体待办事项）\n2. 内容要充分依据录音中讨论的实际细节进行真实提炼，严禁泛泛而谈或空话套话；\n3. 语言干练严谨、体现高维价值，适合直接呈报给客户或团队作为工时与技术交付凭据。\n4. 若会议未附带录音文稿或文字速记，请基于议题与企业级工程最佳实践直接梳理出标准纪要框架与重点跟进待办，严禁输出任何索要录音转写、询问用户或对话聊天式语句。";

        self.execute_completion(system_prompt, &prompt_content).await
    }

    /// 语音转文字处理（支持本地 Whisper CLI / agy CLI / codex CLI / 兼容 HTTP Whisper 接口）
    pub async fn transcribe_audio(&self, input_path: &std::path::Path) -> Result<String, AiError> {
        if !input_path.exists() {
            return Err(AiError::ParseError(format!("音频文件不存在: {}", input_path.display())));
        }

        // 优先尝试将可能达到数百兆的 WAV 文件利用本地 ffmpeg 压缩转码为小体积 MP3 (通常缩减 95% 以上)
        let actual_audio_path = if input_path.extension().and_then(|s| s.to_str()).map(|s| s.eq_ignore_ascii_case("wav")).unwrap_or(false) {
            let path_buf = input_path.to_path_buf();
            let converted = tokio::task::spawn_blocking(move || {
                myth_audio::convert_wav_to_mp3(&path_buf)
            }).await;
            match converted {
                Ok(Ok(mp3_p)) => mp3_p,
                _ => input_path.to_path_buf(),
            }
        } else {
            input_path.to_path_buf()
        };

        // 1. 如果本地系统安装了 whisper 命令行工具，优先尝试执行本地 whisper 提取
        if let Some(whisper_bin) = find_whisper_binary() {
            let audio_str = actual_audio_path.to_string_lossy().to_string();
            let parent_dir = actual_audio_path
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_string_lossy()
                .to_string();
            let res = tokio::task::spawn_blocking(move || {
                std::process::Command::new(&whisper_bin)
                    .arg(&audio_str)
                    .arg("--output_format")
                    .arg("txt")
                    .arg("--output_dir")
                    .arg(&parent_dir)
                    .output()
            })
            .await;
            if let Ok(Ok(out)) = res {
                if out.status.success() {
                    let txt_path = actual_audio_path.with_extension("txt");
                    if let Ok(text) = std::fs::read_to_string(&txt_path) {
                        if !text.trim().is_empty() {
                            return Ok(text.trim().to_string());
                        }
                    }
                }
            }
        }

        let provider = self.config.provider.trim().to_lowercase();
        let proxy_opt = self.config.proxy.clone().filter(|p| !p.trim().is_empty());
        // 2. 如果使用本地 AI 引擎 (agy)，agy 原生具备多模态音频理解与语音转文字能力
        if provider == "agy" {
            let audio_str = actual_audio_path.to_string_lossy().to_string();
            let bin = find_binary("agy");
            let prompt = format!(
                "请直接读取并聆听该本地音频文件：{}\n请直接转写该录音中说话人讲出的所有真实文字内容。要求：只输出转写后的纯中文/对话文字正文，不要输出任何解释说明、格式前缀或客套话。",
                audio_str
            );
            let px = proxy_opt.clone();
            let output = tokio::task::spawn_blocking(move || {
                let mut cmd = std::process::Command::new(bin);
                cmd.arg("-p").arg(&prompt).arg("--dangerously-skip-permissions");
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
        } else if provider == "codex" {
            let audio_str = actual_audio_path.to_string_lossy().to_string();
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

        // 3. 走云端兼容 Whisper 标准的 HTTP API (/audio/transcriptions)
        let file_bytes = tokio::fs::read(&actual_audio_path)
            .await
            .map_err(|e| AiError::ParseError(e.to_string()))?;
        let filename = actual_audio_path
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

