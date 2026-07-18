# ADR 0038: Workspace encryption uses OS keychain and LocalAuthentication in Rust

## Status

Accepted.

## Context

Lattice may need optional encrypted workspaces (or encrypted packages within a
workspace) so that offline-first content is protected at rest and unlock can
require biometrics or device credentials (for example macOS Touch ID).

Tauri’s first-party options do not fit desktop:

- `tauri-plugin-biometric` supports Android and iOS only, not macOS desktop.
- `tauri-plugin-stronghold` is deprecated and will not ship in Tauri v3.
- An official OS keychain / secure-storage plugin is planned but not shipped.

Community biometric or Secure Enclave plugins would add third-party trust into
the secret path. Swift plugins are first-class on iOS only; desktop Tauri is a
Rust host, so Swift would require FFI or a sidecar rather than an official
plugin surface.

Workspace encryption must still respect Lattice invariants: mutations flow
through the semantic command core, the webview is not a privileged writer, and
unlocked content should remain a real directory inspectable outside Lattice
whenever the chosen encryption mode allows it.

## Decision

Implement biometric-gated and keychain-backed workspace encryption **in-repo
in Rust**, not via third-party Tauri plugins and not in the React shell.

On Apple platforms, prefer Apple frameworks through Rust bindings such as
`objc2-local-authentication` plus Security / Keychain APIs (or the
`keyring` / `keyring-core` ecosystem where it maps cleanly to the same OS
stores). Other desktops use the platform credential store equivalents.

Cryptographic shape:

1. Generate a workspace (or package) data-encryption key (DEK) in Rust.
2. Wrap and store the DEK in the OS keychain / credential store, with
   user-presence or biometric policy where the platform supports it.
3. Encrypt and decrypt content only in Rust storage / command paths.
4. Expose unlock, lock, and status through semantic commands and thin Tauri
   adapters. The webview never holds long-lived DEKs or performs encryption.

Do **not** treat biometrics as encryption. LocalAuthentication (or Windows
Hello / equivalent) gates access to the wrapped key; encryption is ordinary
authenticated cryptography under the DEK.

Prefer encryption modes that, after unlock, restore or expose an inspectable
workspace directory (or clearly scoped encrypted packages) rather than making
the webview the only reader of ciphertext. True Secure Enclave-resident keys
may be adopted later for key wrapping where entitlements and code signing
allow; they are not required for the first encryption provider.

## Consequences

- Secret handling stays in trusted Rust code with no community plugin as the
  root of trust.
- macOS production builds need proper code signing for reliable Keychain and
  biometric behavior; unsigned or ad-hoc builds may fail keychain access.
- Encrypted-at-rest and open-native inspectability are in tension while a
  workspace is locked; product copy and Inspect surfaces must state the mode
  honestly.
- Sync and CLI must learn lock/unlock preconditions rather than assuming
  plaintext files are always readable.
- Future official Tauri keychain plugins may wrap the same OS stores, but
  Lattice should keep DEK lifecycle and encryption inside domain crates.
