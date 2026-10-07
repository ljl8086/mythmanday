# ParaDay (平行人天) 🦀

> **Parallel multi-channel time tracker & man-day billing desktop engine for modern developers.**  
> 专为现代开发者打造的 100% Rust 原生多通道并发工时与智能人天结算桌面引擎。

---

## 💡 为什么需要 ParaDay？

在全面进入 AI 编码时代后，开发者的工作流发生了根本性变化：
- **效率悖论**：单纯记录敲键盘时间会惩罚高效率的工程师；
- **并发多任务**：后台跑 Agent / 自动化测试 / 编译，前台进行架构评审或需求对接，传统单线程计时器无法反映真实并行投入；
- **零提交困境**：大量的核心时间耗费在电话沟通、需求对齐等没有 Git 提交的工作上，结算时缺乏有效凭证；
- **人天结算诉求**：商业客户和咨询顾问习惯按**“人天 (Man-Day)”**付费，需要精准且公允的工时-人天折算机制。

**ParaDay 为解决这些痛点而生。**

---

## ✨ 核心特性

- **⚡ 多通道独立并行计时 (Multi-Channel Engine)**：支持商业项目 A、项目 B、个人自研项目同时独立运行，各自独立计算时长、人天与费用，物理重叠合法化。
- **📅 原生人天折算体系 (Man-Day Native)**：内置**半天阶梯制（<=4h 算 0.5 天，<=8h 算 1.0 天）**、严格比例制与全天制，各项目费率完全独立隔离。
- **🛡️ 多维防遗漏守护体系**：
  - **Git 仓库静止预警**：项目运行中若长达 60 分钟无代码或文件变动，发出单项轻提醒；
  - **全局 AFK 离开校准**：离开电脑超过 10 分钟自动挂起所有运行中的秒表，回来后一键批量勾选保留或扣除；
  - **锁屏/跨夜守护**：自动封存工时，杜绝第二天醒来跑出 20 个小时。
- **📝 零提交场景归因**：全局快捷键呼出闪念便签，随手记录电话与会议；自动捕获工作区未提交变更（`git diff HEAD`）。
- **🤖 Git + AI 智能报告生成**：结合时间窗口内的 Commit 列表、Diff 规模与便签，调用大模型（支持 DeepSeek / OpenAI / 本地 Ollama）一键提炼专业客户交付清单。
- **🦀 纯血 Rust + Slint 原生**：零 Electron / 零 Webview 包袱，内存占用仅约 15MB~25MB，秒级冷启动。
- **🔒 本地优先 (Local-First)**：所有客户数据与工时流水完全保存在本地 SQLite，绝不上报云端。

---

## 🏗️ 架构设计 (Cargo Workspace)

```text
paraday/
├── Cargo.toml                    # Workspace 根配置
├── README.md                     # 项目介绍与文档
├── LICENSE                       # MIT 开源许可证
├── docs/
│   └── copyright/                # 软件著作权申请辅助资料
└── crates/
    ├── paraday-core/             # 核心模型、多通道秒表状态机、人天折算算法
    ├── paraday-db/               # 本地 SQLite 嵌入式存储与迁移
    ├── paraday-git/              # Git 仓库活动嗅探与 Diff 提取器
    ├── paraday-ai/               # LLM 结构化交付总结生成器
    └── paraday-desktop/          # 基于 Slint 的原生桌面宿主与心跳调度器
```

---

## 🚀 快速上手与编译

### 环境准备
- [Rust](https://www.rust-lang.org/) (推荐 1.80+ 稳定版)
- GCC / Clang 编译环境

### 构建与运行
```bash
# 1. 运行核心与数据库单元测试
cargo test -p paraday-core -p paraday-db

# 2. 启动桌面应用
cargo run -p paraday-desktop
```

---

## 📜 著作权与开源许可

- **软件全称**：平行人天多项目工时管理与交付结算软件 [简称：ParaDay] V1.0
- **开源协议**：本项目基于 [MIT 许可证](LICENSE) 授权开源。
