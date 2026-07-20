# RESULTS — final-model comparison (voice-eval)

Filled after a measurement-Mac run. Do **not** flip production
`FinalizationMode` from incomplete rows.

## Environment

| Item | Value |
|------|-------|
| Date | 2026-07-19 |
| Host | MacBook Air (Mac14,15), Apple M2, arm64, 8 cores, 16 GB |
| OS / Xcode | macOS 26.5.1 (Build 25F80) / Xcode 26.5 (17F42) / Swift 6.3.2 |
| FluidAudio pin | `0.15.5` @ `19600a485baa4998812e4654b70d2bab8f2c9949` |
| Fixture set | `m0-technical-dictation` only — **`private-audio/` empty** (no real-mic ids) |
| Model cache | `research/voice-m0-fluidaudio/.cache/Models` — `parakeet-unified-en-0.6b` (streaming + offline encoders, ~1.20 GB). **TDT v2 not present** |
| Command | `python3 scripts/voice_eval.py run --provider streaming_flush` then `--provider unified_offline` (`tdt_v2` skipped — would require HuggingFace download) |

## Scores

Keep raw and normalized columns separate.

### Fixture: `m0-technical-dictation`

| Provider | Mode | Raw WER | Raw CER | Norm WER | Norm CER | Tech token acc | Path acc | Finalize / decode ms |
|----------|------|---------|---------|----------|----------|----------------|----------|----------------------|
| streaming_flush | StreamingFlush | 0.556 | 0.089 | 0.095 | 0.036 | 1.000 (2/2) | 0.000 (0/1) | 1670.6 (stream finish warm) |
| unified_offline | SameFamilyOfflineRedecode | 0.500 | 0.083 | 0.190 | 0.036 | 1.000 (2/2) | 0.000 (0/1) | 108.5 (offline decode warm; load 532.9) |
| tdt_v2 | IndependentOfflineRedecode | — | — | — | — | — | — | **not run** (Parakeet TDT v2 Core ML not in cache; no HF download) |

Warm first-partial (streaming): **139.5 ms**. Cache after streaming+offline load: **~1.20 GB** (`CACHE_AFTER_OFFLINE_BYTES=1204494509`).

### Fixture: private real-mic (id: _none_)

| Provider | Mode | Raw WER | Raw CER | Norm WER | Norm CER | Tech token acc | Path acc | Finalize / decode ms |
|----------|------|---------|---------|----------|----------|----------------|----------|----------------------|
| streaming_flush | StreamingFlush | — | — | — | — | — | — | **skipped** — `private-audio/` empty |
| unified_offline | SameFamilyOfflineRedecode | — | — | — | — | — | — | **skipped** |
| tdt_v2 | IndependentOfflineRedecode | — | — | — | — | — | — | **skipped** |

## Transcripts

| Provider | Hypothesis text |
|----------|-----------------|
| streaming_flush | Lattice voice dictation should preserve camelcase identifiers like ASR Manager, File Paths such as users will developer lattice, and punctuation around code |
| unified_offline | Lattice voice dictation should preserve camel case identifiers like ASR Manager, file paths such as users will developer lattice, and punctuation around code |
| tdt_v2 | _(not measured)_ |

## Memory / energy

| Provider | Approx RSS / cache bytes | Notes |
|----------|--------------------------|-------|
| streaming_flush | ~1.20 GB on-disk Unified family after warm session | Production session model is streaming checkpoint alone (~608 MB streaming encoder + shared decoder/joint); this run’s cache already included the offline encoder from a prior local load |
| unified_offline | same ~1.20 GB folder; offline load **532.9 ms** warm | Second encoder export (`parakeet_unified_encoder_int8.mlmodelc`, ~578 MB) in the **same** HF repo — SameFamilyOfflineRedecode, not IndependentOfflineRedecode |
| tdt_v2 | not loaded | Would be a second model family (~parakeet-tdt-0.6b-v2); intentionally not downloaded |

Peak process RSS / energy not instrumented in this harness pass (disk cache footprint recorded above).

## Acceptance checklist (`IndependentOfflineRedecode`)

See [README.md](./README.md#acceptance-criteria--adopt-independentofflineredecode).

| # | Gate | Pass? | Evidence |
|---|------|-------|----------|
| 1 | Technical-token accuracy vs streaming flush | **fail** | No TDT/`IndependentOfflineRedecode` measurement. Unified offline tech acc **ties** streaming (1.000) — not a strict win; private suite absent |
| 2 | Path accuracy vs streaming flush | **fail** | Streaming and unified offline both **0.000** path acc on the locked fixture; README: when both remain 0, independent decode alone is **not** sufficient. Private-mic missing |
| 3 | Normalized WER within +0.02 on ordinary prose | **fail** | No ordinary-prose / private-mic run. On technical fixture, unified offline norm WER **0.190** vs streaming **0.095** (worse by **0.095** > 0.02). TDT not scored |
| 4 | Warm finalize/decode latency budget | **n/a / incomplete** | Unified offline decode **108.5 ms** is under 750 ms, but candidate for adopt is IndependentOfflineRedecode (TDT), which was **not run** |
| 5 | Honest FinalizationMode in production plan | **pass (hold)** | Production remains **StreamingFlush**; no capability claim of `IndependentOfflineRedecode` |
| 6 | Memory policy recorded | **partial** | Disk footprint noted; `latticed` warm/unload policy still stub-level — not a green adopt signal |

## Decision

```text
adopt_independent_offline_redecode: false
```

Rationale (one paragraph):

Fail closed. `private-audio/` is empty, so the README’s required real-mic fixture is absent. `tdt_v2` (the IndependentOfflineRedecode candidate) was not executed because Parakeet TDT v2 weights are not in `.cache/Models` and this pass must not start HuggingFace downloads. On the locked `m0-technical-dictation` fixture, `streaming_flush` and `unified_offline` both score path accuracy 0 and technical-token accuracy 1.0; unified offline normalized WER is worse than streaming by more than 0.02. None of the adopt gates for IndependentOfflineRedecode are satisfied, so production stays on **StreamingFlush**.

## Enabling independent final after eval wins

Production remains **`StreamingFlush`** until the checklist above passes and
`adopt_independent_offline_redecode` flips to `true`.

When measurements win and the FluidAudio offline/TDT bridge is implemented:

1. Set `LATTICE_VOICE_INDEPENDENT_FINAL=1` for the voice host / desktop process
   (or report `IndependentOfflineRedecode` / `SameFamilyOfflineRedecode` on
   provider capabilities once the second decode is real — ADR 0007).
2. Sessions already buffer full-utterance PCM in Rust
   (`UtteranceAudioBuffer` in `lattice-voice` / `FluidAudioSpeechSession`).
3. Wire `OfflineRedecodeBackend` in `lattice-voice-macos` to FluidAudio
   TDT v2 (`AsrManager`) or Unified offline (`UnifiedAsrManager`); replace
   `UnimplementedOfflineRedecode`.
4. Final-model memory: lazy load on first independent attempt, idle unload via
   `FinalModelMemoryPolicy` (stubs exist; `latticed` will own residency).
5. Do **not** claim offline modes while the backend is still a stub — env alone
   keeps `StreamingFlush` until re-decode actually runs.

Eval-only measurement (no production flip) continues to use this harness:

```sh
python3 scripts/voice_eval.py run --provider tdt_v2
# or: --provider unified_offline / --provider all
```

See [README.md](./README.md#acceptance-criteria--adopt-independentofflineredecode).
