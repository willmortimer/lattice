# Implementation Roadmap

## Scope

Milestones converting this documentation package into implementation work.
Each milestone lists deliverables and an exit criterion suitable for GitHub
issues or a project DAG.

## Immediate research questions (before / during M0)

Resolve these before broad implementation:

1. Which exact Parakeet Unified Core ML artifact and FluidAudio release will be pinned?
2. Does the selected model expose sufficiently stable partial-token information for clean provisional rendering?
3. Can the same loaded model efficiently support simultaneous streaming and offline final decoding?
4. What is the memory cost of retaining both streaming and offline decoder state?
5. What is the first-partial latency on the oldest supported M-series Mac?
6. Does offline re-decoding materially improve technical dictation over the final streaming hypothesis?
7. How should FluidAudio callbacks be scheduled relative to Swift concurrency and Rust executors?
8. Can Core ML compilation be performed during onboarding and reused across application updates?
9. What endpoint-detection behavior does FluidAudio expose directly?
10. Does a separate VAD improve or interfere with Parakeet’s own segmentation?
11. How much pre-roll is needed to avoid clipped speech without adding irrelevant audio?
12. What behavior should occur when the final transcript is significantly different from the provisional transcript?
13. Should model ownership move into latticed before or after the first editor prototype? (**Docs recommend:** protocol now; in-process OK for editor prototype; latticed before Quick Note.)
14. Can the main Tauri application provide sufficiently reliable background Quick Note behavior without a helper?
15. What exact attributions and notices are required for the chosen converted model artifact?

These are mirrored in
[docs/31-open-questions-and-decision-register.md](../31-open-questions-and-decision-register.md).

---

## Milestone 0: research spike

**Deliver:**

- Minimal Swift executable using FluidAudio
- Successful Parakeet Unified streaming transcription
- Successful offline re-decoding
- Measured model load time
- Measured first partial latency
- Measured finalization latency
- Confirmed model and runtime licenses
- Decision on minimum macOS version

**Exit criterion:** The selected model runs locally on the target Mac and
demonstrates a meaningful quality or workflow advantage over Apple system
dictation for technical prose.

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
