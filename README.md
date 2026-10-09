# 人天神话-开发者人天中台 (MythManDay) 🦀

> **软件全称**：人天神话-开发者人天中台 [简称：MythManDay / ParaDay] V1.0  
> **Parallel multi-channel time tracker & man-day billing desktop platform for modern developers.**  
> 专为现代开发者打造的高性能多通道并发工时、智能商业计费与人天对账结算桌面中台。

[![Rust Version](https://img.shields.io/badge/Rust-1.80%2B-orange.svg)](https://www.rust-lang.org/)
[![Tauri Version](https://img.shields.io/badge/Tauri-2.0-blue.svg)](https://v2.tauri.app/)
[![React Version](https://img.shields.io/badge/React-19-61dafb.svg)](https://react.dev/)
[![License](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

---

## 💡 为什么需要「人天神话 (MythManDay)」？

在全面进入 AI 辅助编程时代后，开发者的工作流发生了根本性变化：
- **效率悖论**：单纯记录敲键盘时间会惩罚高效率的工程师；
- **并发多任务**：后台跑 Agent / 自动化测试 / 编译，前台进行架构评审或需求对接，传统单线程计时器无法反映真实并行投入；
- **零提交困境**：大量的核心时间耗费在电话沟通、需求评审等没有 Git 提交的工作上，结算时缺乏有效凭证；
- **人天结算诉求**：商业客户和咨询顾问习惯按**“人天 (Man-Day)”**或“工时”结算，需要合规、公允且契合真实商业对账的规则引擎。

**MythManDay 为解决这些痛点而生。**

---

## ✨ 核心特性

- **⚡ 多通道独立并行计时 (Multi-Channel Engine)**：支持商业项目 A、项目 B、个人自研项目同时独立运行，各自独立计算时长、人天与费用，物理重叠合法化。
- **⚖️ 四大商业人天结算与流动策略 (Settlement Engine)**：
  - **🛡️ 合规削峰填谷**：工作日单日封顶 8 小时 (1.0天)；溢出工时回填历史不足工作日或顺延未来工作日，周末不排工时；
  - **💎 保底 8 小时模式**：当天只要有有效投入，基准按 1.0 人天起步保底；超过 8 小时部分自动结转为新工作日追加；
  - **⏩ 封顶顺延模式**：单日 8 小时封顶，多出的工时直接结转出新的工作日追加在后；
  - **⏱️ 实际物理工时模式**：实报实销，同一天同项目多次打卡合并为唯一一行，按当天实际物理投入比例换算人天，不顺延、不平摊。
- **🏢 灵活商业计费与阶梯规则**：
  - **计费方式**：支持按天（默认最小0.5天）或按小时；
  - **多币种支持**：CNY (¥)、USD ($)、EUR (€)、HKD (HK$)、JPY (¥)、GBP (£) 等；
  - **阶梯舍入模式**：进位（向上取整）、四舍五入、舍去（截断）；
  - **最小刻度控制**：支持 0.5、1.0、0.25、0.1 最小时间刻度。
- **🔒 全局金额隐私模式**：
  - 顶栏一键切换「显示金额 / 金额已隐藏」，支持本地记忆持久化；
  - 开启后全局看板、流水明细、日报与统计卡片自动隐去金额数据，仅展示工时与人天。
- **📥 现代化多维 Excel 商业对账单导出**：
  - 支持多选项目、多选月份批量导出；
  - 每个项目自动生成独立的 `.xlsx` 文件，项目内按月拆分独立工作表 (Sheet)；
  - 规范商业抬头排版、工作交付清单自动换行、纯净数字格式无冗余字符；
  - **可配置开关**：支持自由选择「是否在 Excel 中导出金额与单价列」，若隐藏则自适应调整为 7 列排版。
- **🎙️ 会议录音与语音存证**：内置跨平台声卡驱动级音频采集，会议与沟通全流程留痕。
- **🤖 Git 提交分析与 AI 纪要**：自动嗅探关联 Git 仓库的代码提交记录，调用大模型一键提炼专业客户交付清单。
- **🔒 本地优先 (Local-First)**：所有客户数据与工时流水完全保存在本地 SQLite，杜绝数据上报。

---

## 🏗️ 架构设计 (Cargo Workspace + 现代前端)

```text
paraday/
├── Cargo.toml                    # Workspace 根配置
├── install.sh                    # Linux 本机一键安装脚本 (注册桌面菜单与图标)
├── uninstall.sh                  # Linux 本机一键卸载脚本
├── scripts/
│   └── build-deb.sh              # Debian / Ubuntu .deb 安装包自动打包脚本
├── docs/
│   ├── BUILD.md                  # 跨平台 (Linux/Windows/macOS) 编译与打包发布指南
│   └── copyright/                # 软件著作权申请辅助资料与源码生成脚本
├── frontend/                     # 基于 React 19 + TypeScript + Tailwind CSS v4 的现代化前端
└── crates/
    ├── paraday-core/             # 核心模型、多通道秒表状态机、四大商业人天结算引擎
    ├── paraday-db/               # 本地 SQLite 嵌入式存储、Schema 自动迁移
    ├── paraday-git/              # Git 仓库活动嗅探与 Diff 提取器
    ├── paraday-audio/            # 跨平台原生声卡音频录制与存证引擎 (cpal + hound)
    ├── paraday-ai/               # LLM 结构化交付总结生成器
    ├── paraday-app/              # 基于 Tauri 2.0 的跨平台桌面应用程序入口
    └── paraday-desktop/          # 命令行/独立宿主工具集
```

---

## 🚀 快速上手与运行

### 1. 运行核心与数据库单元测试
```bash
cargo test -p paraday-core -p paraday-db
```

### 2. 本地开发与调试
```bash
# 进入前端目录启动 Vite 开发热更新服务器
cd frontend && pnpm dev --port 1420

# 另起终端运行 Tauri 桌面端
cargo run --bin paraday-app
```

---

## 📦 安装与打包发布

详细的多平台（**Linux**、**Windows**、**macOS**）编译、环境依赖与 CI/CD 自动化构建配置请参阅：  
📖 **[跨平台编译与打包发布指南 (docs/BUILD.md)](docs/BUILD.md)**

### 常用命令快捷参考：

#### 🐧 Linux 本机一键安装 (无需 root)
```bash
chmod +x ./install.sh && ./install.sh
# 安装后直接在系统应用菜单中点击「人天神话 (MythManDay)」或终端执行 mythmanday
```

#### 📦 Linux 制作 .deb 安装包 (生成于 dist 目录)
```bash
chmod +x ./scripts/build-deb.sh && ./scripts/build-deb.sh
# 安装: sudo dpkg -i dist/mythmanday_1.0.0_amd64.deb
```

#### 🪟 Windows 编译与打包
```powershell
cd frontend && pnpm build && cd ..
cargo build --release --bin paraday-app
# 生成安装程序 (.exe / .msi): cd frontend && pnpm tauri build
```

#### 🍏 macOS 编译与打包
```bash
cd frontend && pnpm build && cd ..
cargo build --release --bin paraday-app
# 生成 DMG 安装镜像: cd frontend && pnpm tauri build --bundles dmg,app
```

---

## 📜 著作权与开源许可

- **软件全称**：人天神话-开发者人天中台 [简称：MythManDay / ParaDay] V1.0
- **开源协议**：本项目基于 [MIT 许可证](LICENSE) 授权开源。
