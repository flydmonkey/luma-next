@echo off
setlocal
cd /d "%~dp0"
chcp 65001 >nul
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0Install.ps1"
set "LUMA_EXIT=%ERRORLEVEL%"
echo.
if not "%LUMA_EXIT%"=="0" echo 安装失败，退出码：%LUMA_EXIT%
pause
exit /b %LUMA_EXIT%
