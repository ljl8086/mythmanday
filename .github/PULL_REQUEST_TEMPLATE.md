## 变更描述 (Description)

请简要说明本次 Pull Request 的核心改动、解决的问题背景以及带来的影响。

Fixes #(issue_number) <!-- 如果有关联的 Issue，请在此关联，例如 Fixes #12 -->

---

## 变更类型 (Type of Change)

请在适用的选项括号内填入 `x`：

- [ ] 🐛 **Bug 修复 (Bug Fix)**：修复已知缺陷且不破坏向后兼容性
- [ ] ✨ **新特性 (New Feature)**：新增功能且不破坏现有功能
- [ ] ⚡ **性能优化 (Performance Optimization)**：提升计时器、数据库或前端响应性能
- [ ] 🎨 **代码重构 (Code Refactoring)**：无新增功能也未修复 bug 的代码结构调整
- [ ] 📝 **文档补充 (Documentation)**：更新 README、架构说明或开发指南
- [ ] 🧪 **测试用例 (Tests)**：新增或补充单元测试/集成测试
- [ ] 🔧 **构建与工程化 (Build/CI)**：更新 GitHub Actions、打包脚本或依赖版本

---

## 核心设计与数据完整性规范 (Data Integrity & Safety)

- [ ] **原始工时不可篡改**：物理计时流水与商业合同结算规则保持严格分层，未破坏原始事实数据
- [ ] **隐私与 Local-First**：无未经用户授权上传的远程数据传输逻辑
- [ ] **跨平台兼容**：对 Linux / Windows / macOS 平台无破坏性假设

---

## 自测清单 (Self-Verification Checklist)

- [ ] 代码遵循项目规范，并通过 `cargo fmt -- --check`
- [ ] 本地已通过 `cargo clippy --workspace -- -D warnings`
- [ ] 本地单元测试已通过：`cargo test -p myth-core -p myth-db`
- [ ] 前端代码通过检查：`cd frontend && pnpm run lint`
- [ ] 如涉及用户界面改动，已在桌面客户端自测通过
