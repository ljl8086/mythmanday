#!/bin/bash
set -euo pipefail

# 检查当前分支关联的 PR 编号
PR_NUM=$(gh pr view --json number -q '.number' 2>/dev/null || echo "")
if [ -z "$PR_NUM" ]; then
    echo "未检测到关联的 Pull Request。"
    exit 0
fi

HEAD_SHA=$(git rev-parse HEAD)
SHORT_SHA=${HEAD_SHA:0:10}

echo "========================================================"
echo "PR: #$PR_NUM | 待检查 Commit: $SHORT_SHA"
echo "========================================================"

# 获取最新的 review 记录
REVIEWS=$(gh api "repos/:owner/:repo/pulls/$PR_NUM/reviews" 2>/dev/null || echo "[]")

# 检查是否有针对最新 commit 的 review
LATEST_REVIEW=$(python3 -c "
import json, sys
reviews = json.loads('''$REVIEWS''')
matching = [r for r in reviews if r.get('commit_id', '').startswith('$SHORT_SHA') or '$SHORT_SHA' in r.get('body', '')]
if matching:
    latest = matching[-1]
    print(json.dumps({'id': latest['id'], 'user': latest['user']['login'], 'state': latest['state'], 'submitted_at': latest['submitted_at']}))
else:
    print('')
")

if [ -n "$LATEST_REVIEW" ]; then
    echo "发现针对 Commit $SHORT_SHA 的评审结果："
    echo "$LATEST_REVIEW"
    
    # 拉取具体的 review comments
    COMMENTS=$(gh api "repos/:owner/:repo/pulls/$PR_NUM/comments" 2>/dev/null || echo "[]")
    python3 -c "
import json
comments = json.loads('''$COMMENTS''')
matching_comments = [c for c in comments if c.get('commit_id', '').startswith('$SHORT_SHA') or c.get('original_commit_id', '').startswith('$SHORT_SHA')]
print(f'共检测到针对此 Commit 的意见评论: {len(matching_comments)} 条')
for idx, c in enumerate(matching_comments, 1):
    path = c.get('path', '')
    line = c.get('line', '')
    body = c.get('body', '')
    first_line = body.split('\n')[0] if body else ''
    print(f'{idx}. [{path}:{line}] {first_line}')
"
    echo "STATUS: REVIEW_READY"
else
    echo "STATUS: REVIEW_PENDING"
fi
