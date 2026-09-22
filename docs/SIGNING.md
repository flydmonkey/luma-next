# Windows Authenticode 签名

Luma Next 不存储证书、私钥或密码。签名只作用于 `bin\luma-engine.exe`；捆绑的 FFmpeg 与
OBS 文件保持上游状态。时间戳服务器为 `http://timestamp.digicert.com`，摘要和时间戳摘要均为 SHA-256。

## 证书存储区 thumbprint（推荐）

```powershell
$env:LUMA_CODE_SIGN_CERT='SHA-1 certificate thumbprint without spaces'
$env:SIGNTOOL='C:\Program Files (x86)\Windows Kits\10\bin\...\x64\signtool.exe' # PATH 已有可省略
powershell -ExecutionPolicy Bypass -File .\scripts\publish.ps1 -Sign
```

## PFX（仅在安全发布机）

```powershell
$env:LUMA_CODE_SIGN_PFX='D:\secure\luma-signing.pfx'
$env:LUMA_CODE_SIGN_PASSWORD='set only in the release process environment'
powershell -ExecutionPolicy Bypass -File .\scripts\publish.ps1 -Sign
```

不要把 PFX、密码、环境导出文件或签名日志中的敏感内容提交到仓库。PFX 不存在或密码缺失会失败，
不会降级成伪签名。两类证书都未配置时，`-Sign` 明确 SKIP，publish 继续生成标为 `Unsigned` 的包。

## 排错

- 找不到 signtool：安装 Windows SDK，或设置 `SIGNTOOL` 为可信的 x64 工具路径。
- 时间戳失败：检查出站 HTTPS/HTTP 策略后重试；不要去掉时间戳冒充最终发布。
- `UnknownError` / `HashMismatch`：删除本次产物、重新 pack 并签名，不要继续上传。
- 验证：`Get-AuthenticodeSignature .\dist\LumaNext\bin\luma-engine.exe` 必须为 `Valid`；
  `signtool verify /pa /v` 也必须成功。

