# Windows 硬件编码器依赖与诚实探测

Luma 只分发 OBS 官方构建产物和该构建使用的 `obs-deps` 文件，不从 Windows System32、
Driver Store 或显卡驱动安装目录复制闭源厂商 DLL。插件存在仅说明功能代码已打包；最终
`available` 还要求 OBS 在当前适配器和驱动上实际注册对应的 H.264 encoder。

## 发布文件矩阵

| 厂商 | OBS encoder id | 包内强制文件 | 驱动提供/按 OBS deps 有则复制 |
|---|---|---|---|
| Intel | `obs_qsv11_v2` | `obs/obs-plugins/64bit/obs-qsv11.dll`、`bin/obs-qsv-test.exe` | `vpl*.dll`、`libvpl*.dll`、`mfx*.dll`、`libmfx*.dll` |
| NVIDIA | `jim_nvenc`、`ffmpeg_nvenc` | `obs/obs-plugins/64bit/obs-nvenc.dll`、`obs-ffmpeg.dll`、`bin/obs-nvenc-test.exe` | `nvEncodeAPI64.dll`、`nvml.dll`，通常只能由 NVIDIA 驱动提供 |
| AMD | `h264_texture_amf` | `obs/obs-plugins/64bit/obs-ffmpeg.dll`、`bin/obs-amf-test.exe` | `amfrt64.dll`，通常只能由 AMD 驱动提供 |

`pack.ps1` 缺少任一强制插件/助手会失败。对厂商运行库，它只扫描 OBS rundir 的
`bin/64bit` 与同一 OBS 源树的 `.deps`：发现即复制到 Luma `bin/` 并记录到
`manifest.json.encoder_runtime.bundled_dlls`；未发现则打印逐厂商 WARN。此 WARN 不等于
编码器一定不可用，因为 Intel oneVPL 可从 Driver Store 找 implementation，NVENC/AMF
也按官方 OBS 方式从已安装显卡驱动加载。

OBS 当前 QSV 插件把旧 `obs_qsv11` 标为 deprecated，非 deprecated H.264 texture encoder
是 `obs_qsv11_v2`。Luma 保留旧 ID 作为只读诊断项，但设置和回归应选择 v2。

参考：

- [OBS QSV 注册与 adapter capability 判断](https://github.com/obsproject/obs-studio/blob/master/plugins/obs-qsv11/obs-qsv11-plugin-main.c)
- [OBS NVENC 动态加载驱动 API](https://github.com/obsproject/obs-studio/blob/master/plugins/obs-nvenc/nvenc-helpers.c)
- [OBS AMF runtime 探测](https://github.com/obsproject/obs-studio/blob/master/plugins/obs-ffmpeg/texture-amf.cpp)
- [Intel oneVPL Windows implementation 搜索顺序](https://intel.github.io/libvpl/latest/programming_guide/VPL_prg_session.html)

## API 与失败语义

`GET /api/v1/encoders` 对每个候选返回 `available/hardware/vendor/unavailable_reason`。
不可用原因按以下优先级生成：

1. OBS 模块打开或初始化失败（含 module code）；
2. NVENC/AMF 驱动 API DLL 无法加载，包含 DLL 名与 Windows error；
3. 驱动 API存在但 OBS 没注册兼容 H.264 encoder，提示检查 GPU 代际/驱动；
4. QSV 模块加载成功但未注册，提示检查 Intel adapter、媒体驱动和 VPL implementation；
5. deprecated 或未知 ID。

初始化日志逐项打印所有硬件候选的 available 或同一 reason。settings PUT 与 start 的 422
也复用该 reason。已经探测为 available、但实际 create/output start 失败的硬编继续执行 M4
同一次 start 内的 x264 fallback，并把真实创建错误写入 `fallback_reason`；探测阶段明确
unavailable 的请求不会先假启动，而是直接 422。

## 验收

```powershell
# Intel；必须 active=obs_qsv11_v2，不允许 fallback
powershell -ExecutionPolicy Bypass -File .\scripts\record-regression.ps1 `
  -Encoder qsv -RequireHw

# AMD；必须 active=h264_texture_amf
powershell -ExecutionPolicy Bypass -File .\scripts\record-regression.ps1 `
  -Encoder amf -RequireHw
```

无 NVIDIA 机器应看到 `nvEncodeAPI64.dll is unavailable` 或“驱动 API 已加载但没有兼容
encoder”，不得为空。依赖故障测试应只删除发行包自己捆绑、且在 manifest 中列出的 DLL；
不要删除 Driver Store/System32 文件。删除后重启引擎，相关项必须变为 unavailable 并给出
模块或 runtime reason。

macOS/Apple Silicon 与 Windows on ARM 仍不属于第一期发布目标；本页的 DLL和 ID仅描述
Windows x64。其他平台不得把“未支持”呈现成可用编码器。
