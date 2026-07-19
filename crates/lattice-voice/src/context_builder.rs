use crate::protocol::SessionContext;

/// Lower bound for glossary term count when enough local candidates exist.
pub const DEFAULT_MIN_GLOSSARY_TERMS: usize = 50;
/// Upper bound for glossary term count passed to the ASR provider.
pub const DEFAULT_MAX_GLOSSARY_TERMS: usize = 200;

/// Local signals used to assemble a bounded session glossary.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VoiceContextInput {
    pub document_id: Option<String>,
    pub heading_path: Vec<String>,
    pub page_title: Option<String>,
    pub workspace_name: Option<String>,
    pub document_path: Option<String>,
    pub tags: Vec<String>,
    pub extra_glossary_terms: Vec<String>,
    pub known_paths: Vec<String>,
    pub known_symbols: Vec<String>,
}

/// Output of [`VoiceContextBuilder`]: identifiers plus bounded glossary terms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltVoiceContext {
    pub document_id: Option<String>,
    pub heading_path: Vec<String>,
    pub glossary_terms: Vec<String>,
}

/// Hook for future embedding-backed glossary expansion. Returns additional terms
/// that will be merged after local exact-name candidates.
pub trait EmbeddingGlossaryHook: Send + Sync {
    fn suggest_terms(&self, _input: &VoiceContextInput) -> Vec<String> {
        Vec::new()
    }
}

/// Assembles deterministic, bounded glossary terms from local workspace signals.
#[derive(Debug, Clone)]
pub struct VoiceContextBuilder {
    min_terms: usize,
    max_terms: usize,
}

impl Default for VoiceContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl VoiceContextBuilder {
    pub fn new() -> Self {
        Self {
            min_terms: DEFAULT_MIN_GLOSSARY_TERMS,
            max_terms: DEFAULT_MAX_GLOSSARY_TERMS,
        }
    }

    pub fn with_bounds(min_terms: usize, max_terms: usize) -> Self {
        Self {
            min_terms,
            max_terms,
        }
    }

    pub fn build(
        &self,
        input: &VoiceContextInput,
        embedding_hook: Option<&dyn EmbeddingGlossaryHook>,
    ) -> BuiltVoiceContext {
        let mut terms = Vec::new();
        let mut seen = std::collections::HashSet::new();

        let push_term = |term: &str, terms: &mut Vec<String>, seen: &mut std::collections::HashSet<String>| {
            let normalized = normalize_term(term);
            if normalized.is_empty() {
                return;
            }
            let key = normalized.to_ascii_lowercase();
            if seen.insert(key) {
                terms.push(normalized);
            }
        };

        // Prefer exact caller-supplied names first.
        for term in &input.extra_glossary_terms {
            push_term(term, &mut terms, &mut seen);
        }

        if let Some(title) = input.page_title.as_deref() {
            push_term(title, &mut terms, &mut seen);
        }

        for heading in &input.heading_path {
            push_term(heading, &mut terms, &mut seen);
        }

        if let Some(workspace) = input.workspace_name.as_deref() {
            push_term(workspace, &mut terms, &mut seen);
        }

        for tag in &input.tags {
            push_term(tag, &mut terms, &mut seen);
        }

        if let Some(path) = input.document_path.as_deref() {
            for segment in path_segments(path) {
                push_term(&segment, &mut terms, &mut seen);
            }
        }

        for symbol in &input.known_symbols {
            push_term(symbol, &mut terms, &mut seen);
        }

        for path in &input.known_paths {
            for segment in path_segments(path) {
                push_term(&segment, &mut terms, &mut seen);
            }
        }

        if let Some(hook) = embedding_hook {
            for term in hook.suggest_terms(input) {
                push_term(&term, &mut terms, &mut seen);
            }
        }

        let glossary_terms = bound_terms(terms, self.min_terms, self.max_terms);

        BuiltVoiceContext {
            document_id: input.document_id.clone(),
            heading_path: input.heading_path.clone(),
            glossary_terms,
        }
    }

    pub fn build_session_context(
        &self,
        input: &VoiceContextInput,
        command_mode: bool,
        embedding_hook: Option<&dyn EmbeddingGlossaryHook>,
    ) -> SessionContext {
        let built = self.build(input, embedding_hook);
        SessionContext {
            document_id: built.document_id,
            glossary_terms: built.glossary_terms,
            command_mode,
        }
    }
}

fn normalize_term(term: &str) -> String {
    let trimmed = term.trim();
    if trimmed.len() < 2 {
        return String::new();
    }
    if trimmed.chars().all(|ch| ch.is_ascii_digit()) {
        return String::new();
    }
    trimmed.to_string()
}

fn path_segments(path: &str) -> Vec<String> {
    path.split(['/', '\\'])
        .filter_map(|segment| {
            let stem = segment
                .trim()
                .strip_suffix(".md")
                .or_else(|| segment.strip_suffix(".markdown"))
                .unwrap_or(segment)
                .trim();
            if stem.is_empty() || stem == "." || stem == ".." {
                None
            } else {
                Some(stem.to_string())
            }
        })
        .collect()
}

fn bound_terms(mut terms: Vec<String>, min_terms: usize, max_terms: usize) -> Vec<String> {
    if terms.len() > max_terms {
        terms.truncate(max_terms);
    }
    // Keep whatever local exact names we have; min_terms is a soft target for
    // future embedding expansion, not a reason to pad with synthetic terms.
    let _ = min_terms;
    terms
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubEmbeddingHook;

    impl EmbeddingGlossaryHook for StubEmbeddingHook {
        fn suggest_terms(&self, _input: &VoiceContextInput) -> Vec<String> {
            vec!["EmbeddingTerm".into()]
        }
    }

    #[test]
    fn builds_glossary_from_page_workspace_tags_and_path() {
        let builder = VoiceContextBuilder::new();
        let input = VoiceContextInput {
            document_id: Some("doc-1".into()),
            heading_path: vec!["Architecture".into()],
            page_title: Some("Release Notes".into()),
            workspace_name: Some("First Look".into()),
            document_path: Some("Product/Release Notes.md".into()),
            tags: vec!["product".into(), "release".into()],
            extra_glossary_terms: vec!["Lattice".into()],
            ..VoiceContextInput::default()
        };

        let built = builder.build(&input, None);
        assert_eq!(built.document_id.as_deref(), Some("doc-1"));
        assert_eq!(built.heading_path, vec!["Architecture"]);
        assert!(built.glossary_terms.contains(&"Lattice".into()));
        assert!(built.glossary_terms.contains(&"Release Notes".into()));
        assert!(built.glossary_terms.contains(&"First Look".into()));
        assert!(built.glossary_terms.contains(&"product".into()));
        assert!(built.glossary_terms.contains(&"Release Notes".into()));
    }

    #[test]
    fn deduplicates_case_insensitive_terms() {
        let builder = VoiceContextBuilder::new();
        let input = VoiceContextInput {
            page_title: Some("Lattice".into()),
            extra_glossary_terms: vec!["lattice".into(), "LATTICE".into()],
            ..VoiceContextInput::default()
        };

        let built = builder.build(&input, None);
        assert_eq!(built.glossary_terms, vec![String::from("lattice")]);
    }

    #[test]
    fn bounds_glossary_to_max_terms() {
        let builder = VoiceContextBuilder::with_bounds(1, 3);
        let input = VoiceContextInput {
            extra_glossary_terms: (0..10).map(|i| format!("term-{i}")).collect(),
            ..VoiceContextInput::default()
        };

        let built = builder.build(&input, None);
        assert_eq!(built.glossary_terms.len(), 3);
        assert_eq!(
            built.glossary_terms,
            vec![
                String::from("term-0"),
                String::from("term-1"),
                String::from("term-2"),
            ]
        );
    }

    #[test]
    fn embedding_hook_terms_are_merged_after_local_candidates() {
        let builder = VoiceContextBuilder::new();
        let input = VoiceContextInput {
            page_title: Some("Home".into()),
            ..VoiceContextInput::default()
        };
        let hook = StubEmbeddingHook;

        let built = builder.build(&input, Some(&hook));
        assert!(built.glossary_terms.contains(&"Home".into()));
        assert!(built.glossary_terms.contains(&"EmbeddingTerm".into()));
    }

    #[test]
    fn build_session_context_maps_to_protocol_shape() {
        let builder = VoiceContextBuilder::new();
        let input = VoiceContextInput {
            document_id: Some("doc-2".into()),
            page_title: Some("Quick Note".into()),
            ..VoiceContextInput::default()
        };

        let context = builder.build_session_context(&input, false, None);
        assert_eq!(context.document_id.as_deref(), Some("doc-2"));
        assert!(context.glossary_terms.contains(&"Quick Note".into()));
        assert!(!context.command_mode);
    }
}
