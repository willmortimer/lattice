# Implementation Roadmap

## Scope

Milestones converting this documentation package into implementation work.
Each milestone lists deliverables and an exit criterion suitable for GitHub
issues or a project DAG.

M0 findings:
[research/voice-m0-fluidaudio/RESULTS.md](../../research/voice-m0-fluidaudio/RESULTS.md).
Unified production decision:
[research/voice-m0-fluidaudio/DECISION.md](../../research/voice-m0-fluidaudio/DECISION.md),
[research/voice-m0-fluidaudio/RESULTS-unified.md](../../research/voice-m0-fluidaudio/RESULTS-unified.md).
Protocol foundation: `crates/lattice-voice`.
macOS bridge: `crates/lattice-voice-macos`.

## Research questions (M0 status)

| # | Question | M0 status |
|---|----------|-----------|
| 1 | Exact Parakeet artifact + FluidAudio pin | **Resolved** — FluidAudio `0.15.5` / `19600a4…`; production **Unified** `parakeet-unified-320ms`. EOU+TDT was M0 spike only. |
| 2 | Stable partial tokens for provisional UI? | **Resolved** — Unified: 37 partials (Task U); warm first partial **158.3 ms** (M0 EOU was **~405 ms**). |
| 3 | Same model for streaming + offline? | **Resolved** — Unified streaming checkpoint serves stream + `finish()` final (~608 MB one family). EOU+TDT (~890 MB) is non-production. |
| 4 | Memory cost of both decoder states | **Open** — not instrumented; sequential load OK. |
| 5 | First-partial latency on oldest M-series | **Partial** — **~405 ms warm** / **~5 s cold-first-inference** on M2; oldest-target pass still open. |
| 6 | Offline improves technical dictation? | **Resolved (negative for strict quality)** — better casing/punctuation; CamelCase/paths still lost; no Apple baseline. |
| 7 | Callback scheduling vs Swift/Rust | **Resolved** — partials on background threads; hop required for Rust. |
| 8 | Core ML compile reuse across updates | **Resolved for cache reuse** — warm **~681 ms / ~399 ms** vs cold **~98–110 s**; cross-app-update policy TBD. |
| 9 | Endpoint-detection exposure | **Resolved** — `setEouCallback`, `eouDebounceMs` (1280 ms default); EOU not triggered without silence pad on fixture. |
| 10 | Separate VAD vs Parakeet segmentation | **Open** — not measured. |
| 11 | Pre-roll duration | **Open** — product tuning. |
| 12 | Final vs provisional divergence UX | **Open** — product/UX. |
| 13 | Model ownership timing (latticed) | **Open** — docs recommend protocol now; latticed before Quick Note. |
| 14 | Quick Note without helper | **Open** — background reliability. |
| 15 | Attributions required | **Resolved** — Apache-2.0 + NVIDIA Open Model + CC-BY-4.0 for M0 pins. |

**Still open for production:**

- **Apple dictation baseline** comparison for technical prose (M0 exit criterion interpreted strictly)
- **Glossary / vocabulary biasing** for technical tokens (CamelCase, paths)
- Oldest M-series latency pass; optional Unified offline encoder residency (Q4, Q5)

These are mirrored in
[docs/31-open-questions-and-decision-register.md](../31-open-questions-and-decision-register.md).

---

## Milestone 0: research spike

**Status:** **Complete** (2026-07-18). See
[research/voice-m0-fluidaudio/RESULTS.md](../../research/voice-m0-fluidaudio/RESULTS.md).

**Delivered:**

- Minimal Swift executable using FluidAudio `0.15.5`
- Streaming transcription via `parakeet-realtime-eou-120m-coreml` (160 ms)
- Offline re-decode via `parakeet-tdt-0.6b-v2-coreml`
- Measured model load, first partial, finalization, and offline decode latencies
- Confirmed model and runtime licenses
- Provisional minimum macOS **14** (upstream); measured on macOS 26.5 / M2

**Exit criterion:** The selected model runs locally on the target Mac and
demonstrates a meaningful quality or workflow advantage over Apple system
dictation for technical prose.

**Verdict:** **Unblocks M1** — APIs, pins, Float32 sample format, and callback
threading are known. Strict “beats Apple dictation on technical prose” remains
**open** without a baseline; local streaming partials (~405 ms warm first
partial) demonstrate workflow advantage.

## Milestone 1: Rust and Swift bridge

**Status:** **Complete** (2026-07-18). Bridge landed in `crates/lattice-voice-macos`
with C ABI v1, `FluidAudioSpeechProvider`, and Unified (`parakeet-unified-320ms`)
wiring.

**Delivered:**

- Stable C ABI (`LATTICE_VOICE_BRIDGE_ABI_VERSION = 1`)
- Rust wrapper crate with `FluidAudioSpeechProvider`
- Engine/session lifecycle; Float32 audio chunk input
- Partial transcript callbacks (background-thread hop in Rust)
- Authoritative final via `finish_utterance()` (Unified `finish()`)
- Error translation; memory-safety and ABI-version tests
- Live ASR integration test on M0 fixture
  ([crates/lattice-voice-macos/tests/LIVE_RESULTS.md](../../crates/lattice-voice-macos/tests/LIVE_RESULTS.md))

**Exit criterion:** A Rust integration test can transcribe fixture audio
through both streaming and authoritative final paths.

**Verdict:** **Met** — warm-cache live run: **36 partials**, final in **~1.5 s**
([LIVE_RESULTS.md](../../crates/lattice-voice-macos/tests/LIVE_RESULTS.md)).
**No GUI** — microphone capture, provisional overlay, and model setup UI remain
Milestone 2.

## Milestone 2: in-process Tauri prototype

**Status:** **Landed** (mic + PTT + provisional overlay + final insert + setup UI).
**Supersession:** Production capture moves to native client PCM
([ADR 0008](adr/0008-native-client-capture.md)); model ownership moves to
`latticed` ([ADR 0043](../decisions/0043-voice-ownership-in-latticed.md)).
See sprint DAG [docs/dev/voice-d5-quick-note-dag.md](../dev/voice-d5-quick-note-dag.md).

**Deliver:** Microphone permission; audio capture; push-to-talk toolbar;
provisional text overlay; final transcript insertion; cancel; model setup UI.

**Exit criterion:** A user can dictate into one document without unstable text
entering document storage.

**Notes:** Desktop build with `--features voice` (default `pnpm tauri:dev`).
M2 capture was WebView `getUserMedia` → Float32 @ 16 kHz →
`FluidAudioSpeechProvider`. Provisional text uses editor decorations only.

## Milestone 3: editor semantics

**Status:** **In progress** (voice-d5 sprint).

**Deliver:** Logical dictation anchors; one-transaction final insertion; undo;
new paragraph/line controls; cursor-movement policy; error recovery;
document-switch handling.

**Exit criterion:** Dictation behaves predictably under ordinary editing
operations.

## Milestone 4: latticed service

**Status:** **In progress** (voice-d5 sprint / Phase D5).

**Deliver:** Local IPC; versioned protocol; model manager; shared model
residency; client authentication; session ownership; daemon restart recovery;
capability negotiation; `lattice-voice-host` supervision.

**Exit criterion:** The Tauri app no longer owns model state directly
([ADR 0043](../decisions/0043-voice-ownership-in-latticed.md)).

## Milestone 5: Quick Note

**Status:** **Partial** — text Quick Note + global shortcut landed; voice
dictation blocked on M4 exit.

**Deliver:** Global shortcut; pre-roll; background model residency; minimal
overlay; configurable destination; atomic note creation; failure recovery
queue.

**Exit criterion:** A user can create a reliable note from anywhere on macOS
with one shortcut and no cloud dependency.

## Milestone 6: voice formatting

**Deliver:** Deterministic control grammar; new line/paragraph; lists;
headings; checkboxes; undo; delete last phrase; explicit command mode.

**Exit criterion:** Formatting commands work without accidentally interpreting
ordinary prose as commands.

## Milestone 7: slash-command integration

**Deliver:** Voice aliases in command registry; argument parsing; risk
classification; confirmation UI; plugin capability restrictions; execution
through the shared command bus.

**Exit criterion:** A safe subset of existing slash commands can be invoked by
voice.

## Milestone 8: hardening and release

**Deliver:** Performance benchmark suite; golden audio corpus; permission-state
testing; crash recovery; memory-pressure behavior; model attribution; packaged
notices; user documentation; accessibility review.

**Exit criterion:** The system meets documented latency, persistence, privacy,
and command-safety requirements.

---

## Suggested issue DAG

```text
M0 ──► M1 ──► M2 ──► M3
                │
                └──► M4 ──► M5
                       │
                       └──► M6 ──► M7 ──► M8
```

M4 may start in parallel with late M3 once the protocol schema is stable.
M5 **must not** ship before M4 exit.
