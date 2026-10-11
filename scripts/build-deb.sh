#!/usr/bin/env bash
set -e

# ==============================================================================
# 人天神话 (MythManDay) - 自动打包 Debian/Ubuntu .deb 安装包脚本
# ==============================================================================

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_NAME="mythmanday-app"
PACKAGE_NAME="mythmanday"
VERSION="0.1.0"
ARCH="amd64"

BUILD_DIR="${PROJECT_DIR}/target/deb-build"
DEB_ROOT="${BUILD_DIR}/${PACKAGE_NAME}_${VERSION}_${ARCH}"
DIST_DIR="${PROJECT_DIR}/dist"
TARGET_BIN="${PROJECT_DIR}/target/release/${BIN_NAME}"
ICON_SRC="${PROJECT_DIR}/crates/myth-app/icons/icon.png"

echo "=========================================================="
echo "      📦 开始制作 ${PACKAGE_NAME} v${VERSION} .deb 安装包       "
echo "=========================================================="

# 1. 确保 Release 二进制程序已构建
if [ ! -f "${TARGET_BIN}" ]; then
  echo "🔨 构建前端静态产物..."
  export PATH="${HOME}/.local/share/pnpm:${HOME}/.local/bin:${HOME}/.cargo/bin:$PATH"
  export PKG_CONFIG_PATH="${HOME}/.local/usr/lib/x86_64-linux-gnu/pkgconfig:$PKG_CONFIG_PATH"
  export LIBRARY_PATH="${HOME}/.local/usr/lib/x86_64-linux-gnu:$LIBRARY_PATH"
  export LD_LIBRARY_PATH="${HOME}/.local/usr/lib/x86_64-linux-gnu:$LD_LIBRARY_PATH"

  cd "${PROJECT_DIR}/frontend"
  pnpm build

  echo "🦀 编译 Rust Release 产物..."
  cd "${PROJECT_DIR}"
  cargo build --release --bin "${BIN_NAME}"
fi

# 2. 清理旧构建目录
rm -rf "${DEB_ROOT}"
mkdir -p "${DEB_ROOT}/DEBIAN"
mkdir -p "${DEB_ROOT}/usr/bin"
mkdir -p "${DEB_ROOT}/usr/share/applications"
mkdir -p "${DEB_ROOT}/usr/share/icons/hicolor/128x128/apps"
mkdir -p "${DIST_DIR}"

# 3. 生成 DEBIAN/control 文件
cat << EOF > "${DEB_ROOT}/DEBIAN/control"
Package: ${PACKAGE_NAME}
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${ARCH}
Maintainer: ljl8086 <ljl8086@gmail.com>
Depends: libc6, libwebkit2gtk-4.1-0 | libwebkit2gtk-4.0-37, libgtk-3-0
Description: 人天神话-开发者人天中台 (MythManDay)
 开发者人天中台 · 多通道并发工时 · 录音与交付凭证一体化沉淀。
 支持多通道并发计时、AI会议纪要、代码提交嗅探与专业Excel对账单导出。
EOF

# 4. 拷贝可执行文件
cp -f "${TARGET_BIN}" "${DEB_ROOT}/usr/bin/${BIN_NAME}"
chmod +x "${DEB_ROOT}/usr/bin/${BIN_NAME}"

# 5. 写入启动包装器
cat << EOF > "${DEB_ROOT}/usr/bin/mythmanday"
#!/usr/bin/env bash
export WEBKIT_DISABLE_COMPOSITING_MODE=1
exec /usr/bin/${BIN_NAME} "\$@"
EOF
chmod +x "${DEB_ROOT}/usr/bin/mythmanday"

# 6. 拷贝图标
if [ -f "${ICON_SRC}" ]; then
  cp -f "${ICON_SRC}" "${DEB_ROOT}/usr/share/icons/hicolor/128x128/apps/mythmanday.png"
fi

# 7. 生成 desktop 快捷方式
cat << EOF > "${DEB_ROOT}/usr/share/applications/mythmanday.desktop"
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
chmod 644 "${DEB_ROOT}/usr/share/applications/mythmanday.desktop"

# 8. 打包为 .deb 文件
OUTPUT_DEB="${DIST_DIR}/${PACKAGE_NAME}_${VERSION}_${ARCH}.deb"
echo "🛠️ 正在生成 deb 文件..."
dpkg-deb --build "${DEB_ROOT}" "${OUTPUT_DEB}"

echo "=========================================================="
echo "  🎉 打包完成！"
echo "  - 安装包路径：${OUTPUT_DEB}"
echo "  - 文件大小：$(du -h "${OUTPUT_DEB}" | cut -f1)"
echo "  - 安装命令：sudo dpkg -i ${OUTPUT_DEB}"
echo "=========================================================="
