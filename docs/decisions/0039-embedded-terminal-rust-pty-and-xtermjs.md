# ADR 0039: Embedded terminal uses Rust PTY and xterm.js

## Status

Accepted.

## Context

Lattice wants an embedded shell/terminal surface for local-first compute and
developer workflows. Tauri’s first-party `shell` plugin only spawns child
processes; it is not a terminal emulator. There is no official Tauri PTY or
terminal-embed plugin. Community plugins such as `tauri-plugin-pty` exist but
are immature and would become a trust and maintenance dependency for a
security-sensitive surface.

ADR 0006 already requires specialized renderers to own hot paths rather than
routing per-keystroke or per-byte I/O through React.

## Decision

Build the embedded terminal as a first-party Lattice capability:

- **Rust** owns the pseudo-terminal using a mature PTY library such as
  `portable-pty` (WezTerm). Spawn, resize, signal, cwd, env allowlists, and
  process lifecycle live behind semantic commands and capability checks.
- **xterm.js** (or a compatible terminal renderer) owns display, input, and
  scrollback in the desktop shell. React mounts and lifecycles the host; it
  does not mediate the byte stream.
- Stream PTY I/O over Tauri events or an equivalent coarse channel. Do not
  turn every chunk into a JSON object pile, and do not rely on
  `tauri-plugin-shell` as the embed implementation.
- Do **not** adopt a third-party Tauri PTY plugin as the architecture of
  record. Vendoring a thin pattern is acceptable; outsourcing trust is not.

Default cwd is the open workspace root unless a narrower capability grants
another path. Workspace trust and process permissions gate spawn. Browser
demo mode does not imply native PTY authority.

## Consequences

- Terminal fits the same ownership model as pages (ProseMirror), canvas
  (Pixi), and grids: React coordinates; a specialized surface owns the hot
  loop.
- Product scope must keep the terminal behind progressive disclosure (not in
  the primary Page / Canvas / Table / Notebook / File creation vocabulary
  until explicitly enabled).
- Security docs and capability grants must treat PTY spawn like other
  process execution: no ambient shell authority for plugins or untrusted
  workspaces.
- Tests should cover spawn failure, resize, cancellation/kill, and denial
  under untrusted workspace defaults.
