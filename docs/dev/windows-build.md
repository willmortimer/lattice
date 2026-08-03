# Windows build chain (beta)

**Status:** scaffold live — Mac → `will@nixdev` → DevDrive (`D:\lattice`) via
winbuild. Notarization / Authenticode are deferred; unsigned `.exe` builds are
the near-term goal.

## Operator entry

From the Lattice repo (or umbrella with `--flake ./lattice`):

```sh
# Probe + rustup ensure + headless cargo check (includes latticed + lattice-client)
nxr task windows-cargo-check
# or:
./scripts/winbuild/remote-windows-check.sh

# Release build latticed + lattice-client on Windows MSVC:
nxr task windows-latticed-check
# or:
WINBUILD_TASKS='probe ensure-toolchain cargo-build-latticed' \
  ./scripts/winbuild/remote-windows-check.sh

# Unsigned NSIS installer (sidecars + Tauri bundle):
nxr task windows-nsis-bundle
# or:
WINBUILD_TASKS='probe ensure-toolchain build-sidecar verify-sidecars tauri-bundle' \
  ./scripts/winbuild/remote-windows-check.sh
```

On nixdev itself (already synced):

```sh
# after sync to /mnt/d/lattice
winbuild.exe run probe --file 'D:\lattice\.winbuild.json'
winbuild.exe run ensure-toolchain --file 'D:\lattice\.winbuild.json'
winbuild.exe run cargo-check-core --file 'D:\lattice\.winbuild.json'
winbuild.exe run cargo-build-latticed --file 'D:\lattice\.winbuild.json'

# NSIS chain (unsigned)
winbuild.exe run build-sidecar --file 'D:\lattice\.winbuild.json'
winbuild.exe run verify-sidecars --file 'D:\lattice\.winbuild.json'
winbuild.exe run tauri-bundle --file 'D:\lattice\.winbuild.json'
```

| Env | Default | Purpose |
| --- | --- | --- |
| `NIXDEV_HOST` | `will@nixdev` | SSH target |
| `LATTICE_REMOTE` | `/home/will/Developer/lattice-ecosystem/lattice` | Remote Linux tree |
| `WINBUILD_DEST` | `/mnt/d/lattice` | Native DevDrive sync root (`D:\lattice`) |
| `WINBUILD_TASKS` | `probe ensure-toolchain cargo-check-core` | Tasks to run |

## NSIS packaging (T6)

PowerShell scripts under `scripts/windows/` mirror the macOS release leaves in
`scripts/release/`:

| Script | Role |
| --- | --- |
| `build-sidecar.ps1` | Release-build `latticed`, `lattice-agentd`, `lattice-embed-host` |
| `verify-sidecars.ps1` | Assert sidecars exist; embed-host lists `fake` only (no llama-cpp) |
| `assemble-app.ps1` | Copy sidecars beside `Lattice.exe` (called from `tauri-bundle`) |
| `tauri-bundle.ps1` | `tauri build --no-bundle --target …` → assemble → `tauri bundle --bundles nsis --target …` (assemble must not `exit`, or NSIS is skipped) |

Windows sidecars **exclude** seatbelt, voice, and llama-cpp/Metal backends.
Authenticode signing is deferred — installers are unsigned.

Output (when `CARGO_TARGET_DIR=D:\lattice-target\windows-msvc`):

```text
D:\lattice-target\windows-msvc\x86_64-pc-windows-msvc\release\Lattice.exe
D:\lattice-target\windows-msvc\x86_64-pc-windows-msvc\release\bundle\nsis\*-setup.exe
```

`tauri.windows.conf.json` sets `bundle.targets = "nsis"`. Frontend builds use
`pnpm tauri:build:windows` semantics (`--features capture`, no `voice-embedded`).

## Current blockers (ranked)

### P0 — No Windows Rust toolchain on PATH (host has MSVC)

**Update (probe 2026-08-03):** rustc/cargo/rustup **already present** under
`C:\Users\Will Mortimer\.cargo\bin`; MSVC BuildTools + `cl.exe` present; Node/pnpm
present. DevDrive `D:` ~90 GiB free.

`ensure-toolchain` still installs rustup if missing and now installs **protoc**
(needed by `lance` / `prost-build`) under `%LOCALAPPDATA%\NixPlane\protoc`.

### P0 — Missing `protoc` on Windows (hit during first cargo-check-core)

`cargo check` of headless crates pulls `lance` → build scripts require `protoc`.
Fixed by `ensure-toolchain` + `PROTOC` in `cargo-check-core.ps1`.

### Resolved — Daemon IPC (named pipes on Windows)

Named-pipe transport landed in T2 (`apps/daemon` + `lattice-client`). Winbuild
`cargo-check-core` and `cargo-build-latticed` now include `latticed` and
`lattice-client` and are expected to succeed on Windows MSVC.

### Resolved — NSIS packaging scaffold (T6)

- `scripts/windows/{build-sidecar,verify-sidecars,assemble-app,tauri-bundle}.ps1`
- `.winbuild.json` tasks + `nxr task windows-nsis-bundle`
- Unsigned NSIS only; Authenticode deferred

### P1 — macOS-only desktop features

Voice (`voice-embedded` / FluidAudio), Seatbelt, Quick Look, and overlay title bars
are cfg’d off or degraded for Windows beta builds (`tauri.windows.conf.json`,
macOS-only Cargo features for `voice` / `voice-embedded`). Screen capture
(`capture` / WGC) is enabled on Windows NSIS builds via `--features capture` in
`tauri-bundle.ps1` and `tauri:build:windows`. Manual smoke:
[capture-smoke.md](./capture-smoke.md).

### P2 — Frontend toolchain on Windows host

`node` / `pnpm` must be on Windows PATH for `tauri-bundle`. Headless Rust can
proceed first; NSIS needs Node on the Windows side or a prebuilt `apps/desktop/dist/`.

## Target layout on DevDrive

```text
D:\lattice\                 # synced sources (.winbuild.json at root)
D:\lattice-target\          # CARGO_TARGET_DIR for Windows builds
D:\NixPlane\bin\winbuild.exe
```

## Related

- NixPlane winbuild contract: `~/Developer/NixPlane/docs/WINBUILD.md`
- Cloud browser SIWA (public Mac/Windows builds): desktop deep-link handoff
