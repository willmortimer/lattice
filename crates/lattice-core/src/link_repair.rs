use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::links::{MarkdownLinkKind, ResourceCatalog, ResourceLinkResolution, ResourceLinkTarget};

/// One indexed link occurrence in a source page, including a repairable span.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkOccurrence {
    pub source_path: PathBuf,
    pub kind: MarkdownLinkKind,
    pub raw_target: String,
    pub anchor: Option<String>,
    pub label: Option<String>,
    pub source_start_byte: usize,
    pub source_end_byte: usize,
    pub source_start_line: usize,
    pub source_start_column: usize,
    pub source_end_line: usize,
    pub source_end_column: usize,
}

/// Whether a repair candidate can be applied automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LinkRepairStatus {
    Resolved,
    Ambiguous,
    Skipped,
}

/// One proposed rewrite for an indexed link occurrence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkRepairCandidate {
    pub id: String,
    pub occurrence: LinkOccurrence,
    pub old_target: String,
    pub new_target: String,
    pub new_text: String,
    pub status: LinkRepairStatus,
    pub ambiguity: Option<Vec<ResourceLinkTarget>>,
}

/// Origin of a link-repair plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LinkRepairSource {
    LatticeRename,
    ExternalRename,
}

/// Reviewed plan for rewriting parseable links after a resource path change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkRepairPlan {
    pub id: String,
    pub rename_from: PathBuf,
    pub rename_to: PathBuf,
    pub source: LinkRepairSource,
    pub candidates: Vec<LinkRepairCandidate>,
    pub created_at: u64,
}

/// One from→to path change in a batch move/rename repair preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkRepairPathChange {
    pub from: PathBuf,
    pub to: PathBuf,
}

/// Soft warning threshold for batch repair candidate count (UI may warn).
pub const LINK_REPAIR_BATCH_CANDIDATE_WARN_THRESHOLD: usize = 200;

/// Hard cap: batch preview truncates candidates beyond this count.
pub const LINK_REPAIR_BATCH_CANDIDATE_HARD_CAP: usize = 500;

/// Combined repair plan for moving/renaming multiple resources in one review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchLinkRepairPlan {
    pub id: String,
    pub moves: Vec<LinkRepairPathChange>,
    pub source: LinkRepairSource,
    pub candidates: Vec<LinkRepairCandidate>,
    pub created_at: u64,
    /// Candidates dropped because their source page is also being moved in this
    /// batch (PageUpdate + ResourceRename on the same path is rejected).
    pub omitted_co_moved_count: usize,
    /// True when [`candidates`] was truncated to
    /// [`LINK_REPAIR_BATCH_CANDIDATE_HARD_CAP`].
    pub truncated: bool,
    /// Candidate count after co-moved filtering, before the hard cap.
    pub candidate_total_before_cap: usize,
}

impl BatchLinkRepairPlan {
    /// Whether the UI should warn that the repair set is large.
    pub fn warn_threshold_exceeded(&self) -> bool {
        self.candidate_total_before_cap >= LINK_REPAIR_BATCH_CANDIDATE_WARN_THRESHOLD
            || self.truncated
    }

    /// Projection used by single-plan review UIs (combined candidate list).
    pub fn as_link_repair_plan(&self) -> LinkRepairPlan {
        let (rename_from, rename_to) = self
            .moves
            .first()
            .map(|change| (change.from.clone(), change.to.clone()))
            .unwrap_or_else(|| (PathBuf::new(), PathBuf::new()));
        LinkRepairPlan {
            id: self.id.clone(),
            rename_from,
            rename_to,
            source: self.source,
            candidates: self.candidates.clone(),
            created_at: self.created_at,
        }
    }
}

/// True when `source` is one of the moved paths or nested under a moved folder.
pub fn path_is_co_moved(source: &Path, moved_froms: &[PathBuf]) -> bool {
    moved_froms
        .iter()
        .any(|from| source == from || source.starts_with(from))
}

/// Merge per-path repair plans into one batch plan: filter co-moved sources,
/// re-id candidates, and apply the hard candidate cap.
pub fn merge_batch_link_repair_plans(
    plan_id: impl Into<String>,
    created_at: u64,
    source: LinkRepairSource,
    plans: Vec<LinkRepairPlan>,
) -> BatchLinkRepairPlan {
    let plan_id = plan_id.into();
    let moves: Vec<LinkRepairPathChange> = plans
        .iter()
        .map(|plan| LinkRepairPathChange {
            from: plan.rename_from.clone(),
            to: plan.rename_to.clone(),
        })
        .collect();
    let moved_froms: Vec<PathBuf> = moves.iter().map(|change| change.from.clone()).collect();

    let mut omitted_co_moved_count = 0usize;
    let mut candidates = Vec::new();
    for plan in plans {
        for candidate in plan.candidates {
            if path_is_co_moved(&candidate.occurrence.source_path, &moved_froms) {
                omitted_co_moved_count += 1;
                continue;
            }
            candidates.push(candidate);
        }
    }

    let candidate_total_before_cap = candidates.len();
    let truncated = candidate_total_before_cap > LINK_REPAIR_BATCH_CANDIDATE_HARD_CAP;
    if truncated {
        candidates.truncate(LINK_REPAIR_BATCH_CANDIDATE_HARD_CAP);
    }
    for (index, candidate) in candidates.iter_mut().enumerate() {
        candidate.id = format!("{plan_id}-{index}");
    }

    BatchLinkRepairPlan {
        id: plan_id,
        moves,
        source,
        candidates,
        created_at,
        omitted_co_moved_count,
        truncated,
        candidate_total_before_cap,
    }
}

/// Summary of a persisted deferred repair proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkRepairProposalSummary {
    pub id: String,
    pub rename_from: PathBuf,
    pub rename_to: PathBuf,
    pub source: LinkRepairSource,
    pub candidate_count: usize,
    pub unresolved_count: usize,
    pub created_at: u64,
}

impl LinkRepairPlan {
    pub fn summary(&self) -> LinkRepairProposalSummary {
        LinkRepairProposalSummary {
            id: self.id.clone(),
            rename_from: self.rename_from.clone(),
            rename_to: self.rename_to.clone(),
            source: self.source,
            candidate_count: self.candidates.len(),
            unresolved_count: self
                .candidates
                .iter()
                .filter(|candidate| candidate.status != LinkRepairStatus::Resolved)
                .count(),
            created_at: self.created_at,
        }
    }
}

/// Apply byte-span replacements from the end of `content` so earlier offsets
/// remain valid while multiple links in one page are rewritten.
pub fn apply_span_replacements(
    content: &str,
    replacements: &[(usize, usize, &str)],
) -> Option<String> {
    let mut output = content.to_string();
    let mut ordered = replacements.to_vec();
    ordered.sort_by(|left, right| right.0.cmp(&left.0));
    for (start, end, text) in ordered {
        if start > end || end > output.len() || !output.is_char_boundary(start) || !output.is_char_boundary(end) {
            return None;
        }
        output.replace_range(start..end, text);
    }
    Some(output)
}

/// Build the replacement link text, preserving wiki vs markdown syntax,
/// anchors, and labels.
pub fn format_link_text(
    kind: MarkdownLinkKind,
    target: &str,
    anchor: Option<&str>,
    label: Option<&str>,
) -> String {
    let anchor_suffix = anchor
        .filter(|value| !value.is_empty())
        .map(|value| format!("#{value}"))
        .unwrap_or_default();
    match kind {
        MarkdownLinkKind::Wiki => match label.filter(|value| !value.is_empty()) {
            Some(display) => format!("[[{target}{anchor_suffix}|{display}]]"),
            None => format!("[[{target}{anchor_suffix}]]"),
        },
        MarkdownLinkKind::Markdown => {
            let display = label.filter(|value| !value.is_empty()).unwrap_or(target);
            format!("[{display}]({target}{anchor_suffix})")
        }
    }
}

/// Compute the raw target string that should appear inside a link after a
/// rename, preserving wiki vs markdown conventions and relative paths.
pub fn rewrite_link_target(
    source_path: &Path,
    old_raw_target: &str,
    old_resolved: &Path,
    new_resolved: &Path,
    kind: MarkdownLinkKind,
) -> String {
    match kind {
        MarkdownLinkKind::Wiki => rewrite_wiki_target(old_raw_target, old_resolved, new_resolved),
        MarkdownLinkKind::Markdown => relative_markdown_target(source_path, new_resolved),
    }
}

fn rewrite_wiki_target(old_raw: &str, old_resolved: &Path, new_resolved: &Path) -> String {
    let (target, _) = split_anchor(old_raw);
    if target.contains('/') {
        let old_key = path_key(old_resolved);
        let _new_key = path_key(new_resolved);
        if path_key(Path::new(target)) == old_key {
            return wiki_path_for(new_resolved);
        }
        if let Some(suffix) = path_key(Path::new(target)).strip_prefix(&format!("{old_key}#")) {
            return format!("{}#{}", wiki_path_for(new_resolved), suffix);
        }
        wiki_path_for(new_resolved)
    } else {
        wiki_stem_for(new_resolved)
    }
}

fn wiki_path_for(path: &Path) -> String {
    let key = path_key(path);
    key.strip_suffix(".md")
        .or_else(|| key.strip_suffix(".markdown"))
        .unwrap_or(&key)
        .to_string()
}

fn wiki_stem_for(path: &Path) -> String {
    path.file_stem()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| wiki_path_for(path))
}

fn relative_markdown_target(source_path: &Path, new_resolved: &Path) -> String {
    let source_dir = source_path.parent().unwrap_or_else(|| Path::new(""));
    let relative = relative_path(source_dir, new_resolved).unwrap_or_else(|| path_key(new_resolved));
    if !relative.starts_with('.') && !relative.starts_with('/') {
        format!("./{relative}")
    } else {
        relative
    }
}

/// Whether a catalog resolution refers to `path`, including ambiguous basename matches.
pub fn resolution_targets_path(resolution: &ResourceLinkResolution, path: &Path) -> bool {
    match resolution {
        ResourceLinkResolution::Found { target, .. } => Path::new(&target.path) == path,
        ResourceLinkResolution::Ambiguous { candidates, .. } => candidates
            .iter()
            .any(|candidate| Path::new(&candidate.path) == path),
        ResourceLinkResolution::Missing { .. } => false,
    }
}

fn relative_path(from_dir: &Path, to: &Path) -> Option<String> {
    let from = normalize_relative(from_dir)?;
    let to = normalize_relative(to)?;
    let mut shared = 0;
    for (left, right) in from.components().zip(to.components()) {
        if left != right {
            break;
        }
        shared += 1;
    }
    let mut output = PathBuf::new();
    for _ in shared..from.components().count() {
        output.push("..");
    }
    for component in to.components().skip(shared) {
        output.push(component);
    }
    if output.as_os_str().is_empty() {
        output.push(
            to.file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("")),
        );
    }
    Some(path_key(&output))
}

fn normalize_relative(path: &Path) -> Option<PathBuf> {
    let mut output = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => output.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if !output.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(output)
}

fn split_anchor(raw: &str) -> (&str, Option<String>) {
    match raw.split_once('#') {
        Some((target, anchor)) => (
            target.trim(),
            (!anchor.trim().is_empty()).then(|| anchor.trim().to_string()),
        ),
        None => (raw.trim(), None),
    }
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Build one repair candidate for an occurrence that currently resolves to
/// `rename_from` and should resolve to `rename_to` after the rename.
pub fn build_repair_candidate(
    catalog: &ResourceCatalog,
    occurrence: LinkOccurrence,
    rename_from: &Path,
    rename_to: &Path,
    id: &str,
) -> LinkRepairCandidate {
    let old_target = occurrence.raw_target.clone();
    let resolution = catalog.resolve(
        Some(&occurrence.source_path),
        &occurrence.raw_target,
    );
    if !resolution_targets_path(&resolution, rename_from) {
        return LinkRepairCandidate {
            id: id.to_string(),
            occurrence,
            old_target: old_target.clone(),
            new_target: old_target,
            new_text: String::new(),
            status: LinkRepairStatus::Skipped,
            ambiguity: None,
        };
    }

    let new_target = rewrite_link_target(
        &occurrence.source_path,
        &occurrence.raw_target,
        rename_from,
        rename_to,
        occurrence.kind,
    );
    let new_resolution = catalog.resolve(Some(&occurrence.source_path), &new_target);
    let (status, ambiguity) = match new_resolution {
        ResourceLinkResolution::Found { .. } => (LinkRepairStatus::Resolved, None),
        ResourceLinkResolution::Ambiguous { candidates, .. } => {
            (LinkRepairStatus::Ambiguous, Some(candidates))
        }
        ResourceLinkResolution::Missing { .. } => (LinkRepairStatus::Resolved, None),
    };
    let new_text = format_link_text(
        occurrence.kind,
        &new_target,
        occurrence.anchor.as_deref(),
        occurrence.label.as_deref(),
    );
    LinkRepairCandidate {
        id: id.to_string(),
        occurrence,
        old_target,
        new_target,
        new_text,
        status,
        ambiguity,
    }
}

/// Build a reviewed repair plan from indexed occurrences and a rename pair.
pub fn build_link_repair_plan(
    catalog: &ResourceCatalog,
    occurrences: Vec<LinkOccurrence>,
    rename_from: PathBuf,
    rename_to: PathBuf,
    source: LinkRepairSource,
    plan_id: impl Into<String>,
    created_at: u64,
) -> LinkRepairPlan {
    let plan_id = plan_id.into();
    let candidates = occurrences
        .into_iter()
        .enumerate()
        .map(|(index, occurrence)| {
            build_repair_candidate(
                catalog,
                occurrence,
                &rename_from,
                &rename_to,
                &format!("{plan_id}-{index}"),
            )
        })
        .filter(|candidate| candidate.status != LinkRepairStatus::Skipped)
        .collect();
    LinkRepairPlan {
        id: plan_id,
        rename_from,
        rename_to,
        source,
        candidates,
        created_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Resource, ResourceKind};

    fn catalog() -> ResourceCatalog {
        ResourceCatalog::new(&[
            Resource {
                path: "Notes/Home.md".into(),
                kind: ResourceKind::Page,
            },
            Resource {
                path: "Notes/Other.md".into(),
                kind: ResourceKind::Page,
            },
            Resource {
                path: "Notes/Renamed.md".into(),
                kind: ResourceKind::Page,
            },
            Resource {
                path: "Archive/Other.md".into(),
                kind: ResourceKind::Page,
            },
        ])
    }

    #[test]
    fn preserves_wiki_piped_and_anchor_syntax() {
        let occurrence = LinkOccurrence {
            source_path: "Notes/Home.md".into(),
            kind: MarkdownLinkKind::Wiki,
            raw_target: "Other".into(),
            anchor: Some("start".into()),
            label: Some("display".into()),
            source_start_byte: 0,
            source_end_byte: 1,
            source_start_line: 1,
            source_start_column: 1,
            source_end_line: 1,
            source_end_column: 1,
        };
        let candidate = build_repair_candidate(
            &catalog(),
            occurrence,
            Path::new("Notes/Other.md"),
            Path::new("Notes/Renamed.md"),
            "c1",
        );
        assert_eq!(candidate.new_text, "[[Renamed#start|display]]");
        assert_eq!(candidate.status, LinkRepairStatus::Resolved);
    }

    #[test]
    fn preserves_markdown_relative_syntax() {
        let occurrence = LinkOccurrence {
            source_path: "Notes/Home.md".into(),
            kind: MarkdownLinkKind::Markdown,
            raw_target: "./Other.md".into(),
            anchor: Some("body".into()),
            label: Some("other".into()),
            source_start_byte: 0,
            source_end_byte: 1,
            source_start_line: 1,
            source_start_column: 1,
            source_end_line: 1,
            source_end_column: 1,
        };
        let candidate = build_repair_candidate(
            &catalog(),
            occurrence,
            Path::new("Notes/Other.md"),
            Path::new("Notes/Renamed.md"),
            "c1",
        );
        assert_eq!(candidate.new_text, "[other](./Renamed.md#body)");
    }

    #[test]
    fn marks_ambiguous_basenames_without_auto_rewrite() {
        let occurrence = LinkOccurrence {
            source_path: "Notes/Home.md".into(),
            kind: MarkdownLinkKind::Wiki,
            raw_target: "Other".into(),
            anchor: None,
            label: None,
            source_start_byte: 0,
            source_end_byte: 1,
            source_start_line: 1,
            source_start_column: 1,
            source_end_line: 1,
            source_end_column: 1,
        };
        let candidate = build_repair_candidate(
            &catalog(),
            occurrence,
            Path::new("Notes/Other.md"),
            Path::new("Archive/Other.md"),
            "c1",
        );
        assert_eq!(candidate.status, LinkRepairStatus::Ambiguous);
        assert!(candidate.ambiguity.is_some());
    }

    #[test]
    fn span_replacements_apply_from_end_of_file() {
        let content = "[[A]] then [[B]]";
        let updated = apply_span_replacements(
            content,
            &[(11, 16, "[[C]]"), (0, 5, "[[D]]")],
        )
        .unwrap();
        assert_eq!(updated, "[[D]] then [[C]]");
    }

    fn candidate(id: &str, source: &str, start: usize, end: usize) -> LinkRepairCandidate {
        LinkRepairCandidate {
            id: id.into(),
            occurrence: LinkOccurrence {
                source_path: source.into(),
                kind: MarkdownLinkKind::Wiki,
                raw_target: "X".into(),
                anchor: None,
                label: None,
                source_start_byte: start,
                source_end_byte: end,
                source_start_line: 1,
                source_start_column: 1,
                source_end_line: 1,
                source_end_column: 2,
            },
            old_target: "X".into(),
            new_target: "Y".into(),
            new_text: "[[Y]]".into(),
            status: LinkRepairStatus::Resolved,
            ambiguity: None,
        }
    }

    #[test]
    fn merge_batch_plans_omits_co_moved_sources_and_reids() {
        let plans = vec![
            LinkRepairPlan {
                id: "a".into(),
                rename_from: "Notes/A.md".into(),
                rename_to: "Archive/A.md".into(),
                source: LinkRepairSource::LatticeRename,
                created_at: 1,
                candidates: vec![
                    candidate("a-0", "Notes/Home.md", 0, 5),
                    candidate("a-1", "Notes/B.md", 0, 5),
                ],
            },
            LinkRepairPlan {
                id: "b".into(),
                rename_from: "Notes/B.md".into(),
                rename_to: "Archive/B.md".into(),
                source: LinkRepairSource::LatticeRename,
                created_at: 1,
                candidates: vec![candidate("b-0", "Notes/Home.md", 10, 15)],
            },
        ];
        let batch = merge_batch_link_repair_plans(
            "batch",
            2,
            LinkRepairSource::LatticeRename,
            plans,
        );
        assert_eq!(batch.moves.len(), 2);
        assert_eq!(batch.omitted_co_moved_count, 1);
        assert_eq!(batch.candidates.len(), 2);
        assert_eq!(batch.candidates[0].id, "batch-0");
        assert_eq!(batch.candidates[1].id, "batch-1");
        assert!(!batch.truncated);
        assert_eq!(batch.candidate_total_before_cap, 2);
    }

    #[test]
    fn merge_batch_plans_applies_hard_cap() {
        let mut candidates = Vec::new();
        for index in 0..(LINK_REPAIR_BATCH_CANDIDATE_HARD_CAP + 3) {
            candidates.push(candidate(
                &format!("c-{index}"),
                "Notes/Home.md",
                index * 10,
                index * 10 + 5,
            ));
        }
        let plans = vec![LinkRepairPlan {
            id: "a".into(),
            rename_from: "Notes/A.md".into(),
            rename_to: "Archive/A.md".into(),
            source: LinkRepairSource::LatticeRename,
            created_at: 1,
            candidates,
        }];
        let batch = merge_batch_link_repair_plans(
            "batch",
            1,
            LinkRepairSource::LatticeRename,
            plans,
        );
        assert!(batch.truncated);
        assert_eq!(batch.candidates.len(), LINK_REPAIR_BATCH_CANDIDATE_HARD_CAP);
        assert_eq!(
            batch.candidate_total_before_cap,
            LINK_REPAIR_BATCH_CANDIDATE_HARD_CAP + 3
        );
        assert!(batch.warn_threshold_exceeded());
    }
}
