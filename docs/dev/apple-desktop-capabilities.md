# Apple desktop capabilities (client)

## Portal inventory

| Kind | Identifier |
| --- | --- |
| Mac app | `dev.lattice.desktop` |
| Quick Look appex | `dev.lattice.desktop.quicklook` |
| App Group | `group.dev.lattice.shared` |
| Keychain access group | `PQNKMDU3U3.group.dev.lattice.shared` |
| SIWA Services ID | `dev.lattice.web` |
| Team (Developer ID) | `PQNKMDU3U3` |

## What ships in-repo

| Feature | Status |
| --- | --- |
| Entitlements + hardened runtime codesign | `Entitlements.plist` + `codesign-app.sh` |
| App Group SecItem tokens (+ legacy migrate) | `lattice-connectors` `MigratingAppGroupTokenStore` |
| Approval LA + signed audit evidence | `approval_signer.rs` (software ES256; SE backend hook) |
| Deep links / Open With files | `deep_link.rs` + `RunEvent::Opened` |
| Finder document types | `Info.plist` + `fileAssociations` |
| Spotlight catalog for helpers | `spotlight_index_workspace` → App Group JSON |
| Quick Look appex sources + scripted build | `apps/desktop/macos/LatticeQuickLook/` + `scripts/macos/build-quicklook-appex.sh` → embedded by `assemble-app` / `desktop-install` |
| SE approval CryptoKit bridge | `crates/lattice-approval-macos` (`libLatticeApprovalBridge.dylib`) wired into `ApprovalSigner` |

## App Store Connect API

Notarization secrets (`APPLE_ID` / app-specific password / team id) are **not**
an App Store Connect API key. To query Bundle ID capabilities via API, create
**Users and Access → Integrations → App Store Connect API** key, store the
`.p8` + Issuer ID + Key ID (e.g. in `lattice/secrets/apple.env`), then:

```sh
# Example once APPLE_API_KEY_ID / APPLE_API_ISSUER / APPLE_API_KEY_PATH are set:
# jwt → GET https://api.appstoreconnect.apple.com/v1/bundleIds
```

## Verify after install

```sh
codesign -d --entitlements :- /Applications/Lattice.app | plutil -p -
open 'lattice://open?root=/path/to/ws&path=Notes/Hello.md'
# Approve a proposal → Touch ID → `.lattice/approvals/<id>.jsonl`
```
