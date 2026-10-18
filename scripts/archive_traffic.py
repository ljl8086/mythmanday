#!/usr/bin/env python3
"""
Traffic Data Archiver for GitHub Repository
===========================================
Fetches Views, Clones, Referrers, and Paths from GitHub Traffic API,
merges them cumulatively to avoid GitHub's 14-day data eviction,
and generates a human-readable traffic report.
"""

import json
import os
import re
import subprocess
import sys
import urllib.request
import urllib.error
from datetime import datetime, timezone

def get_env_or_git_token():
    token = os.environ.get("TRAFFIC_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if token:
        return token.strip()
    try:
        res = subprocess.run(["gh", "auth", "token"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, check=True)
        return res.stdout.strip()
    except Exception:
        return None

def get_repository():
    repo = os.environ.get("GITHUB_REPOSITORY")
    if repo:
        return repo.strip()
    try:
        res = subprocess.run(["git", "config", "--get", "remote.origin.url"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, check=True)
        url = res.stdout.strip()
        # Match git@github.com:owner/repo.git or https://github.com/owner/repo.git
        m = re.search(r"github\.com[:/]([^/]+)/([^/\.]+)(?:\.git)?", url)
        if m:
            return f"{m.group(1)}/{m.group(2)}"
    except Exception:
        pass
    return "ljl8086/mythmanday"

def fetch_api(endpoint, token):
    url = f"https://api.github.com/{endpoint.lstrip('/')}"
    req = urllib.request.Request(url)
    req.add_header("Accept", "application/vnd.github+json")
    req.add_header("X-GitHub-Api-Version", "2022-11-28")
    req.add_header("User-Agent", "MythManDay-Traffic-Archiver")
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        print(f"❌ HTTP Error {e.code} for {url}: {e.read().decode('utf-8', errors='ignore')}", file=sys.stderr)
        return None
    except Exception as e:
        print(f"❌ Failed to fetch {url}: {e}", file=sys.stderr)
        return None

def load_json(filepath, default):
    if os.path.exists(filepath):
        try:
            with open(filepath, "r", encoding="utf-8") as f:
                return json.load(f)
        except Exception as e:
            print(f"⚠️ Warning: Could not read {filepath}: {e}")
    return default

def save_json(filepath, data):
    os.makedirs(os.path.dirname(filepath), exist_ok=True)
    with open(filepath, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2, ensure_ascii=False)
        f.write("\n")

def merge_time_series(existing_items, new_items, key_field="views"):
    merged_dict = {}
    for item in existing_items:
        ts = item.get("timestamp")
        if ts:
            merged_dict[ts] = item

    for item in new_items:
        ts = item.get("timestamp")
        if not ts:
            continue
        if ts in merged_dict:
            merged_dict[ts]["count"] = max(merged_dict[ts].get("count", 0), item.get("count", 0))
            merged_dict[ts]["uniques"] = max(merged_dict[ts].get("uniques", 0), item.get("uniques", 0))
        else:
            merged_dict[ts] = item

    sorted_list = sorted(merged_dict.values(), key=lambda x: x.get("timestamp", ""))
    total_count = sum(x.get("count", 0) for x in sorted_list)
    total_uniques = sum(x.get("uniques", 0) for x in sorted_list)
    return {
        "count": total_count,
        "uniques": total_uniques,
        key_field: sorted_list
    }

def merge_referrers(existing_refs, new_refs):
    ref_map = {r["referrer"]: r for r in existing_refs if "referrer" in r}
    for item in new_refs:
        name = item.get("referrer")
        if not name:
            continue
        if name in ref_map:
            ref_map[name]["count"] = max(ref_map[name].get("count", 0), item.get("count", 0))
            ref_map[name]["uniques"] = max(ref_map[name].get("uniques", 0), item.get("uniques", 0))
            ref_map[name]["last_seen"] = datetime.now(timezone.utc).strftime("%Y-%m-%d")
        else:
            item["last_seen"] = datetime.now(timezone.utc).strftime("%Y-%m-%d")
            ref_map[name] = item
    return sorted(ref_map.values(), key=lambda x: x.get("count", 0), reverse=True)

def generate_markdown_report(target_file, repo, views_data, clones_data, referrers_data, paths_data, repo_info):
    now_str = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")
    stars = repo_info.get("stargazers_count", 0)
    forks = repo_info.get("forks_count", 0)
    
    total_views = views_data.get("count", 0)
    total_clones = clones_data.get("count", 0)
    
    views_list = views_data.get("views", [])
    clones_list = clones_data.get("clones", [])
    
    # Combined date map
    dates = sorted(list(set(
        [v.get("timestamp", "")[:10] for v in views_list if v.get("timestamp")] +
        [c.get("timestamp", "")[:10] for c in clones_list if c.get("timestamp")]
    )), reverse=True)

    view_map = {v.get("timestamp", "")[:10]: v for v in views_list}
    clone_map = {c.get("timestamp", "")[:10]: c for c in clones_list}

    md = []
    md.append(f"# 📊 仓库流量统计历史 (Traffic Archive)")
    md.append(f"\n> 仓库：[`{repo}`](https://github.com/{repo})  ")
    md.append(f"> 最后更新时间：`{now_str}`  ")
    md.append(f"> 历史数据自动累加存档，防止 GitHub 14 天滚动清理丢弃。")
    md.append("")
    md.append("## 📌 历史总览")
    md.append("")
    md.append("| 指标 | 历史累计总量 | 说明 |")
    md.append("| :--- | :---: | :--- |")
    md.append(f"| **⭐ GitHub Stars** | **{stars}** | 当前关注与点赞总数 |")
    md.append(f"| **🍴 Forks** | **{forks}** | 复刻分支数 |")
    md.append(f"| **👀 网页总浏览 (Views)** | **{total_views}** | 历史累计页面展示与访问 |")
    md.append(f"| **🚀 代码总克隆 (Clones)** | **{total_clones}** | 历史累计 git clone 下载总数 |")
    md.append("")
    md.append("## 📅 每日流量明细")
    md.append("")
    md.append("| 日期 | 浏览量 (Views) | 独立访客 (Visitors) | 克隆数 (Clones) | 独立克隆者 (Cloners) |")
    md.append("| :---: | :---: | :---: | :---: | :---: |")

    for d in dates:
        v = view_map.get(d, {})
        c = clone_map.get(d, {})
        vc = v.get("count", 0)
        vu = v.get("uniques", 0)
        cc = c.get("count", 0)
        cu = c.get("uniques", 0)
        md.append(f"| {d} | {vc} | {vu} | {cc} | {cu} |")

    if referrers_data:
        md.append("")
        md.append("## 🌐 热门引流渠道 (Top Referrers)")
        md.append("")
        md.append("| 渠道来源 (Referrer) | 浏览次数 (Views) | 独立访客 (Uniques) | 最近更新 |")
        md.append("| :--- | :---: | :---: | :---: |")
        for r in referrers_data:
            md.append(f"| `{r.get('referrer')}` | {r.get('count', 0)} | {r.get('uniques', 0)} | {r.get('last_seen', '-')} |")

    if paths_data:
        md.append("")
        md.append("## 📄 热门内容路径 (Top Paths)")
        md.append("")
        md.append("| 路径 (Path) | 浏览次数 (Views) | 独立访客 (Uniques) |")
        md.append("| :--- | :---: | :---: |")
        for p in paths_data[:15]:
            md.append(f"| `{p.get('path')}` | {p.get('count', 0)} | {p.get('uniques', 0)} |")

    with open(target_file, "w", encoding="utf-8") as f:
        f.write("\n".join(md) + "\n")

def main():
    token = get_env_or_git_token()
    if not token:
        print("⚠️ 未找到 GITHUB_TOKEN / TRAFFIC_TOKEN 且无法从 gh 获取。以只读/匿名模式尝试...", file=sys.stderr)

    repo = get_repository()
    print(f"🚀 开始采集仓库流量数据: {repo}")

    data_dir = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "traffic-data")
    os.makedirs(data_dir, exist_ok=True)

    views_file = os.path.join(data_dir, "views.json")
    clones_file = os.path.join(data_dir, "clones.json")
    referrers_file = os.path.join(data_dir, "referrers.json")
    paths_file = os.path.join(data_dir, "paths.json")
    readme_file = os.path.join(data_dir, "README.md")

    # 1. Fetch Views
    new_views = fetch_api(f"repos/{repo}/traffic/views", token)
    if new_views and "views" in new_views:
        old_views = load_json(views_file, {"count": 0, "uniques": 0, "views": []}).get("views", [])
        merged_views = merge_time_series(old_views, new_views["views"], key_field="views")
        save_json(views_file, merged_views)
        print(f"✅ Views 流量已更新并归档: 累计 {merged_views['count']} 次")
    else:
        merged_views = load_json(views_file, {"count": 0, "uniques": 0, "views": []})

    # 2. Fetch Clones
    new_clones = fetch_api(f"repos/{repo}/traffic/clones", token)
    if new_clones and "clones" in new_clones:
        old_clones = load_json(clones_file, {"count": 0, "uniques": 0, "clones": []}).get("clones", [])
        merged_clones = merge_time_series(old_clones, new_clones["clones"], key_field="clones")
        save_json(clones_file, merged_clones)
        print(f"✅ Clones 流量已更新并归档: 累计 {merged_clones['count']} 次")
    else:
        merged_clones = load_json(clones_file, {"count": 0, "uniques": 0, "clones": []})

    # 3. Fetch Referrers
    new_referrers = fetch_api(f"repos/{repo}/traffic/popular/referrers", token)
    if new_referrers is not None and isinstance(new_referrers, list):
        old_referrers = load_json(referrers_file, [])
        merged_referrers = merge_referrers(old_referrers, new_referrers)
        save_json(referrers_file, merged_referrers)
        print(f"✅ Referrers 引流来源已更新: 共 {len(merged_referrers)} 个来源")
    else:
        merged_referrers = load_json(referrers_file, [])

    # 4. Fetch Paths
    new_paths = fetch_api(f"repos/{repo}/traffic/popular/paths", token)
    if new_paths is not None and isinstance(new_paths, list):
        save_json(paths_file, new_paths)
        print(f"✅ 热门路径已更新: 共 {len(new_paths)} 条热门内容")
    else:
        new_paths = load_json(paths_file, [])

    # 5. Fetch Repo Basic Info
    repo_info = fetch_api(f"repos/{repo}", token) or {}

    # 6. Generate Markdown Report
    generate_markdown_report(readme_file, repo, merged_views, merged_clones, merged_referrers, new_paths, repo_info)
    print(f"🎉 流量归档看板生成完成：{readme_file}")

if __name__ == "__main__":
    main()
