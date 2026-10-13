#!/usr/bin/env bash
set -e

# ==============================================================================
# 人天神话 (MythManDay) - Linux 一键安装脚本
# 适用平台：Ubuntu / Debian / Deepin / Fedora / Arch 等主流 Linux 桌面环境
# ==============================================================================

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_NAME="mythmanday-app"
APP_NAME="MythManDay"
APP_TITLE="人天神话"
VERSION="0.1.0"

INSTALL_PREFIX="${HOME}/.local"
BIN_INSTALL_DIR="${INSTALL_PREFIX}/bin"
APP_INSTALL_DIR="${INSTALL_PREFIX}/share/applications"
ICON_INSTALL_DIR="${INSTALL_PREFIX}/share/icons/hicolor/128x128/apps"
TARGET_BIN="${PROJECT_DIR}/target/release/${BIN_NAME}"
ICON_SRC="${PROJECT_DIR}/crates/myth-app/icons/icon.png"

echo "=========================================================="
echo "           🚀 正在安装 ${APP_TITLE} v${VERSION}           "
echo "=========================================================="

# 1. 检查或编译 Release 二进制文件
FORCE_REBUILD=0
if [ "$1" = "--rebuild" ] || [ "$1" = "-f" ] || [ "$1" = "--force" ] || [ "${REBUILD:-0}" = "1" ]; then
  FORCE_REBUILD=1
fi

if [ ! -f "${TARGET_BIN}" ] || [ "${FORCE_REBUILD}" -eq 1 ]; then
  echo "📦 正在执行前端构建与 Rust Release 编译..."
  export PATH="${HOME}/.local/share/pnpm:${HOME}/.local/bin:${HOME}/.cargo/bin:$PATH"
  export PKG_CONFIG_PATH="${HOME}/.local/usr/lib/x86_64-linux-gnu/pkgconfig:$PKG_CONFIG_PATH"
  export LIBRARY_PATH="${HOME}/.local/usr/lib/x86_64-linux-gnu:$LIBRARY_PATH"
  export LD_LIBRARY_PATH="${HOME}/.local/usr/lib/x86_64-linux-gnu:$LD_LIBRARY_PATH"

  # 构建前端资源
  echo "🔨 构建前端静态产物..."
  cd "${PROJECT_DIR}/frontend"
  pnpm build

  # 构建后端独立程序
  echo "🦀 编译 Rust 高性能 Release 程序..."
  cd "${PROJECT_DIR}"
  cargo build --release --bin "${BIN_NAME}"
fi

# 2. 创建用户级目录
echo "📁 创建用户级应用目录..."
mkdir -p "${BIN_INSTALL_DIR}"
mkdir -p "${APP_INSTALL_DIR}"
mkdir -p "${ICON_INSTALL_DIR}"

# 3. 安装可执行文件
echo "🚚 安装应用程序到 ${BIN_INSTALL_DIR}/${BIN_NAME}..."
cp -f "${TARGET_BIN}" "${BIN_INSTALL_DIR}/${BIN_NAME}"
chmod +x "${BIN_INSTALL_DIR}/${BIN_NAME}"

# 4. 安装启动脚本包装器 (确保环境变数如 WebKit 渲染优化正确加载)
LAUNCHER="${BIN_INSTALL_DIR}/mythmanday"
cat << EOF > "${LAUNCHER}"
#!/usr/bin/env bash
export WEBKIT_DISABLE_COMPOSITING_MODE=1
exec "${BIN_INSTALL_DIR}/${BIN_NAME}" "\$@"
EOF
chmod +x "${LAUNCHER}"

# 5. 安装图标
echo "🎨 安装高分辨率应用图标..."
if [ -f "${ICON_SRC}" ]; then
  cp -f "${ICON_SRC}" "${ICON_INSTALL_DIR}/mythmanday.png"
fi

# 6. 生成桌面启动快捷方式 (.desktop)
echo "🖥️ 注册桌面应用菜单项..."
DESKTOP_FILE="${APP_INSTALL_DIR}/mythmanday.desktop"
cat << EOF > "${DESKTOP_FILE}"
[Desktop Entry]
Name=人天神话
GenericName=开发者人天中台
Comment=开发者人天中台 · 多通道并发工时 · 录音与交付凭证一体化沉淀
Exec=${LAUNCHER}
Icon=mythmanday
Terminal=false
Type=Application
Categories=Development;Office;Utility;
StartupWMClass=mythmanday-app
StartupNotify=true
Keywords=man-day;mythmanday;timesheet;billing;工时;人天;
EOF
chmod +x "${DESKTOP_FILE}"

# 7. 刷新桌面数据库
if command -v update-desktop-database > /dev/null 2>&1; then
  update-desktop-database "${APP_INSTALL_DIR}" 2>/dev/null || true
fi

echo "=========================================================="
echo "  🎉 安装成功！"
echo "  - 可执行程序：${BIN_INSTALL_DIR}/${BIN_NAME}"
echo "  - 终端启动命令：mythmanday"
echo "  - 桌面应用菜单：已注册「人天神话 (MythManDay)」"
echo "  (您可以在系统应用列表或通过快捷键搜索 MythManDay 直接启动)"
echo "=========================================================="
