# Editor Integration

## Scope

How dictation interacts with the Notion-like editor (Tiptap/ProseMirror),
document storage, undo, and future collaboration.
Decision: [adr/0005](./adr/0005-final-text-only-document-commit.md).

## Dictation anchor

At session start, create a logical anchor:

```rust
pub struct DictationAnchor {
    pub workspace_id: WorkspaceId,
    pub document_id: DocumentId,
    pub block_id: BlockId,
    pub logical_offset: usize,
    pub document_revision: RevisionId,
    pub selection: Option<TextRange>,
}
```

The anchor **must** be resolved through editor state rather than raw DOM
coordinates. Lattice page editing already centers on ProseMirror transactions
([ADR 0006](../decisions/0006-react-shell-specialized-renderers.md),
[ADR 0036](../decisions/0036-incremental-long-page-performance.md)).

## Provisional text

Provisional text **should** be represented as one of:

- An editor decoration
- A composition range
- A transient local-only overlay

It **must not**:

- Be written to Markdown or structured document storage
- Enter the undo stack
- Produce CRDT operations
- Sync to other clients
- Trigger automations
- Trigger indexing
- Be interpreted as a slash command

## Final transcript transaction

When final text arrives:

1. Resolve the current logical anchor.
2. Remove the provisional decoration.
3. Submit one editor transaction.
4. Preserve marks and block context where appropriate.
5. Add one coherent undo entry.
6. Trigger normal persistence and synchronization via the semantic command /
   page-update path ([ADR 0007](../decisions/0007-semantic-command-transaction-core.md)).
7. Record the transcript source in optional transaction metadata.

Frontend code **must not** become a privileged writer of workspace files.

## Concurrent editing

Future behavior for:

| Situation | Initial policy |
|-----------|----------------|
| Remote insertions before the anchor | Keep dictation on original logical block; resolve offset through document mapping when collaboration exists |
| Remote deletion of the target block | Pause final insertion; show recovery prompt |
| Local cursor movement during dictation | Allowed; does **not** retarget active dictation unless the user explicitly retargets |
| User edits inside provisional text | Discouraged; if it occurs, cancel or rebase policy TBD in M3 — prefer cancel with notice in v1 |
| Switching documents mid-session | Cancel or park session; do not insert into the wrong document |
| Multiple Lattice windows | Only one active dictation session per client initially |

## Paragraph handling

Spoken controls map to structured editor operations, not literal Markdown:

| Spoken control | Editor operation |
|----------------|------------------|
| “new line” | Soft break |
| “new paragraph” | Split block |
| “bullet” | Create or convert to bullet-list item |
| “checkbox” | Create or convert to task item |

Full grammar: [voice-commands.md](./voice-commands.md).

## Security implications

- Voice-driven edits inherit the same capability and trust checks as keyboard
  edits.
- Destructive voice commands require confirmation
  ([voice-commands.md](./voice-commands.md)).

## Testing requirements

- Anchor resolution after local edits near the insertion point
- Final replacement removes all provisional chrome
- One undo step reverses one utterance
- Target block deleted → recovery prompt
- Document switch does not leak insertion

## Open questions

- Exact ProseMirror decoration vs composition API choice
- Collaboration mapping when Yrs lands
- Behavior when user types into provisional range (research Q12 adjacent)

## Acceptance criteria

- [ ] Provisional text never reaches `apply_page_update` / storage
- [ ] Final utterance = one undoable transaction
- [ ] Deleted target shows recovery rather than silent drop
- [ ] Only one active session per client in v1
