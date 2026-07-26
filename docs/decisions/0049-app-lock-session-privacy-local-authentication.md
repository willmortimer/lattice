# ADR 0049: App lock is session privacy via LocalAuthentication, not encryption

## Status

Accepted.

## Context

Users want a Touch ID (or device-password) gate when stepping away from an open
Lattice session. ADR 0038 describes biometric-gated **workspace encryption**
with DEK wrapping in the OS keychain. That work remains an open product
question and is much larger than a session privacy control.

Separately, future privileged-action and automation approval flows will need a
reusable user-presence prompt in trusted Rust code. Building app lock on that
primitive avoids a one-off Touch ID path that cannot grow into approvals.

## Decision

Ship **app lock** as:

1. A privacy overlay in the desktop shell while the session is locked.
2. A privileged IPC gate in the Tauri host (`app-locked` rejection) for
   workspace reads/mutations, agent, proposals, connectors, and similar
   commands.
3. A reusable `request_user_presence(reason)` helper backed by macOS
   LocalAuthentication (`objc2-local-authentication`), with Touch ID and
   device-password fallback.

Lock triggers when enabled: launch, manual Lock (menu / shortcut), idle
unfocus timeout (user-configurable minutes), and macOS sleep / screen sleep
when detectable.

Do **not** treat app lock as encryption. Workspace files remain ordinary
inspectable content on disk. CLI / `latticed` access outside the desktop
session is out of scope for this gate and must stay honest in product copy.

Keep this surface separate from ADR 0038. Encryption, if pursued later, still
owns DEK lifecycle and storage paths; it may call the same presence helper for
unlock UX but is not implied by enabling app lock.

## Consequences

- Secret handling for presence stays in Rust; no community biometric Tauri
  plugin enters the trust path.
- macOS production builds need proper code signing for reliable LocalAuthentication.
- Other platforms show app lock as unavailable until an equivalent presence
  backend exists.
- Future privileged approvals should call `request_user_presence` with a
  distinct reason string rather than inventing a second biometric path.
