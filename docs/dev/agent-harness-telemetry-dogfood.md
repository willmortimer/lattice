# Dogfood checklist — agent harness, spatial tools, telemetry

SIWA Development install rebuild after this pass:

```sh
cd lattice
# Prefer NXR apple context (injects signing secrets):
nix develop -c nxr context run apple-development -- task desktop-install -j 8

# Or ecosystem secrets helper (requires `sops` on PATH):
../scripts/with-secrets.sh apple -- nxr task desktop-install -j 8

# Run:
# from ecosystem root
./scripts/exec-for-dev.sh -- "/Applications/Lattice.app/Contents/MacOS/lattice-desktop"
```

## Smoke

1. **Tool rounds:** Ask a multi-doc architecture/roadmap question. Confirm it does not fail at 8 rounds; exhausted loops (if forced via `LATTICE_AGENT_MAX_TOOL_ROUNDS=2`) name the cap and the env override.
2. **Spatial fake:** Agent provider = fake; prompt exactly `spatial-demo`. Trail shows search step; overlay highlight/replay works.
3. **Spatial live:** With Luna/account AI, ask to highlight a known block on `Product/Release Notes.md` (path + `blockId` from search). Confirm Guide/Quiet highlight.
4. **Propose path:** Write ask creates a proposal (inbox), does not claim applied.
5. **Privacy toggles:** Settings → Privacy → AI request audit / Anonymous product telemetry. Signed-in: toggles sync via `PUT /v1/me/preferences`. Signed-out: local-only copy.
6. **AI audit:** With audit on, paid AI leaves metadata-only `ai_request` audit rows; with audit off, quota still increments but no audit row.
7. **Telemetry:** With telemetry on, opening Settings / Agent panel does not error (best-effort POST `/v1/telemetry/events`). With telemetry off, events are skipped locally.

## Next dogfood (not this rebuild)

- `desktop-install-dist` + browser SIWA callout/back flow (SIWA-less quality pass).
- BYO/local AI opt-in cloud logging.
- Full OTel exporter.
