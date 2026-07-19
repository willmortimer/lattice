# Voice Architecture

## Scope

Defines application architecture and component boundaries for Lattice voice
dictation. Normative ownership decisions live here; provider-specific detail
belongs in [fluid-audio-bridge.md](./fluid-audio-bridge.md) and
[daemon-protocol.md](./daemon-protocol.md).

Accepted decisions: [adr/0001](./adr/0001-voice-service-boundary.md),
[adr/0002](./adr/0002-fluid-audio-macos-provider.md),
[adr/0004](./adr/0004-client-owned-audio-capture.md).

## Responsibilities

### Tauri application

**Must:**

- Request and maintain microphone permission
- Select the active input device
- Capture microphone audio
- Resample and normalize audio to the canonical format
  ([audio-capture.md](./audio-capture.md))
- Maintain push-to-talk and continuous-session UI
- Render provisional transcripts
- Submit final transcript operations to the editor
- Handle global shortcuts and Quick Note presentation
- Display model download and preparation status

**Must not:**

- Own the model files as the long-term source of truth
- Compile Core ML models directly in the WebView layer
- Keep separate recognition models loaded per window
- Commit unstable provisional text into document state
- Interpret arbitrary voice commands independently of the shared command registry

### latticed (target owner of inference)

**Must:**

- Own model installation and verification
- Load and warm the FluidAudio provider
- Maintain active transcription sessions
- Accept PCM audio from trusted local clients
- Emit partial, stable, and final transcript events
- Buffer utterance audio for offline final re-decoding
- Manage vocabulary and workspace glossary data
- Apply transcript normalization
- Parse deterministic voice commands
- Expose backend capabilities
- Handle model unloading and memory pressure
- Record local performance telemetry when enabled

`latticed` is optional elsewhere in Lattice
([docs/04](../04-system-architecture.md), [docs/18](../18-automation-events-workflows-and-daemon.md)).
For voice, the **protocol** is daemon-compatible from day one; process
placement may start in-process (see below).

### macOS FluidAudio bridge

**Must:**

- Wrap the minimum FluidAudio APIs needed by Lattice
- Provide a stable C ABI between Swift and Rust
- Translate Rust session configuration into FluidAudio configuration
- Deliver streaming transcript callbacks
- Perform offline re-decoding
- Normalize Swift errors into stable bridge error codes
- Prevent Swift framework details from leaking into shared Rust interfaces

### Editor integration

**Must:**

- Create and track logical insertion anchors
- Display provisional text without persisting it
- Replace provisional text when final text arrives
- Commit final text through the normal editor transaction API and semantic
  command path
- Preserve undo, collaboration, and sync semantics
- Execute voice commands through the same command bus as slash commands
- Handle stale or deleted insertion targets safely

Details: [editor-integration.md](./editor-integration.md).

## Process placement

### Initial integrated mode

```text
Tauri process
  ├── WebView/editor
  ├── Rust backend
  └── Swift FluidAudio bridge
```

Advantages:

- Simpler initial integration
- Lower IPC complexity
- Easier debugging
- No independent daemon lifecycle issues

Disadvantages:

- Each running app process owns model state
- Quick Note background support becomes harder
- App restarts unload the model
- Future CLI or automation consumers cannot reuse speech services

### Target daemon mode

```text
Tauri application
    │
    │ local IPC
    ▼
latticed
    └── FluidAudio bridge and model runtime
```

Advantages:

- One model instance per user
- Warm model available for Quick Note
- Voice can be reused by CLI, menu-bar helper, automation, and future clients
- Better separation between editor and inference

### Recommendation

1. Define the daemon-compatible protocol immediately
   ([daemon-protocol.md](./daemon-protocol.md)).
2. Permit an in-process implementation during the earliest prototype
   (roadmap M1–M3).
3. Move model ownership into latticed before shipping global Quick Note
   (roadmap M4 before M5).

## Provider abstraction

Shared Rust interface (illustrative; serialization library not locked):

```rust
pub trait SpeechProvider: Send + Sync {
    fn capabilities(&self) -> SpeechCapabilities;

    async fn prepare(
        &self,
        request: PrepareModelRequest,
    ) -> Result<ModelStatus, SpeechError>;

    async fn start_session(
        &self,
        config: SpeechSessionConfig,
        events: SpeechEventSender,
    ) -> Result<Box<dyn SpeechSession>, SpeechError>;
}

pub trait SpeechSession: Send {
    async fn push_audio(
        &mut self,
        chunk: AudioChunk,
    ) -> Result<(), SpeechError>;

    async fn finish_utterance(
        &mut self,
    ) -> Result<FinalTranscript, SpeechError>;

    async fn cancel(
        self: Box<Self>,
    ) -> Result<(), SpeechError>;
}
```

Capabilities are independent of providers:

```rust
pub struct SpeechCapabilities {
    pub streaming: bool,
    pub partial_transcripts: bool,
    pub offline_final_decode: bool,
    pub punctuation: bool,
    pub word_timestamps: bool,
    pub language_detection: bool,
    pub vocabulary_biasing: bool,
    pub endpoint_detection: bool,
    pub supported_languages: Vec<LanguageTag>,
}
```

The initial macOS provider is FluidAudio `0.15.5` with the M0-measured
**EOU streaming + TDT v2 offline** pair
([adr/0002](./adr/0002-fluid-audio-macos-provider.md),
[RESULTS.md](../../research/voice-m0-fluidaudio/RESULTS.md)). Production may
instead pin upstream **Unified** (`parakeet-unified-en-0.6b-coreml`); that path
was not benchmarked in M0.

Shared protocol types and the in-process `SpeechProvider` foundation live in
`crates/lattice-voice`. Linux and Windows providers are future
`SpeechProvider` implementations and are out of scope for the first release.

## Interfaces

| Boundary | Contract |
|----------|----------|
| Capture → inference | Canonical PCM chunks ([audio-capture.md](./audio-capture.md)) |
| Rust ↔ Swift | C ABI ([fluid-audio-bridge.md](./fluid-audio-bridge.md)) |
| Client ↔ latticed | Versioned local IPC ([daemon-protocol.md](./daemon-protocol.md)) |
| Inference → editor | Provisional events + final transcript ([transcription-pipeline.md](./transcription-pipeline.md)) |
| Final text → storage | Editor transaction + semantic command core |

## Security implications

- Only trusted local clients may push audio ([privacy-security.md](./privacy-security.md)).
- Voice commands must not bypass plugin or capability permissions
  ([voice-commands.md](./voice-commands.md)).
- Model artifacts are hash-verified ([model-management.md](./model-management.md)).

## Testing requirements

Architecture tests should verify:

- Provider capability negotiation
- Session ownership across process restart
- That provisional paths cannot call persistence APIs
- That final insert goes through the command/transaction path

## Open questions

- Exact timing for moving model ownership into latticed (research Q13)
- Whether a separate login-item helper is required for Quick Note (research Q14)

## Acceptance criteria

- [ ] Component ownership table is uncontested by implementers
- [ ] In-process and daemon modes share one protocol schema
- [ ] `SpeechProvider` / `SpeechSession` traits compile in a Rust crate skeleton
- [ ] No document requires the WebView to load Core ML models
