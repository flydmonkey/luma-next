# Luma Next {{VERSION}}

Luma Next 是 Windows 10/11 x64 屏幕录制工具，通过系统托盘和默认浏览器控制。本版本包含
libobs、WebUI 以及固定版本的 FFmpeg/ffprobe，不依赖 WebView2。

## 安装

1. 下载并完整解压 `{{ZIP_NAME}}`。
2. 双击 ZIP 根目录的 `Install.cmd`；无需管理员权限。
3. 安装完成后从系统托盘打开控制页。卸载可双击安装目录或解压包内的 `Uninstall.cmd`；
   卸载会保留录像和本地设置。

## 完整性与签名

- Authenticode: **{{SIGNATURE_STATUS}}**
- SHA-256: `{{SHA256}}`
- 另附 `SHA256SUMS`，安装前可用 `Get-FileHash` 核对。
- 无签名证书时该版本会明确标为 Unsigned，Windows SmartScreen 警告属于预期，不代表已经签名。

## 已知限制

- 硬件编码器依赖对应 Intel/AMD/NVIDIA GPU 与驱动；不可用时 API/UI 会显示原因，并可诚实降级 x264。
- 全屏播放器的硬件叠加层在部分显卡/播放器组合中可能出现黑边或黑屏；这属于捕获兼容性限制，
  与停录状态机和成片收尾无关。
- 游戏捕获受游戏、反作弊和权限影响；不提供绕过反作弊的注入能力。
- 多显示器、混合 DPI、真实游戏及全局热键应按 `docs/RELEASE_SMOKE.md` 在目标机器人工复核。

本发行包包含并链接 libobs，并捆绑 GPLv3 FFmpeg；整体以 GPL-3.0-or-later 分发。源码：
https://github.com/flydmonkey/luma-next
