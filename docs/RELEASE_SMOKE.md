# Luma Next 正式发布冒烟清单

本清单用于 Windows 10/11 x64 正式发行。自动脚本负责可重复的构建、安装和录制检查；GPU、
显示器、DPI、托盘等依赖真实桌面的项目保留人工证据。任何失败都应先修复，不得通过放宽
成片校验或把硬件失败伪装为成功来放行。

## 0. 发布候选与自动矩阵

- [ ] 工作树仅包含本次预期修改，`.agents/`、`.cursor/`、证书和 token 未进入提交。
- [ ] 在待发布 commit 上执行：

  ```powershell
  powershell -ExecutionPolicy Bypass -File .\scripts\release-matrix.ps1
  ```

  预期：`cargo test --workspace`、clippy、录制回归、安装冒烟和单实例检查均为 `PASS`，报告
  位于 `artifacts/release-matrix-<sha>.md`。任一 `AUTO_FAIL` 均禁止发布；真实 GPU/游戏不应
  被加入无对应硬件的 CI 强制项。

## 1. 全新目录安装与控制面

- [ ] 用 `publish.ps1` 生成 ZIP，将其解压到一个全新临时目录，双击 `Install.cmd`。
- [ ] 安装无需管理员权限，安装根为 `%LOCALAPPDATA%\LumaNext`，托盘进程能够启动。
- [ ] 执行：

  ```powershell
  curl.exe --fail http://127.0.0.1:18765/api/v1
  curl.exe --fail http://127.0.0.1:18765/api/v1/session
  ```

  预期：两者在 2 秒内返回 `{ok:true,...}`，session 初始为 `idle`。连接超时、白屏、第二实例
  抢占端口或返回 `ok:false` 均为失败。

## 2. 捕获模式与成片

每段建议 20–60 秒，停止后必须回到 `idle`；用包内 `bin\ffprobe.exe` 确认时长合理、预期
视频/音频流存在。纯音频模式只要求音频流。黑帧、极短文件、缺失所选音源或 session
宣称成功但文件不可读均为失败。

- [ ] **全屏**：选择主屏，录制动态桌面；画面、系统声和时长有效。
- [ ] **窗口**：选择一个持续变化的普通窗口；目标正确，关闭/最小化警告符合 M5 文档。
- [ ] **区域**：用原生 picker 选择已知区域；成片尺寸与物理像素矩形一致。
- [ ] **游戏**：优先真实 DirectX 游戏；无游戏时使用仓内 DX11 probe，并确认走的是真
  `game_capture` 而非 `window_capture`。无兼容目标可记为人工 `N/A`，不可伪造 PASS。
- [ ] **音频**：至少运行系统声或麦克风之一；双关请求应被明确拒绝。

可复现的自动入口：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\record-regression.ps1 -Seconds 20
```

## 3. 暂停、热键、托盘与自启

- [ ] 录制数秒后暂停 5–10 秒，再恢复并停止；暂停期间媒体时钟冻结，成片不包含完整暂停段。
- [ ] 默认全局热键 `Ctrl+Shift+R` 开始/停止、`Ctrl+Shift+P` 暂停/恢复；失效或冲突必须显示
  原因，不能静默。
- [ ] 托盘菜单能打开浏览器控制页，开始/暂停/恢复/停止文案与同一 session 状态同步。
- [ ] `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\LumaNext` 指向
  `%LOCALAPPDATA%\LumaNext\bin\luma-engine.exe`；卸载自启后该值消失。

## 4. 多显示器与高 DPI

- [ ] `/api/v1/targets` 的 `displays` 非空；下拉显示器名称、物理分辨率和主屏标识，且含
  `#`/`&` 的 OBS 设备路径不会造成空选项。
- [ ] 在 125%/150%/200% 缩放下，4K 屏报告 `3840x2160` 而非逻辑 `2560x1440`；区域 picker
  与成片裁剪均使用物理像素。
- [ ] 有双屏时选择非主屏短录并检查画面来源；单屏机器明确记录 `N/A`，不算自动失败。

## 5. 编码器诚实性

- [ ] `GET /api/v1/encoders` 中本机硬编（Intel QSV、AMD AMF 或 NVIDIA NVENC）可用时，选择
  它短录并确认 `encoder_active` 为对应 OBS id。
- [ ] 不具备的厂商编码器为 `available:false` 且 `unavailable_reason` 非空；选择不可用编码器
  时必须拒绝或明确记录 x264 fallback，禁止静默冒充硬编成功。
- [ ] x264 路径至少短录一次并通过成片校验。

## 6. 停录控制面与长录

- [ ] 停止请求发出后，以约 200 ms 间隔轮询 `/api/v1/session`；`stopping` 期间每次请求应在
  2 秒内返回，`/api/v1` 同样健康。任何整站超时均为失败。
- [ ] 日常候选版至少跑 10–20 分钟硬编长录；重要版本建议 30–60 分钟。最终应为 `idle` 且
  ffprobe 可读。允许 `validation.stop_forced=true`，但必须有 warning、文件验证通过；force 后
  仍 active、文件不可用或硬上限超时必须为 `failed`，不能伪装成功。
- [ ] 记录 stop 调用、graceful wait、force wait、ffprobe 与最终文件路径，供回归比较。

## 7. 卸载与发布产物

- [ ] 从安装目录或解压包双击 `Uninstall.cmd`；正在运行时应提示先退出，不得误杀其他进程。
- [ ] 卸载后无 Luma Next 关键进程、HKCU Run 值消失，`%LOCALAPPDATA%\LumaNext` 可删除。
- [ ] `%USERPROFILE%\Videos\Luma` 成片与 `%APPDATA%` 本地设置保留，这是预期行为。
- [ ] ZIP 根含 `Install.cmd`、`Uninstall.cmd`、对应 PowerShell、中文 `README.txt`、`bin/`、
  `obs/`、许可证与 manifest；`SHA256SUMS` 与 ZIP 实算一致。
- [ ] Authenticode 有证书时必须为 `Valid`；无证书时发布说明明确写 `Unsigned` 及 SmartScreen
  预期，绝不声称已签名。

发布自动化和 tag/上传顺序见 [RELEASE.md](RELEASE.md)。人工硬件证据可填写在由
`release-matrix.ps1` 生成的 `artifacts/release-manual-checklist-<sha>.md` 中。
