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
| `APPLE_ID` | Apple ID email for notarization |
| `APPLE_PASSWORD` | **App-specific** password from [appleid.apple.com](https://appleid.apple.com) — not your login password |
| `APPLE_TEAM_ID` | Membership → Membership details (readable in the file) |
| `APPLE_SIGNING_IDENTITY` | `security find-identity -v -p codesigning` (often `Developer ID Application: …`) |

Then `direnv reload`. `nxr desktop-install` / `nix run .#desktop-install` read
`APPLE_SIGNING_IDENTITY` and `APPLE_TEAM_ID` from the environment.

Notarization (`APPLE_ID` / `APPLE_PASSWORD`) is for the release/DMG path when
that is wired; keep the values in sops now so they are ready.

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
