# Voice eval — final-model comparison harness

Research-only harness to measure whether Lattice should adopt an independent
final ASR model (`IndependentOfflineRedecode`) versus keeping the current
production path (`StreamingFlush` via Unified `finish()`).

This package sits beside [voice-m0-fluidaudio](../voice-m0-fluidaudio/) and does
**not** change production providers, capabilities, or `FinalizationMode`
reporting. See ADR
[0007-finalization-mode](../../docs/voice/adr/0007-finalization-mode.md) and the
evaluation plan in
[current-implementation-review-and-ml-architecture.md](../../docs/voice/current-implementation-review-and-ml-architecture.md).

After a winning RESULTS.md decision, enable production independent final via
`LATTICE_VOICE_INDEPENDENT_FINAL=1` plus a real `OfflineRedecodeBackend` —
see [RESULTS.md](./RESULTS.md#enabling-independent-final-after-eval-wins).
Utterance PCM buffering already exists in `lattice-voice`; CI does not download
TDT models.

## Layout

```text
research/voice-eval/
├── README.md                 # this file
├── RESULTS.md                # measurement template (fill on the Mac)
├── manifest.yaml             # human-editable fixture / provider catalog
├── manifest.json             # runtime sidecar (no PyYAML required)
├── references/               # reference transcripts
├── private-audio/            # gitignored real-mic WAVs
└── scripts/
    ├── voice_eval.py         # CLI (dry-run / score / run)
    ├── metrics.py            # WER/CER + technical / path accuracy
    └── run_fluidaudio_provider.sh
```

## CI-safe commands (exit 0)

No models, no fixture WAVs, no Swift toolchain required:

```sh
cd research/voice-eval
python3 scripts/voice_eval.py              # help banner
python3 scripts/voice_eval.py --help
python3 scripts/voice_eval.py --dry-run    # plan + fixture presence; always OK
python3 scripts/test_metrics.py            # offline WER/CER unit check
```

`--dry-run` reports missing required audio as a note and still exits **0**. A
full `run` fails loudly when fixtures or models are missing (exit 2 / 3).

## Score without ASR

```sh
python3 scripts/voice_eval.py score \
  --reference references/technical-dictation.txt \
  --hypothesis "Lattice voice dictation should preserve camel case identifiers like ASR Manager, file paths such as users will developer lattice, and punctuation around code." \
  --technical AsrManager \
  --path /Users/will/Developer/lattice \
  --json
```

## Full comparison (measurement Mac)

Requires Apple Silicon, Xcode Swift, generated M0 fixture, and FluidAudio model
cache (same as [voice-m0-fluidaudio/README.md](../voice-m0-fluidaudio/README.md)):

```sh
# 1. Generate shared fixture if needed
cd research/voice-m0-fluidaudio && ./scripts/generate-fixture.sh && cd -

# 2. Baseline: streaming flush only
cd research/voice-eval
python3 scripts/voice_eval.py run --provider streaming_flush

# 3. Optional finals (loads additional models; first run downloads)
python3 scripts/voice_eval.py run --provider unified_offline
python3 scripts/voice_eval.py run --provider tdt_v2
python3 scripts/voice_eval.py run --provider all
```

Machine-readable scores land in `.results/last-run.json` (gitignored). Copy
numbers into `RESULTS.md`.

If models or Swift are unavailable:

```text
ERROR: FluidAudio provider runner requires macOS …
HINT: use --dry-run or score --hypothesis-file for offline metric checks.
```

Exit codes: `2` fixture missing, `3` model/deps missing, `4` provider failed.

## Providers under test

| Provider id | FinalizationMode | Source |
|-------------|------------------|--------|
| `streaming_flush` | `StreamingFlush` | Unified streaming `finish()` (production baseline) |
| `unified_offline` | `SameFamilyOfflineRedecode` | `UnifiedAsrManager` offline encoder |
| `tdt_v2` | `IndependentOfflineRedecode` | Parakeet TDT v2 via M0 `eou-tdt` offline path |

Fixtures reuse the M0 technical WAV and the same path used by
`crates/lattice-voice-macos/tests/live_asr.rs`. Optional private-mic clips go
under `private-audio/` (gitignored).

## Metrics

Report **raw** and **normalized** separately so a strong normalizer cannot hide
a weak acoustic model.

| Metric | Definition |
|--------|------------|
| WER | Word error rate (Levenshtein / reference words) |
| CER | Character error rate |
| Technical-token accuracy | Fraction of listed CamelCase / identifier tokens recovered (allows spaced CamelCase) |
| Path accuracy | Exact path substring hits only (`/Users/…` must appear) |
| Latency (from M0) | First partial, finalize / offline decode ms — paste into RESULTS.md |

Normalized scores case-fold and strip punctuation before WER/CER. Raw scores
do not.

## Acceptance criteria — adopt `IndependentOfflineRedecode`

Do **not** ship an independent final model until a filled `RESULTS.md` on the
target Mac shows **all** of the following versus `streaming_flush` on the locked
technical fixture **and** at least one real-mic fixture in `private-audio/`:

1. **Technical-token accuracy** strictly better than streaming flush
   (CamelCase / identifiers), or equal with a clear qualitative win on a larger
   private suite.
2. **Path accuracy** strictly better, or streaming flush and candidate both
   remain 0 — in that case independent decode alone is **not** sufficient; keep
   StreamingFlush until context/ITN (V1.2) improves paths.
3. **Normalized WER** not worse by more than **0.02** absolute on ordinary prose
   (and not worse on technical prose without a documented trade-off).
4. **Finalization latency** (offline decode after utterance end) ≤ **750 ms**
   warm on the measurement Mac, or ≤ **1.5×** streaming finalize — whichever is
   higher — with peak memory / energy notes recorded.
5. **Honest mode:** production capabilities report
   `IndependentOfflineRedecode` only when the second model actually re-decodes
   buffered audio (ADR 0007). No boolean `offline_final_decode` lies.
6. **Memory policy:** second model load/unload behavior is specified for
   `latticed` (warm while dictating; unload under pressure). Footprint recorded
   in RESULTS.md.

If criteria fail, keep production on `StreamingFlush` and treat TDT / Unified
offline as research-only.

## Out of scope

- Flipping the production final model or bridge defaults
- Committing private microphone audio or Core ML weights
- CI jobs that download HuggingFace models
