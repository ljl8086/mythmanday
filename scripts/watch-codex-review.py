#!/usr/bin/env python3
import json
import subprocess
import sys

def run(cmd):
    try:
        return subprocess.check_output(cmd, text=True, stderr=subprocess.DEVNULL)
    except Exception:
        return ""

def main():
    pr_num = run(["gh", "pr", "view", "--json", "number", "-q", ".number"]).strip()
    if not pr_num:
        print("未检测到关联的 Pull Request。")
        sys.exit(0)

    head_sha = run(["git", "rev-parse", "HEAD"]).strip()
    short_sha = head_sha[:10]

    print(f"PR: #{pr_num} | 待检查 Commit: {short_sha}")

    reviews_raw = run(["gh", "api", f"repos/:owner/:repo/pulls/{pr_num}/reviews"])
    try:
        reviews = json.loads(reviews_raw) if reviews_raw else []
    except Exception:
        reviews = []

    matching = [r for r in reviews if r.get("commit_id", "").startswith(short_sha) or short_sha in r.get("body", "")]
    
    if matching:
        latest = matching[-1]
        print(f"✅ 发现针对 Commit {short_sha} 的 Codex 审查结果：")
        print(f"  审查 ID: {latest.get('id')}")
        print(f"  状态: {latest.get('state')}")
        print(f"  提交时间: {latest.get('submitted_at')}")

        comments_raw = run(["gh", "api", f"repos/:owner/:repo/pulls/{pr_num}/comments"])
        try:
            comments = json.loads(comments_raw) if comments_raw else []
        except Exception:
            comments = []

        matching_comments = [
            c for c in comments 
            if c.get("commit_id", "").startswith(short_sha) or c.get("original_commit_id", "").startswith(short_sha)
        ]

        if matching_comments:
            print(f"\n⚠️ 检测到 {len(matching_comments)} 条审查意见：")
            for idx, c in enumerate(matching_comments, 1):
                path = c.get("path", "")
                line = c.get("line", "")
                body = c.get("body", "").split("\n")[0]
                print(f"  {idx}. [{path}:{line}] {body}")
            print("\nSTATUS: REVIEW_HAS_FINDINGS")
        else:
            print("\n🎉 该 Commit 审查通过，没有发现新的问题意见。")
            print("STATUS: REVIEW_PASSED")
    else:
        print(f"⏳ 审查结果尚在生成中，尚未检测到针对 Commit {short_sha} 的 Codex Review。")
        print("STATUS: REVIEW_PENDING")

if __name__ == "__main__":
    main()
