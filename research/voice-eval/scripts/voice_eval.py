#!/usr/bin/env python3
"""research/voice-eval harness — compare streaming flush vs optional final models.

CI-safe entrypoints (exit 0):
  python3 scripts/voice_eval.py
  python3 scripts/voice_eval.py --help
  python3 scripts/voice_eval.py --dry-run

Full ASR runs require macOS + FluidAudio models + fixture WAVs and exit
non-zero with a clear message when those are missing (not for CI).
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

# Allow `python3 scripts/voice_eval.py` without installing a package.
SCRIPTS_DIR = Path(__file__).resolve().parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from metrics import (  # noqa: E402
    EditRates,
    TokenAccuracy,
    edit_rates,
    path_accuracy,
    token_accuracy,
)

ROOT = SCRIPTS_DIR.parent
DEFAULT_MANIFEST = ROOT / "manifest.yaml"

EXIT_OK = 0
EXIT_USAGE = 1
EXIT_FIXTURE_MISSING = 2
EXIT_MODEL_OR_DEPS_MISSING = 3
EXIT_PROVIDER_FAILED = 4


@dataclass
class ScoredHypothesis:
    fixture_id: str
    provider_id: str
    finalization_mode: str
    hypothesis: str
    raw: EditRates
    normalized: EditRates
    technical: TokenAccuracy
    paths: TokenAccuracy


def load_manifest(path: Path) -> dict[str, Any]:
    """Load manifest.yaml, preferring a checked-in manifest.json sidecar (no PyYAML)."""
    sidecar = path.with_suffix(".json")
    if sidecar.is_file():
        data = json.loads(sidecar.read_text(encoding="utf-8"))
        if not isinstance(data, dict):
            raise SystemExit(f"ERROR: manifest root must be a mapping: {sidecar}")
        return data
    try:
        import yaml  # type: ignore
    except ImportError as exc:
        print(
            "ERROR: PyYAML is not installed and no manifest.json sidecar exists.\n"
            "HINT: keep research/voice-eval/manifest.json in sync with manifest.yaml.",
            file=sys.stderr,
        )
        raise ImportError("pyyaml missing") from exc
    with path.open(encoding="utf-8") as fh:
        data = yaml.safe_load(fh)
    if not isinstance(data, dict):
        raise SystemExit(f"ERROR: manifest root must be a mapping: {path}")
    return data


def resolve_path(base: Path, maybe_relative: str) -> Path:
    p = Path(maybe_relative)
    if p.is_absolute():
        return p
    return (base / p).resolve()


def print_help_banner() -> None:
    print(
        """voice-eval — final-model comparison harness (research only)

Does NOT flip the production final model. Measures StreamingFlush vs optional
Unified offline / TDT v2 before adopting IndependentOfflineRedecode.

CI-safe:
  python3 scripts/voice_eval.py              # help + exit 0
  python3 scripts/voice_eval.py --help
  python3 scripts/voice_eval.py --dry-run    # plan fixtures/providers; exit 0

Measurement Mac (models + fixtures required):
  python3 scripts/voice_eval.py run --provider streaming_flush
  python3 scripts/voice_eval.py run --provider all
  python3 scripts/voice_eval.py score --reference references/technical-dictation.txt \\
      --hypothesis-file /tmp/hyp.txt --technical AsrManager --path /Users/will/Developer/lattice

See README.md for metrics and IndependentOfflineRedecode acceptance criteria.
"""
    )


def cmd_dry_run(manifest_path: Path) -> int:
    print("=== voice-eval dry-run ===")
    print(f"manifest: {manifest_path}")
    yaml_ok = manifest_path.is_file()
    json_ok = manifest_path.with_suffix(".json").is_file()
    if not yaml_ok and not json_ok:
        print(
            f"ERROR: manifest not found (expected {manifest_path} or .json sidecar)",
            file=sys.stderr,
        )
        return EXIT_FIXTURE_MISSING

    try:
        data = load_manifest(manifest_path)
    except ImportError:
        text = manifest_path.read_text(encoding="utf-8") if yaml_ok else ""
        if "schema_version" not in text or "fixtures:" not in text:
            print("ERROR: manifest missing expected keys (schema_version, fixtures).", file=sys.stderr)
            return EXIT_FIXTURE_MISSING
        print("WARN: could not parse manifest; performing structural dry-run only.")
        print("status: DRY_RUN_OK (structural)")
        print("NOTE: production final model is unchanged (StreamingFlush).")
        return EXIT_OK

    fixtures = data.get("fixtures") or []
    providers = data.get("providers") or []
    print(f"providers: {len(providers)}")
    for p in providers:
        opt = " optional" if p.get("optional") else ""
        print(f"  - {p.get('id')} [{p.get('finalization_mode')}]{opt}")
    print(f"fixtures: {len(fixtures)}")
    missing_required = []
    for fx in fixtures:
        audio = resolve_path(ROOT, fx["audio"])
        ref = resolve_path(ROOT, fx["reference"]) if fx.get("reference") else None
        optional = bool(fx.get("optional"))
        audio_ok = audio.is_file()
        ref_ok = ref is None or ref.is_file()
        status = "ok" if audio_ok and ref_ok else ("missing(optional)" if optional else "MISSING")
        print(f"  - {fx.get('id')}: audio={audio_ok} ref={ref_ok} → {status}")
        if not optional and (not audio_ok or not ref_ok):
            missing_required.append(fx.get("id"))
    if missing_required:
        print(
            f"NOTE: required fixtures not on disk yet: {', '.join(missing_required)}. "
            "Dry-run still succeeds; full `run` will exit 2 until generated."
        )
    print("status: DRY_RUN_OK")
    print("NOTE: production final model is unchanged (StreamingFlush).")
    return EXIT_OK


def score_pair(
    *,
    fixture_id: str,
    provider_id: str,
    finalization_mode: str,
    reference: str,
    hypothesis: str,
    technical_tokens: list[str],
    path_tokens: list[str],
) -> ScoredHypothesis:
    return ScoredHypothesis(
        fixture_id=fixture_id,
        provider_id=provider_id,
        finalization_mode=finalization_mode,
        hypothesis=hypothesis,
        raw=edit_rates(reference, hypothesis, normalized=False),
        normalized=edit_rates(reference, hypothesis, normalized=True),
        technical=token_accuracy(hypothesis, technical_tokens, case_sensitive=False),
        paths=path_accuracy(hypothesis, path_tokens),
    )


def format_score(s: ScoredHypothesis) -> str:
    return (
        f"fixture={s.fixture_id} provider={s.provider_id} mode={s.finalization_mode}\n"
        f"  hyp={s.hypothesis!r}\n"
        f"  raw:      WER={s.raw.wer:.3f} CER={s.raw.cer:.3f}\n"
        f"  normalized: WER={s.normalized.wer:.3f} CER={s.normalized.cer:.3f}\n"
        f"  technical_token_acc={s.technical.accuracy:.3f} ({s.technical.hit}/{s.technical.total})\n"
        f"  path_acc={s.paths.accuracy:.3f} ({s.paths.hit}/{s.paths.total})"
    )


def cmd_score(args: argparse.Namespace) -> int:
    reference = Path(args.reference).read_text(encoding="utf-8").strip()
    if args.hypothesis_file:
        hypothesis = Path(args.hypothesis_file).read_text(encoding="utf-8").strip()
    else:
        hypothesis = args.hypothesis or ""
    if not hypothesis:
        print("ERROR: provide --hypothesis or --hypothesis-file", file=sys.stderr)
        return EXIT_USAGE
    technical = args.technical or []
    paths = args.path or []
    scored = score_pair(
        fixture_id=args.fixture_id or "adhoc",
        provider_id=args.provider_id or "adhoc",
        finalization_mode=args.finalization_mode or "unknown",
        reference=reference,
        hypothesis=hypothesis,
        technical_tokens=technical,
        path_tokens=paths,
    )
    print(format_score(scored))
    if args.json:
        payload = {
            "fixture_id": scored.fixture_id,
            "provider_id": scored.provider_id,
            "finalization_mode": scored.finalization_mode,
            "hypothesis": scored.hypothesis,
            "raw": asdict(scored.raw),
            "normalized": asdict(scored.normalized),
            "technical": asdict(scored.technical) | {"accuracy": scored.technical.accuracy},
            "paths": asdict(scored.paths) | {"accuracy": scored.paths.accuracy},
        }
        print(json.dumps(payload, indent=2))
    return EXIT_OK


def _parse_m0_output(text: str) -> dict[str, str]:
    out: dict[str, str] = {}
    for line in text.splitlines():
        if "=" not in line:
            continue
        key, _, value = line.partition("=")
        if key in {
            "MODE",
            "STREAMING_TEXT",
            "OFFLINE_TEXT",
            "STREAMING_FINALIZE_MS",
            "OFFLINE_DECODE_MS",
            "FIRST_PARTIAL_MS",
        }:
            out[key] = value
    return out


def cmd_run(args: argparse.Namespace) -> int:
    manifest_path = Path(args.manifest)
    try:
        data = load_manifest(manifest_path)
    except ImportError:
        print(
            "ERROR: cannot load manifest (need manifest.json or PyYAML).",
            file=sys.stderr,
        )
        return EXIT_MODEL_OR_DEPS_MISSING

    providers = {p["id"]: p for p in data.get("providers") or []}
    fixtures = data.get("fixtures") or []

    wanted = list(providers.keys()) if args.provider == "all" else [args.provider]
    for pid in wanted:
        if pid not in providers:
            print(f"ERROR: unknown provider {pid!r}; known={sorted(providers)}", file=sys.stderr)
            return EXIT_USAGE

    required_fixtures = [fx for fx in fixtures if not fx.get("optional")]
    missing_audio: list[str] = []
    seen: set[str] = set()
    for fx in required_fixtures:
        audio = resolve_path(ROOT, fx["audio"])
        key = str(audio)
        if not audio.is_file() and key not in seen:
            seen.add(key)
            missing_audio.append(key)
    if missing_audio:
        print("ERROR: required fixture audio missing:", file=sys.stderr)
        for path in missing_audio:
            print(f"  - {path}", file=sys.stderr)
        print(
            "HINT: cd research/voice-m0-fluidaudio && ./scripts/generate-fixture.sh",
            file=sys.stderr,
        )
        return EXIT_FIXTURE_MISSING

    runner = ROOT / "scripts" / "run_fluidaudio_provider.sh"
    if not runner.is_file():
        print(f"ERROR: provider runner missing: {runner}", file=sys.stderr)
        return EXIT_MODEL_OR_DEPS_MISSING

    results: list[ScoredHypothesis] = []
    # Group providers by M0 mode to avoid redundant Swift runs.
    modes_needed: dict[str, list[str]] = {}
    for pid in wanted:
        p = providers[pid]
        mode = p.get("m0_mode", "unified")
        modes_needed.setdefault(mode, []).append(pid)

    m0_outputs: dict[str, dict[str, str]] = {}
    for mode, pids in modes_needed.items():
        print(f"=== invoking FluidAudio M0 mode={mode} for {pids} ===")
        try:
            completed = subprocess.run(
                [str(runner), mode],
                check=False,
                capture_output=True,
                text=True,
            )
        except OSError as exc:
            print(f"ERROR: failed to exec provider runner: {exc}", file=sys.stderr)
            return EXIT_MODEL_OR_DEPS_MISSING
        if completed.returncode != 0:
            sys.stderr.write(completed.stderr or "")
            sys.stderr.write(completed.stdout or "")
            if completed.returncode in (2, 3):
                return completed.returncode
            print(
                f"ERROR: provider runner failed with exit {completed.returncode}",
                file=sys.stderr,
            )
            return EXIT_PROVIDER_FAILED
        parsed = _parse_m0_output(completed.stdout)
        if "STREAMING_TEXT" not in parsed and "OFFLINE_TEXT" not in parsed:
            print("ERROR: M0 output missing STREAMING_TEXT/OFFLINE_TEXT lines.", file=sys.stderr)
            print(completed.stdout[-2000:], file=sys.stderr)
            return EXIT_PROVIDER_FAILED
        m0_outputs[mode] = parsed

    # Score against the primary M0 technical fixture (shared audio).
    primary = next((fx for fx in fixtures if fx.get("id") == "m0-technical-dictation"), None)
    if primary is None:
        print("ERROR: manifest missing fixture id=m0-technical-dictation", file=sys.stderr)
        return EXIT_FIXTURE_MISSING
    ref_path = resolve_path(ROOT, primary["reference"])
    reference = ref_path.read_text(encoding="utf-8").strip()
    technical = list(primary.get("technical_tokens") or [])
    paths = list(primary.get("path_tokens") or [])

    for pid in wanted:
        p = providers[pid]
        mode = p.get("m0_mode", "unified")
        which = p.get("m0_path", "streaming")
        parsed = m0_outputs[mode]
        key = "STREAMING_TEXT" if which == "streaming" else "OFFLINE_TEXT"
        hyp = parsed.get(key)
        if not hyp:
            print(
                f"ERROR: provider {pid} expected {key} in M0 output for mode={mode}",
                file=sys.stderr,
            )
            return EXIT_PROVIDER_FAILED
        scored = score_pair(
            fixture_id=primary["id"],
            provider_id=pid,
            finalization_mode=p.get("finalization_mode", "unknown"),
            reference=reference,
            hypothesis=hyp,
            technical_tokens=technical,
            path_tokens=paths,
        )
        results.append(scored)
        print(format_score(scored))

    out_dir = ROOT / ".results"
    out_dir.mkdir(exist_ok=True)
    out_path = out_dir / "last-run.json"
    payload = [
        {
            "fixture_id": r.fixture_id,
            "provider_id": r.provider_id,
            "finalization_mode": r.finalization_mode,
            "hypothesis": r.hypothesis,
            "raw_wer": r.raw.wer,
            "raw_cer": r.raw.cer,
            "norm_wer": r.normalized.wer,
            "norm_cer": r.normalized.cer,
            "technical_acc": r.technical.accuracy,
            "path_acc": r.paths.accuracy,
        }
        for r in results
    ]
    out_path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {out_path}")
    print("NOTE: Do not flip production final model from these numbers alone;")
    print("      fill RESULTS.md and check README acceptance criteria first.")
    return EXIT_OK


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="voice_eval",
        description="Final-model comparison harness (research/voice-eval).",
        add_help=True,
    )
    parser.add_argument(
        "--manifest",
        default=str(DEFAULT_MANIFEST),
        help="Path to manifest.yaml (default: research/voice-eval/manifest.yaml)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Validate manifest / report fixture presence; always exit 0 on success",
    )

    sub = parser.add_subparsers(dest="command")

    run_p = sub.add_parser("run", help="Invoke FluidAudio providers and score (macOS)")
    run_p.add_argument(
        "--provider",
        default="streaming_flush",
        help="Provider id from manifest, or 'all'",
    )
    run_p.add_argument("--manifest", default=str(DEFAULT_MANIFEST))

    score_p = sub.add_parser("score", help="Score a hypothesis against a reference (no ASR)")
    score_p.add_argument("--reference", required=True)
    score_p.add_argument("--hypothesis")
    score_p.add_argument("--hypothesis-file")
    score_p.add_argument("--technical", action="append", default=[])
    score_p.add_argument("--path", action="append", default=[])
    score_p.add_argument("--fixture-id")
    score_p.add_argument("--provider-id")
    score_p.add_argument("--finalization-mode")
    score_p.add_argument("--json", action="store_true")

    return parser


def main(argv: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    if not argv:
        print_help_banner()
        return EXIT_OK

    parser = build_parser()
    args = parser.parse_args(argv)

    if args.dry_run:
        return cmd_dry_run(Path(args.manifest))

    if args.command is None:
        print_help_banner()
        return EXIT_OK
    if args.command == "score":
        return cmd_score(args)
    if args.command == "run":
        return cmd_run(args)

    print(f"ERROR: unknown command {args.command!r}", file=sys.stderr)
    return EXIT_USAGE


if __name__ == "__main__":
    sys.exit(main())
