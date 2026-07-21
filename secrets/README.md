# Secrets (sops + age)

Encrypted secrets for Lattice live under `secrets/` and are decrypted locally
with [sops](https://github.com/getsops/sops) + [age](https://age-encryption.org).

## Layout

| Path | Purpose |
| --- | --- |
| [`.sops.yaml`](../.sops.yaml) | Encryption rules + age recipient |
| [`secrets/cloudflare.env`](./cloudflare.env) | Cloudflare Pages deploy token (encrypted) |
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
# or: cd . && cd -
```

## Deploy with the token (no interactive wrangler login)

With direnv loaded (decrypts into the environment):

```sh
nix run .#site-deploy
```

One-shot without relying on direnv:

```sh
sops exec-env secrets/cloudflare.env -- nix run .#site-deploy
```

Wrangler reads `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` from the
environment. Account ID is not secret; the API token is.

## What not to do

- Do not put API tokens in `.env` (gitignored, but easy to paste into chats).
- Do not commit `secrets/*.decrypted`, `*.plain`, or age private keys.
- Do not reuse a token after it appears in logs, screenshots, or agent context —
  rotate it.
