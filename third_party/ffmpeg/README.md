# FFmpeg packaging input

`scripts/pack.ps1` downloads the pinned Gyan.dev FFmpeg 9.0.2 Windows x64
essentials ZIP into the ignored `cache/` directory and verifies this SHA-256:

```text
60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba
```

Gyan.dev is linked by the FFmpeg project as a Windows binary provider. The
selected static build is GPLv3 and supplies `ffmpeg.exe`, `ffprobe.exe`, and
`ffplay.exe`. Its license is copied into every Luma Next package.
