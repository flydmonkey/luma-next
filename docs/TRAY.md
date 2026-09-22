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

# 保留托盘，但不注册全局热键
cargo run -p luma-engine -- --no-hotkeys
```

端口仍默认为 `127.0.0.1:18765`，并支持原有 `--bind`、`--port`。即便修改端口，
“打开控制页”也会使用本次实际端口。绑定失败会在进程日志中明确报错，且不会创建
一个无法工作的托盘。

## 单实例

默认使用 `Local\LumaNext.Engine.Singleton.v1` Windows 命名 mutex，因此每个已登录的
Windows 会话只运行一个产品实例，和 `--port` 无关。第二次启动不会初始化 libobs、
创建托盘或争抢端口，而是打印“Luma Next 已在当前 Windows 会话中运行”并以退出码
2 结束。进程正常退出、崩溃或被强杀时，Windows 内核都会释放 mutex，不存在陈旧
lock 文件。

需要同时运行隔离开发实例时必须显式绕过，并为每个实例提供不同端口：

```powershell
cargo run -p luma-engine -- --allow-second-instance --no-tray --port 18766
```

`--no-tray` 本身不会绕过单例。回归脚本若发现默认端口已有引擎会复用它；否则只启动
一个带 `--no-tray` 的引擎。

## 当前用户登录自启

推荐通过发布脚本安装稳定、自包含的运行目录：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\pack.ps1
powershell -ExecutionPolicy Bypass -File .\scripts\install.ps1 -SkipPack

# 检查
Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name LumaNext

# 卸载
powershell -ExecutionPolicy Bypass -File .\scripts\uninstall.ps1
```

安装位置是
`HKCU\Software\Microsoft\Windows\CurrentVersion\Run` 的 `LumaNext` 字符串值，内容为
带引号的 `%LOCALAPPDATA%\LumaNext\bin\luma-engine.exe`，不包含 `cargo run`、
`--no-tray` 或 `--open-ui`。因此用户登录后启动正常托盘，但不会自动弹浏览器。
直接对 `target\debug` 或 `target\release` 中的开发 exe 执行 `--install-autostart` 仍允许，
但引擎会警告该路径不稳定。卸载只删除 `LumaNext` 值，不删除整个 Run 键。
正式安装的 `bin\` 同时包含 ffmpeg/ffprobe/ffplay，托盘 stop 校验不依赖系统 PATH。

源码开发与已安装实例共用产品单例。调试前应从托盘退出已安装实例；需要并行时使用
`--allow-second-instance --no-tray --port <独立端口>`。完整布局见 [INSTALL.md](INSTALL.md)。

## 菜单

- **打开控制页**：调用 Windows 默认浏览器打开 localhost WebUI。
- **开始录制 / 停止录制**：同一个菜单项随共享 session 立即切换；开始使用当前
  settings，停止执行完整 ffprobe 诚实校验。
- **暂停录制 / 恢复录制**：录制中可用；暂停时媒体计时冻结，仍可直接停止。
- **状态**：只读显示空闲、录制 elapsed/目标摘要/实际编码器、最近错误或最近输出路径。
- **退出**：若正在录制，先同步 stop 并验证；失败会写明日志并保留 session error，
  随后退出，不会假报保存成功。

托盘线程负责 Win32 消息泵。start/stop 从菜单派发到工作线程，因此录制初始化不会
卡死托盘消息；菜单每 200ms 从引擎同一 session 数据源刷新，不通过 obs-websocket
或第二个 HTTP 后端猜状态。录制时图标变为红色。

默认全局热键是 `Ctrl+Shift+R`（开始/停止）和 `Ctrl+Shift+P`（暂停/恢复）。注册失败
会在日志中说明组合键可能被占用，不会静默宣称可用。`--no-tray` 同时禁用热键；
`--no-hotkeys` 只禁用热键。详见 [M6.md](M6.md)。

托盘“开始录制”始终读取当前 settings，因此 M7 的区域或游戏模式也走相同 session。
原生区域框选由浏览器控制页发起；游戏 hook 失败会进入最近错误，不会回退为窗口捕获。
详见 [M7.md](M7.md)。

## 与旧壳的差异

旧 `luma-shell` 使用 tao+wry/WebView2 承载网页和自绘窗口铬，现已从 workspace 与
依赖锁文件中移除。浏览器负责网页窗口行为，托盘只提供系统级常驻控制。

托盘要求已登录的交互式 Windows 桌面；Windows 服务、Session 0 和 CI 应使用
`--no-tray`。回归脚本自动使用该参数。

## 手工验收

1. 启动 `luma-engine`，再启动第二次；确认第二个进程退出码为 2，首实例 API 仍可用。
2. 用独立端口加 `--allow-second-instance`，确认开发实例可按需并行。
3. 运行 `scripts/install.ps1` 后读取上述 HKCU 值，确认指向稳定安装路径；再次 install
   确认可覆盖升级；uninstall 后确认值与发布文件均不存在。
4. 登录自启的真实发布验收应注销再登录，确认只出现一个托盘且不会自动打开浏览器。
