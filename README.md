<div align="center">

# SSMT4 Linux

第四代超简单 Linux 游戏工具箱  
Super Simple Linux Game Tools 4th

[![GitHub Stars](https://img.shields.io/github/stars/xiaobai01111/SSMT4-Linux?style=flat-square&logo=github)](https://github.com/xiaobai01111/SSMT4-Linux/stargazers)
[![GitHub Forks](https://img.shields.io/github/forks/xiaobai01111/SSMT4-Linux?style=flat-square&logo=github)](https://github.com/xiaobai01111/SSMT4-Linux/network)
[![GitHub Issues](https://img.shields.io/github/issues/xiaobai01111/SSMT4-Linux?style=flat-square&logo=github)](https://github.com/xiaobai01111/SSMT4-Linux/issues)
[![GitHub License](https://img.shields.io/github/license/xiaobai01111/SSMT4-Linux?style=flat-square)](./LICENSE)
[![GitHub Release](https://img.shields.io/github/v/release/xiaobai01111/SSMT4-Linux?style=flat-square)](https://github.com/xiaobai01111/SSMT4-Linux/releases)

</div>

![SSMT4 Preview](image.png)

## 项目信息

SSMT4 Linux 是一个基于 `Tauri + Vue 3 + Rust` 的 Linux 游戏工具箱，目标是统一管理游戏下载、启动、Wine/Proton、DXVK 与游戏配置。

当前项目内置/预置支持的游戏配置包括：

- `GenshinImpact`
- `HonkaiStarRail`
- `ZenlessZoneZero`
- `WutheringWaves`
- `HonkaiImpact3rd`
- `SnowbreakContainmentZone`

## 架构说明

整体架构分为三层：

1. 前端层（Vue）
- 页面与组件位于 `src/`
- 通过 `src/api.ts` 调用 Tauri 命令

2. 桌面桥接层（Tauri）
- 命令注册位于 `src-tauri/src/commands_registry.rs`
- 启动初始化位于 `src-tauri/src/bootstrap.rs`

3. 核心能力层（Rust）
- 游戏扫描与配置：`src-tauri/src/commands/game_scanner.rs`、`src-tauri/src/commands/game_config.rs`
- 启动与兼容层：`src-tauri/src/commands/game_launcher.rs`、`src-tauri/src/wine/`
- 设置与数据库：`src-tauri/src/commands/settings.rs`、`src-tauri/src/configs/database.rs`

关键资源目录：

- 游戏资源：`src-tauri/resources/Games/`
- 启动种子数据：`src-tauri/resources/bootstrap/`
- 版本信息：根目录 `version`、`version-log`

## 安装方式

### 方式 1：使用打包产物安装（推荐）

执行：

```bash
npm run package:linux
```

产物输出目录：

- `Installation package/`

包含：

- `.deb`
- `.rpm`
- `.pkg.tar.zst`（pacman）

安装示例：

```bash
# Debian / Ubuntu
sudo dpkg -i "Installation package/SSMT4-Linux_*.deb"

# Fedora / RHEL
sudo rpm -ivh "Installation package/SSMT4-Linux-*.rpm"

# Arch / Manjaro
sudo pacman -U "Installation package/ssmt-linux-bin-*.pkg.tar.zst"
```

### 方式 2：开发环境运行

环境要求：

- Node.js / npm
- Bun（推荐）
- Rust（stable）
- Tauri v2 依赖（`webkit2gtk`、`gtk3`、`libsoup3` 等）

执行：

```bash
npm install
npm run tauri dev
```

## 配置与数据目录

默认目录（Linux）：

- 配置目录：`~/.config/ssmt4`
- 数据目录：`~/.local/share/ssmt4`
- 缓存目录：`~/.cache/ssmt4`

如果你在设置中配置了自定义数据目录，游戏配置、下载内容等会使用自定义路径。

## 反馈与支持

- 使用文档（Wiki）：<https://github.com/xiaobai01111/SSMT4-Linux/wiki>
- Issues：<https://github.com/xiaobai01111/SSMT4-Linux/issues>
- 讨论/建议：<https://github.com/xiaobai01111/SSMT4-Linux/discussions>
- 反馈 QQ 群：`836016004`

## 仓库 Star

如果这个项目对你有帮助，欢迎点一个 Star：

- <https://github.com/xiaobai01111/SSMT4-Linux>

[![Star History Chart](https://api.star-history.com/svg?repos=xiaobai01111/SSMT4-Linux&type=Date)](https://star-history.com/#xiaobai01111/SSMT4-Linux&Date)

## License

本项目采用 `GPL-3.0` 许可证（以仓库实际 License 文件为准）。
