# 停止录制与控制面可用性

`POST /api/v1/session/stop` 是异步完成操作。请求只在短临界区内冻结媒体/墙钟计时、
把 session 切换为 `stopping`，并把唯一的 recorder 所有权移交给专用 blocking 线程；
随后立即返回旧信封。浏览器、托盘与热键使用同一条路径。

后台线程依次执行 libobs stop、输出收尾和 ffprobe 校验，完成后短暂取得 Controller 锁并
写回 `idle` 或 `failed`。`GET /api/v1` 与 `/api/v1/session` 在整个 stopping 阶段保持可用。
WebUI 每秒轮询，stopping 时禁用重复停止并显示“正在结束并校验”。

## 上限与故障语义

- C bridge 在调用 `obs_output_stop` 前连接 output 的 `stop` signal 并启动 watchdog。如果
  stop 调用本身 15 秒未返回，watchdog 从独立线程调用官方 `obs_output_force_stop`。
- stop 返回后优先等待官方 `stop` signal（同时以 inactive 为兼容兜底），正常收尾宽限
  45 秒。只有宽限耗尽才 force stop；force 后再等 15 秒。期间完成则继续 ffprobe，校验
  通过即回到 `idle`，`validation.stop_forced=true` 且 session warning 会说明曾强制结束。
- 只有 force 后仍没有 stop signal 且 output 仍 active，才以分阶段错误进入 `failed`。
- ffprobe 校验最多运行 15 秒；超时会终止子进程并把 session 置为 `failed`。
- Rust 控制面 watchdog 在 120 秒时仍看到 `stopping`，会将 session 标记为 `failed`，
  保留可读错误并继续服务 HTTP。此情形表示 OBS/驱动线程未返回，安全恢复录制前应重启
  Luma；不会再要求为了恢复 API 而强杀进程。

OBS output 的正常完成以 `stop` signal / active 状态为准。参考
[libobs Output API](https://docs.obsproject.com/reference-outputs) 与
[obs-output.c](https://github.com/obsproject/obs-studio/blob/master/libobs/obs-output.c)。

## 回归

`scripts/record-regression.ps1` 在 stop 后每 200ms 请求 session，单次请求设置 2 秒超时，
直到 `idle`/`failed`；超过 130 秒或任何控制面超时均失败。成功后仍执行原有 ffprobe
音视频流、分辨率、文件大小和媒体时长断言。
