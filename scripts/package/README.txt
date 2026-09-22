Luma Next {{VERSION}}

安装（三步）：
1. 将 ZIP 完整解压到一个普通文件夹。
2. 双击 Install.cmd；无需管理员权限，也无需手动修改 PowerShell 执行策略。
3. 安装完成后从系统托盘打开控制页；默认会立即启动，并在以后登录时自动启动。

卸载：从解压包或 %LOCALAPPDATA%\LumaNext 双击 Uninstall.cmd。
卸载会保留 Videos\Luma 中的录像和 %APPDATA% 中的本地设置。
升级：退出托盘中的 Luma Next，解压新版后再次双击 Install.cmd。

This distribution includes and links libobs. OBS Studio/libobs is GPL-2.0-or-later;
see LICENSE-OBS-GPL.txt. Luma Next is GPL-3.0-or-later.
Corresponding source: https://github.com/flydmonkey/luma-next
OBS source: https://github.com/obsproject/obs-studio

This distribution bundles the Gyan.dev FFmpeg {{FFMPEG_VERSION}} essentials build
(ffmpeg, ffprobe and ffplay when supplied), licensed as GPLv3. See
LICENSE-FFMPEG.txt. FFmpeg source: https://github.com/FFmpeg/FFmpeg/tree/n{{FFMPEG_VERSION}}
