# Cargo / CI compile performance

Lattice’s expensive graph is DuckDB (bundled C++), Wasmtime, Arrow/Lance, and
Tauri. Nix/NXR do not make those compiles cheaper; they decide how often a
fresh machine repeats them.

## What this repo does

- **sccache in flake apps**, not only `nix develop`. `RUSTC_WRAPPER=sccache`
  is prepended to every NXR cargo leaf. Current `cc` (1.2.x) can wrap the C++
  that `libduckdb-sys` builds.
- **CI rust leaf is `rust-validate`**: clippy then tests share one `target/`
  (one machine). Do not run `rust-clippy` and `rust-test` as separate jobs.
  Clippy in this leaf uses default lint levels (correctness is already deny).
  `nxr task rust-clippy` still passes `-D warnings` locally; promoting that bar
  in tagged GitHub Actions is a follow-up now that the matrix actually
  schedules this leaf.
- **Tagged GitHub Actions cargo is non-incremental** because those workflows
  export `CARGO_INCREMENTAL=0`. Flake apps do not infer that from `CI=true`.
  Local `nxr task rust-validate` keeps Cargo’s default incremental rustc.
- **Release sidecars are one Cargo cohort** (`nxr task build-sidecars` /
  Windows `build-sidecar.ps1` default path). Cargo schedules the shared crate
  DAG; NXR does not serialize four `exclusive = ["cargo-target"]` leaves.
- **Dev profiles**: `debug = "line-tables-only"` and
  `[profile.dev.package."*"] debug = false`. Use `--profile debugging` when
  you need full DWARF.
- **Linux links with mold** when the flake toolchain is active.
- **Tests prefer cargo-nextest** when the flake provides `cargo-nextest`.
- **Wasmtime defaults are off.** Workspace pin (lockstep with kernelfs) is
  `default-features = false` plus `async`, `component-model`, `cranelift`,
  `runtime`, `std`. `wasmtime-wasi` still requires `async` + `component-model`;
  cranelift compiles guests from binary. gc/wat/cache/profiling/addr2line/
  coredump/threads/pooling-allocator/parallel-compilation stay off.

## Measured on this branch (2026-09-02, Mac, rustc 1.98, no sccache)

Cold `CARGO_TARGET_DIR` under `/tmp`, `origin/main` vs this branch’s
`[profile.dev]` (`debug = "line-tables-only"`, deps `debug = false`).
Skipped desktop-install (~57 min).

| Leaf | Before (s) | After (s) | Delta |
| --- | ---: | ---: | ---: |
| `cargo test -p lattice-core --lib` | 29.05 | 23.74 | −18% |
| `cargo test -p lattice-commands --lib` | 37.30 | 29.10 | −22% |
| `cargo test -p lattice-duckdb --lib` | 236.06 | 151.90 | −36% |

User-time on DuckDB dropped 847s → 681s, so this is less DWARF work, not
just a warmer disk. A follow-up sccache miss/hit on `lattice-core` with
`CARGO_INCREMENTAL=0` showed **cc-rs wrapping** (1 C/C++ miss then hit for
`libsqlite3-sys`) but **no Rust object hits** on rustc 1.98 / sccache 0.16
with a fresh target. Still wire the wrapper: DuckDB’s C++ is the cache we
care about. Measure rustc vs C/C++ hit rate from local `sccache --show-stats`
after `nxr task rust-validate` (GitHub Actions only runs on `v*` tags).

## Measured Wasmtime feature prune (2026-09-03)

Cold `cargo test --no-run -p lattice-agentd` (Nix rustc 1.96.1, no sccache,
no incremental, fresh `CARGO_TARGET_DIR`). Agentd’s graph does not include
DuckDB or Lance, so this isolates Wasmtime.

| Run | Cargo | Wall |
| --- | ---: | ---: |
| Defaults on | 3m 33s | 221s |
| Pruned (first) | 1m 57s | 123s |
| Pruned (repeat) | 2m 57s | 184s |

Keep the pin: even the slower repeat is ~40s (17%) under the default-feature
baseline. Per-crate timings dropped `wat`, `wasmtime-cache`, `addr2line`,
`wasmtime-jit-debug`, and `rayon`; `wasmtime-cranelift` shrank (gc/threads
off). Cranelift-codegen remains the long pole (~1 min) and is required.

Did not re-run a full cold `rust-validate`: the worktree `target/` is already
~53 GiB and the volume had ~21 GiB free. `rust-validate` wall is still
Lance/DataFusion + DuckDB. KernelFS `cargo test --workspace` (including 20
integration guests) passed on the pruned pin.

## What this repo does not do yet

- Unify workspace Arrow **59** with Lance’s Arrow **58** (two stacks in one
  graph). Resume when Lance publishes a matching Arrow pin — that is the next
  compile-graph win.
- Nix-built DuckDB linked via `DUCKDB_LIB_DIR` (keeps C++ out of Cargo).
  Needed before Tauri can stop shipping a Cargo-built dylib. Local sccache
  already hits ~99% on DuckDB C++; skip until Tauri packaging needs it.
- crane `buildDepsOnly` for clippy/nextest (FlakeHub-cached dep artifacts,
  including `../kernelfs`).
- Remote sccache (R2 / Attic / private plane). Unify lattice `nixpkgs-unstable`
  vs umbrella `nixos-unstable` first or the cache keys miss.
- Pulling Swift / llama.cpp out of `build.rs` into NXR/Nix tasks.
- Dropping DataFusion: `lancedb` 0.31 already has empty default features;
  DataFusion is a hard dependency, not a flag.

See the umbrella note `docs/dev/build-plane-followups.md` for NXR / NixPlane /
NXB / winbuild work that does not belong in this client tree.

## Commands

```sh
python3 scripts/ci-plan-to-matrix.test.py
sccache --show-stats
nxr task rust-validate
nxr graph desktop-release   # verify-sidecars depends on build-sidecars
```
