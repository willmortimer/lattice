//! Deterministic inverse text normalization for voice finals.
//!
//! Applies conservative, evidence-backed corrections for spoken punctuation,
//! slash-delimited path reconstruction, and glossary identifier casing.

use serde::{Deserialize, Serialize};

use crate::protocol::{FinalTranscript, SessionContext};

/// Version string recorded in transcript provenance.
pub const NORMALIZER_VERSION: &str = "v1";

/// Local vocabulary and paths used for contextual corrections.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NormalizationContext {
    pub glossary_terms: Vec<String>,
    pub known_paths: Vec<String>,
}

impl From<&SessionContext> for NormalizationContext {
    fn from(context: &SessionContext) -> Self {
        Self {
            glossary_terms: context.glossary_terms.clone(),
            known_paths: context.known_paths.clone(),
        }
    }
}

/// Output of the deterministic normalizer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedTranscript {
    pub raw: String,
    pub normalized: String,
    pub corrections: Vec<CorrectionProvenance>,
}

/// Attributable correction applied during normalization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorrectionProvenance {
    pub kind: CorrectionKind,
    pub raw_start: usize,
    pub raw_end: usize,
    pub replacement: String,
    pub source: CorrectionSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrectionKind {
    SpokenPunctuation,
    PathReconstruction,
    IdentifierCasing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrectionSource {
    DeterministicRule,
    GlossaryExactMatch,
    KnownPathMatch,
}

/// Normalize raw ASR text using local glossary and path evidence.
pub fn normalize_transcript(raw: &str, context: &NormalizationContext) -> NormalizedTranscript {
    let mut text = raw.to_string();
    let mut corrections = Vec::new();

    if let Some((start, end, path)) = find_path_replacement(&text, &context.known_paths) {
        corrections.push(CorrectionProvenance {
            kind: CorrectionKind::PathReconstruction,
            raw_start: start,
            raw_end: end,
            replacement: path.clone(),
            source: CorrectionSource::KnownPathMatch,
        });
        text.replace_range(start..end, &path);
    }

    let (cased, id_corrections) = apply_identifier_casing(&text, &context.glossary_terms);
    text = cased;
    corrections.extend(id_corrections);

    let (punctuated, punct_corrections) = apply_spoken_punctuation(&text);
    text = punctuated;
    corrections.extend(punct_corrections);

    NormalizedTranscript {
        raw: raw.to_string(),
        normalized: text,
        corrections,
    }
}

/// Apply normalization to a provider final, preserving raw text when changed.
pub fn normalize_final_transcript(
    final_transcript: FinalTranscript,
    context: &NormalizationContext,
) -> FinalTranscript {
    let normalized = normalize_transcript(&final_transcript.text, context);
    if normalized.corrections.is_empty() {
        return final_transcript;
    }

    FinalTranscript {
        text: normalized.normalized,
        raw_text: Some(normalized.raw),
        corrections: normalized.corrections,
        ..final_transcript
    }
}

fn find_path_replacement(text: &str, known_paths: &[String]) -> Option<(usize, usize, String)> {
    let tokens = tokenize_with_spans(text);
    if tokens.is_empty() {
        return None;
    }

    let token_words: Vec<&str> = tokens.iter().map(|token| token.word).collect();
    let mut best: Option<(usize, usize, String)> = None;

    for path in known_paths {
        let path_words = flattened_path_words(path);
        if path_words.len() < 2 {
            continue;
        }

        for start in 0..tokens.len() {
            let Some(end_token) = match_path_words_flexible(&token_words, start, &path_words) else {
                continue;
            };
            if !span_has_slash_marker(&token_words[start..end_token]) {
                continue;
            }

            let start_byte = tokens[start].start;
            let end_byte = tokens[end_token.saturating_sub(1)].end;
            let candidate = (start_byte, end_byte, path.clone());

            if best.as_ref().is_none_or(|(s, e, _)| (end_byte - start_byte) > (e - s)) {
                best = Some(candidate);
            }
        }
    }

    best
}

fn flattened_path_words(path: &str) -> Vec<String> {
    path_components(path)
        .into_iter()
        .flatten()
        .collect()
}

fn match_path_words_flexible(
    tokens: &[&str],
    start: usize,
    path_words: &[String],
) -> Option<usize> {
    let mut path_idx = 0;
    let mut pos = start;
    let mut last_matched = start;

    while path_idx < path_words.len() && pos < tokens.len() {
        if is_slash_marker(tokens[pos]) {
            pos += 1;
            continue;
        }

        if tokens[pos].to_ascii_lowercase() == path_words[path_idx] {
            path_idx += 1;
            last_matched = pos + 1;
            pos += 1;
        } else {
            return None;
        }
    }

    if path_idx == path_words.len() {
        Some(last_matched)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy)]
struct TokenSpan<'a> {
    word: &'a str,
    start: usize,
    end: usize,
}

fn tokenize_with_spans(text: &str) -> Vec<TokenSpan<'_>> {
    let mut tokens = Vec::new();
    let mut in_word = false;
    let mut start = 0;

    for (index, ch) in text.char_indices() {
        if ch.is_whitespace() {
            if in_word {
                tokens.push(TokenSpan {
                    word: &text[start..index],
                    start,
                    end: index,
                });
                in_word = false;
            }
        } else if !in_word {
            start = index;
            in_word = true;
        }
    }

    if in_word {
        tokens.push(TokenSpan {
            word: &text[start..],
            start,
            end: text.len(),
        });
    }

    tokens
}

fn path_components(path: &str) -> Vec<Vec<String>> {
    path.split(['/', '\\'])
        .filter(|segment| !segment.is_empty())
        .map(component_words)
        .filter(|component| !component.is_empty())
        .collect()
}

fn component_words(component: &str) -> Vec<String> {
    component
        .split(|ch| ch == '-' || ch == '_')
        .filter(|part| !part.is_empty())
        .map(|part| part.to_ascii_lowercase())
        .collect()
}

fn is_slash_marker(token: &str) -> bool {
    matches!(token.to_ascii_lowercase().as_str(), "slash" | "/")
}

fn span_has_slash_marker(tokens: &[&str]) -> bool {
    tokens.iter().any(|token| is_slash_marker(token))
}

fn apply_identifier_casing(
    text: &str,
    glossary_terms: &[String],
) -> (String, Vec<CorrectionProvenance>) {
    let mut replacements = Vec::new();
    for term in glossary_terms {
        if !is_identifier_candidate(term) {
            continue;
        }
        for variant in spoken_variants(term) {
            if variant.contains(' ') || term.contains('_') {
                replacements.push((variant, term.clone()));
            }
        }
    }

    replacements.sort_by(|left, right| right.0.len().cmp(&left.0.len()));
    replacements.dedup_by(|left, right| left.0 == right.0);

    let mut output = text.to_string();
    let mut corrections = Vec::new();
    for (variant, canonical) in replacements {
        let mut search_from = 0;
        while let Some(relative) = find_case_insensitive_phrase(&output[search_from..], &variant) {
            let start = search_from + relative.start;
            let end = search_from + relative.end;
            if !phrase_has_word_boundaries(&output, start, end) {
                search_from = end;
                continue;
            }

            corrections.push(CorrectionProvenance {
                kind: CorrectionKind::IdentifierCasing,
                raw_start: start,
                raw_end: end,
                replacement: canonical.clone(),
                source: CorrectionSource::GlossaryExactMatch,
            });
            output.replace_range(start..end, &canonical);
            search_from = start + canonical.len();
        }
    }

    (output, corrections)
}

fn is_identifier_candidate(term: &str) -> bool {
    term.contains('_')
        || term
            .chars()
            .any(|ch| ch.is_ascii_uppercase())
            && term.chars().any(|ch| ch.is_ascii_lowercase())
}

fn spoken_variants(term: &str) -> Vec<String> {
    let mut variants = vec![term.to_ascii_lowercase()];

    let camel = split_camel_case(term)
        .join(" ")
        .to_ascii_lowercase();
    if !camel.is_empty() {
        variants.push(camel);
    }

    if term.contains('_') {
        variants.push(term.replace('_', " ").to_ascii_lowercase());
    }

    variants.sort();
    variants.dedup();
    variants
}

fn split_camel_case(value: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();

    for ch in value.chars() {
        if ch.is_ascii_uppercase() && !current.is_empty() {
            parts.push(current.clone());
            current.clear();
        }
        current.push(ch);
    }

    if !current.is_empty() {
        parts.push(current);
    }

    parts
}

struct RelativeSpan {
    start: usize,
    end: usize,
}

fn find_case_insensitive_phrase(haystack: &str, needle: &str) -> Option<RelativeSpan> {
    if needle.is_empty() {
        return None;
    }

    let haystack_lower = haystack.to_ascii_lowercase();
    let start = haystack_lower.find(needle)?;
    Some(RelativeSpan {
        start,
        end: start + needle.len(),
    })
}

fn phrase_has_word_boundaries(text: &str, start: usize, end: usize) -> bool {
    let before_ok = start == 0 || !text[..start].ends_with(|ch: char| ch.is_alphanumeric());
    let after_ok = end == text.len() || !text[end..].starts_with(|ch: char| ch.is_alphanumeric());
    before_ok && after_ok
}

const SPOKEN_PUNCTUATION: &[(&str, &str)] = &[
    ("question mark", "?"),
    ("exclamation mark", "!"),
    ("exclamation point", "!"),
    ("period", "."),
    ("comma", ","),
    ("colon", ":"),
    ("semicolon", ";"),
    ("new line", "\n"),
    ("newline", "\n"),
];

fn apply_spoken_punctuation(text: &str) -> (String, Vec<CorrectionProvenance>) {
    let mut output = text.to_string();
    let mut corrections = Vec::new();

    for (spoken, symbol) in SPOKEN_PUNCTUATION {
        let mut search_from = 0;
        while let Some(relative) = find_case_insensitive_phrase(&output[search_from..], spoken) {
            let mut start = search_from + relative.start;
            let end = search_from + relative.end;
            if !phrase_has_word_boundaries(&output, start, end) {
                search_from = end;
                continue;
            }
            if start > 0 && output.as_bytes()[start - 1] == b' ' {
                start -= 1;
            }

            corrections.push(CorrectionProvenance {
                kind: CorrectionKind::SpokenPunctuation,
                raw_start: start,
                raw_end: end,
                replacement: (*symbol).to_string(),
                source: CorrectionSource::DeterministicRule,
            });
            output.replace_range(start..end, symbol);
            search_from = start + symbol.len();
        }
    }

    (output, corrections)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::FinalizationMode;

    fn context(glossary: &[&str], paths: &[&str]) -> NormalizationContext {
        NormalizationContext {
            glossary_terms: glossary.iter().map(|term| (*term).to_string()).collect(),
            known_paths: paths.iter().map(|path| (*path).to_string()).collect(),
        }
    }

    #[test]
    fn reconstructs_known_absolute_path_from_spoken_slash_markers() {
        let raw = "users will developer lattice slash crates slash lattice voice";
        let normalized = normalize_transcript(
            raw,
            &context(
                &[],
                &["/Users/will/Developer/lattice/crates/lattice-voice"],
            ),
        );

        assert_eq!(
            normalized.normalized,
            "/Users/will/Developer/lattice/crates/lattice-voice"
        );
        assert_eq!(normalized.corrections.len(), 1);
        assert_eq!(
            normalized.corrections[0].kind,
            CorrectionKind::PathReconstruction
        );
        assert_eq!(
            normalized.corrections[0].source,
            CorrectionSource::KnownPathMatch
        );
    }

    #[test]
    fn does_not_reconstruct_path_without_slash_markers() {
        let raw = "users will developer lattice crates lattice voice";
        let normalized = normalize_transcript(
            raw,
            &context(
                &[],
                &["/Users/will/Developer/lattice/crates/lattice-voice"],
            ),
        );

        assert_eq!(normalized.normalized, raw);
        assert!(normalized.corrections.is_empty());
    }

    #[test]
    fn restores_camelcase_identifier_from_glossary() {
        let raw = "preserve camelcase identifiers like ASR Manager and punctuation";
        let normalized = normalize_transcript(raw, &context(&["AsrManager"], &[]));

        assert_eq!(
            normalized.normalized,
            "preserve camelcase identifiers like AsrManager and punctuation"
        );
        assert!(normalized
            .corrections
            .iter()
            .any(|correction| correction.kind == CorrectionKind::IdentifierCasing));
    }

    #[test]
    fn restores_snake_case_identifier_from_glossary() {
        let raw = "the lattice voice crate handles finals";
        let normalized = normalize_transcript(raw, &context(&["lattice_voice"], &[]));

        assert_eq!(
            normalized.normalized,
            "the lattice_voice crate handles finals"
        );
    }

    #[test]
    fn converts_spoken_punctuation_words() {
        let raw = "end of sentence period next clause comma really question mark";
        let normalized = normalize_transcript(raw, &context(&[], &[]));

        assert_eq!(
            normalized.normalized,
            "end of sentence. next clause, really?"
        );
        assert_eq!(normalized.corrections.len(), 3);
        assert!(normalized
            .corrections
            .iter()
            .all(|correction| correction.kind == CorrectionKind::SpokenPunctuation));
    }

    #[test]
    fn normalize_final_transcript_preserves_metadata() {
        let final_transcript = FinalTranscript {
            session_id: "voice-1".into(),
            utterance_id: "utt-1".into(),
            replaces_revision: 2,
            text: "open AsrManager period".into(),
            raw_text: None,
            corrections: Vec::new(),
            finalization_mode: FinalizationMode::StreamingFlush,
            duration_ms: 10,
            processing_ms: 5,
        };

        let normalized = normalize_final_transcript(
            final_transcript,
            &context(&["AsrManager"], &[]),
        );

        assert_eq!(normalized.text, "open AsrManager.");
        assert_eq!(
            normalized.raw_text.as_deref(),
            Some("open AsrManager period")
        );
        assert!(!normalized.corrections.is_empty());
        assert_eq!(normalized.replaces_revision, 2);
    }

    #[test]
    fn leaves_unmatched_text_unchanged() {
        let raw = "ordinary prose without technical tokens";
        let normalized = normalize_transcript(raw, &context(&["AsrManager"], &[]));

        assert_eq!(normalized.normalized, raw);
        assert!(normalized.corrections.is_empty());
    }
}
