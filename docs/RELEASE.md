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
ZIP 根目录必须包含 `Install.cmd`、`Install.ps1`、`Uninstall.cmd`、`Uninstall.ps1`；发布员
应从临时解压目录运行包内安装器完成一次升级/卸载冒烟，而不是要求用户访问源码仓库。

## 3. 创建 GitHub Draft Release

安装并认证 GitHub CLI，或只在当前进程设置具有 `repo` 权限的 token。Token 不写入脚本或日志：

```powershell
gh auth login
# 或：$env:GH_TOKEN='token from a secure secret store'

powershell -ExecutionPolicy Bypass -File .\scripts\publish.ps1 -SkipFfplay
powershell -ExecutionPolicy Bypass -File .\scripts\github-release.ps1 -SkipBuild -Draft:`$true
```

脚本在上传前重新计算 ZIP SHA-256，并要求 `SHA256SUMS` 完全一致。默认创建 Draft；人工六项
未完成前应保持 Draft。已有正式 Release 永不覆盖；已有草稿也会失败，只有显式
`-ReplaceDraft` 才删除并重建该草稿。先检查而不创建：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\github-release.ps1 `
  -Version 5920ec4 -SkipBuild -DryRun
```

无证书的 Draft 允许上传，但正文会醒目标记 `Unsigned` 和 SmartScreen 预期。默认附件为 ZIP
与 `SHA256SUMS`；本地矩阵只写摘要和路径说明，不会上传可能包含机器信息的日志。

## 4. 人工放行与发布

完成并保存人工清单，重新用 `-RequireManualPass` 运行矩阵，然后核对：

1. `SHA256SUMS` 与 ZIP 一致；
2. 发布说明中的 Signed/Unsigned 与 `Get-AuthenticodeSignature` 一致；
3. GPL/OBS/FFmpeg 许可证和源码位置仍在包中；
4. ZIP 解压后可直接双击 `Install.cmd`，安装目录内也保留可双击的 `Uninstall.cmd`；
5. 在 GitHub 网页检查 Draft 的正文、附件和人工清单，再手工点击 Publish。
