# lattice-capture-windows

Windows still-image capture for Lattice clipper via **Windows Graphics Capture (WGC)**.

## Stack

| Piece | Choice |
| --- | --- |
| Capture API | `Windows.Graphics.Capture` (`GraphicsCaptureItem` + `Direct3D11CaptureFramePool::CreateFreeThreaded`) |
| Monitor item | `IGraphicsCaptureItemInterop::CreateForMonitor` |
| GPU | D3D11 hardware device with `D3D11_CREATE_DEVICE_BGRA_SUPPORT`, staging texture CPU read |
| Encode | `image` crate PNG from RGBA (BGRA staging converted at copy time) |
| Region | Full-display WGC frame + CPU crop in virtual-desktop coordinates |
| Interactive region | Minimal Win32 layered overlay rubber-band (Esc / right-click cancels) |
| Window sources | `CaptureError::Unsupported` (deferred) |
| Recording | Trait default `Unsupported` |
| Self-exclusion | Capture-time `WDA_EXCLUDEFROMCAPTURE` on this process's visible top-level HWNDs; picker HWND excluded too. Always-on shelf exclusion remains B1. |
| Permissions | Best-effort: `GraphicsCaptureSession::IsSupported` → Authorized / Unsupported. Win32 has no reliable macOS-style TCC query; capture is not blocked on an unreadable privacy toggle. Settings deep-link: `ms-settings:privacy-graphicscapture`. |

Non-Windows hosts compile the public types and return `CaptureError::Unsupported` so CI/unit tests stay green without a Windows GPU.

Host selection continues through A0 `apps/desktop/src-tauri/src/capture/platform.rs`.

## Tests

```sh
cargo test -p lattice-capture-windows
cargo test -p lattice-capture-core
cargo check -p lattice-capture-windows --target x86_64-pc-windows-msvc
cargo check -p lattice-desktop --features capture
```

## Out of scope

- Window capture
- Screen recording sessions
- Shelf always-on `WDA_EXCLUDEFROMCAPTURE` polish (B1)
- NSIS / installer feature flip (A2)
