# Luma Next 安装与发布

第一期采用可重复的 PowerShell 发布流程，不使用 MSI/MSIX。安装范围仅为当前用户，
无需管理员权限；登录自启仍使用 HKCU Run，托盘运行在交互式用户会话中。

## 打包

```powershell
# 默认复用 docs/M1.md 记录的现有 OBS RelWithDebInfo rundir
powershell -ExecutionPolicy Bypass -File .\scripts\pack.ps1

# 自定义已有 rundir（不会触发 OBS 重编）
powershell -ExecutionPolicy Bypass -File .\scripts\pack.ps1 `
  -ObsRundir 'D:\obs-studio\build_x64\rundir\RelWithDebInfo'
```

脚本构建 release 引擎，在 `dist\LumaNext` 组装目录，并默认生成带 git describe 版本的
x64 zip。`-NoZip` 仅组装目录。`dist/` 是生成物，不进入 Git。

```text
LumaNext/
  bin/
    luma-engine.exe
    obs.dll、FFmpeg/Qt-free 运行 DLL 与 obs-ffmpeg-mux.exe
  obs/
    data/
    obs-plugins/64bit/
  manifest.json
  README.txt
  LICENSE-OBS-GPL.txt
```

引擎从 `bin` 上一级自动发现 `obs`。运行时设置 `LUMA_OBS_RUNDIR` 可显式覆盖；源码
开发则回落到编译时 rundir。包不包含 obs-studio 源码树。

## 安装、升级与卸载

```powershell
# 默认会先重新 pack；已有 dist 时可加 -SkipPack
powershell -ExecutionPolicy Bypass -File .\scripts\install.ps1

# 升级：退出托盘后，用新版本重复执行
powershell -ExecutionPolicy Bypass -File .\scripts\install.ps1 -SkipPack

# 卸载：先从托盘退出
powershell -ExecutionPolicy Bypass -File .\scripts\uninstall.ps1
```

稳定安装根为 `%LOCALAPPDATA%\LumaNext`。install 先在独立目录暂存包，并只替换自己管理的
`bin`、`obs`、manifest、README 与许可证文件，然后由已安装 exe 注册：

```text
HKCU\Software\Microsoft\Windows\CurrentVersion\Run\LumaNext
"%LOCALAPPDATA%\LumaNext\bin\luma-engine.exe"
```

重复安装会覆盖相同路径，所以升级后自启不漂移。安装器若发现该路径的引擎正在运行，
会要求先从托盘退出，不会强杀同名进程。卸载同样拒绝粗暴终止进程，删除 Run 值与
发布文件；`settings.json`、OBS 插件配置以及 `%USERPROFILE%\Videos\Luma` 中的成片保留。

## 开发与 CI

`cargo run -p luma-engine` 仍是源码开发入口。已安装实例与开发实例默认共享产品单例；
请先退出托盘，或为隔离调试显式使用不同端口：

```powershell
cargo run -p luma-engine -- --allow-second-instance --no-tray --port 18766
```

对 Cargo `target` 中的 exe 直接运行 `--install-autostart` 会打印不稳定路径警告，适合
短期测试但不推荐日常使用。CI 与 `record-regression.ps1` 继续使用源码树和 `--no-tray`，
不要求安装发布包。

## GPL 与发布责任

发布目录包含并链接 libobs 及其插件，整体分发必须履行 GPL 源码提供、许可证声明等
义务。包内 `LICENSE-OBS-GPL.txt` 保留 OBS 许可证，`README.txt` 给出 Luma Next 与 OBS
对应源码位置。代码签名、SmartScreen 商业信誉与 MSI/MSIX 不在第一期范围内。
