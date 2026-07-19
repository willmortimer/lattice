# ADR 0032: Separate versioned profile settings from operational state

## Status

Accepted.

## Decision

The native desktop profile lives under `~/Lattice`:

```text
Settings/appearance.yaml
Settings/desktop.yaml
Settings/workspaces.yaml
State/desktop.sqlite
```

Human-editable, low-frequency preferences use versioned YAML with defaults,
optimistic revisions, migrations, atomic replacement, and visible diagnostics.
Invalid sources remain untouched until an explicit save or reset, which first
preserves a backup. Frequently changing recents, sessions, window state, and
sidebar state use SQLite.

Native WebView `localStorage` is limited to the first-paint theme mirror (including
separate dark/light variants when appearance mode is auto). The
browser demo may use separate fixture state. Existing native localStorage keys
are imported once and removed only after profile persistence succeeds.

## Consequences

Preferences are inspectable and portable without turning high-frequency UI
state into write-heavy YAML. Startup degrades to defaults when a settings file
is malformed or newer than the running application, and explains the fallback
without blocking access to workspaces.
