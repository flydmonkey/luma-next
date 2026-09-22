# Luma Next — 技术边界

> 定稿日期：2026-09-22  
> 产品名：Luma Next｜仓库：luma-next  
> 旧项目：Projects/record（Luma / WinUI + 自研 MF）归档只读，不继续作为主开发线。

## 目标

本地屏幕录制引擎：**进程内嵌 libobs**，对外提供 HTTP API，用现有 **WebUI**（与 WinUI 对齐）作为第一客户端；可选以后再套薄壳。Windows 优先验收，目录与配置预留跨平台。

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
- 不把 WinUI 迁入本仓，也不在壳中复制录制后端。2026-09-22 起允许独立的
  `luma-shell` 薄壳：它只托管 localhost WebUI、提供窗口铬和系统入口，仍由
  `luma-engine` 独占录制能力。
- 旧 `record` 仅归档；可摘产品教训与 API 习惯，不搬 MF 代码

## 桌面薄壳边界（2026-09-22 补充）

- 壳是 Windows-first 的 WebView2 客户端，不是第二个业务后端。
- 壳仅可访问固定的 `http://127.0.0.1:18765/`，不开放公网调试或任意导航。
- 当前开发形态要求先启动引擎；未来可把 HTTP 服务与壳合并到同一进程，但 HTTP
  契约和录制管线所有权不变。
- 壳任务不引入 libobs、编码器、采集或 M1 录制范围。

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
| UI | WinUI + WebUI | WebUI + 可选薄壳 |
| 引擎 | 自研 DXGI/WGC + MF | 嵌 libobs |
| 控制 | LAN /api/v1 | 同形子集，逐步对齐 |
| 许可 | 自有代码为主 | **GPL（因 libobs）** |

M3 起正式 WebUI 位于 `web/`，由引擎静态托管。它可呈现未来模式，但只有引擎真实
支持的“主显示器 + 系统声 + 1080p30 x264”可发起录制；区域、窗口、游戏、纯音频、
麦克风与硬件编码均须显示为未接入，不得假成功。

## 成功标准（产品）

用户用 WebUI/API 能稳定录全屏+系统声；Chrome 播放动态视频时**不成「音频满长、视频短一截」**；多厂商硬编以 OBS 后端为准，失败要明文，不假报帧率。
