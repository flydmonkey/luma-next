# Luma Next

本地屏幕录制：**嵌入 libobs 的 Rust 引擎** + **HTTP API** + **WebUI**。

旧版 Luma（../record，WinUI + 自研 Media Foundation）已归档，不再作为主开发线。原因见 [docs/BOUNDARY.md](docs/BOUNDARY.md)：多厂商硬编与 MF 异步样本生命周期成本过高，改由 OBS 管线承担。

## 架构

`
WebUI (browser)  --HTTP-->  luma-engine (Rust)
                                |
                            libobs (capture / encode / mux)
                                |
                            output file (e.g. mp4)
`

- 单仓单进程；引擎可托管静态 WebUI
- 平台：Windows 优先验收；结构预留跨平台
- 许可：因链接 libobs，本项目按 **GPL** 约束分发（详见下文）

## 许可（必读）

本仓库链接 **libobs（GPL）**。二进制分发需遵守 GPL 义务（源码提供、许可声明等）。不要在未做合规方案前，把本项目当作闭源商业一体包发布。

## 仓库布局

`
luma-next/
  crates/          # Rust 引擎与 workspace（后续）
  web/             # WebUI 静态资源或子项目（后续接入）
  docs/
    BOUNDARY.md    # 技术边界
    CADENCE.md     # 开发节奏与里程碑
    superpowers/specs/  # 设计规格
  README.md
`

## 要求（开发机）

- Windows 10/11（第一期验收平台）
- Rust toolchain（stable）
- 能获取与 libobs 版本匹配的 OBS 二进制（具体 bootstrap 方式在 M0 落地时写入本文）

## 快速开始

> M0 落地前此处为占位。

`ash
# 预期形态（实现后）：
cargo run -p luma-engine
# 浏览器打开 http://127.0.0.1:<port>/
`

## API

控制面兼容旧 Luma **/api/v1 子集**（probe、session、target、settings、library）。完整字段以后续 openapi / skill 文档为准；默认只监听本机，局域网需显式开启并配置 accessKey。

## 开发节奏

见 [docs/CADENCE.md](docs/CADENCE.md)（**第一版里程碑**）。

## 路线图（摘要）

| 里程碑 | 目标 |
|---|---|
| M0 | HTTP 起服 + /api/v1 probe/session 占位 |
| M1 | libobs 全屏 + 系统声录到文件 |
| M2 | WebUI 对接 start/stop/状态 |
| M3 | library / settings 持久化 |
| M4 | 硬编选择 + 诚实降级/遥测 |

## 相关文档

- [技术边界](docs/BOUNDARY.md)
- [开发节奏](docs/CADENCE.md)
- [设计规格 2026-09-22](docs/superpowers/specs/2026-09-22-luma-next-design.md)
