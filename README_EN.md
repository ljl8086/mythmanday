# MythManDay (The Mythical Man-Day)

[**中文文档**](README.md) | **English**

> **A local-first, multi-channel time tracking and contract billing platform tailored for modern AI-assisted software development.**

![MythManDay Dashboard](docs/images/dashboard_preview.png)

---

## 💡 Why MythManDay?

In the era of AI-assisted engineering and autonomous coding agents, developer workflow has changed fundamentally:
- **The Efficiency Paradox**: Simply logging active keystroke hours penalizes high-leverage engineers who ship faster with AI.
- **Concurrent Multitasking**: Running background agents, automated test suites, and compilation workflows while simultaneously attending architecture reviews or client syncs cannot be represented by traditional single-threaded stopwatches.
- **Zero-Commit Gaps**: Strategic developer value is often spent in design discussions, problem triage, and verbal client alignments without Git commits, leaving developers vulnerable during milestone billing.
- **Commercial Man-Day Billing**: Enterprises, client contracts, and consulting engagements operate on **Man-Days** and milestone rules that require auditable, fair, and contract-aligned settlement models.

**MythManDay is engineered to solve these challenges.**

---

## ✨ Key Features

- **⚡ Multi-Channel Parallel Tracking**: Run independent stopwatches for multiple client projects and personal initiatives simultaneously. Accurately capture multi-project engagement without mutually exclusive constraints.
- **⚖️ Four Contract Settlement Strategies**:
  > ⚠️ **Auditability Note**: The platform strictly separates **raw physical elapsed time** (immutable ground truth) from **contract billing calculations** (commercial rules). Raw work logs remain tamper-proof and fully auditable.
  - **Contract Peak Shaving & Valley Filling**: Caps daily billable hours at 8h (1.0 man-day), automatically balancing weekly workload across eligible workdays while excluding weekends.
  - **Consultant Minimum Guarantee**: Automatically guarantees 1.0 man-day baseline upon any qualified engagement, carrying over surplus hours to supplementary billing days.
  - **Capped Postpone Carryover**: Strictly enforces an 8h daily billing cap, rolling overflow forward as sequential billing days.
  - **Proportional Physical Ratio**: Pure time-and-materials mode converting actual physical hours proportionally (`hours / 8.0`) without smoothing.
- **🏢 Commercial Billing & Granularity Rules**:
  - Flexible billing units: By Day (default minimum 0.5 day) or by Hour.
  - Multi-currency: CNY (`¥`), USD (`$`), EUR (`€`), HKD (`HK$`), JPY (`¥`), GBP (`£`), etc.
  - Step rounding modes: Ceiling (round up), Half-up (standard), or Floor (truncate).
  - Minimum step control: 0.5, 1.0, 0.25, or 0.1 increments.
- **🔒 Global Amount Privacy Mask**:
  - One-click top-bar toggle to hide monetary rates and totals across dashboards, time entries, and reports during public presentations or internal team syncs.
- **📊 Multi-Dimensional Excel Statement Export**:
  - Batch export across multiple projects and billing months into clean `.xlsx` workbooks.
  - Automatically splits billing months into dedicated sheets with enterprise layout, wrapped delivery scopes, and configurable rate privacy switches.
- **🎙️ Hardware-Level Audio Meeting Evidence**:
  - Native cross-platform sound card audio capture (`cpal`), recording meetings directly into local tamper-evident WAV archives.
- **🤖 Git Activity Sniffing & AI Summaries**:
  - Inspects local repository commits and code diffs, distilling fragmented commit messages into client-ready structured milestone reports.
- **🛡️ Local-First & Zero Data Telemetry**:
  - Built on embedded SQLite with automatic schema migrations. No telemetry, no cloud dependency, and total confidentiality.

---

## 🏗️ Architecture

```text
mythmanday/
├── Cargo.toml                    # Root workspace manifest
├── install.sh                    # Linux desktop installer (menus, icons, binaries)
├── uninstall.sh                  # Clean uninstallation script
├── scripts/
│   └── build-deb.sh              # Debian / Ubuntu package builder
├── docs/
│   ├── BUILD.md                  # Comprehensive cross-platform build guide
│   └── copyright/                # Official copyright registration materials
├── frontend/                     # Modern GUI built with React 19 + TypeScript + Tailwind CSS
└── crates/
    ├── myth-core/                # State machines, domain models, and settlement engines
    ├── myth-db/                  # Embedded SQLite storage & auto-migrations
    ├── myth-git/                 # Git commit inspector & diff extractor
    ├── myth-audio/               # Cross-platform hardware audio recorder (cpal + hound)
    ├── myth-ai/                  # Structured delivery summary generator
    ├── myth-app/                 # Tauri 2.0 application entrypoint (mythmanday-app)
    └── myth-desktop/             # Host utilities and system tray integration
```

---

## 🚀 Quick Start

### 1. Run Unit Tests
```bash
cargo test -p myth-core -p myth-db
```

### 2. Local Development
```bash
# Start frontend Vite dev server (port 1420)
cd frontend && pnpm install && pnpm dev --port 1420

# In another terminal, run the desktop client
cargo run --bin mythmanday-app
```

---

## 📦 Building & Releases

For detailed multi-platform build requirements on **Linux**, **Windows**, and **macOS**, please refer to:  
📖 **[Cross-Platform Build & Packaging Guide (docs/BUILD.md)](docs/BUILD.md)**.

### Quick Packaging References:

#### 🐧 Linux (Native User Install)
```bash
chmod +x ./install.sh && ./install.sh
# Launch via app launcher or run 'mythmanday' in terminal
```

#### 📦 Linux (.deb Package)
```bash
chmod +x ./scripts/build-deb.sh && ./scripts/build-deb.sh
# Install: sudo dpkg -i dist/mythmanday_1.0.0_amd64.deb
```

#### 🪟 Windows (.exe / .msi)
```powershell
cd frontend && pnpm build && cd ..
cargo build --release --bin mythmanday-app
# Or bundle with installer: cd frontend && pnpm tauri build
```

#### 🍏 macOS (.dmg / Universal Binary)
```bash
cd frontend && pnpm build && cd ..
cargo build --release --bin mythmanday-app
# Generate DMG: cd frontend && pnpm tauri build --bundles dmg,app
```

---

## 🛠️ Technology Stack

| Component | Official Reference | Architectural Role |
| :--- | :--- | :--- |
| **Rust 1.80+** | [www.rust-lang.org](https://www.rust-lang.org/) | High-performance, memory-safe parallel tracking & billing engine |
| **Tauri 2.0** | [v2.tauri.app](https://v2.tauri.app/) | Secure, lightweight native desktop runtime and IPC bridge |
| **React 19** | [react.dev](https://react.dev/) | Modern declarative component system for responsive desktop UI |
| **Tailwind CSS** | [tailwindcss.com](https://tailwindcss.com/) | Atomic styling with modern slate dark-mode sidebar layout |
| **SQLite (rusqlite)** | [www.sqlite.org](https://www.sqlite.org/) | Embedded local-first storage ensuring zero cloud exposure |

---

## 📄 License & Intellectual Property

- **Official Name**: 人天神话-开发者人天中台 (MythManDay)
- **License**: Released under the [MIT License](LICENSE).
- **Contributing**: Please review [CONTRIBUTING.md](CONTRIBUTING.md) before submitting pull requests.
