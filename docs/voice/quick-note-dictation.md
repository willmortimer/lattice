# Quick Note Dictation

## Scope

Highly responsive global dictation that creates a note without requiring a
focused editor window. Depends on warm model residency and daemon ownership
before ship ([architecture.md](./architecture.md) recommendation #3).

## User flow

```text
Hold Quick Note shortcut
    ↓
Overlay appears
    ↓
Pre-roll and live audio start streaming
    ↓
Provisional text appears
    ↓
Release shortcut
    ↓
Offline final decode
    ↓
Note is created atomically
    ↓
Overlay confirms save
```

## Startup target

The overlay **should** appear before the speech model is fully ready.

Possible states:

- Listening
- Preparing local voice model
- Transcribing
- Finalizing
- Saved to Inbox

Cold-model behavior: remain interactive; show preparation immediately; may
capture audio into a bounded buffer while the model loads
([performance-budget.md](./performance-budget.md)).

## Model residency

Settings:

| Mode | Behavior |
|------|----------|
| Always warm when Quick Note is enabled | Lowest latency; highest memory |
| Warm while Lattice is running | Default bias for responsiveness |
| Unload after idle timeout | Balance |
| Load only on demand | Lowest memory; highest first-use latency |
| Automatically unload under memory pressure | Always available as a safety valve |

The default **should** favor responsiveness while clearly communicating memory
use in Settings / Inspect.

## Destinations

Support:

- Inbox
- Daily note
- Last active document
- Configured workspace page
- New standalone note

Store the destination as a **local preference** (profile settings; see
[ADR 0032](../decisions/0032-versioned-profile-settings-and-operational-state.md)).

Atomic creation **must** go through the semantic command core so CLI and UI
share the same mutation path.

## Failure recovery

If note persistence fails:

1. Keep the final transcript in the overlay.
2. Copy it into a local recovery queue.
3. Retry after the database / workspace becomes available.
4. **Never** discard successfully recognized text.

Raw audio retention remains off by default
([privacy-security.md](./privacy-security.md)).

## Native helper

Prefer main app menu-bar/background mode unless measured otherwise
([macos-integration.md](./macos-integration.md)). Enable via desktop setting
`services.keepAppInMenuBar` (tray residency while the process is already
running — not a login-item helper).

## Security implications

- Global shortcut must not grant ambient authority beyond creating the
  configured note destination.
- Recovery queue stores **text**, not audio, unless the user opts into
  recording features separately.

## Testing requirements

- Overlay before model ready
- Atomic note create
- Persistence failure → recovery queue → retry
- Destination preference honored
- Memory-pressure unload during idle Quick Note enabled

## Open questions

- Helper vs main app (research Q14)
- Model ownership before Quick Note (research Q13) — **must** be latticed before ship

## Acceptance criteria

- [ ] Shortcut → overlay < 100 ms when app is background-capable (warm)
- [ ] Note create is atomic and undoable/inspectable as a normal resource
- [ ] Recognized text never discarded on persistence failure
- [ ] Works offline with no cloud account
