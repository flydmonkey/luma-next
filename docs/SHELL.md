# Luma Next Windows 桌面壳

`luma-shell` 是 WebUI 的薄桌面宿主。它使用 `tao` 创建窗口、使用 `wry` 托管系统
WebView2；不包含录制、编码或 libobs 逻辑。

## 运行

要求：Windows 10/11、Rust stable、Microsoft Edge WebView2 Runtime。

```powershell
# 先让引擎监听 127.0.0.1:18765
cargo run -p luma-engine

# 再启动独立桌面窗口
cargo run -p luma-shell
```

联调时先在浏览器或 `curl.exe http://127.0.0.1:18765/` 确认正式 WebUI 可访问，
再启动壳；壳探测到端口后会把该页面装入内容区。

当前仓库若尚未包含 `luma-engine`，可直接运行壳检查离线状态：

```powershell
cargo run -p luma-shell
```

窗口会明确显示“无法连接 Luma 引擎”和重试按钮，而不是白屏。开发期刻意不由壳
自动拉起引擎，避免把二进制路径、生命周期和日志策略写死；后续同进程整合时可保留
现有 localhost HTTP 边界。

静态外观基准见 [连接状态截图](screenshots/luma-shell-connecting.png)。该图由同一份
`shell.html` 在 980×700 视口渲染，用于审查标题栏尺寸、按钮排列和状态区布局；真实
窗口的 DWM 圆角与阴影由系统绘制，因此不出现在该无边框页面截图中。

## 手动验收

1. 在端口未监听时启动壳，确认出现连接错误；启动引擎后点“重试”。
2. 按住标题“录制”附近拖动，确认窗口跟随；双击同一区域，确认最大化/还原。
3. 依次点击最小化、最大化/还原、关闭；确认操作作用于应用窗口。
4. 点击文件夹按钮，确认打开 `%USERPROFILE%\Videos\Luma`（目录不存在时创建）。
5. 点击设置，确认内容区导航到 `/#/settings`，顶栏不会叠出第二套窗口按钮。

## 外观与系统限制

标题栏沿用旧 Luma 的 48 DIP 高度、左标题、40 DIP 工具按钮和最右侧 46 DIP
窗口按钮。Windows 11 通过 DWM 请求系统圆角，并让 `tao` 请求无边框阴影；这些是
系统偏好，Windows 10、远程桌面、高对比度或关闭桌面合成时可能不完全呈现。为一个
装饰效果重写 DWM 非客户区不属于本壳范围。

## 安全边界

- 导航只允许内嵌壳页面和 `http://127.0.0.1:18765/`。
- release 构建不启用 WebView2 DevTools；没有远程调试端口。
- IPC 只接受列举的窗口/壳命令，未知消息会被忽略。
- 本壳不接触录制管线，也不链接 libobs。仓库若链接 libobs，整体分发仍按 GPL
  合规要求执行。
