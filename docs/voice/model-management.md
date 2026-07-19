# Model Management

## Scope

Model installation, verification, caching, loading, updates, and residency for
voice providers. Production pins **Unified**
(`parakeet-unified-en-0.6b-coreml`, 320 ms streaming tier) per
[research/voice-m0-fluidaudio/DECISION.md](../../research/voice-m0-fluidaudio/DECISION.md).
M0 measured a **dual-artifact** EOU+TDT stack as a historical spike
([research/voice-m0-fluidaudio/RESULTS.md](../../research/voice-m0-fluidaudio/RESULTS.md));
that pair is a documented non-production alternative.

## Model manifest

### Primary (production)

```json
{
  "schema_version": 1,
  "id": "parakeet-unified-en-0.6b-coreml",
  "display_name": "Parakeet Unified English 0.6B (320ms)",
  "provider": "fluid-audio",
  "version": "0.15.5",
  "upstream_id": "FluidInference/parakeet-unified-en-0.6b-coreml",
  "languages": ["en"],
  "license": "CC-BY-4.0",
  "source": "FluidAudio HuggingFace cache (converted from nvidia/parakeet-unified-en-0.6b)",
  "sha256": "verify-at-install",
  "size_bytes": 0,
  "runtime": "coreml",
  "streaming": true,
  "offline_decode": false,
  "streaming_variant": "parakeet-unified-320ms",
  "authoritative_final": "streaming_finish"
}
```

Streaming partials and the authoritative final both use one loaded streaming
checkpoint (`StreamingUnifiedAsrManager.finish()`). An optional offline encoder
from the same HF repo (~+578 MB) is **not** required for the production
stream→final path
([RESULTS-unified.md](../../research/voice-m0-fluidaudio/RESULTS-unified.md)).

### Non-production alternative (M0 measured EOU+TDT)

The following manifests document the historical M0 spike path. Hashes and sizes
are placeholders until install-time verification is wired; pins and licenses are
from M0:

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
  "chunk_variant_ms": 160,
  "production": false
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
  "offline_decode": true,
  "production": false
}
```

Combined EOU+TDT cache footprint after M0 download: **~890 MB** under
FluidAudio’s `.cache/Models/`. Streaming-only Unified cache after Task U:
**~608 MB**
([RESULTS-unified.md](../../research/voice-m0-fluidaudio/RESULTS-unified.md)).

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
      parakeet-unified-en-0.6b-coreml/
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

Unified warm streaming load (cached `.mlmodelc`, M2 MacBook Air): **~504 ms**
([RESULTS-unified.md](../../research/voice-m0-fluidaudio/RESULTS-unified.md)).
M0 EOU/TDT warm loads for comparison: streaming **~681 ms**, offline **~399 ms**.
Cold first download + Core ML compile: **~59.7 s** Unified streaming (Task U) vs
**~98–110 s** per EOU/TDT model on M0 host.

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

- Core ML compile reuse across app updates — **partially answered**: warm load
  reuses cached `.mlmodelc` under FluidAudio cache; cross-app-update policy TBD.
- Attribution text for converted weights — **resolved for production pin**
  (Apache-2.0 FluidAudio + CC-BY-4.0 Unified); EOU+TDT adds NVIDIA Open Model
  if used as fallback. See [licensing-distribution.md](./licensing-distribution.md).

## Acceptance criteria

- [ ] Setup UI always shows size + license before download
- [ ] Hash verification is mandatory
- [ ] Previous version retained during upgrade
- [ ] Self-test gates “ready”
