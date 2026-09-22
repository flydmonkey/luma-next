# Luma Next Windows 系统托盘

系统托盘是 Luma Next 的常驻入口，与 HTTP、session 和 libobs 位于同一个
`luma-engine` 进程；控制页使用系统默认浏览器，不需要 WebView2 Runtime。

## 运行

```powershell
# 默认创建托盘，不自动弹浏览器
cargo run -p luma-engine

# 启动后自动打开控制页
cargo run -p luma-engine -- --open-ui

# CI、录制回归或无交互桌面会话
cargo run -p luma-engine -- --no-tray
```

端口仍默认为 `127.0.0.1:18765`，并支持原有 `--bind`、`--port`。即便修改端口，
“打开控制页”也会使用本次实际端口。绑定失败会在进程日志中明确报错，且不会创建
一个无法工作的托盘。

## 菜单

- **打开控制页**：调用 Windows 默认浏览器打开 localhost WebUI。
- **开始录制 / 停止录制**：同一个菜单项随共享 session 立即切换；开始使用当前
  settings，停止执行完整 ffprobe 诚实校验。
- **状态**：只读显示空闲、录制 elapsed/实际编码器、最近错误或最近输出路径。
- **退出**：若正在录制，先同步 stop 并验证；失败会写明日志并保留 session error，
  随后退出，不会假报保存成功。

托盘线程负责 Win32 消息泵。start/stop 从菜单派发到工作线程，因此录制初始化不会
卡死托盘消息；菜单每 200ms 从引擎同一 session 数据源刷新，不通过 obs-websocket
或第二个 HTTP 后端猜状态。录制时图标变为红色。

## 与旧壳的差异

旧 `luma-shell` 使用 tao+wry/WebView2 承载网页和自绘窗口铬，现已从 workspace 与
依赖锁文件中移除。浏览器负责网页窗口行为，托盘只提供系统级常驻控制。

托盘要求已登录的交互式 Windows 桌面；Windows 服务、Session 0 和 CI 应使用
`--no-tray`。回归脚本自动使用该参数。
