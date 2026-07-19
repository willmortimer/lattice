#!/usr/bin/env python3
"""WER / CER and technical-token metrics for research/voice-eval.

Raw scores use lightly tokenized text (lowercase, punctuation stripped for WER).
Normalized scores apply the same lightweight normalizer to both hypothesis and
reference so acoustic quality is not confused with post-ITN gains.
"""

from __future__ import annotations

import re
import unicodedata
from dataclasses import dataclass


_WORD_RE = re.compile(r"[a-z0-9]+(?:'[a-z]+)?", re.IGNORECASE)
_PATH_RE = re.compile(r"(?:/[\w.-]+)+")


@dataclass(frozen=True)
class EditRates:
    wer: float
    cer: float
    word_edits: int
    word_ref: int
    char_edits: int
    char_ref: int


@dataclass(frozen=True)
class TokenAccuracy:
    hit: int
    total: int

    @property
    def accuracy(self) -> float:
        if self.total == 0:
            return 1.0
        return self.hit / self.total


def normalize_for_wer(text: str) -> str:
    """Case-fold and strip punctuation for generic WER tokenization."""
    folded = unicodedata.normalize("NFKC", text).casefold()
    return " ".join(_WORD_RE.findall(folded))


def normalize_chars(text: str) -> str:
    folded = unicodedata.normalize("NFKC", text).casefold()
    return re.sub(r"\s+", " ", folded).strip()


def levenshtein(a: list[str] | str, b: list[str] | str) -> int:
    if len(a) < len(b):
        return levenshtein(b, a)
    if not b:
        return len(a)
    previous = list(range(len(b) + 1))
    for i, ca in enumerate(a, start=1):
        current = [i]
        for j, cb in enumerate(b, start=1):
            insert = current[j - 1] + 1
            delete = previous[j] + 1
            replace = previous[j - 1] + (0 if ca == cb else 1)
            current.append(min(insert, delete, replace))
        previous = current
    return previous[-1]


def edit_rates(reference: str, hypothesis: str, *, normalized: bool) -> EditRates:
    if normalized:
        ref_words = normalize_for_wer(reference).split()
        hyp_words = normalize_for_wer(hypothesis).split()
        ref_chars = list(normalize_chars(reference).replace(" ", ""))
        hyp_chars = list(normalize_chars(hypothesis).replace(" ", ""))
    else:
        # Raw: whitespace tokenize / characterize without case-folding.
        ref_words = reference.split()
        hyp_words = hypothesis.split()
        ref_chars = list(reference)
        hyp_chars = list(hypothesis)

    word_edits = levenshtein(ref_words, hyp_words)
    char_edits = levenshtein(ref_chars, hyp_chars)
    word_ref = max(len(ref_words), 1)
    char_ref = max(len(ref_chars), 1)
    return EditRates(
        wer=word_edits / word_ref,
        cer=char_edits / char_ref,
        word_edits=word_edits,
        word_ref=len(ref_words),
        char_edits=char_edits,
        char_ref=len(ref_chars),
    )


def token_accuracy(hypothesis: str, tokens: list[str], *, case_sensitive: bool) -> TokenAccuracy:
    if not tokens:
        return TokenAccuracy(hit=0, total=0)
    hay = hypothesis if case_sensitive else hypothesis.casefold()
    hit = 0
    for token in tokens:
        needle = token if case_sensitive else token.casefold()
        # Accept common ASR space-splitting of CamelCase (AsrManager → ASR Manager).
        alt = re.sub(r"(?<=[a-z])(?=[A-Z])", " ", token)
        alts = {needle}
        if not case_sensitive:
            alts.add(alt.casefold())
        else:
            alts.add(alt)
            alts.add(alt.upper())
        if any(a in hay for a in alts if a):
            hit += 1
    return TokenAccuracy(hit=hit, total=len(tokens))


def path_accuracy(hypothesis: str, paths: list[str]) -> TokenAccuracy:
    """Path accuracy: exact substring or slash-stripped fallback still fails."""
    if not paths:
        return TokenAccuracy(hit=0, total=0)
    hit = 0
    for path in paths:
        if path in hypothesis:
            hit += 1
            continue
        # Soft match: all path segments present in order (still counts as miss for
        # acceptance gates; surfaced separately for debugging).
        _ = _PATH_RE.findall(hypothesis)
    return TokenAccuracy(hit=hit, total=len(paths))
