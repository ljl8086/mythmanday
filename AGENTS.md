# Git & Code Review Rules

## 1. 自动审查监听与即时响应 (Codex Review Loop)
每次在本地提交并推送代码到 GitHub PR 分支后，**严禁立即结束或被动等待用户告知**，必须严格执行以下闭环：
1. **自动触发哨兵脚本**：
   运行 `./scripts/watch-codex-review.py` 检测最新 commit 的 review 状态。
2. **使用 schedule 工具设置定时检查**：
   若状态为 `REVIEW_PENDING`，使用 `schedule` 设置 25 秒的定时器并结束本轮交互。定时器唤醒后再次运行脚本，直到检测到 Codex 评审结论。
3. **即时自主修复**：
   一旦检测到 `REVIEW_HAS_FINDINGS`，立刻根据 Codex 给出的具体行号与建议分析根因，在本地完成代码修复。
4. **全套自测与前置验证**：
   在推送前必须依次通过：
   - `cargo fmt -- --check`
   - `cargo clippy --workspace --no-deps`
   - `pnpm --dir frontend run build`
   - `cargo test -p myth-core -p myth-db -p myth-git -p myth-ai`
5. **构建本地发布并交付**：
   自动编译 release 并覆盖 `/home/beatle/.local/bin/mythmanday-app`，推送 PR 分支，再次启动哨兵监听，直到 Codex 审查显示 `REVIEW_PASSED`（👍 无新问题）。

## 2. 界面与异步交互铁律
- **防重复点击与加载状态闭环**：所有异步操作（AI 提炼、Git 写入、会议结束、文件选择等），点击瞬间必须将按钮置为 `disabled` 并展示旋转动画（`Loader2`）与明确文案，直到状态机完全流转到目标状态。
- **状态机互斥**：对于处于过渡状态（如 `summarizing`）的对象，必须隐藏触发操作按钮，杜绝连击风险。
