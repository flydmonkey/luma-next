# Luma Next — 技术边界

> 定稿日期：2026-09-22  
> 产品名：Luma Next｜仓库：luma-next  
> 旧项目：Projects/record（Luma / WinUI + 自研 MF）归档只读，不继续作为主开发线。

## 目标

本地屏幕录制引擎：**进程内嵌 libobs**，对外提供 HTTP API，用默认浏览器中的
**WebUI**（与 WinUI 体验对齐）作为第一客户端；Windows 系统托盘提供常驻入口。

## 架构（第一期）

- **单仓单进程**：Rust 宿主初始化 libobs，提供 HTTP，并可托管 WebUI 静态资源。
- **控制面**：HTTP；尽量兼容旧 Luma /api/v1 子集（probe、session、target、settings、library）。
- **引擎**：libobs 负责采集 / 混音 / 编码 / 封装；不自研 DXGI→MF SinkWriter 管线。
- **UI**：WebUI 优先；参考旧 WinUI 的信息架构、文案与使用体验重新实现，绝不迁入
  XAML/C# 或旧录制代码。营销站不是录制 App UI 的来源。
- **许可**：链接 libobs → **GPL**；分发与闭源策略必须在 README 中明示，不可默认当专有软件发。

## 做（In scope）

- Rust 宿主：libobs init、场景/源/编码器、start / pause / stop、输出路径
- HTTP：health / /api/v1 子集；默认绑定 127.0.0.1；局域网需显式开启 + accessKey
- WebUI 对接上述 API
- 遥测与诚实行为：可见丢帧、编码器名、成片时长与墙钟关系；禁止「提交成功就当 30fps」
- Windows 上验证常见硬编后端（AMF / QSV / NVENC / x264 软件兜底）——**能力来自 OBS，不是自研三条 MFT**

## 不做（Out of scope / 第一期）

- 不移植旧 C# RecordingPipeline / GpuSurfaceWriter / MF 异步 sample 池
- 不承诺自研路径上的 Intel/AMD 对等「修厂商」
- 不把 Mac / Linux 列入第一期验收（仅结构预留）
- 不做推流、滤镜市场、插件商城
- 不把 WinUI 迁入本仓，不恢复 WebView2/Wry 桌面壳，也不复制录制后端。
- 旧 `record` 仅归档；可摘产品教训与 API 习惯，不搬 MF 代码

## 托盘宿主边界（2026-09-22 更新）

- `luma-engine` 同进程拥有托盘、HTTP 与 libobs；托盘直接读取同一 session 数据源。
- 控制页由系统默认浏览器打开固定 localhost 地址，不嵌 WebView，不依赖 WebView2。
- `--no-tray` 是 CI、回归和无交互会话的纯服务路径。
- 默认仍只绑定 `127.0.0.1`，托盘不扩大网络边界。

## M1 实现说明（2026-09-22）

- `luma-engine` 通过同进程 C ABI 桥接本机已构建的 libobs；不使用 obs-websocket，
  不启动 `obs64.exe`，也不复活旧 MF 管线。
- 第一条稳定路径固定为主显示器 WGC、默认 WASAPI 输出设备、x264/AAC 与 MKV；
  多厂商硬编选择仍属于 M4。
- stop 后必须以实际编码帧、文件大小及 ffprobe 音视频流/时长共同验证，验证失败的
  文件不会被 API 宣称为成功录制。

## 与旧 Luma 的关系

| | 旧 Luma (`record`) | Luma Next |
|---|---|---|
| UI | WinUI + WebUI | 浏览器 WebUI + Windows 托盘 |
| 引擎 | 自研 DXGI/WGC + MF | 嵌 libobs |
| 控制 | LAN /api/v1 | 同形子集，逐步对齐 |
| 许可 | 自有代码为主 | **GPL（因 libobs）** |

M3 起正式 WebUI 位于 `web/`，由引擎静态托管。M4 允许从 libobs 真实枚举出的 H.264
编码器中选择，并在硬编 start 失败时明文降级至 x264。捕获能力仍只有“主显示器 +
系统声 + 1080p30”；区域、窗口、游戏、纯音频与麦克风继续显示为未接入。

## 成功标准（产品）

用户用 WebUI/API 能稳定录全屏+系统声；Chrome 播放动态视频时**不成「音频满长、视频短一截」**；多厂商硬编以 OBS 后端为准，失败要明文，不假报帧率。
