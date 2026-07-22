# Secrets (sops + age)

Encrypted secrets for Lattice live under `secrets/` and are **committed** to
this public repository. That is intentional: sops ciphertext is safe to
publish; only the age **private** key decrypts it.

Do **not** gitignore `secrets/*.env`. Gitignore plaintext dumps and the age
private key only (see root `.gitignore`).

## Layout

| Path | Purpose |
| --- | --- |
| [`.sops.yaml`](../.sops.yaml) | Encryption rules + age recipient |
| [`secrets/cloudflare.env`](./cloudflare.env) | Cloudflare Pages deploy token (encrypted) |
| [`secrets/apple.env`](./apple.env) | Apple ID / app-specific password / team + signing identity |
| `~/.config/sops/age/keys.txt` | **Private** age key (never commit) |
| [`.env`](../.env) | Optional non-secret local overrides only |

## One-time machine setup

You already need `sops` and `age` (Homebrew or the ops nix shell). The age
private key defaults to:

```text
~/.config/sops/age/keys.txt
```

If you generate a new key:

```sh
mkdir -p ~/.config/sops/age
age-keygen -o ~/.config/sops/age/keys.txt
age-keygen -y ~/.config/sops/age/keys.txt   # print public recipient
```

Put that public key in `.sops.yaml` under `creation_rules[].age` (and re-encrypt
existing files if the recipient changes).

Then:

```sh
direnv allow
```

## Edit / rotate Cloudflare token

1. Create a new token in the Cloudflare dashboard: **Account → Cloudflare Pages → Edit**.
2. Revoke any token that may have leaked.
3. Edit the encrypted file (opens your `$EDITOR` with plaintext temporarily):

```sh
sops secrets/cloudflare.env
```

Replace `CLOUDFLARE_API_TOKEN` with the new value. Save and quit — sops
re-encrypts on write.

4. Reload the shell env:

```sh
direnv reload
```

## Apple Developer (signing / notarization)

Paid Apple Developer Program membership is required for Developer ID signing and
notarization (distribution beyond your Mac). Local `desktop-install` can still
use an Apple Development identity from Keychain.

```sh
sops secrets/apple.env
```

Fill:

| Key | Notes |
| --- | --- |
| `APPLE_ID` | Your **Apple ID email** (account login). One per person/account — not per app. Used for notarization. |
| `APPLE_PASSWORD` | **App-specific password** for that same Apple ID (from [appleid.apple.com](https://appleid.apple.com) → Sign-In → App-Specific Passwords). Not your iCloud login password. Generate one labeled e.g. `lattice-notarize`. |
| `APPLE_TEAM_ID` | **10-character team id** for your developer membership (e.g. `BKM26M422Q`). Shared by everyone on the team; not an email. Membership details on developer.apple.com, or the `(XXXXXXXXXX)` suffix on a codesign identity. |
| `APPLE_SIGNING_IDENTITY` | Full Keychain identity **string**. Quote it — spaces break dotenv/direnv. Local install: `APPLE_SIGNING_IDENTITY="Apple Development: you@example.com (TEAMID)"`. Distribution / `desktop-release`: `APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)"`. |

**Apple ID ≠ Team ID.** Email identifies *you*; team id identifies the *paid/free developer team* that owns certificates.

Then `direnv reload`.

- Local install: `nxr desktop-install` / `nix run .#desktop-install` reads `APPLE_SIGNING_IDENTITY` (and optional `APPLE_TEAM_ID`).
- Release DMG: `nxr desktop-release` / `nix run .#desktop-release` also requires `APPLE_ID`, `APPLE_PASSWORD`, and `APPLE_TEAM_ID`, and rejects Apple Development identities.

```sh
# check env only:
LATTICE_RELEASE_VALIDATE_ONLY=1 nix run .#desktop-release

# full notarized DMG:
sops exec-env secrets/apple.env -- nix run .#desktop-release
```

See [docs/dev/nix-workflows.md](../docs/dev/nix-workflows.md) and
[docs/dev/environment.md](../docs/dev/environment.md).

## Deploy Cloudflare with the token

With direnv loaded (decrypts into the environment):

```sh
nix run .#site-deploy
```

One-shot without relying on direnv:

```sh
sops exec-env secrets/cloudflare.env -- nix run .#site-deploy
```

## What not to do

- Do not put API tokens or Apple passwords in `.env`.
- Do not gitignore encrypted `secrets/*.env` — ciphertext belongs in git.
- Do not commit `secrets/*.decrypted`, `*.plain`, or age private keys.
- Do not reuse a token after it appears in logs, screenshots, or agent context —
  rotate it.
