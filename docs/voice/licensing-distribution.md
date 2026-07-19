# Licensing and Distribution

## Scope

How FluidAudio and model weights fit Lattice’s AGPL distribution
([ADR 0031](../decisions/0031-agpl-3-or-later.md),
[docs/35](../35-licensing-governance-and-sustainability.md)).

## Required inventory

Track separately:

| Artifact | Notes |
|----------|-------|
| Lattice source code | AGPL-3.0-or-later |
| FluidAudio source and binaries | Preserve upstream license (expected Apache-2.0 — verify at pin) |
| Swift bridge code | Lattice-owned; AGPL unless stated otherwise |
| Parakeet model weights | Separate license (manifest field; often CC-BY — verify) |
| Core ML conversion artifacts | Attribution and redistribution rights of converter + weights |
| Any voice-activity detection model | Separate inventory row |
| Any bundled audio-resampling dependencies | License + notices |

## Per-dependency record

For each dependency record:

- Name
- Version
- Source
- Copyright holder
- License
- Modification status
- Required notices
- Redistribution rights
- Commercial-use rights
- Whether source disclosure is required
- Whether the artifact is bundled or downloaded separately

## Packaging strategy

Prefer:

- AGPL for Lattice-owned Rust and Swift bridge code
- Preservation of Apache-2.0 (or actual) notices for FluidAudio
- Separate model attribution and CC-BY (or actual) notice
- Model manifest visible in Settings
- Full attribution included in source and packaged application
- **No** claim that model weights themselves are AGPL

## Distribution alternatives

### Bundle model with application

Advantages: works immediately; no post-install download.

Disadvantages: larger application; more complicated attribution and updates;
longer notarization and distribution cycle.

### Download model on first use

Advantages: smaller application; easier model updates; explicit license
display; users who do not use voice avoid downloading it.

**Recommended initial approach:** download and prepare the model during
explicit Voice Dictation setup.

## Freemium compatibility

Local voice inference **must not** be plan-gated in a way that contradicts
docs/35 guidance on local AI-provider integration. Monetization may attach to
cloud sync or hosted services, not to the right to run local models the user
already obtained under their licenses.

## Testing requirements

- License manifest generation in CI
- Settings attribution surface includes all inventory rows
- Setup UI blocks download until license acknowledged

## Open questions

- Exact attributions for the chosen converted artifact (research Q15)
- FluidAudio pin license confirmation (research Q1)

## Acceptance criteria

- [ ] Inventory table is complete before first public beta with voice
- [ ] Model weights are not labeled AGPL
- [ ] First-use download shows license + size
- [ ] Packaged app includes notices
