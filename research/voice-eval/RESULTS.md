# RESULTS — final-model comparison (voice-eval)

Fill this after a measurement-Mac run. Do **not** flip production
`FinalizationMode` from incomplete rows.

## Environment

| Item | Value |
|------|-------|
| Date | YYYY-MM-DD |
| Host | |
| OS / Xcode | |
| FluidAudio pin | (see voice-m0-fluidaudio Package.resolved) |
| Fixture set | `m0-technical-dictation` + (list private-audio ids) |
| Model cache | `research/voice-m0-fluidaudio/.cache/Models` |
| Command | `python3 scripts/voice_eval.py run --provider all` |

## Scores

Keep raw and normalized columns separate.

### Fixture: `m0-technical-dictation`

| Provider | Mode | Raw WER | Raw CER | Norm WER | Norm CER | Tech token acc | Path acc | Finalize / decode ms |
|----------|------|---------|---------|----------|----------|----------------|----------|----------------------|
| streaming_flush | StreamingFlush | | | | | | | |
| unified_offline | SameFamilyOfflineRedecode | | | | | | | |
| tdt_v2 | IndependentOfflineRedecode | | | | | | | |

### Fixture: private real-mic (id: ________)

| Provider | Mode | Raw WER | Raw CER | Norm WER | Norm CER | Tech token acc | Path acc | Finalize / decode ms |
|----------|------|---------|---------|----------|----------|----------------|----------|----------------------|
| streaming_flush | StreamingFlush | | | | | | | |
| unified_offline | SameFamilyOfflineRedecode | | | | | | | |
| tdt_v2 | IndependentOfflineRedecode | | | | | | | |

## Transcripts

| Provider | Hypothesis text |
|----------|-----------------|
| streaming_flush | |
| unified_offline | |
| tdt_v2 | |

## Memory / energy

| Provider | Approx RSS / cache bytes | Notes |
|----------|--------------------------|-------|
| streaming_flush | | |
| unified_offline | | second encoder? |
| tdt_v2 | | second model family? |

## Acceptance checklist (`IndependentOfflineRedecode`)

See [README.md](./README.md#acceptance-criteria--adopt-independentofflineredecode).

| # | Gate | Pass? | Evidence |
|---|------|-------|----------|
| 1 | Technical-token accuracy vs streaming flush | | |
| 2 | Path accuracy vs streaming flush | | |
| 3 | Normalized WER within +0.02 on ordinary prose | | |
| 4 | Warm finalize/decode latency budget | | |
| 5 | Honest FinalizationMode in production plan | | |
| 6 | Memory policy recorded | | |

## Decision

```text
adopt_independent_offline_redecode: pending
```

Rationale (one paragraph):
