# Voice Implementation Review and Local ML Architecture

> Repository snapshot reviewed: `willmortimer/lattice` on `main`, through commit
> `ab5da941c27bd3594c2cec6a0ddd00e7e165e22b` on 2026-07-19.

## Executive assessment

The current voice implementation has a good domain boundary but an
underpowered capture and finalization pipeline.

The provider abstraction, provisional editor decoration, C ABI, model pinning,
and live fixture test are all worth retaining. Poor recognition is not evidence
that local Core ML ASR is inherently inadequate. The current result is explained
by a combination of:

1. The selected model's known weaknesses on technical formatting and paths.
2. Browser/WebView audio capture and low-quality resampling.
3. JSON-array audio transport and repeated per-chunk allocations.
4. No pre-roll, VAD, endpoint policy, or real gap detection.
5. No workspace glossary, contextual biasing, or deterministic technical-text
   normalization.
6. A capability contract that says "offline final decode" even though the
   production path currently finalizes the same streaming checkpoint.
7. Evaluation based mostly on a synthetic `say` fixture rather than a real
   microphone corpus.

The immediate goal should not be "replace FluidAudio." It should be:

> Build a correct native capture and evaluation pipeline, then compare a
> streaming model plus a genuinely higher-accuracy final model.

## Current implementation

### Shared Rust provider contract

`crates/lattice-voice` defines provider-neutral speech concepts.
`crates/lattice-voice-macos` implements the macOS provider through a stable C
ABI into Swift and FluidAudio.

This is the right seam. It keeps Swift and Core ML details out of shared Rust
code.

### Tauri-owned session state

`apps/desktop/src-tauri/src/voice.rs` currently owns:

- Provider preparation.
- One active session.
- Partial and final event forwarding.
- Audio chunk sequence assignment.
- Finalization and cancellation.
- Tauri event emission.

The provider is prepared with:

```text
parakeet-unified-320ms
```

The current session context sets:

```rust
SessionContext {
    document_id: None,
    glossary_terms: Vec::new(),
    command_mode: false,
}
```

That means the existing architecture already has a place for contextual terms,
but the application never supplies them.

### WebView capture

`apps/desktop/src/lib/voice.ts` currently:

- Requests the microphone through `getUserMedia`.
- Uses a deprecated `ScriptProcessorNode`.
- Mixes channels in TypeScript.
- Downsamples with a simple sample-bucket average.
- Stores samples in a JavaScript `number[]`.
- Converts each 160 ms `Float32Array` to a normal array.
- Sends every chunk through a Tauri command.
- Enables browser automatic gain control.
- Disables browser echo cancellation and noise suppression.

### Swift inference

`VoiceEngine.swift` loads:

```text
StreamingUnifiedAsrManager
parakeet-unified-320ms
int8 encoder
```

`VoiceSession.swift` allocates an `AVAudioPCMBuffer` for every incoming chunk,
then calls:

```swift
try await manager.appendAudio(buffer)
try await manager.processBufferedAudio()
```

Finalization calls:

```swift
let text = try await manager.finish()
```

The same loaded streaming checkpoint generates partials and the authoritative
final.

### Editor provisional state

`apps/desktop/src/editor/DictationProvisional.ts` uses a ProseMirror decoration
for provisional text. It does not enter canonical document state, undo,
autosave, or collaboration history.

This is correct and should remain unchanged.

## What the repository's own measurements show

The live result in
`crates/lattice-voice-macos/tests/LIVE_RESULTS.md` produced:

```text
Lattice voice dictation should preserve camelcase identifiers like ASR Manager,
File Paths such as users will developer lattice, and punctuation around code
```

The reference included:

```text
CamelCase
AsrManager
/Users/will/Developer/lattice
punctuation
```

The repository's `research/voice-m0-fluidaudio/RESULTS-unified.md` explicitly
records that:

- True CamelCase was not preserved.
- `AsrManager` became `ASR Manager`.
- The absolute path collapsed into ordinary words.
- Terminal punctuation disappeared.
- The optional Unified offline encoder did not materially fix those examples.
- The selected production final is the streaming manager's `finish()`, not the
  separate offline encoder.

The test therefore already demonstrates a model-domain mismatch for technical
dictation. The implementation should not hide that behind generic "ASR
quality" language.

## Likely causes of poor results

## 1. The selected checkpoint is optimized for streaming, not technical text fidelity

The current model is valuable because it produces fast partials from one
roughly 608 MB model family. It is not proven to be the best final dictation
model.

Paths, case-sensitive identifiers, punctuation, and source-code tokens are
hard for general ASR because the acoustic signal does not uniquely encode:

- Slash versus the spoken word "slash."
- `ASRManager` versus `ASR Manager`.
- Lowercase versus uppercase.
- Hyphens, underscores, dots, and file extensions.
- Whether a phrase is prose, code, a symbol, or a path.

A language model, contextual vocabulary, or deterministic normalization layer
must supply that missing structure.

FluidAudio's current documentation distinguishes:

- Streaming ASR models for partials and low latency.
- English Parakeet TDT v2 for higher English recall.
- Multilingual TDT v3.
- Custom-vocabulary/keyword-spotting models.
- Inverse text normalization.

References:

- <https://github.com/FluidInference/FluidAudio>
- <https://github.com/FluidInference/FluidAudio/blob/main/Documentation/ASR/GettingStarted.md>
- <https://github.com/FluidInference/FluidAudio/blob/main/Documentation/Models.md>
- <https://github.com/FluidInference/FluidAudio/blob/main/Documentation/API.md>

The final model should be selected by measured dictation quality, not by the
convenience of sharing one checkpoint.

## 2. The "offline final decode" capability is currently misleading

`FluidAudioSpeechProvider::capabilities_inner()` reports:

```rust
offline_final_decode: true
```

But `VoiceEngine.swift` explicitly says that the optional offline Unified
encoder is not loaded, and `VoiceSession.finishUtterance()` calls
`StreamingUnifiedAsrManager.finish()`.

This is an authoritative final, but it is not an independent offline re-decode
of buffered utterance audio.

Fix the contract immediately:

```rust
pub struct SpeechCapabilities {
    pub streaming: bool,
    pub partial_transcripts: bool,
    pub finalization_mode: FinalizationMode,
    pub punctuation: bool,
    pub word_timestamps: bool,
    pub language_detection: bool,
    pub vocabulary_biasing: bool,
    pub endpoint_detection: bool,
    pub supported_languages: Vec<String>,
}

pub enum FinalizationMode {
    StreamingFlush,
    SameFamilyOfflineRedecode,
    IndependentOfflineRedecode,
}
```

Report the current provider as `StreamingFlush`. Only report
`IndependentOfflineRedecode` when the complete utterance is actually decoded
by a separate final model.

## 3. The TypeScript resampler is not production-grade

The current downsampler averages source samples that fall into each destination
bucket. This is a crude box filter.

Problems:

- It is not a well-specified anti-aliasing filter.
- Behavior varies between 44.1 kHz and 48 kHz input.
- It allocates new arrays for every callback.
- It runs in the WebView audio callback path.
- There are no frequency-response, impulse, or fixture tests.
- It does not record the actual device format and conversion path for
  diagnostics.

For 48 kHz to 16 kHz, averaging groups of three may appear acceptable in clean
speech. It is still not equivalent to a proper sample-rate converter. For
44.1 kHz input, the varying bucket width can introduce more inconsistent
artifacts.

The best Mac-first fix is native capture through `AVAudioEngine` and
`AVAudioConverter`, not a more elaborate JavaScript resampler.

## 4. `ScriptProcessorNode` is the wrong capture primitive

`ScriptProcessorNode` is deprecated and executes callbacks on the main
JavaScript event loop. It is susceptible to:

- UI stalls.
- Garbage-collection pauses.
- Timing jitter.
- Unbounded callback work.
- Lost or delayed chunks under renderer load.

An `AudioWorklet` would be better than `ScriptProcessorNode`, but for a
first-class macOS experience, native CoreAudio/AVFoundation capture is better
than either.

"Client-owned capture" should mean the trusted native desktop client owns the
microphone—not necessarily that the WebView owns it.

## 5. Automatic gain control is enabled without quality evidence

The current constraints set:

```text
autoGainControl: true
noiseSuppression: false
echoCancellation: false
```

Automatic gain control can help a quiet microphone, but it can also pump room
noise, clip transients, and change the signal distribution expected by the ASR
model. The voice documentation already says gain normalization must be
benchmarked rather than assumed beneficial.

Start with native unprocessed microphone capture. Add optional conservative
gain normalization only after A/B evaluation.

## 6. There is no pre-roll

The architecture document calls for approximately 250–500 ms of pre-roll, but
the implementation clears its buffer when the session starts and captures only
after permission, session creation, and media setup.

This can clip the first phoneme or word, especially in hold-to-talk usage.

Implement a continuously maintained native ring buffer while dictation is
armed. On activation, prepend approximately 300 ms and mark the actual
activation timestamp.

## 7. Audio timing metadata is discarded

Rust creates each `AudioChunk` with:

```text
captured_at_ns: 0
```

The sequence number exists, but there is no meaningful timestamp or duration
validation.

Without timing, diagnostics cannot distinguish:

- Slow inference.
- Delayed capture callback.
- IPC congestion.
- Dropped audio.
- Duplicate audio.
- A pause in speech.
- A pause caused by the WebView.

Populate monotonic capture timestamps and validate expected frame counts.

## 8. Backpressure is specified but not implemented end to end

The documentation requires bounded queues and visible gap events. The current
frontend uses:

```text
pending: number[]
pushing: boolean
```

Problems:

- A JavaScript `number[]` is much larger than a packed `Float32Array`.
- `splice(0, n)` repeatedly shifts array contents.
- Every chunk is copied into several representations.
- There is no maximum queued duration.
- There is no dropped-frame event.
- There is no gap detection.
- Stop/finalize does not await an explicit drain promise representing all
  previously scheduled chunk sends.
- The service assigns sequences after receiving chunks rather than validating
  client capture sequence and timestamps.

Use a fixed-capacity ring buffer and explicit producer/consumer state. If the
queue exceeds a bounded duration, fail visibly rather than silently growing or
dropping speech.

## 9. JSON-shaped Tauri audio transport wastes work

Each 160 ms chunk currently follows this path:

```text
Float32Array
  -> Array.from()
  -> Tauri JSON serialization
  -> Vec<f32>
  -> Vec<u8> little-endian bytes
  -> Rust decode back into Vec<f32>
  -> Swift [Float]
  -> AVAudioPCMBuffer allocation and copy
```

At 16 kHz mono Float32, raw audio is only about 64 KiB/s. The bandwidth is
trivial. The repeated representation conversion is the problem.

Use a persistent binary IPC stream:

```text
framed header + packed Float32 bytes
```

A Unix-domain socket is sufficient. Shared memory is optional and should be
introduced only if measurements show the binary socket is inadequate.

## 10. The provider allocates and schedules too much per chunk

Every Rust `push_audio` call:

- Converts packed bytes back to floats.
- Uses `spawn_blocking`.
- Crosses the C ABI.
- Allocates a Swift PCM buffer.
- Copies samples.
- Calls the streaming manager.

At 160 ms intervals, this is not catastrophic, but it is unnecessary overhead
and creates more scheduling variability than a persistent audio consumer.

Use:

- Native 20 ms capture frames.
- A lock-free or bounded ring buffer.
- 320 ms inference blocks for the current 320 ms model tier.
- A dedicated inference task that drains the ring.
- Reused audio buffers where FluidAudio permits.
- Fewer process and executor transitions.

## 11. No endpoint detection or utterance segmentation is active

~~The current provider reports `endpoint_detection: false`.~~

**Status (v13):** Endpoint policy is wired. `SpeechCapabilities.endpoint_detection`
is `true` for the FluidAudio / null providers. Lattice owns energy VAD + silence
debounce + max utterance length (Unified has no FluidAudio `setEouCallback`).
Continuous mode auto-finalizes via `EndpointOptions.auto_finalize_on_endpoint`
or `LATTICE_VOICE_AUTO_FINALIZE_ON_ENDPOINT=1`. Hold-to-talk still finalizes via
explicit `FinishUtterance` without requiring VAD onset.

Suggested knobs (defaults):

- Speech start.
- Speech end.
- Silence debounce (default 800 ms domain / 1280 ms Swift bridge to match EOU).
- Maximum utterance length (default 45 s).
- Interruption.
- Cancellation.
- Optional automatic finalization.

Evaluate FluidAudio's EOU model or a separate VAD. Keep VAD decisions distinct
from transcript content.

## 12. Workspace context is unused

The session contract contains `glossary_terms`, but the Tauri wrapper always
passes an empty list.

This is a major missed opportunity. Lattice already knows:

- Current document title.
- Heading ancestry.
- Workspace name.
- Linked pages.
- Tags.
- Project names.
- File and directory names.
- Code symbols.
- Recent commands.
- User-defined vocabulary.

These terms can improve technical transcription through contextual biasing,
keyword rescoring, or deterministic post-processing.

## 13. There is no technical-text normalization pipeline

The raw ASR output is inserted as final text. There is no explicit stage for:

- Inverse text normalization.
- Punctuation restoration.
- Path reconstruction.
- Identifier casing.
- Known symbol replacement.
- Spoken command parsing.
- Confidence-aware review.

A local deterministic normalizer should run after final ASR and before editor
commit.

Example:

```text
raw ASR:
users will developer lattice slash crates slash lattice voice

workspace-aware normalized candidate:
/Users/will/Developer/lattice/crates/lattice-voice
```

Do not silently make an aggressive replacement merely because a path is
semantically similar. Require strong evidence from local known paths, spoken
markers such as "slash," or user review.

## 14. The evaluation corpus is too narrow

The current fixture is useful as a smoke test, but it is generated speech and
does not represent:

- The user's microphone.
- Room noise.
- Laptop fan noise.
- Different speaking rates.
- Hesitation and correction.
- Long dictation.
- Technical vocabulary.
- Commands.
- Near-field versus far-field capture.
- Bluetooth microphones.
- Clipped first words.
- Mixed prose and code.

Build a small private local corpus. Store audio outside Git when necessary and
commit only manifests, reference transcripts, hashes, and aggregate results.

## Recommended Mac-first pipeline

```text
AVAudioEngine input tap
        |
        v
AVAudioConverter
device format -> 16 kHz mono Float32
        |
        v
native pre-roll + bounded ring buffer
        |
        +----------------------------+
        |                            |
        v                            v
streaming partial ASR          full utterance buffer
        |                            |
        v                            v
provisional decoration        high-quality final ASR
                                     |
                                     v
                            ITN and punctuation
                                     |
                                     v
                   workspace-aware deterministic correction
                                     |
                                     v
                          editor semantic transaction
```

## Native capture implementation

Create a macOS capture component separate from the ASR provider:

```text
crates/lattice-audio/
    provider-neutral audio types and ring-buffer contracts

crates/lattice-audio-macos/
    Rust-facing C ABI

crates/lattice-audio-macos/swift/
    AVAudioEngine, AVAudioConverter, device selection, permission
```

The native component should:

- Request microphone permission through the app.
- Select and report the active input device.
- Install an `AVAudioEngine` input tap.
- Convert with `AVAudioConverter`.
- Produce packed 16 kHz mono Float32.
- Maintain a bounded pre-roll.
- Record monotonic timestamps.
- Compute clipping and RMS diagnostics locally.
- Never persist audio unless an explicit debug recording is enabled.
- Stream binary frames to the local voice host.

Keep the WebView responsible for presentation and session intent, not PCM
processing.

## Streaming and final model strategy

### Provisional path

Retain a low-latency streaming model for provisional text.

Candidates must be measured on the user's Mac and corpus:

- Current Unified 320 ms model.
- FluidAudio Parakeet EOU at 320 ms.
- FluidAudio Nemotron streaming tiers if their quality and footprint are
  acceptable.

Do not optimize provisional text for perfect punctuation. Optimize it for:

- Fast first partial.
- Stable corrections.
- Low distraction.
- Low energy use.
- Adequate rough content.

### Final path

Use a genuinely independent final decode for committed text.

The first serious comparison should include:

1. Parakeet TDT v2 English.
2. Parakeet TDT v3 if multilingual support is required.
3. Unified offline encoder.
4. Current streaming `finish()` as the baseline.

FluidAudio's current documentation describes TDT v2 as the English-focused
higher-recall model. For a Mac-first English release, that is the most
important candidate.

Buffer the complete utterance in memory, then decode it once on release or
endpoint. The final model can be:

- Loaded during voice preparation.
- Loaded lazily after the first session begins.
- Unloaded under memory pressure.
- Kept warm while dictation is active.

A second roughly 0.6B model is a real memory cost. It is still reasonable on a
16 GB Apple Silicon Mac if Lattice unloads unrelated embedding or generation
models and measures pressure. Model scheduling belongs in `latticed`.

## Workspace-aware vocabulary and embeddings

Embeddings do not directly transcribe audio. They help select the context that
the ASR and normalizer should consider.

### Context-building flow

```text
current editor location
        |
        v
local FTS + embedding retrieval
        |
        v
bounded candidate terms
        |
        +--> ASR contextual biasing / keyword rescoring
        |
        +--> deterministic transcript normalizer
```

Build a `VoiceContextBuilder` in the Rust daemon:

```rust
pub struct VoiceContext {
    pub document_id: Option<String>,
    pub heading_path: Vec<String>,
    pub glossary_terms: Vec<GlossaryTerm>,
    pub known_paths: Vec<KnownPath>,
    pub known_symbols: Vec<KnownSymbol>,
    pub command_phrases: Vec<String>,
    pub context_revision: String,
}
```

Sources:

- Current document and nearby blocks.
- Explicit page glossary.
- Workspace manifest vocabulary.
- FTS title/path matches.
- Embedding-related notes.
- Linked pages.
- Recently used paths and symbols.
- Open project roots.
- Slash-command registry.

Bound this aggressively:

- Perhaps 50–200 highest-value terms.
- Prefer exact local names over generic semantic neighbors.
- Include provenance for every term.
- Never send workspace context to a remote service in the local mode.

### Corrections

Use tiers:

1. **Exact deterministic rules**
   - Spoken punctuation.
   - Known command phrases.
   - Exact known paths.
2. **Contextual candidate scoring**
   - ASR token similarity.
   - FTS match.
   - Embedding relevance to current document.
   - Recent use.
3. **User review**
   - Ambiguous identifier or path replacement.
4. **Optional local language-model rewrite**
   - Later, disabled by default, with original transcript retained.

Embeddings help choose the candidate set. They should not be treated as proof
that a candidate is correct.

## Shared ML management

Voice and embeddings should share model-management infrastructure but not
necessarily a process.

`latticed` should own:

- Model manifests.
- Download and hash verification.
- License and attribution records.
- Model status.
- Memory and thermal policy.
- Job scheduling.
- Provenance.
- Privacy policy.
- Inference-host supervision.

Use separate inference hosts initially:

```text
latticed
  ├── lattice-voice-host
  │     └── Swift + FluidAudio + Core ML
  └── lattice-embed-host
        └── Rust/C++ + llama.cpp + Metal
```

Reasons:

- Voice is a continuous low-latency stream.
- Embedding is a batch/query workload.
- Swift/Core ML and C++/Metal failures stay isolated.
- Either host can be unloaded independently.
- A voice crash cannot corrupt the workspace or search index.
- Core ML embeddings can later replace the embedding host backend without
  changing daemon APIs.

Do not create two independent model download systems. Both hosts consume
verified artifacts and manifests owned by the daemon.

## Transcript provenance

Every final transcript should carry:

```rust
pub struct TranscriptProvenance {
    pub session_id: String,
    pub audio_format: AudioFormat,
    pub capture_device_id: String,
    pub capture_started_at_ns: u64,
    pub captured_frames: u64,
    pub dropped_frames: u64,
    pub streaming_model: ModelProvenance,
    pub final_model: ModelProvenance,
    pub normalizer_version: String,
    pub context_revision: String,
    pub glossary_term_ids: Vec<String>,
    pub raw_transcript_hash: String,
    pub normalized_transcript_hash: String,
}
```

Normal app operation need not retain raw audio. The editor operation can retain
the text provenance and model versions without retaining the recording.

## Immediate fixes before daemon migration

1. Change finalization capability reporting from a boolean to an explicit mode.
2. Add native microphone capture and `AVAudioConverter`.
3. Disable automatic gain control by default.
4. Add pre-roll and monotonic capture timestamps.
5. Replace JavaScript `number[]` buffering.
6. Add a bounded queue and explicit drain on finish.
7. Populate client sequence numbers and reject gaps.
8. Batch according to the model's 320 ms tier.
9. Add a true full-utterance audio buffer.
10. Implement a final-model comparison harness.
11. Pass the current document and a bounded glossary into session creation.
12. Add deterministic ITN, punctuation, path, and identifier normalization.
13. Add real-microphone fixtures and WER/CER reporting.

## Evaluation plan

Create:

```text
research/voice-eval/
├── manifest.yaml
├── references/
├── scripts/
├── RESULTS.md
└── private-audio/   # gitignored
```

Test categories:

- Ordinary prose.
- Technical prose.
- CamelCase and snake_case.
- Absolute and relative paths.
- URLs and email-like strings.
- Rust, TypeScript, SQL, and shell terms.
- Slash commands.
- Punctuation words.
- Long dictation.
- Corrections and restarts.
- Background noise.
- Built-in Mac microphone.
- AirPods or Bluetooth microphone.
- First-word clipping.
- Silence endpoint behavior.

Metrics:

- Word error rate.
- Character error rate.
- Technical-token accuracy.
- Path accuracy.
- Identifier accuracy.
- Punctuation F1.
- First partial latency.
- Stable partial latency.
- Finalization latency.
- Dropped audio.
- Peak memory.
- Energy impact.
- User correction rate.

Keep raw and normalized scores separate. Otherwise a strong normalizer can hide
a weak acoustic model.

## Staged implementation

### Voice V1: correct capture

- Native Swift capture.
- Proper sample-rate conversion.
- Pre-roll.
- Binary transport.
- Bounded ring.
- Timestamps and sequence validation.
- Existing Unified streaming provider.
- No change to editor provisional behavior.

### Voice V1.1: real final decode

- Buffer the utterance.
- Benchmark TDT v2, TDT v3, Unified offline, and streaming flush.
- Select the best English final provider.
- Report the true finalization mode.
- Add model memory policy.

### Voice V1.2: local context

- Implement `VoiceContextBuilder`.
- Feed document, headings, paths, symbols, and explicit vocabulary.
- Use FTS first, then semantic retrieval for context expansion.
- Add ITN and deterministic normalization.
- Preserve raw and normalized transcript provenance.

### Voice V1.3: continuous dictation

- Add VAD/EOU.
- Add endpoint policy.
- Add interruption and resume.
- Add command mode through the shared command registry.
- Add Quick Note background preparation through `latticed`.

## Definition of done

- Real microphone audio never passes through JSON arrays.
- Capture and resampling do not execute in the WebView.
- First words are not clipped.
- Queue growth is bounded and gaps are visible.
- Final committed text comes from a declared finalization mode.
- Technical-token quality is measured separately from generic WER.
- Workspace context remains local.
- Every contextual correction is attributable to a local source or rule.
- Provisional text never enters canonical document state.
- Model crashes do not crash or corrupt the workspace process.
