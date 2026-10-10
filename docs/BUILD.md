# 跨平台编译与打包发布指南 (Multi-Platform Build Guide)

> **软件全称**：人天神话-开发者人天中台 [简称：MythManDay / ParaDay] V1.0  
> 本文档介绍如何在 **Linux**、**Windows** 和 **macOS** 平台上编译、打包与安装 **人天神话-开发者人天中台 (MythManDay)** 客户端。

---

## 目录
- [技术栈与前提条件](#技术栈与前提条件)
- [1. Linux 平台编译与打包](#1-linux-平台编译与打包)
  - [系统依赖安装](#系统依赖安装)
  - [源码一键安装 (本机)](#源码一键安装-本机)
  - [打包 Debian/Ubuntu (.deb) 安装包](#打包-debianubuntu-deb-安装包)
  - [跨发行版通用性建议](#跨发行版通用性建议)
- [2. Windows 平台编译与打包](#2-windows-平台编译与打包)
  - [环境准备](#环境准备)
  - [编译与运行](#编译与运行)
  - [生成 Windows 安装包 (.exe / .msi)](#生成-windows-安装包-exe--msi)
- [3. macOS 平台编译与打包 (Apple Silicon & Intel)](#3-macos-平台编译与打包-apple-silicon--intel)
  - [环境准备](#环境准备-1)
  - [编译与运行](#编译与运行-1)
  - [生成 macOS 安装包 (.dmg / .app)](#生成-macos-安装包-dmg--app)
- [4. CI/CD 自动多平台构建 (GitHub Actions 示例)](#4-cicd-自动多平台构建-github-actions-示例)

---

## 技术栈与前提条件

本项目采用 **Tauri 2.0 + Rust 核心 + React 19 + TypeScript + SQLite** 架构：
- **Rust Toolchain**: 1.80+ (推荐使用 `rustup` 安装 Stable 版本)
- **Node.js**: 20.x 或 22.x+
- **包管理器**: `pnpm` (推荐 9.x / 10.x)

---

## 1. Linux 平台编译与打包

### 系统依赖安装

不同 Linux 发行版需要安装 WebKitGTK 与 GTK3 开发库：

#### Ubuntu / Debian / Deepin / Mint
```bash
sudo apt update
sudo apt install -y \
    build-essential \
    curl \
    wget \
    libwebkit2gtk-4.1-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    libasound2-dev \
    libssl-dev
```
*(注：较老版本的 Ubuntu 20.04 使用 `libwebkit2gtk-4.0-dev`)*

#### Fedora / RHEL
```bash
sudo dnf install -y \
    gcc \
    gcc-c++ \
    webkit2gtk4.1-devel \
    gtk3-devel \
    libayatana-appindicator3-devel \
    librsvg2-devel \
    alsa-lib-devel \
    openssl-devel
```

#### Arch Linux / Manjaro
```bash
sudo pacman -S --needed \
    base-devel \
    webkit2gtk-4.1 \
    gtk3 \
    libayatana-appindicator \
    librsvg \
    alsa-lib \
    openssl
```

---

### 源码一键安装 (本机)
适合在个人工作机上直接编译并注册到系统启动菜单：

```bash
# 赋予执行权限并运行
chmod +x ./install.sh
./install.sh
```
- **安装位置**：`~/.local/bin/mythmanday-app`
- **命令行快捷入口**：`mythmanday`
- **桌面应用快捷方式**：自动写入 `~/.local/share/applications/mythmanday.desktop`，可直接在应用启动台搜索图标启动。
- **一键卸载**：运行 `./uninstall.sh`。

---

### 打包 Debian/Ubuntu (.deb) 安装包
用于生成标准安装包分发给其他 Debian/Ubuntu 用户：

```bash
# 执行打包脚本
chmod +x ./scripts/build-deb.sh
./scripts/build-deb.sh
```
- **生成产物**：`dist/mythmanday_1.0.0_amd64.deb`
- **目标机安装**：
  ```bash
  sudo dpkg -i mythmanday_1.0.0_amd64.deb
  ```

---

### 跨发行版通用性建议
- **glibc 兼容性**：在哪个版本的 Linux 上编译，生成的二进制通常只能运行在**相同或更新版本**的 glibc 系统上。
- **推荐策略**：若需对外公开发布通用的 Linux 二进制，建议在较旧版本的基底（如 **Ubuntu 22.04 LTS** 的 Docker 容器）中进行编译，或构建为 **AppImage / Flatpak**。

---

## 2. Windows 平台编译与打包

在 Windows 10/11 (64位) 系统上：

### 环境准备
1. 安装 **[C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)** (勾选 "Desktop development with C++")。
2. 安装 **[Rust](https://www.rust-lang.org/)** (选择 `x86_64-pc-windows-msvc`)。
3. 安装 **Node.js** 与 **pnpm**。
4. Windows 10/11 默认自带 WebView2 运行时；如为企业版老系统，可下载安装 [WebView2 Evergreen Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)。

### 编译与运行
```powershell
# 1. 安装前端依赖并构建前端产物
cd frontend
pnpm install
pnpm build
cd ..

# 2. 编译并运行 Debug 版
cargo run --bin mythmanday-app

# 3. 编译独立的 Release 二进制
cargo build --release --bin mythmanday-app
```
编译产物位于：`target\release\mythmanday-app.exe`。双击即可独立运行。

### 生成 Windows 安装包 (.exe / .msi)
使用 Tauri CLI 一键打包：
```powershell
cd frontend
pnpm tauri build
```
打包成功后，将在 `target\release\bundle\msi\` 和 `nsis\` 目录下生成：
- `MythManDay_1.0.0_x64-setup.exe` (NSIS 安装向导)
- `MythManDay_1.0.0_x64_en-US.msi` (MSI 安装包)

---

## 3. macOS 平台编译与打包 (Apple Silicon & Intel)

在 macOS (macOS 12+，支持 M1/M2/M3/M4 与 Intel x86_64)：

### 环境准备
1. 安装 Xcode 命令行工具：
   ```bash
   xcode-select --install
   ```
2. 安装 Rust：
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
3. 安装 Node.js 与 pnpm：
   ```bash
   brew install node pnpm
   ```

### 编译与运行
```bash
# 1. 前端构建
cd frontend
pnpm install
pnpm build
cd ..

# 2. 本地调试运行
cargo run --bin mythmanday-app

# 3. 编译独立的 Release 产物
cargo build --release --bin mythmanday-app
```

### 生成 macOS 安装包 (.dmg / .app)
运行 Tauri 打包命令：
```bash
cd frontend
pnpm tauri build --bundles dmg,app
```
打包成功后，在 `target/release/bundle/dmg/` 目录下将生成：
- **`MythManDay_1.0.0_aarch64.dmg`** (Apple Silicon) 或 **`x64.dmg`** (Intel)
- 用户双击挂载后，直接将应用拖拽到 `Applications` 目录即可完成安装。

> **提示（通用二进制 Universal Binary）**：
> 如需构建同时兼容 M 系列芯片与 Intel 的 Universal 安装包：
> ```bash
> rustup target add aarch64-apple-darwin x86_64-apple-darwin
> cd frontend && pnpm tauri build --target universal-apple-darwin
> ```

---

## 4. CI/CD 自动多平台构建 (GitHub Actions 示例)

建议在代码仓库中添加 `.github/workflows/release.yml`，在每次打 tag 时由云端自动矩阵编译 Windows、macOS 和 Linux 三端安装包：

```yaml
name: Release Multi-Platform App

on:
  push:
    tags:
      - 'v*'

jobs:
  release:
    permissions:
      contents: write
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: 'macos-latest'
            args: '--bundles dmg'
          - platform: 'ubuntu-22.04'
            args: '--bundles deb'
          - platform: 'windows-latest'
            args: '--bundles nsis'

    runs-on: ${{ matrix.platform }}
    steps:
      - uses: actions/checkout@v4

      - name: Setup node
        uses: actions/setup-node@v4
        with:
          node-version: 22

      - name: Setup pnpm
        uses: pnpm/action-setup@v4
        with:
          version: 10

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Install Linux dependencies
        if: matrix.platform == 'ubuntu-22.04'
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libasound2-dev libssl-dev

      - name: Install frontend dependencies & build
        run: |
          cd frontend
          pnpm install
          pnpm build

      - name: Build Tauri bundles
        run: |
          cd frontend
          pnpm tauri build ${{ matrix.args }}

      - name: Upload Release Assets
        uses: softprops/action-gh-release@v2
        with:
          files: |
            target/release/bundle/dmg/*.dmg
            target/release/bundle/nsis/*.exe
            target/release/bundle/deb/*.deb
```

---

## 常见问题与排除

1. **Linux 提示 WebKit 不兼容或白屏**：
   启动时加环境变量运行：`WEBKIT_DISABLE_COMPOSITING_MODE=1 mythmanday`（本项目的桌面快捷方式已默认包含此配置）。
2. **Windows 无法麦克风录音**：
   检查系统设置中的“麦克风隐私权限”，确保允许桌面应用访问麦克风。
3. **数据文件存储在哪里？**
   - **Linux**: `~/.config/mythmanday/mythmanday.db`
   - **Windows**: `C:\Users\<用户名>\AppData\Roaming\mythmanday\mythmanday.db`
   - **macOS**: `~/Library/Application Support/mythmanday/mythmanday.db`
