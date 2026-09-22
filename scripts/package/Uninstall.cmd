@echo off
setlocal
cd /d "%~dp0"
chcp 65001 >nul
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0Uninstall.ps1"
set "LUMA_EXIT=%ERRORLEVEL%"
echo.
if not "%LUMA_EXIT%"=="0" echo 卸载失败，退出码：%LUMA_EXIT%
pause
if "%LUMA_EXIT%"=="0" if /i "%~dp0"=="%LOCALAPPDATA%\LumaNext\" (
  cd /d "%TEMP%"
  start "" /b powershell.exe -NoLogo -NoProfile -WindowStyle Hidden -Command "Start-Sleep -Seconds 2; Remove-Item -LiteralPath '%LOCALAPPDATA%\LumaNext\Install.cmd','%LOCALAPPDATA%\LumaNext\Install.ps1','%LOCALAPPDATA%\LumaNext\Uninstall.cmd','%LOCALAPPDATA%\LumaNext\Uninstall.ps1' -Force -ErrorAction SilentlyContinue; Remove-Item -LiteralPath '%LOCALAPPDATA%\LumaNext' -Force -ErrorAction SilentlyContinue"
)
exit /b %LUMA_EXIT%
