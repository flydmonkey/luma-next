# Luma Next

本地屏幕录制：**嵌入 libobs 的 Rust 引擎** + **HTTP API** + **WebUI**。

旧版 Luma（../record，WinUI + 自研 Media Foundation）已归档，不再作为主开发线。原因见 [docs/BOUNDARY.md](docs/BOUNDARY.md)：多厂商硬编与 MF 异步样本生命周期成本过高，改由 OBS 管线承担。

## 架构

```
WebUI (default browser)  --HTTP-->  luma-engine (Rust + system tray)
                                |
                            libobs (capture / encode / mux)
                                |
                            output file (e.g. mp4)
```

- 单进程引擎同时拥有 HTTP、libobs 与 Windows 系统托盘；浏览器只是控制客户端
- 平台：Windows 优先验收；结构预留跨平台
- 许可：因链接 libobs，本项目按 **GPL** 约束分发（详见下文）

## 许可（必读）

本仓库链接 **libobs（GPL）**。二进制分发需遵守 GPL 义务（源码提供、许可声明等）。不要在未做合规方案前，把本项目当作闭源商业一体包发布。

## 仓库布局

```
luma-next/
  crates/          # Rust workspace
    luma-engine/   # localhost HTTP 引擎
  web/             # 正式 WebUI（录制 / 片库 / 设置）
  scripts/         # 可重复录制回归
  docs/
    BOUNDARY.md    # 技术边界
    CADENCE.md     # 开发节奏与里程碑
    TRAY.md        # 系统托盘运行与验收
    superpowers/specs/  # 设计规格
  README.md
```

## 要求（开发机）

- Windows 10/11（第一期验收平台）
- Rust toolchain（stable）
- 本机 OBS RelWithDebInfo 构建（默认读取
  `C:\Users\Administrator\Projects\obs-studio\build_x64\rundir\RelWithDebInfo`）
- `ffprobe.exe` 在 `PATH` 中，用于 stop 后强制校验音视频流与时长

## 快速开始

```powershell
cargo run -p luma-engine

# 默认不会自动弹浏览器：从托盘选择“打开控制页”，或直接访问
start http://127.0.0.1:18765/
curl.exe http://127.0.0.1:18765/api/v1
curl.exe http://127.0.0.1:18765/api/v1/session
```

默认监听 `127.0.0.1:18765`；可用 `--bind` 和 `--port` 覆盖。编译时可用
`LUMA_OBS_SOURCE` 与 `LUMA_OBS_RUNDIR` 覆盖 OBS 源码/运行目录。

最小录制闭环：

```powershell
curl.exe -X POST http://127.0.0.1:18765/api/v1/session/start
Start-Sleep -Seconds 15
curl.exe http://127.0.0.1:18765/api/v1/session
curl.exe -X POST http://127.0.0.1:18765/api/v1/session/stop
```

输出写入 `%USERPROFILE%\Videos\Luma\luma-<timestamp>.mkv`。stop 只有在 OBS 已停止、
文件非空、存在实际编码帧，并且 ffprobe 确认 H.264 视频、AAC 音频及可信时长后才
返回 `ok:true`。

完整回归（默认依次录制 20 秒 x264，以及本机存在时的首选硬编）运行：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\record-regression.ps1
# 要求硬编存在且不得降级
powershell -ExecutionPolicy Bypass -File .\scripts\record-regression.ps1 -Encoder amf -RequireHw
```

脚本再次用 ffprobe 独立检查音视频流、分辨率、文件大小以及媒体/墙钟时长。硬案例建议
先在 Chrome 播放一段动态且有声音的视频，再运行脚本。

## 托盘与无界面运行

```powershell
# 默认：HTTP + libobs + 系统托盘
cargo run -p luma-engine

# 自动打开默认浏览器
cargo run -p luma-engine -- --open-ui

# CI / 回归 / 无交互桌面会话
cargo run -p luma-engine -- --no-tray
```

菜单提供打开控制页、开始/停止录制、状态和退出；录制动作与 HTTP 共用同一份 session
状态及 stop 校验。项目不再包含 WebView2 桌面壳，也不依赖 WebView2 Runtime。详见
[托盘说明](docs/TRAY.md)。

## API

控制面兼容旧 Luma **/api/v1 子集**（probe、session、settings、library）。M3 示例：

```powershell
curl.exe http://127.0.0.1:18765/api/v1/library
curl.exe http://127.0.0.1:18765/api/v1/encoders
curl.exe http://127.0.0.1:18765/api/v1/settings
curl.exe -X PUT -H "Content-Type: application/json" -d '{"output_directory":"C:\\Users\\me\\Videos\\Luma","record_system_audio":true,"record_microphone":false,"quality":"1080p30"}' http://127.0.0.1:18765/api/v1/settings
curl.exe -X DELETE http://127.0.0.1:18765/api/v1/library/luma-123.mkv
```

默认只监听本机。详情见 [M3 API 与持久化](docs/M3.md) 与 [WebUI 迁移说明](docs/WEBUI.md)。

## 开发节奏

见 [docs/CADENCE.md](docs/CADENCE.md)（**第一版里程碑**）。

## 路线图（摘要）

| 里程碑 | 目标 |
|---|---|
| M0 | HTTP 起服 + /api/v1 probe/session 占位 |
| M1 | libobs 全屏 + 系统声录到文件（已完成本机验收） |
| M2 | 正式 WebUI 对接 start/stop/状态（完成） |
| M3 | library / settings 持久化与回归脚本（完成） |
| M4 | 硬编探测/选择 + 诚实 x264 降级/遥测（完成） |

## 相关文档

- [技术边界](docs/BOUNDARY.md)
- [开发节奏](docs/CADENCE.md)
- [M0 HTTP 控制面](docs/M0.md)
- [M1 libobs 录制](docs/M1.md)
- [M2 最小 WebUI](docs/M2.md)
- [M3 API 与持久化](docs/M3.md)
- [WebUI 迁移说明](docs/WEBUI.md)
- [M4 硬件编码](docs/M4.md)
- [Windows 托盘](docs/TRAY.md)
- [设计规格 2026-09-22](docs/superpowers/specs/2026-09-22-luma-next-design.md)
