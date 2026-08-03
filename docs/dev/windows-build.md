# Windows build chain (beta)

**Status:** scaffold live — Mac → `will@nixdev` → DevDrive (`D:\lattice`) via
winbuild. Notarization / Authenticode are deferred; unsigned `.exe` builds are
the near-term goal.

## Operator entry

From the Lattice repo (or umbrella with `--flake ./lattice`):

```sh
# Probe + rustup ensure + headless cargo check (no Tauri)
nxr task windows-cargo-check
# or:
./scripts/winbuild/remote-windows-check.sh

# Intentionally fail to surface latticed IPC compile errors:
WINBUILD_TASKS='probe ensure-toolchain cargo-build-latticed' \
  ./scripts/winbuild/remote-windows-check.sh
```

On nixdev itself (already synced):

```sh
# after sync to /mnt/d/lattice
winbuild.exe run probe --file 'D:\lattice\.winbuild.json'
winbuild.exe run ensure-toolchain --file 'D:\lattice\.winbuild.json'
winbuild.exe run cargo-check-core --file 'D:\lattice\.winbuild.json'
```

| Env | Default | Purpose |
| --- | --- | --- |
| `NIXDEV_HOST` | `will@nixdev` | SSH target |
| `LATTICE_REMOTE` | `/home/will/Developer/lattice-ecosystem/lattice` | Remote Linux tree |
| `WINBUILD_DEST` | `/mnt/d/lattice` | Native DevDrive sync root (`D:\lattice`) |
| `WINBUILD_TASKS` | `probe ensure-toolchain cargo-check-core` | Tasks to run |

## Current blockers (ranked)

### P0 — Daemon IPC is Unix-domain only

`apps/daemon/src/server.rs` still uses `tokio::net::{UnixListener,UnixStream}`
with no `cfg(windows)` path. The client crate now connects via a
transport-neutral layer (`crates/lattice-client/src/transport.rs`); Windows
desktop still cannot talk to `latticed` until the daemon server accepts named
pipes.

**Direction (locked for beta):**

1. Introduce a thin framed transport over `AsyncRead + AsyncWrite` shared by
   client + server (handshake + length-delimited envelopes stay unchanged).
2. **Windows only:** named pipe `\\.\pipe\lattice-latticed-<user>` (from
   `USERNAME` / equivalent). No TCP interim.
3. Keep Unix domain sockets on macOS/Linux
   (`…/Lattice/run/latticed.sock`).

Touched surfaces: `apps/daemon/src/server.rs`, `crates/lattice-client/src/daemon.rs`,
`apps/daemon/src/spawn.rs`, desktop `daemon_session` connect/spawn, default
endpoint path helpers.

### P0 — No Windows Rust toolchain on PATH (host has MSVC)

**Update (probe 2026-08-03):** rustc/cargo/rustup **already present** under
`C:\Users\Will Mortimer\.cargo\bin`; MSVC BuildTools + `cl.exe` present; Node/pnpm
present. DevDrive `D:` ~90 GiB free.

`ensure-toolchain` still installs rustup if missing and now installs **protoc**
(needed by `lance` / `prost-build`) under `%LOCALAPPDATA%\NixPlane\protoc`.

### P0 — Missing `protoc` on Windows (hit during first cargo-check-core)

`cargo check` of headless crates pulls `lance` → build scripts require `protoc`.
Fixed by `ensure-toolchain` + `PROTOC` in `cargo-check-core.ps1`.

### P1 — Tauri / NSIS packaging not wired

- No `scripts/release/*windows*`, no NSIS/MSI flake apps, no CI Windows job.
- `tauri.conf.json` `bundle.targets = "all"` is generic only.
- Release scripts hard-require Darwin.
- Azure / Authenticode signing deferred — ship unsigned NSIS/`exe` first.

### P1 — macOS-only desktop features

Voice (`voice-embedded` / FluidAudio), Seatbelt, Quick Look, overlay title bars
must be cfg’d off or degraded for Windows beta builds.

### P2 — Frontend toolchain on Windows host

`node` / `pnpm` not on Windows PATH for full Tauri UI builds. Headless Rust can
proceed first; Tauri needs Node on the Windows side or a prebuilt `dist/`.

## Target layout on DevDrive

```text
D:\lattice\                 # synced sources (.winbuild.json at root)
D:\lattice-target\          # CARGO_TARGET_DIR for Windows builds
D:\NixPlane\bin\winbuild.exe
```

## Related

- NixPlane winbuild contract: `~/Developer/NixPlane/docs/WINBUILD.md`
- Cloud browser SIWA (public Mac/Windows builds): desktop deep-link handoff
