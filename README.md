# Luma Next

本地屏幕录制：**嵌入 libobs 的 Rust 引擎** + **HTTP API** + **WebUI**。

旧版 Luma（../record，WinUI + 自研 Media Foundation）已归档，不再作为主开发线。原因见 [docs/BOUNDARY.md](docs/BOUNDARY.md)：多厂商硬编与 MF 异步样本生命周期成本过高，改由 OBS 管线承担。

## 架构

```
WebUI (browser / luma-shell)  --HTTP-->  luma-engine (Rust)
                                |
                            libobs (capture / encode / mux)
                                |
                            output file (e.g. mp4)
```

- 生产方向保持单仓单进程；当前开发壳与引擎分开启动，接口保持可合并
- 平台：Windows 优先验收；结构预留跨平台
- 许可：因链接 libobs，本项目按 **GPL** 约束分发（详见下文）

## 许可（必读）

本仓库链接 **libobs（GPL）**。二进制分发需遵守 GPL 义务（源码提供、许可声明等）。不要在未做合规方案前，把本项目当作闭源商业一体包发布。

## 仓库布局

```
luma-next/
  crates/          # Rust workspace
    luma-engine/   # localhost HTTP 引擎
    luma-shell/    # Windows WebView2 薄壳
  web/             # WebUI 静态资源或子项目（后续接入）
  docs/
    BOUNDARY.md    # 技术边界
    CADENCE.md     # 开发节奏与里程碑
    SHELL.md       # 桌面壳运行与验收
    superpowers/specs/  # 设计规格
  README.md
```

## 要求（开发机）

- Windows 10/11（第一期验收平台）
- Rust toolchain（stable）
- 能获取与 libobs 版本匹配的 OBS 二进制（具体 bootstrap 方式在 M0 落地时写入本文）

## 快速开始

```powershell
cargo run -p luma-engine

# 另一个终端
curl.exe http://127.0.0.1:18765/api/v1
curl.exe http://127.0.0.1:18765/api/v1/session
```

默认监听 `127.0.0.1:18765`；可用 `--bind` 和 `--port` 覆盖。M0 只提供控制面
占位，不包含 libobs 或真实录制。

## 运行 Windows 桌面壳

开发期采用“先引擎、后壳”的明确流程；壳不会猜测或拉起一个未定义路径的引擎
二进制：

```powershell
# 终端 1
cargo run -p luma-engine

# 终端 2
cargo run -p luma-shell
```

壳固定加载 `http://127.0.0.1:18765/`。端口未监听时会显示错误页，启动引擎后点击
“重试”即可，不会停在 WebView2 的白屏。开发机需安装 Microsoft Edge WebView2
Runtime（Windows 11 通常已包含）。详细手测步骤见 [桌面壳说明](docs/SHELL.md)。

`luma-shell` 自身不链接 libobs，也不改变录制管线。整个仓库未来一旦链接 libobs，
分发仍须遵守上文的 GPL 要求；拆出薄壳不规避该义务。

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
