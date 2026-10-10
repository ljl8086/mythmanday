# Contributing to MythManDay (人天神话)

Thank you for your interest in contributing to **MythManDay**! We welcome bug reports, feature suggestions, architectural feedback, and pull requests from developers worldwide.

---

## 🛠️ Local Development Setup

### Prerequisites
- **Rust Toolchain**: 1.80+ (`rustup default stable`)
- **Node.js**: v20+ & **pnpm**: 10+
- **Linux Packages** (if on Linux): `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `libasound2-dev`

### Running the Project
```bash
# 1. Install frontend dependencies and start Vite dev server
cd frontend
pnpm install
pnpm dev --port 1420

# 2. In another terminal, run Tauri desktop application
cargo run --bin mythmanday-app
```

### Running Tests
```bash
cargo test -p myth-core -p myth-db
```

---

## 📋 Pull Request Guidelines

1. **Branching**: Create a topic branch from `master` (`git checkout -b feature/your-feature-name`).
2. **Commit Style**: We follow [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `docs:`, `refactor:`, etc.).
3. **Data Integrity**: Any changes to `myth-core` and `myth-db` must maintain separation between **raw physical elapsed time** and **contract billing calculations**, ensuring that raw tracked data is never destructively overwritten.
4. **Local-First & Privacy**: All features must prioritize local storage and protect user data confidentiality. Never transmit user time entries or billing rates to unconfigured remote endpoints.

---

## 🐛 Reporting Issues

- Please search existing Issues before submitting a new one.
- Provide your operating system version, reproduction steps, and relevant log outputs.
