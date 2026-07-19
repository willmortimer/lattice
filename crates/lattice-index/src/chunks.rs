use std::path::Path;

use lattice_core::ResourceFormatProfile;
use sha2::{Digest, Sha256};

/// Stable chunker identity included in every derived chunk id.
pub const CHUNKER_VERSION: &str = "lattice-chunker-v1";

/// Approximate lower bound for merged chunks (~250 tokens at ~4 chars/token).
const MIN_CHUNK_CHARS: usize = 1_000;
/// Target upper bound for merged chunks (~700 tokens).
const TARGET_MAX_CHUNK_CHARS: usize = 2_800;
/// Hard cap for a single emitted chunk (~1,000 tokens).
const HARD_MAX_CHUNK_CHARS: usize = 4_000;

/// One structural chunk ready for persistence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchChunkDraft {
    pub chunk_id: String,
    pub block_id: Option<String>,
    pub ordinal: u32,
    pub heading_path: Vec<String>,
    pub source_start_byte: u64,
    pub source_end_byte: u64,
    pub text: String,
    pub content_hash: String,
}

/// Build structural chunks for one indexed resource.
pub fn chunk_resource(
    workspace_id: &str,
    resource_path: &Path,
    text: &str,
    text_base_byte: usize,
    profile: ResourceFormatProfile,
    title: &str,
    tags: &[String],
) -> Vec<SearchChunkDraft> {
    let _ = (title, tags);
    let blocks = match profile {
        ResourceFormatProfile::Markdown => split_markdown_blocks(text),
        ResourceFormatProfile::PlainText | ResourceFormatProfile::Code => {
            split_plain_text_blocks(text)
        }
        _ => {
            if text.trim().is_empty() {
                Vec::new()
            } else {
                split_plain_text_blocks(text)
            }
        }
    };
    if blocks.is_empty() {
        return Vec::new();
    }

    let resource_key = crate::paths::path_key(resource_path);
    let mut heading_stack: Vec<(u8, String)> = Vec::new();
    let mut merged = Vec::new();
    let mut current: Option<MergedBlock> = None;

    for block in blocks {
        update_heading_stack(&mut heading_stack, &block);
        let heading_path = heading_stack
            .iter()
            .map(|(_, text)| text.clone())
            .collect::<Vec<_>>();
        let block_id = structural_block_id(&heading_path, &block);
        let piece = BlockPiece {
            block,
            heading_path,
            block_id,
        };

        match &mut current {
            Some(active) if can_merge(active, &piece) => {
                active.merge(piece);
            }
            Some(active) => {
                merged.push(active.clone());
                current = Some(MergedBlock::from_piece(piece));
            }
            None => current = Some(MergedBlock::from_piece(piece)),
        }
    }
    if let Some(active) = current {
        merged.push(active);
    }

    let merged = split_oversized(merged);

    merged
        .into_iter()
        .enumerate()
        .map(|(ordinal, block)| {
            let structural_key = block
                .block_id
                .clone()
                .unwrap_or_else(|| format!("ordinal:{ordinal}"));
            SearchChunkDraft {
                chunk_id: stable_chunk_id(workspace_id, &resource_key, &structural_key),
                block_id: block.block_id,
                ordinal: ordinal as u32,
                heading_path: block.heading_path,
                source_start_byte: (text_base_byte + block.start_byte) as u64,
                source_end_byte: (text_base_byte + block.end_byte) as u64,
                content_hash: content_hash(&block.text),
                text: block.text,
            }
        })
        .collect()
}

fn stable_chunk_id(workspace_id: &str, resource_path: &str, structural_key: &str) -> String {
    let mut hasher = Sha256::new();
    for part in [workspace_id, resource_path, structural_key, CHUNKER_VERSION] {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn content_hash(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    format!("sha256:{}", hex::encode(digest))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceBlock {
    kind: BlockKind,
    start_byte: usize,
    end_byte: usize,
    text: String,
    heading_level: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BlockKind {
    Heading,
    Paragraph,
    CodeFence,
    Table,
    List,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BlockPiece {
    block: SourceBlock,
    heading_path: Vec<String>,
    block_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MergedBlock {
    kind: BlockKind,
    start_byte: usize,
    end_byte: usize,
    text: String,
    heading_path: Vec<String>,
    block_id: Option<String>,
}

impl MergedBlock {
    fn from_piece(piece: BlockPiece) -> Self {
        Self {
            kind: piece.block.kind,
            start_byte: piece.block.start_byte,
            end_byte: piece.block.end_byte,
            text: piece.block.text,
            heading_path: piece.heading_path,
            block_id: piece.block_id,
        }
    }

    fn merge(&mut self, piece: BlockPiece) {
        if self.text.is_empty() {
            self.text = piece.block.text;
        } else {
            self.text.push_str("\n\n");
            self.text.push_str(&piece.block.text);
        }
        self.end_byte = piece.block.end_byte;
        self.block_id = None;
    }

    fn char_len(&self) -> usize {
        self.text.chars().count()
    }
}

fn can_merge(active: &MergedBlock, piece: &BlockPiece) -> bool {
    if active.heading_path != piece.heading_path {
        return false;
    }
    if matches!(active.kind, BlockKind::CodeFence | BlockKind::Table) {
        return false;
    }
    if matches!(piece.block.kind, BlockKind::CodeFence | BlockKind::Table | BlockKind::Heading) {
        return false;
    }
    let combined = active.char_len() + 2 + piece.block.text.chars().count();
    if combined > TARGET_MAX_CHUNK_CHARS {
        return false;
    }
    active.char_len() < MIN_CHUNK_CHARS || piece.block.text.chars().count() < MIN_CHUNK_CHARS
}

fn update_heading_stack(stack: &mut Vec<(u8, String)>, block: &SourceBlock) {
    if let BlockKind::Heading = block.kind {
        if let Some(level) = block.heading_level {
            while stack.last().is_some_and(|(existing, _)| *existing >= level) {
                stack.pop();
            }
            let heading_text = block
                .text
                .trim_start_matches('#')
                .trim()
                .to_string();
            stack.push((level, heading_text));
        }
    }
}

fn structural_block_id(heading_path: &[String], block: &SourceBlock) -> Option<String> {
    let path = if heading_path.is_empty() {
        "root".to_string()
    } else {
        heading_path.join("/")
    };
    let kind = match block.kind {
        BlockKind::Heading => "heading",
        BlockKind::Paragraph => "paragraph",
        BlockKind::CodeFence => "code",
        BlockKind::Table => "table",
        BlockKind::List => "list",
    };
    Some(format!("{path}|{kind}@{}", block.start_byte))
}

fn split_markdown_blocks(text: &str) -> Vec<SourceBlock> {
    let mut blocks = Vec::new();
    let mut index = 0;
    while index < text.len() {
        if let Some(block) = parse_code_fence(text, index) {
            blocks.push(block);
            index = blocks.last().unwrap().end_byte;
            continue;
        }
        if let Some((line_start, line_end)) = line_bounds(text, index) {
            let line = &text[line_start..line_end];
            if is_table_line(line) {
                if let Some(block) = parse_table(text, line_start) {
                    blocks.push(block);
                    index = blocks.last().unwrap().end_byte;
                    continue;
                }
            }
            if let Some(level) = heading_level(line) {
                blocks.push(SourceBlock {
                    kind: BlockKind::Heading,
                    start_byte: line_start,
                    end_byte: line_end,
                    text: line.to_string(),
                    heading_level: Some(level),
                });
                index = next_line_start(text, line_end);
                continue;
            }
            if is_list_line(line) {
                if let Some(block) = parse_list(text, line_start) {
                    blocks.push(block);
                    index = blocks.last().unwrap().end_byte;
                    continue;
                }
            }
            if line.trim().is_empty() {
                index = next_line_start(text, line_end);
                continue;
            }
            if let Some(block) = parse_paragraph(text, line_start) {
                blocks.push(block);
                index = blocks.last().unwrap().end_byte;
                continue;
            }
        }
        index += 1;
    }
    blocks
}

fn split_plain_text_blocks(text: &str) -> Vec<SourceBlock> {
    let mut blocks = Vec::new();
    let mut paragraph_start = 0;
    let mut index = 0;
    while index <= text.len() {
        let at_end = index == text.len();
        let paragraph_break = !at_end
            && text.as_bytes()[index] == b'\n'
            && text[index + 1..].starts_with('\n');
        if at_end || paragraph_break {
            let end = if at_end { text.len() } else { index };
            let slice = text[paragraph_start..end].trim();
            if !slice.is_empty() {
                let start_byte = paragraph_start + text[paragraph_start..end].find(slice).unwrap_or(0);
                let end_byte = start_byte + slice.len();
                blocks.push(SourceBlock {
                    kind: BlockKind::Paragraph,
                    start_byte,
                    end_byte,
                    text: slice.to_string(),
                    heading_level: None,
                });
            }
            if at_end {
                break;
            }
            index += 2;
            while index < text.len() && text.as_bytes()[index] == b'\n' {
                index += 1;
            }
            paragraph_start = index;
            continue;
        }
        index += 1;
    }
    if blocks.is_empty() && !text.trim().is_empty() {
        let trimmed = text.trim();
        let start_byte = text.find(trimmed).unwrap_or(0);
        blocks.push(SourceBlock {
            kind: BlockKind::Paragraph,
            start_byte,
            end_byte: start_byte + trimmed.len(),
            text: trimmed.to_string(),
            heading_level: None,
        });
    }
    blocks
}

fn parse_code_fence(text: &str, index: usize) -> Option<SourceBlock> {
    let (line_start, line_end) = line_bounds(text, index)?;
    let line = text[line_start..line_end].trim();
    if !line.starts_with("```") && !line.starts_with("~~~") {
        return None;
    }
    let marker = if line.starts_with("```") { "```" } else { "~~~" };
    let mut cursor = next_line_start(text, line_end);
    while cursor < text.len() {
        let (next_start, next_end) = line_bounds(text, cursor)?;
        let next_line = text[next_start..next_end].trim();
        if next_line.starts_with(marker) {
            return Some(SourceBlock {
                kind: BlockKind::CodeFence,
                start_byte: line_start,
                end_byte: next_end,
                text: text[line_start..next_end].to_string(),
                heading_level: None,
            });
        }
        cursor = next_line_start(text, next_end);
    }
    None
}

fn parse_table(text: &str, line_start: usize) -> Option<SourceBlock> {
    let mut cursor = line_start;
    let start_byte = line_start;
    while cursor < text.len() {
        let (row_start, row_end) = line_bounds(text, cursor)?;
        let row = &text[row_start..row_end];
        if row.trim().is_empty() {
            break;
        }
        if !is_table_line(row) {
            if row_start == start_byte {
                return None;
            }
            break;
        }
        cursor = next_line_start(text, row_end);
    }
    if cursor == start_byte {
        return None;
    }
    Some(SourceBlock {
        kind: BlockKind::Table,
        start_byte,
        end_byte: cursor,
        text: text[start_byte..cursor].trim_end().to_string(),
        heading_level: None,
    })
}

fn parse_list(text: &str, line_start: usize) -> Option<SourceBlock> {
    let mut cursor = line_start;
    let start_byte = line_start;
    while cursor < text.len() {
        let (row_start, row_end) = line_bounds(text, cursor)?;
        let row = &text[row_start..row_end];
        if row.trim().is_empty() {
            break;
        }
        if !is_list_line(row) && !row.starts_with("  ") && !row.starts_with('\t') {
            if row_start == start_byte {
                return None;
            }
            break;
        }
        cursor = next_line_start(text, row_end);
    }
    if cursor == start_byte {
        return None;
    }
    Some(SourceBlock {
        kind: BlockKind::List,
        start_byte,
        end_byte: cursor,
        text: text[start_byte..cursor].trim_end().to_string(),
        heading_level: None,
    })
}

fn parse_paragraph(text: &str, line_start: usize) -> Option<SourceBlock> {
    let mut cursor = line_start;
    let start_byte = line_start;
    while cursor < text.len() {
        let (row_start, row_end) = line_bounds(text, cursor)?;
        let row = &text[row_start..row_end];
        if row.trim().is_empty() {
            break;
        }
        if heading_level(row).is_some()
            || is_table_line(row)
            || is_list_line(row)
            || row.trim().starts_with("```")
            || row.trim().starts_with("~~~")
        {
            if row_start == start_byte {
                return None;
            }
            break;
        }
        cursor = next_line_start(text, row_end);
    }
    let slice = text[start_byte..cursor].trim_end();
    if slice.trim().is_empty() {
        return None;
    }
    let trimmed_start = start_byte + text[start_byte..cursor].find(slice.trim_start()).unwrap_or(0);
    let trimmed = slice.trim();
    Some(SourceBlock {
        kind: BlockKind::Paragraph,
        start_byte: trimmed_start,
        end_byte: trimmed_start + trimmed.len(),
        text: trimmed.to_string(),
        heading_level: None,
    })
}

fn line_bounds(text: &str, index: usize) -> Option<(usize, usize)> {
    if index > text.len() {
        return None;
    }
    let start = index;
    let end = text[index..]
        .find('\n')
        .map(|offset| index + offset)
        .unwrap_or(text.len());
    Some((start, end))
}

fn next_line_start(text: &str, line_end: usize) -> usize {
    if line_end < text.len() && text.as_bytes()[line_end] == b'\n' {
        line_end + 1
    } else {
        line_end
    }
}

fn heading_level(line: &str) -> Option<u8> {
    let hashes = line.chars().take_while(|ch| *ch == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = line[hashes..].trim();
    if rest.is_empty() {
        return None;
    }
    Some(hashes as u8)
}

fn is_table_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.contains('|')
}

fn is_list_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("- ")
        || trimmed.starts_with("* ")
        || trimmed.starts_with("+ ")
        || trimmed
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .count()
            > 0
            && trimmed.contains(". ")
}

fn split_oversized(blocks: Vec<MergedBlock>) -> Vec<MergedBlock> {
    let mut out = Vec::new();
    for block in blocks {
        if block.char_len() <= HARD_MAX_CHUNK_CHARS {
            out.push(block);
            continue;
        }
        out.extend(split_block_by_char_budget(block));
    }
    out
}

fn split_block_by_char_budget(block: MergedBlock) -> Vec<MergedBlock> {
    let mut parts = Vec::new();
    let mut cursor = 0;
    while cursor < block.text.len() {
        let remaining = &block.text[cursor..];
        if remaining.chars().count() <= HARD_MAX_CHUNK_CHARS {
            let start = cursor;
            let end = block.text.len();
            parts.push(merged_slice(&block, start, end, parts.len()));
            break;
        }
        let mut end = cursor;
        let mut char_count = 0;
        for (offset, _) in remaining.char_indices() {
            if char_count >= HARD_MAX_CHUNK_CHARS {
                break;
            }
            end = cursor + offset + remaining[offset..].chars().next().map(char::len_utf8).unwrap_or(0);
            char_count += 1;
        }
        if end <= cursor {
            end = block.text.len();
        } else {
            end = prefer_word_boundary(&block.text, end);
        }
        parts.push(merged_slice(&block, cursor, end, parts.len()));
        cursor = end;
        while cursor < block.text.len() && block.text[cursor..].starts_with('\n') {
            cursor += 1;
        }
    }
    parts
}

fn merged_slice(block: &MergedBlock, start: usize, end: usize, part_index: usize) -> MergedBlock {
    let text = block.text[start..end].trim().to_string();
    let start_byte = block.start_byte + start;
    let end_byte = start_byte + text.len();
    MergedBlock {
        kind: block.kind.clone(),
        start_byte,
        end_byte,
        text,
        heading_path: block.heading_path.clone(),
        block_id: block
            .block_id
            .as_ref()
            .map(|id| format!("{id}/part:{part_index}")),
    }
}

fn prefer_word_boundary(text: &str, index: usize) -> usize {
    if index >= text.len() {
        return text.len();
    }
    let slice = &text[..index];
    if let Some(pos) = slice.rfind(char::is_whitespace) {
        if pos > 0 {
            return pos;
        }
    }
    index
}

#[cfg(test)]
fn estimate_tokens(text: &str) -> usize {
    (text.chars().count() + 3) / 4
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const WORKSPACE_ID: &str = "019b0000-0000-7000-8000-000000000001";

    #[test]
    fn chunk_ids_are_stable_for_same_structure() {
        let text = "# Intro\n\nHello world.\n";
        let first = chunk_resource(
            WORKSPACE_ID,
            Path::new("Notes/Demo.md"),
            text,
            0,
            ResourceFormatProfile::Markdown,
            "Demo",
            &[],
        );
        let second = chunk_resource(
            WORKSPACE_ID,
            Path::new("Notes/Demo.md"),
            text,
            0,
            ResourceFormatProfile::Markdown,
            "Demo",
            &[],
        );
        assert_eq!(first.len(), second.len());
        assert_eq!(first[0].chunk_id, second[0].chunk_id);
        assert_eq!(first[0].content_hash, second[0].content_hash);
    }

    #[test]
    fn markdown_keeps_code_fence_intact() {
        let text = "# Code\n\n```rust\nfn main() {\n    println!(\"hi\");\n}\n```\n\nAfter.\n";
        let chunks = chunk_resource(
            WORKSPACE_ID,
            Path::new("code.md"),
            text,
            0,
            ResourceFormatProfile::Markdown,
            "Code",
            &[],
        );
        assert!(chunks.iter().any(|chunk| chunk.text.contains("```rust")));
        assert!(chunks.iter().any(|chunk| chunk.text.contains("fn main()")));
        assert!(chunks.iter().any(|chunk| chunk.text.contains("After.")));
    }

    #[test]
    fn markdown_table_stays_in_one_chunk() {
        let text = "# Data\n\n| Name | Value |\n| --- | --- |\n| alpha | 1 |\n| beta | 2 |\n";
        let chunks = chunk_resource(
            WORKSPACE_ID,
            Path::new("table.md"),
            text,
            0,
            ResourceFormatProfile::Markdown,
            "Data",
            &[],
        );
        let table_chunk = chunks
            .iter()
            .find(|chunk| chunk.text.contains("| alpha |"))
            .expect("table chunk");
        assert!(table_chunk.text.contains("| beta |"));
    }

    #[test]
    fn plain_text_paragraphs_merge_under_target_size() {
        let text = "First paragraph.\n\nSecond paragraph.\n";
        let chunks = chunk_resource(
            WORKSPACE_ID,
            Path::new("note.txt"),
            text,
            0,
            ResourceFormatProfile::PlainText,
            "note",
            &[],
        );
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("First paragraph."));
        assert!(chunks[0].text.contains("Second paragraph."));
    }

    #[test]
    fn heading_path_is_preserved() {
        let text = "# One\n\n## Two\n\nBody under two.\n";
        let chunks = chunk_resource(
            WORKSPACE_ID,
            Path::new("headings.md"),
            text,
            0,
            ResourceFormatProfile::Markdown,
            "Headings",
            &[],
        );
        let body = chunks
            .iter()
            .find(|chunk| chunk.text.contains("Body under two."))
            .expect("body chunk");
        assert_eq!(body.heading_path, vec!["One".to_string(), "Two".to_string()]);
    }

    #[test]
    fn source_byte_ranges_include_base_offset() {
        let text = "Body text.\n";
        let chunks = chunk_resource(
            WORKSPACE_ID,
            Path::new("offset.md"),
            text,
            42,
            ResourceFormatProfile::PlainText,
            "Offset",
            &[],
        );
        assert_eq!(chunks[0].source_start_byte, 42);
        assert_eq!(chunks[0].source_end_byte, 42 + "Body text.".len() as u64);
    }

    #[test]
    fn oversized_paragraphs_are_split() {
        let paragraph = "word ".repeat(900);
        let text = format!("{paragraph}\n\nTail paragraph.\n");
        let chunks = chunk_resource(
            WORKSPACE_ID,
            Path::new("large.md"),
            &text,
            0,
            ResourceFormatProfile::PlainText,
            "Large",
            &[],
        );
        assert!(chunks.len() >= 2);
        assert!(chunks.iter().all(|chunk| estimate_tokens(&chunk.text) <= 1_100));
    }
}
