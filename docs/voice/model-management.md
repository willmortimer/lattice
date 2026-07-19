# Model Management

## Scope

Model installation, verification, caching, loading, updates, and residency for
voice providers. M0 measured a **dual-artifact** FluidAudio stack: EOU streaming
plus TDT v2 offline ([research/voice-m0-fluidaudio/RESULTS.md](../../research/voice-m0-fluidaudio/RESULTS.md)).
Production must still decide whether to ship that pair or the upstream
**Unified** single-checkpoint alternative (`parakeet-unified-en-0.6b-coreml`).

## Model manifest

M0 used two artifacts. Example manifests (hashes and sizes are placeholders
until install-time verification is wired; pins and licenses are from M0):

```json
{
  "schema_version": 1,
  "id": "parakeet-realtime-eou-120m-coreml",
  "display_name": "Parakeet Realtime EOU 120M (160ms)",
  "provider": "fluid-audio",
  "version": "0.15.5",
  "upstream_id": "FluidInference/parakeet-realtime-eou-120m-coreml",
  "languages": ["en"],
  "license": "NVIDIA Open Model License",
  "license_url": "https://www.nvidia.com/en-us/agreements/enterprise-software/nvidia-open-model-license/",
  "source": "FluidAudio HuggingFace cache (download-on-setup)",
  "sha256": "verify-at-install",
  "size_bytes": 0,
  "runtime": "coreml",
  "streaming": true,
  "offline_decode": false,
  "chunk_variant_ms": 160
}
```

```json
{
  "schema_version": 1,
  "id": "parakeet-tdt-0.6b-v2-coreml",
  "display_name": "Parakeet TDT 0.6B v2",
  "provider": "fluid-audio",
  "version": "0.15.5",
  "upstream_id": "FluidInference/parakeet-tdt-0.6b-v2-coreml",
  "languages": ["en"],
  "license": "CC-BY-4.0",
  "source": "FluidAudio HuggingFace cache (converted from nvidia/parakeet-tdt-0.6b-v2)",
  "sha256": "verify-at-install",
  "size_bytes": 0,
  "runtime": "coreml",
  "streaming": false,
  "offline_decode": true
}
```

**Unified alternative (not measured in M0):** upstream also ships
`FluidInference/parakeet-unified-en-0.6b-coreml` via `UnifiedAsrManager` /
`StreamingUnifiedAsrManager` — one checkpoint for streaming and offline. If
production pins Unified, manifests collapse to a single entry; the EOU+TDT pair
above remains the documented M0 spike path until that decision lands.

Combined cache footprint after M0 download: **~890 MB** under FluidAudio’s
`.cache/Models/` (gitignored in the research spike).

## Installation flow

1. Present model size and license.
2. Download into a temporary path.
3. Verify hash.
4. Atomically move into the model directory.
5. Compile or prepare Core ML assets.
6. Run a short self-test.
7. Mark the model ready.
8. Retain the previous working version during upgrades.

Recommended packaging: download and prepare during explicit Voice Dictation
setup ([licensing-distribution.md](./licensing-distribution.md)).

## Storage layout

Example (macOS):

```text
~/Library/Application Support/Lattice/
  voice/
    manifests/
    models/
      parakeet-realtime-eou-120m-coreml/
        <version>/
      parakeet-tdt-0.6b-v2-coreml/
        <version>/
    compiled/
    downloads/
    diagnostics/
```

## Updates

Document and implement:

- Manual versus automatic model updates (default: manual or explicit opt-in)
- Rollback to previous working version
- Manifest signing (when available)
- Hash verification on every activate
- Metered-network behavior (warn / require confirmation)
- Bundled vs fetched — prefer fetched on first use for v1
- Attribution retention across updates

## Memory management

States:

| State | Meaning |
|-------|---------|
| Cold | On disk only |
| Loaded | Mapped / prepared |
| Warm | Ready for low-latency session start |
| Active-session | Decoding |

Also define:

- Idle timeout
- Memory-pressure unloading
- Maximum concurrent sessions (start at 1 for v1)

Owner of residency policy long-term: `latticed`
([architecture.md](./architecture.md)).

M0 warm loads (cached `.mlmodelc`, M2 MacBook Air): streaming **~681 ms**,
offline **~399 ms**. Cold first download + Core ML compile: **~98–110 s** per
model ([RESULTS.md](../../research/voice-m0-fluidaudio/RESULTS.md)).

## Security implications

- Compromised downloads are mitigated by hash (and optional signature) checks.
- Diagnostics **must not** include audio or transcript content by default.

## Testing requirements

- Manifest schema validation
- Hash mismatch rejects install
- Atomic replace and rollback
- Self-test failure does not mark ready
- Memory-pressure unload

## Open questions

- **Unified vs EOU+TDT production pin** — Unified exists upstream but was not
  measured in M0; dual residency memory cost (research Q4) also remains open.
- Core ML compile reuse across app updates — **partially answered**: warm load
  reuses cached `.mlmodelc` under FluidAudio cache; cross-app-update policy TBD.
- Attribution text for converted weights — **resolved for M0 pins** (Apache-2.0
  FluidAudio + NVIDIA Open Model EOU + CC-BY-4.0 TDT v2); see
  [licensing-distribution.md](./licensing-distribution.md).

## Acceptance criteria

- [ ] Setup UI always shows size + license before download
- [ ] Hash verification is mandatory
- [ ] Previous version retained during upgrade
- [ ] Self-test gates “ready”
