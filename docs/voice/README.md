# Lattice Voice Dictation

Implementation-ready documentation for high-quality, fully local speech-to-text
on macOS, with a reusable voice-service architecture that can later support
Linux and Windows.

This package is accepted by
[ADR 0040](../decisions/0040-local-voice-dictation-documentation.md).
Subsystem decisions live under [adr/](./adr/).

## Product goals

Initial user-facing capabilities:

- Push-to-talk dictation inside any editable Lattice document
- Continuous in-app dictation
- Global Quick Note dictation
- Provisional text while the user is speaking
- Higher-quality final text after a pause or session completion
- Local-only audio processing
- Spoken punctuation and basic formatting
- Future voice access to slash commands
- No requirement for a Lattice cloud account

## Non-goals for the initial release

Explicitly excluded:

- General macOS-wide dictation outside Lattice
- Arbitrary computer control
- Fully conversational voice agents
- Cloud transcription
- Speaker diarization
- Meeting recording
- Voice authentication
- Arbitrary shell, SQL, MCP, or plugin invocation
- Linux and Windows implementations in the first release

## System summary

```text
Microphone
    ↓
Tauri/macOS native capture (client-owned mic; ADR 0008)
    ↓
Float32 PCM @ 16 kHz mono (packed binary chunks)
    ↓
latticed voice session (DaemonClient thin client — M4 preferred)
    ↓
lattice-voice-host (FluidAudio / Parakeet; supervised like embed-host)
    ↓
Provisional transcript events → Tauri `voice-event` → editor decorations

At utterance boundary:
FinishUtterance → FinalTranscript (daemon may normalize glossary)
    ↓
Editor transaction via semantic command core

Transitional fallback (feature `voice-embedded` only):
  if latticed/voice-host is unavailable, Tauri may prepare in-process
  FluidAudio with an explicit "degraded embedded" log. Set
  `LATTICE_VOICE_DAEMON=1` to require the daemon path (no FluidAudio
  symbols when building with `--features voice` alone).
```

Production provider stack (normative — see
[research/voice-m0-fluidaudio/DECISION.md](../../research/voice-m0-fluidaudio/DECISION.md)):

- **FluidAudio `0.15.5`** — Apple-native inference framework (Apache-2.0)
- **Parakeet Unified 0.6B** — streaming provisional decode + authoritative
  `finish()` final from one checkpoint (`parakeet-unified-320ms`, CC-BY-4.0)
- Warm first-partial on M0 fixture: **158.3 ms**
  ([RESULTS-unified.md](../../research/voice-m0-fluidaudio/RESULTS-unified.md))

Historical M0 EOU+TDT pair (non-production): documented in
[RESULTS.md](../../research/voice-m0-fluidaudio/RESULTS.md).

Shared protocol and in-process foundation: **`crates/lattice-voice`**.
macOS FluidAudio bridge and `FluidAudioSpeechProvider`: **`crates/lattice-voice-macos`**.

## Documentation map

| Document | Responsibility |
|----------|----------------|
| [architecture.md](./architecture.md) | Component boundaries, process placement, provider traits |
| [macos-integration.md](./macos-integration.md) | Permissions, lifecycle, shortcuts, native helper strategy |
| [audio-capture.md](./audio-capture.md) | Canonical PCM format, capture pipeline, pre-roll, backpressure |
| [fluid-audio-bridge.md](./fluid-audio-bridge.md) | Swift↔Rust C ABI, ownership, build integration |
| [transcription-pipeline.md](./transcription-pipeline.md) | Session state machine, provisional/final paths, failures |
| [editor-integration.md](./editor-integration.md) | Anchors, provisional decorations, final transactions, undo |
| [quick-note-dictation.md](./quick-note-dictation.md) | Global Quick Note flow, residency, destinations, recovery |
| [voice-commands.md](./voice-commands.md) | Deterministic formatting and slash-command voice aliases |
| [daemon-protocol.md](./daemon-protocol.md) | Versioned local IPC between clients and latticed |
| [model-management.md](./model-management.md) | Manifests, install, cache, warm/cold residency |
| [privacy-security.md](./privacy-security.md) | Local-only guarantees, threat model, security requirements |
| [licensing-distribution.md](./licensing-distribution.md) | AGPL + FluidAudio + model weight attribution |
| [observability-testing.md](./observability-testing.md) | Metrics, unit/integration tests, golden audio suite |
| [performance-budget.md](./performance-budget.md) | Latency and quality targets |
| [implementation-roadmap.md](./implementation-roadmap.md) | Milestones M0–M8 with exit criteria |
| [current-implementation-review-and-ml-architecture.md](./current-implementation-review-and-ml-architecture.md) | Capture/finalization review and Mac-first ML pipeline (post-M1) |
| [adr/](./adr/) | Accepted subsystem architecture decisions |
| [research/voice-m0-fluidaudio/RESULTS.md](../../research/voice-m0-fluidaudio/RESULTS.md) | M0 FluidAudio spike measurements (EOU+TDT historical path) |
| [research/voice-m0-fluidaudio/DECISION.md](../../research/voice-m0-fluidaudio/DECISION.md) | M1 production path decision (`production_path: unified`) |
| [research/voice-m0-fluidaudio/RESULTS-unified.md](../../research/voice-m0-fluidaudio/RESULTS-unified.md) | Unified production measurements |
| `crates/lattice-voice` | Shared voice protocol types and in-process provider foundation |
| `crates/lattice-voice-macos` | macOS FluidAudio C ABI bridge and `FluidAudioSpeechProvider` (M1 landed) |

Normative requirements use **must** / **must not**. Recommended defaults use
**should**. Optional extensions use **may**. Open questions are listed in
[implementation-roadmap.md](./implementation-roadmap.md) and mirrored in
[docs/31-open-questions-and-decision-register.md](../31-open-questions-and-decision-register.md).

## Alignment with Lattice invariants

Voice must respect:

- Offline is the normal state ([docs/02](../02-principles-and-invariants.md) #2)
- GUI mutations flow through the semantic command core (#6; [ADR 0007](../decisions/0007-semantic-command-transaction-core.md))
- Specialized surfaces own hot paths; React does not mediate audio samples (#16; [ADR 0006](../decisions/0006-react-shell-specialized-renderers.md))
- Capabilities are lazy and contextual ([ADR 0019](../decisions/0019-lazy-workspace-capabilities.md))
- No silent cloud inference or content telemetry (#20)
- Progressive disclosure: voice is not in the primary Page / Canvas / Table /
  Notebook / File creation vocabulary until explicitly enabled

Related product hooks already in the tree:

- Dictation as an input method — [docs/36](../36-platforms-accessibility-localization-and-mobile.md)
- Voice / transcript import — [docs/21](../21-search-links-context-and-ai-interoperability.md)
- Optional `latticed` — [docs/18](../18-automation-events-workflows-and-daemon.md)
- Third-party license disclosure — [docs/35](../35-licensing-governance-and-sustainability.md)

## Definition of done (documentation phase)

This documentation phase is complete when:

- [x] All listed documents exist
- [x] Every major component has an explicit owner
- [x] Audio format and IPC contract are specified
- [x] Provisional-versus-final transcript lifecycle is unambiguous
- [x] Editor persistence and undo semantics are defined
- [x] Quick Note behavior is defined from shortcut through atomic save
- [x] Swift/Rust boundary has an explicit ABI and ownership model
- [x] Privacy and command-safety requirements are testable
- [x] Model distribution and attribution requirements are documented
- [x] Performance targets are measurable
- [x] Open questions are assigned to research spikes
- [x] Implementation roadmap can convert directly into GitHub issues or a project DAG
