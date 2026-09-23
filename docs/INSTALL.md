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
  Install.cmd
  Install.ps1
  Uninstall.cmd
  Uninstall.ps1
  bin/
    luma-engine.exe
    ffmpeg.exe
    ffprobe.exe
    ffplay.exe
    obs.dll、FFmpeg/Qt-free 运行 DLL 与 obs-ffmpeg-mux.exe
  obs/
    data/
    obs-plugins/64bit/
  manifest.json
  README.txt
  LICENSE-OBS-GPL.txt
  LICENSE-FFMPEG.txt
```

引擎从 `bin` 上一级自动发现 `obs`。运行时设置 `LUMA_OBS_RUNDIR` 可显式覆盖；源码
开发则回落到编译时 rundir。包不包含 obs-studio 源码树。

硬编依赖以 OBS 构建为准：pack 强制校验 QSV/NVENC/AMF 的 OBS 插件和三个官方探测助手，
并从 rundir/`.deps` 发现 VPL/MFX、NVENC/NVML、AMF runtime DLL。发现即复制到 `bin` 并
写入 manifest，未发现则逐厂商 WARN；不会从 System32/Driver Store复制闭源驱动文件。
完整矩阵与失败语义见 [ENCODERS.md](ENCODERS.md)，4K/DPI 验收见 [M10.md](M10.md)。

FFmpeg 命令行工具固定使用 Gyan.dev 的 Windows x64 essentials build 9.0.2。该提供方由
FFmpeg 官方下载页列为 Windows 二进制来源。脚本下载
`ffmpeg-9.0.2-essentials_build.zip` 到未入库的 `third_party\ffmpeg\cache`，并验证
SHA-256 `60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba` 后才解包。
正式包包含该构建提供的 `ffmpeg.exe`、`ffprobe.exe` 和 `ffplay.exe`；三个静态工具合计
约 303 MiB，ZIP 相比仅 OBS 包约增加 112 MiB。OBS 自带的 avcodec DLL 不能替代这些
命令行工具。

引擎查找 ffprobe 的顺序是 `LUMA_FFPROBE`、引擎同目录、安装根下
`ffmpeg\bin`，最后才是 PATH。预留的 ffmpeg 解析规则同理使用 `LUMA_FFMPEG`；目前
录制主路径只直接调用 ffprobe，ffmpeg/ffplay 用于后续转码、抽帧和人工诊断。

## 普通用户安装、升级与卸载

发布 ZIP 是自包含安装包。完整解压后双击根目录的 `Install.cmd` 即可；CMD 会自动使用
`PowerShell -NoProfile -ExecutionPolicy Bypass` 调用同目录安装逻辑，用户不需要管理员权限、
Cargo、源码仓库或手工修改执行策略。安装完成默认立即启动托盘，并注册当前用户登录自启。

升级前从托盘退出 Luma Next，完整解压新版并再次双击 `Install.cmd`。卸载时可双击原解压包
或 `%LOCALAPPDATA%\LumaNext` 内的 `Uninstall.cmd`。卸载删除发行文件和 HKCU Run 项，但保留
`%USERPROFILE%\Videos\Luma` 中的录像以及 `%APPDATA%` 中的本地设置。

## 开发者安装脚本

```powershell
# 源码仓库使用：默认先 pack；已有 dist 时可加 -SkipPack
powershell -ExecutionPolicy Bypass -File .\scripts\install.ps1

# 升级：退出托盘后，用新版本重复执行
powershell -ExecutionPolicy Bypass -File .\scripts\install.ps1 -SkipPack

# 卸载：先从托盘退出
powershell -ExecutionPolicy Bypass -File .\scripts\uninstall.ps1
```

仓库中的 `scripts/install.ps1`、`scripts/uninstall.ps1` 是开发/CI 入口；发布包内的
`Install.cmd`/`Uninstall.cmd` 才是终端用户主路径。两者使用相同的稳定安装根
`%LOCALAPPDATA%\LumaNext`。安装会先在独立目录暂存包，并只替换自己管理的发行文件，
然后由已安装 exe 注册：

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

发布目录包含并链接 libobs 及其插件，并捆绑 Gyan.dev 的 GPLv3 FFmpeg essentials
静态构建。整体分发必须履行相应 GPL 源码提供、许可证声明等义务。包内
`LICENSE-OBS-GPL.txt` 与 `LICENSE-FFMPEG.txt` 保留两者许可证，`README.txt` 给出
Luma Next、OBS 与 FFmpeg 对应源码位置。M8 提供可选 Authenticode 挂钩，但不提供或
伪造证书；SmartScreen 商业信誉与 MSI/MSIX 仍不在第一期范围内。

## 发布检查清单

1. `scripts/pack.ps1` 生成目录与 ZIP；需要时用 `-SkipFfplay`，但不得删除 ffmpeg/ffprobe。
2. 配置证书 thumbprint 后执行 `pack.ps1 -Sign`；无证书应看到明确 SKIP。
3. 执行 `scripts/release-smoke.ps1`，通过包内 `Install.ps1`/`Uninstall.ps1` 验证安装、HKCU Run、旁路 ffprobe、录音 start/stop 和卸载。
4. 执行 `scripts/record-regression.ps1`，确认 audio-only 与仓内 DX11 game_capture 探针均 PASS。
5. 发布前另在真实游戏、麦克风和目标 DPI/多屏环境手测。
6. 检查 pack 的三家硬编 WARN、`manifest.json.encoder_runtime`，并至少在目标 Intel/AMD/NVIDIA 机器执行对应的 `-RequireHw` 回归；无对应 GPU 时核对非空 `unavailable_reason`。

完整顺序与报告格式见 [RELEASE.md](RELEASE.md)，逐项验收见
[RELEASE_SMOKE.md](RELEASE_SMOKE.md)，证书配置及故障排查见 [SIGNING.md](SIGNING.md)。
