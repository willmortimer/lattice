# macOS Integration

## Scope

macOS-specific application concerns for Lattice voice dictation. Shared
architecture remains in [architecture.md](./architecture.md). Capture format
is in [audio-capture.md](./audio-capture.md).

## Supported platform baseline

| Item | Requirement |
|------|-------------|
| Minimum macOS version | **Open** — set by Milestone 0 after measuring Parakeet/FluidAudio on oldest target M-series Mac. Provisional floor: macOS 14 (Sonoma) unless spike proves otherwise. |
| Apple Silicon | Required for the FluidAudio / Core ML provider |
| Intel Macs | **Unsupported** for the FluidAudio provider in v1. May later receive a separate fallback provider; must not silently degrade to cloud ASR. |
| Xcode / Swift | Version pinned in CI and `lattice-voice-macos` docs after M0 |
| Entitlements | Microphone; may need audio-input related sandbox exceptions as measured |
| Sandboxing | App Sandbox assumptions must be validated for CoreAudio capture and model filesystem access under Application Support |
| Signing / notarization | Developer ID signing required for distribution; native bridge dylibs / frameworks must be signed consistently with the app |

## Microphone permissions

### Usage description

`NSMicrophoneUsageDescription` **must** explain that Lattice uses the microphone
for local dictation and that audio is processed on-device.

### Who requests access

**Recommended decision** ([adr/0004](./adr/0004-client-owned-audio-capture.md)):

- Only the Tauri application or Quick Note helper requests microphone access.
- `latticed` receives already-captured PCM and **must not** access audio hardware.

### When to request

- Prefer requesting on first explicit Voice Dictation setup or first dictation
  activation, not at cold app launch.
- Setup UI **must** show model size, license, and privacy summary before
  download ([licensing-distribution.md](./licensing-distribution.md)).

### Denied / restricted states

UI **must** distinguish:

| State | User-visible behavior |
|-------|------------------------|
| Not determined | Prompt on first activation |
| Denied | Inline explanation + “Open System Settings” affordance |
| Restricted | Clear non-recoverable messaging (managed device) |
| Granted | Normal dictation chrome |

### Permission changes while running

If permission is revoked mid-session:

1. Cancel the active voice session.
2. Stop capture immediately.
3. Discard buffered audio for incomplete utterances (privacy default).
4. Show a non-blocking error; do not crash the editor.

### latticed and mic access

`latticed` **must not** declare microphone usage. If a future helper captures
audio, that helper owns the permission prompt.

## Application lifecycle

| Event | Required behavior |
|-------|-------------------|
| Primary window closes | Dictation UI may hide; if menu-bar mode is enabled, capture/session policy follows Quick Note residency settings |
| Lattice remains in menu bar | Warm-model policy may keep inference loaded ([quick-note-dictation.md](./quick-note-dictation.md)) |
| System sleep | Pause or cancel active sessions; flush or discard per privacy policy; do not leave the mic open across sleep without user intent |
| Audio device disconnected | Fail the session with a recoverable error; allow device reselection |
| Default microphone changes | Apply on next session unless the user pinned a device |
| Model still loading | Allow overlay/UI to show Preparing; may buffer bounded pre-roll/audio ([performance-budget.md](./performance-budget.md)) |
| Daemon restarts mid-session | Client cancels or recovers per [daemon-protocol.md](./daemon-protocol.md); never commit partial provisional text |
| Memory pressure | Unload warm model when possible; cancel lowest-priority sessions first |
| User switches accounts | Tear down sessions; do not reuse another user’s model cache paths |

### Menu-bar residency preference

Desktop setting `services.keepAppInMenuBar` (Settings → Performance & lifecycle)
controls main-window close behavior:

- **On:** closing the main window **hides** it; the process stays resident with a
  tray menu (Show Lattice, Quick Note, Quit). The dock icon remains (not
  `LSUIElement` accessory-only mode). Global Quick Note shortcuts keep working
  while the main window is hidden.
- **Off:** closing the main window quits the app.
- **Not a login item:** this preference does not install a Launch Agent or
  helper that starts Lattice at login. Use the tray Quit item for a full exit.

Related: `services.keepServicesRunning` leaves `latticed` up after the last
client disconnects (daemon warm residency), independent of the tray preference.

## Global shortcut behavior

Configurable shortcuts **must** be separate:

| Action | Notes |
|--------|-------|
| Hold to dictate into current editor | Push-to-talk |
| Toggle continuous dictation | Latch on/off |
| Hold to create a Quick Note | Global overlay |
| Enter command mode | Deterministic command grammar ([voice-commands.md](./voice-commands.md)) |

### Collision detection

- Detect registration failure when another application owns the shortcut.
- Surface a settings warning with the conflicting shortcut identity when the OS
  exposes it; otherwise show a generic “shortcut unavailable” state.
- Do not silently fall back to an unbound shortcut.

## Native helper strategy

Options for global Quick Note:

1. Main Tauri application running in menu-bar / background mode
2. Lightweight login-item helper
3. Menu-bar-only companion process

**Initial recommendation:** use the main application in menu-bar/background mode
unless Milestone 0 / M5 measurements show startup time or lifecycle constraints
that justify a helper.

## Security implications

- Entitlements and sandbox exceptions **must** be minimal and documented.
- Signed bridge binaries **must** match the ABI version expected by Rust
  ([fluid-audio-bridge.md](./fluid-audio-bridge.md)).

## Testing requirements

- Permission denied / restricted / revoked mid-session
- Device disconnect during push-to-talk
- Shortcut collision
- Sleep/wake during continuous dictation
- Menu-bar-only Quick Note without a focused editor window

## Open questions

- Exact minimum macOS version (research Q1 / M0)
- Whether Core ML compilation artifacts survive app updates (research Q8)
- Helper necessity for reliable Quick Note (research Q14)

## Acceptance criteria

- [ ] Microphone permission copy and Settings deep-link ship before first dictation beta
- [ ] latticed never requests mic permission
- [ ] Shortcut matrix is configurable and collision-visible
- [ ] Lifecycle table is implemented for sleep, device loss, and permission revoke
