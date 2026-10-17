#!/usr/bin/env python3
import json
import subprocess
import sys

CODEX_BOT_LOGIN = "chatgpt-codex-connector[bot]"

def run(cmd):
    try:
        res = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, check=True)
        return res.stdout
    except Exception as e:
        return None

def fetch_json_paginated(endpoint):
    out = run(["gh", "api", "--paginate", endpoint])
    if out is None:
        return None
    try:
        data = json.loads(out)
        return data if isinstance(data, list) else []
    except Exception:
        return None

def main():
    pr_out = run(["gh", "pr", "view", "--json", "number", "-q", ".number"])
    if pr_out is None or not pr_out.strip():
        print("未检测到关联的 Pull Request。")
        sys.exit(0)
    pr_num = pr_out.strip()

    head_out = run(["git", "rev-parse", "HEAD"])
    if head_out is None or not head_out.strip():
        print("无法获取当前 HEAD commit hash。")
        sys.exit(1)
    head_sha = head_out.strip()
    short_sha = head_sha[:10]

    print(f"PR: #{pr_num} | 待检查 Commit: {short_sha}")

    reviews = fetch_json_paginated(f"repos/:owner/:repo/pulls/{pr_num}/reviews")
    if reviews is None:
        print("⚠️ 无法获取 PR 审查列表，保持等待状态。")
        print("STATUS: REVIEW_PENDING")
        sys.exit(0)

    # 1. 严格过滤仅属于 Codex reviewer 且针对当前 head commit 的审查记录
    matching_reviews = [
        r for r in reviews
        if (r.get("user", {}).get("login") == CODEX_BOT_LOGIN or CODEX_BOT_LOGIN in r.get("body", ""))
        and (r.get("commit_id", "").startswith(short_sha) or short_sha in r.get("body", ""))
    ]

    if not matching_reviews:
        print(f"⏳ 审查结果尚在生成中，尚未检测到针对 Commit {short_sha} 的 Codex Review。")
        print("STATUS: REVIEW_PENDING")
        sys.exit(0)

    latest_review = matching_reviews[-1]
    review_id = latest_review.get("id")
    print(f"✅ 发现针对 Commit {short_sha} 的 Codex 审查结果：")
    print(f"  审查 ID: {review_id}")
    print(f"  状态: {latest_review.get('state')}")
    print(f"  提交时间: {latest_review.get('submitted_at')}")

    # 2. 拉取此 review 或此 commit 的 inline comments，fail closed
    comments = fetch_json_paginated(f"repos/:owner/:repo/pulls/{pr_num}/comments")
    if comments is None:
        print("⚠️ 无法获取审查意见详情 (API 调用或解析失败)，执行 fail-closed 保持等待。")
        print("STATUS: REVIEW_PENDING")
        sys.exit(1)

    matching_comments = [
        c for c in comments
        if (c.get("pull_request_review_id") == review_id
            or (c.get("user", {}).get("login") == CODEX_BOT_LOGIN and (c.get("commit_id", "").startswith(short_sha) or c.get("original_commit_id", "").startswith(short_sha))))
    ]

    if matching_comments:
        print(f"\n⚠️ 检测到 {len(matching_comments)} 条审查意见：")
        for idx, c in enumerate(matching_comments, 1):
            path = c.get("path", "")
            line = c.get("line") or c.get("original_line", "")
            body = c.get("body", "").split("\n")[0]
            print(f"  {idx}. [{path}:{line}] {body}")
        print("\nSTATUS: REVIEW_HAS_FINDINGS")
    else:
        print("\n🎉 该 Commit 审查通过，没有发现新的问题意见。")
        print("STATUS: REVIEW_PASSED")

if __name__ == "__main__":
    main()
