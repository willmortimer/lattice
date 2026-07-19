# Implementation Roadmap

## Scope

Milestones converting this documentation package into implementation work.
Each milestone lists deliverables and an exit criterion suitable for GitHub
issues or a project DAG.

M0 findings:
[research/voice-m0-fluidaudio/RESULTS.md](../../research/voice-m0-fluidaudio/RESULTS.md).
Protocol foundation: `crates/lattice-voice`.

## Research questions (M0 status)

| # | Question | M0 status |
|---|----------|-----------|
| 1 | Exact Parakeet artifact + FluidAudio pin | **Resolved** — FluidAudio `0.15.5` / `19600a4…`; streaming EOU 120M 160ms; offline TDT v2. Unified exists separately. |
| 2 | Stable partial tokens for provisional UI? | **Resolved** — 39 monotonic partials; warm first partial **~405 ms** (M2). |
| 3 | Same model for streaming + offline? | **Resolved for spike** — two model families (~890 MB cache). Unified claims one checkpoint; **not measured**. |
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

- **Unified vs EOU+TDT** production model pin
- **Apple dictation baseline** comparison for technical prose (M0 exit criterion interpreted strictly)
- **Glossary / vocabulary biasing** for technical tokens (CamelCase, paths)
- Oldest M-series latency pass; dual-model residency memory (Q4, Q5)

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

**Deliver:** Stable C ABI; Rust wrapper crate; engine/session lifecycle; audio
chunk input; partial transcript callback; offline finalization; error
translation; memory-safety tests.

**Exit criterion:** A Rust integration test can transcribe fixture audio
through both streaming and offline paths.

## Milestone 2: in-process Tauri prototype

**Deliver:** Microphone permission; audio capture; push-to-talk toolbar;
provisional text overlay; final transcript insertion; cancel; model setup UI.

**Exit criterion:** A user can dictate into one document without unstable text
entering document storage.

## Milestone 3: editor semantics

**Deliver:** Logical dictation anchors; one-transaction final insertion; undo;
new paragraph/line controls; cursor-movement policy; error recovery;
document-switch handling.

**Exit criterion:** Dictation behaves predictably under ordinary editing
operations.

## Milestone 4: latticed service

**Deliver:** Local IPC; versioned protocol; model manager; shared model
residency; client authentication; session ownership; daemon restart recovery;
capability negotiation.

**Exit criterion:** The Tauri app no longer owns model state directly.

## Milestone 5: Quick Note

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
