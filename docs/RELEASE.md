# Luma Next 发布 Runbook

发布操作在 Windows x64 的交互式用户会话中完成。脚本只准备本地产物，默认不创建或上传 GitHub Release。

## 1. 自动矩阵

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\release-matrix.ps1
```

报告和逐项日志写入未纳入 Git 的 `artifacts/`。任何自动项失败都会返回非零。首次运行会从
[`release-manual-checklist.md`](release-manual-checklist.md) 复制一份可填写清单；未填写默认只计为
`MANUAL_PENDING`。正式放行时使用：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\release-matrix.ps1 `
  -RequireManualPass -ManualChecklist .\artifacts\release-manual-checklist-<sha>.md
```

`-SkipClippy`、`-SkipRegression`、`-SkipReleaseSmoke` 仅供定位问题，不应作为正式发布证据。

## 2. 准备发布产物

```powershell
# 无证书：正常生成，状态明确为 Unsigned
powershell -ExecutionPolicy Bypass -File .\scripts\publish.ps1 -Sign

# 自定义版本与目录；可省略 ffplay 约 102 MiB
powershell -ExecutionPolicy Bypass -File .\scripts\publish.ps1 `
  -Version 0.9.0 -OutDir .\dist\publish -SkipFfplay -Sign
```

输出包含版本 ZIP、`SHA256SUMS` 和 `RELEASE-NOTES.md`。若签名环境已配置，脚本要求引擎
Authenticode 状态为 `Valid`；否则立即失败。FFmpeg 第三方二进制默认不由 Luma 重新签名。

## 3. 人工放行与上传

完成并保存人工清单，重新用 `-RequireManualPass` 运行矩阵，然后核对：

1. `SHA256SUMS` 与 ZIP 一致；
2. 发布说明中的 Signed/Unsigned 与 `Get-AuthenticodeSignature` 一致；
3. GPL/OBS/FFmpeg 许可证和源码位置仍在包中；
4. 手工将 ZIP、校验和及发布说明上传到 GitHub Releases。

