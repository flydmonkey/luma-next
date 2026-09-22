# 录制计时：Media 与 Wall

## 口径

- `media_elapsed_seconds`：录制中由 `obs_output_get_total_frames(output) / 30fps` 得到，
  表示 OBS 已送入输出的媒体进度，是 WebUI 和托盘的主计时。
- `wall_elapsed_seconds`：C 桥在 `obs_output_start` 成功处用 `GetTickCount64` 打点，到
  stop 请求进入桥接层为止。它只用于发现输出阻塞或时间轴漂移。
- `elapsed_seconds`：录制中等于 media；stop 成功后等于 ffprobe 的容器 duration，
  因而 UI 停表值与最终成片一致。
- validation 同时返回 `duration_seconds`（ffprobe）、`media_seconds`（OBS 输出帧）、
  `wall_seconds`、`media_ffprobe_delta_seconds` 和 `wall_media_delta_seconds`。

## 旧问题与修复

旧实现从 `POST start` 返回后才在 Rust 层创建 `Instant`。一次 20:19 成片中，UI 只显示
约 20:06，稳定少 13.3 秒；同时 35% 的宽松容差隐藏了问题。现在 UI 直接轮询 OBS
输出帧进度，不再拿晚启动的墙钟冒充成片时长。

stop 校验采用两条独立门槛：

- OBS media 与 ffprobe duration：最多 1.5 秒。
- OBS media 与 C 侧输出墙钟：最多 `max(1.5 秒, wall × 0.5%)`。

任一超限都会使 stop 返回失败，不会以 warning 掩盖。回归脚本还要求 stop 后
`elapsed_seconds` 与 ffprobe 相差小于 1 秒。

## 回归

```powershell
# 20 秒短测
powershell -ExecutionPolicy Bypass -File .\scripts\record-regression.ps1 `
  -Encoder x264 -SkipMicrophone

# 3 分钟累积漂移测试
powershell -ExecutionPolicy Bypass -File .\scripts\record-regression.ps1 `
  -Encoder x264 -SkipMicrophone -Seconds 180
```

2026-09-22 本机 180 秒回归结果：ffprobe 183.10s、OBS media 183.03s、stop UI
183.10s、OBS wall 183.02s；最大偏差 0.08s。pre-stop 查询为 181.90s，随后 stop 排空
约 1.2 秒编码队列，因此它仅作为诊断输出，不与最终文件时长直接等同。
