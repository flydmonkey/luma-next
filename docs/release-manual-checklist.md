# Luma Next 发布人工验收清单

复制本文件到 `artifacts/`，填写证据后把通过项改为 `[x]`。无对应硬件时可写清理由并勾选为 N/A。

- [ ] **真实 DirectX 游戏**：启动真实游戏，以 `record-regression.ps1 -GameTitleSubstring "标题"` 录制；期望 `game_capture` 非 SKIP、动态画面和音频时长通过。证据：`待填写样片/日志路径`。
- [ ] **混合 DPI 区域框选**：在 125%/150% 显示器上用原生 picker 框选已知矩形；期望成片尺寸与物理坐标一致、取消不改设置。证据：`待填写截图/样片路径`。
- [ ] **双屏**：选择非主屏 display 录制；期望目标屏动态画面正确。仅单屏机器可标 N/A。证据：`待填写样片或 N/A 理由`。
- [ ] **托盘与浏览器**：依次从托盘开始、暂停、恢复、停止并打开控制页；期望菜单和 WebUI session 同步。证据：`待填写截图/样片路径`。
- [ ] **登录自启**：安装 release 包并检查 `HKCU\...\Run\LumaNext`；期望指向 `%LOCALAPPDATA%\LumaNext\bin\luma-engine.exe`，登录后托盘出现且不自动弹浏览器。证据：`待填写截图/导出路径`。
- [ ] **SmartScreen / 签名状态**：有证书时签名必须 Valid；无证书时确认发布说明为 Unsigned，并记录 SmartScreen 预期。证据：`待填写签名输出/截图`。
