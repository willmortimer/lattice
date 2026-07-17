# Workspace context

This file orients humans and agents working inside this Second Brain workspace.

## Purpose

Support **local-first knowledge work**: capture quickly, distill into atomic linked notes, and navigate with maps of content. The canonical graph lives in `Notes/`; `MOCs/` orients; `Sources/` holds citable references.

## Conventions

- **One idea per note** in `Notes/`. Prefer concept titles over dates.
- **Wiki-link liberally** — e.g. link to [[MOCs/Knowledge Base]] and peer notes under `Notes/`.
- **Inbox is ephemeral** — triage into `Notes/`, `Sources/`, or delete.
- **Do not reorganize by agent fiat** — suggest links and splits; let the human commit structure.
- **Citations** belong in `Sources/`; summaries and claims live in `Notes/`.

## Key entry points

- Map of content: `MOCs/Knowledge Base.md`
- Workflow: `Notes/Capture Workflow.md`
- Linking model: `Notes/Linking Philosophy.md`
- Prompt library: `Prompts/`

## Agent behavior

1. Read `MOCs/Knowledge Base.md` before large edits.
2. Preserve existing links; add missing cross-links when content overlaps.
3. Keep outputs inspectable Markdown on disk — no hidden indexes.
4. When synthesizing, cite source pages in `Sources/` when available.
5. Use prompts in `Prompts/` as templates; do not overwrite them without asking.

## Tone

Clear, concise, and exploratory. Favor questions that sharpen notes over bulk rewrites.
