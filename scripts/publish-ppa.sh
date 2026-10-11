#!/usr/bin/env bash
set -e

# ==============================================================================
# 人天神话 (MythManDay) - Launchpad PPA 自动打包与上传脚本
# ==============================================================================
# 用法：
#   ./scripts/publish-ppa.sh [PPA地址] [GPG_KEY_ID]
# 示例：
#   ./scripts/publish-ppa.sh ppa:ljl8086/ppa 665439FD49541297
# ==============================================================================

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PPA_TARGET="${1:-${PPA_TARGET:-ppa:ljl8086/ppa}}"
GPG_KEY_ID="${2:-${GPG_KEY_ID:-E7BAFF8B2E1CFD95}}"
# 注意：MAINTAINER_NAME 和 MAINTAINER_EMAIL 必须与 Launchpad 账号及 GPG 密钥的 UID 完全一致
MAINTAINER_NAME="${MAINTAINER_NAME:-sunny}"
MAINTAINER_EMAIL="${MAINTAINER_EMAIL:-ljl8086@gmail.com}"

PACKAGE_NAME="mythmanday"
BIN_NAME="mythmanday-app"
# 从 Cargo.toml 提取版本号（或使用环境变量 VERSION）
VERSION="${VERSION:-$(grep -m1 '^version =' "${PROJECT_DIR}/Cargo.toml" | sed -E 's/.*"([^"]+)".*/\1/')}"
# 目标 Ubuntu 发行版代号列表（默认覆盖当前主流 LTS 版本：22.04 jammy, 24.04 noble, 26.04 resolute）
UBUNTU_SERIES="${UBUNTU_SERIES:-jammy noble resolute}"

TARGET_BIN="${PROJECT_DIR}/target/release/${BIN_NAME}"
ICON_SRC="${PROJECT_DIR}/crates/myth-app/icons/icon.png"
PPA_WORK_ROOT="${PROJECT_DIR}/target/ppa-build"

echo "=========================================================="
echo "  🚀 准备构建并发布 ${PACKAGE_NAME} v${VERSION} 至 ${PPA_TARGET}"
echo "=========================================================="

# 1. 确保 Release 二进制已构建
if [ ! -f "${TARGET_BIN}" ]; then
  echo "❌ 未找到 ${TARGET_BIN}，请先编译前端和 Rust Release 产物！"
  exit 1
fi

rm -rf "${PPA_WORK_ROOT}"
mkdir -p "${PPA_WORK_ROOT}"

# 2. 针对每个 Ubuntu 发行版生成 Debian Source Package 并上传
PPA_REV="${PPA_REV:-2}"
PPA_OWNER_PATH=$(echo "${PPA_TARGET}" | sed -E 's|^ppa:([^/]+)/(.+)$|~\1/+archive/ubuntu/\2|')
for SERIES in ${UBUNTU_SERIES}; do
  DEB_VERSION="${VERSION}~${SERIES}${PPA_REV}"
  echo "----------------------------------------------------------"
  echo "📦 正在处理 Ubuntu [${SERIES}] (${PACKAGE_NAME} ${DEB_VERSION})..."
  echo "----------------------------------------------------------"

  # 幂等检查：如果 Launchpad PPA 中该发行版已存在当前版本的包，则自动跳过
  EXISTING=$(curl -s "https://api.launchpad.net/1.0/${PPA_OWNER_PATH}?ws.op=getPublishedSources&source_name=${PACKAGE_NAME}" || true)
  if echo "${EXISTING}" | grep -q "\"source_package_version\": \"${VERSION}~${SERIES}"; then
    echo "⏭️ [${SERIES}] ${PACKAGE_NAME} (${VERSION}~${SERIES}*) 已存在于 ${PPA_TARGET}，自动跳过！"
    continue
  fi

  SERIES_DIR="${PPA_WORK_ROOT}/${SERIES}"
  SRC_DIR="${SERIES_DIR}/${PACKAGE_NAME}-${VERSION}"
  mkdir -p "${SRC_DIR}/bin"
  mkdir -p "${SRC_DIR}/share/applications"
  mkdir -p "${SRC_DIR}/share/icons/hicolor/128x128/apps"
  mkdir -p "${SRC_DIR}/debian/source"

  # 拷贝二进制文件
  cp -f "${TARGET_BIN}" "${SRC_DIR}/bin/${BIN_NAME}"
  chmod 755 "${SRC_DIR}/bin/${BIN_NAME}"

  # 生成启动包装脚本
  cat << 'EOF' > "${SRC_DIR}/bin/mythmanday"
#!/usr/bin/env bash
export WEBKIT_DISABLE_COMPOSITING_MODE=1
exec /usr/bin/mythmanday-app "$@"
EOF
  chmod 755 "${SRC_DIR}/bin/mythmanday"

  # 拷贝图标
  if [ -f "${ICON_SRC}" ]; then
    cp -f "${ICON_SRC}" "${SRC_DIR}/share/icons/hicolor/128x128/apps/mythmanday.png"
  fi

  # 生成 desktop 文件
  cat << 'EOF' > "${SRC_DIR}/share/applications/mythmanday.desktop"
[Desktop Entry]
Name=人天神话
GenericName=开发者人天中台
Comment=开发者人天中台 · 多通道并发工时 · 录音与交付凭证一体化沉淀
Exec=/usr/bin/mythmanday
Icon=mythmanday
Terminal=false
Type=Application
Categories=Development;Office;Utility;
StartupWMClass=mythmanday-app
StartupNotify=true
Keywords=man-day;mythmanday;timesheet;billing;工时;人天;
EOF

  # 写入 debian/source/format (使用 native 格式以免去额外 orig.tar 校验问题)
  echo "3.0 (native)" > "${SRC_DIR}/debian/source/format"

  # 写入 debian/control
  cat << EOF > "${SRC_DIR}/debian/control"
Source: ${PACKAGE_NAME}
Section: utils
Priority: optional
Maintainer: ${MAINTAINER_NAME} <${MAINTAINER_EMAIL}>
Build-Depends: debhelper-compat (= 12)
Standards-Version: 4.5.0
Homepage: https://github.com/ljl8086/mythmanday

Package: ${PACKAGE_NAME}
Architecture: amd64
Depends: libc6, libwebkit2gtk-4.1-0 | libwebkit2gtk-4.0-37, libgtk-3-0, \${misc:Depends}
Description: MythManDay - Developer Man-Day & Timesheet Hub
 人天神话-开发者人天中台 (MythManDay)
 开发者人天中台 · 多通道并发工时 · 录音与交付凭证一体化沉淀。
 支持多通道并发计时、AI会议纪要、代码提交嗅探与专业Excel对账单导出。
EOF

  # 写入 debian/install
  cat << 'EOF' > "${SRC_DIR}/debian/install"
bin/mythmanday-app usr/bin/
bin/mythmanday usr/bin/
share/applications/mythmanday.desktop usr/share/applications/
share/icons/hicolor/128x128/apps/mythmanday.png usr/share/icons/hicolor/128x128/apps/
EOF

  # 写入 debian/rules (跳过 dh_strip 与 dh_shlibdeps，确保预编译 Rust 产物在各发行版顺畅打包)
  cat << 'EOF' > "${SRC_DIR}/debian/rules"
#!/usr/bin/make -f
%:
	dh $@

override_dh_strip:
	# Skip stripping prebuilt Rust binary

override_dh_shlibdeps:
	# Dependencies are explicitly declared in debian/control
EOF
  chmod +x "${SRC_DIR}/debian/rules"

  # 写入 debian/copyright
  cat << EOF > "${SRC_DIR}/debian/copyright"
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: ${PACKAGE_NAME}
Source: https://github.com/ljl8086/mythmanday

Files: *
Copyright: $(date +%Y) ${MAINTAINER_NAME} <${MAINTAINER_EMAIL}>
License: MIT
 Permission is hereby granted, free of charge, to any person obtaining a copy
 of this software and associated documentation files (the "Software"), to deal
 in the Software without restriction, including without limitation the rights
 to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 copies of the Software, and to permit persons to whom the Software is
 furnished to do so, subject to the following conditions:
 .
 The above copyright notice and this permission notice shall be included in all
 copies or substantial portions of the Software.
EOF

  # 写入 debian/changelog
  CHANGELOG_DATE="$(LC_ALL=C date -R)"
  cat << EOF > "${SRC_DIR}/debian/changelog"
${PACKAGE_NAME} (${DEB_VERSION}) ${SERIES}; urgency=medium

  * Release ${PACKAGE_NAME} v${VERSION} for Ubuntu ${SERIES}.

 -- ${MAINTAINER_NAME} <${MAINTAINER_EMAIL}>  ${CHANGELOG_DATE}
EOF

  # 构建并签名 Debian 源码包 (-S: source package only, -sa: include original source)
  pushd "${SRC_DIR}" > /dev/null
  debuild --no-lintian -S -sa -k"${GPG_KEY_ID}"
  popd > /dev/null

  # 推送到 Launchpad PPA（带自动重试，防止 Launchpad FTP 连续上传偶发 550 限流）
  CHANGES_FILE="${SERIES_DIR}/${PACKAGE_NAME}_${DEB_VERSION}_source.changes"
  if [ "${DRY_RUN:-0}" = "1" ]; then
    echo "🔍 [DRY_RUN] 已生成源码包：${CHANGES_FILE}（跳过 dput 上传）"
  else
    echo "⬆️ 正在上传 ${CHANGES_FILE} 到 ${PPA_TARGET} ..."
    for ATTEMPT in 1 2 3 4 5; do
      rm -f "${SERIES_DIR}"/*.upload
      if dput -f "${PPA_TARGET}" "${CHANGES_FILE}"; then
        echo "✅ [${SERIES}] 上传成功！等待 35 秒供 Launchpad 队列处理..."
        sleep 35
        break
      else
        echo "⚠️ [${SERIES}] 第 ${ATTEMPT} 次上传遇到 Launchpad FTP 队列锁定，等待 20 秒后重试..."
        sleep 20
      fi
    done
  fi
done

echo "=========================================================="
echo "  🎉 所有 Ubuntu 发行版源码包已处理完毕！"
echo "=========================================================="
