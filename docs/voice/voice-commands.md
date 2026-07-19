# Voice Commands

## Scope

Formatting and slash-command support without blocking basic dictation.
Decision: [adr/0006](./adr/0006-deterministic-voice-commands.md).

## Command registry integration

Extend the existing slash-command registry with voice metadata:

```rust
pub struct CommandDefinition {
    pub id: CommandId,
    pub slash_aliases: Vec<String>,
    pub voice_aliases: Vec<String>,
    pub argument_schema: CommandArgumentSchema,
    pub contexts: Vec<CommandContext>,
    pub risk: CommandRisk,
}
```

Today the desktop slash menu is still largely editor-local
(`PageEditor.tsx`). Voice work **should** drive unification toward a shared
registry that both slash UX and voice aliases consume, without inventing a
second command bus.

## Interaction modes

### Dictation mode

Most speech becomes text.

A small reserved grammar handles:

- New line
- New paragraph
- Undo that
- Delete last phrase
- Stop dictation

### Command mode

All speech is parsed as a command.

Activation options:

- Separate shortcut
- Toolbar button
- Modifier plus dictation shortcut

### Mixed mode

Require an explicit wake prefix:

- “Lattice command, heading two.”
- “Lattice command, turn this into a checklist.”

**Do not** attempt implicit command detection in the initial release.

## Command safety

```rust
enum CommandRisk {
    ReversibleEditorChange,
    DestructiveLocalChange,
    ExternalSideEffect,
    PrivilegedOperation,
}
```

Rules:

- Reversible formatting **may** execute immediately.
- Destructive actions **must** require confirmation.
- External operations **must** require confirmation.
- Voice **must not** directly execute arbitrary SQL, shell commands, MCP calls,
  or undeclared plugin actions.
- Voice **must not** bypass plugin capability grants
  ([ADR 0018](../decisions/0018-explicit-capabilities-and-proposed-writes.md)).

## Initial command grammar

Start with:

- New paragraph
- New line
- Heading one through heading three
- Bullet list
- Numbered list
- Checklist
- Quote
- Code block
- Bold selected text
- Italicize selected text
- Undo
- Redo
- Delete last phrase
- Stop dictation

Defer natural-language compound commands until the deterministic system is
stable.

## Security implications

Prompt-like content embedded in spoken prose **must not** escalate privilege.
Only registered aliases in the active mode may trigger actions.

## Testing requirements

- Grammar unit tests per alias
- Risk classification matrix
- False activation rate on prose fixtures
  ([observability-testing.md](./observability-testing.md))
- Confirmation gating for destructive commands

## Open questions

- Exact wake-prefix localization
- Argument parsing for slash commands with parameters (M7)

## Acceptance criteria

- [ ] Dictation mode reserved grammar is exhaustive and tested
- [ ] Implicit NL command detection is absent in v1
- [ ] Destructive/external voice commands always confirm
- [ ] False destructive activation rate is effectively zero on golden suite
