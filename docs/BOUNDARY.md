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
- **UI**：WebUI 优先；不把 WinUI 迁入本仓。
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
- 不在第一期上 Tauri/WinUI 桌面壳（引擎稳定后再说）
- 旧 
ecord 仅归档；可摘产品教训与 API 习惯，不搬 MF 代码

## 与旧 Luma 的关系

| | 旧 Luma (
ecord) | Luma Next |
|---|---|---|
| UI | WinUI + WebUI | WebUI（壳后置） |
| 引擎 | 自研 DXGI/WGC + MF | 嵌 libobs |
| 控制 | LAN /api/v1 | 同形子集，逐步对齐 |
| 许可 | 自有代码为主 | **GPL（因 libobs）** |

## 成功标准（产品）

用户用 WebUI/API 能稳定录全屏+系统声；Chrome 播放动态视频时**不成「音频满长、视频短一截」**；多厂商硬编以 OBS 后端为准，失败要明文，不假报帧率。
