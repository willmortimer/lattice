# Model Management

## Scope

Model installation, verification, caching, loading, updates, and residency for
voice providers. Initial model: Parakeet Unified English Core ML via FluidAudio.

## Model manifest

```json
{
  "schema_version": 1,
  "id": "parakeet-unified-en-coreml",
  "display_name": "Parakeet Unified English",
  "provider": "fluid-audio",
  "version": "pinned-version",
  "languages": ["en"],
  "license": "CC-BY-4.0",
  "source": "documented-upstream-location",
  "sha256": "expected-hash",
  "size_bytes": 0,
  "runtime": "coreml",
  "streaming": true,
  "offline_decode": true
}
```

Fields marked `pinned-version`, hash, size, and source **must** be filled by
Milestone 0 before any user download ships.

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
      parakeet-unified-en-coreml/
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

- Exact artifact pin (research Q1)
- Core ML compile reuse across app updates (research Q8)
- Attribution text for converted weights (research Q15)

## Acceptance criteria

- [ ] Setup UI always shows size + license before download
- [ ] Hash verification is mandatory
- [ ] Previous version retained during upgrade
- [ ] Self-test gates “ready”
