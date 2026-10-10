#!/usr/bin/env bash
set -e

# ==============================================================================
# 人天神话 (MythManDay) - 卸载脚本
# ==============================================================================

INSTALL_PREFIX="${HOME}/.local"
BIN_INSTALL_DIR="${INSTALL_PREFIX}/bin"
APP_INSTALL_DIR="${INSTALL_PREFIX}/share/applications"
ICON_INSTALL_DIR="${INSTALL_PREFIX}/share/icons/hicolor/128x128/apps"

echo "🗑️ 正在卸载 人天神话 (MythManDay)..."

rm -f "${BIN_INSTALL_DIR}/mythmanday-app"
rm -f "${BIN_INSTALL_DIR}/mythmanday"
rm -f "${APP_INSTALL_DIR}/mythmanday.desktop"
rm -f "${ICON_INSTALL_DIR}/mythmanday.png"

if command -v update-desktop-database > /dev/null 2>&1; then
  update-desktop-database "${APP_INSTALL_DIR}" 2>/dev/null || true
fi

echo "✅ 卸载完成！用户数据文件保存在 ~/.config/mythmanday 中，未作删除。"
