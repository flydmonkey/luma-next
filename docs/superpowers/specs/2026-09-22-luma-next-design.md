# Luma Next Design Spec

- **Date**: 2026-09-22
- **Status**: Approved (cadence = first-version milestones)

## Decisions

1. Product name: **Luma Next**; repo: luma-next; archive 
ecord.
2. Engine: **embed libobs** in-process (not obs-websocket, not shell-first placeholder engine).
3. Platform: **Windows-first** (cross-platform structure reserved; Mac/Linux not in v1 acceptance).
4. Shape: **single repo, single process** — Rust HTTP hosts API (+ static WebUI later); thin desktop shell deferred.
5. License: **GPL** implications via libobs — documented in README.
6. Cadence: **first-version** M0→M4 (HTTP placeholder → real capture → WebUI → library → honest HW encode telemetry). Engine-first reorder rejected.

## Non-goals (v1)

No port of C# MF pipeline; no custom per-vendor MFT parity work; no streaming/plugin marketplace; no Tauri in M0–M1.

## References

- docs/BOUNDARY.md
- docs/CADENCE.md
- README.md
