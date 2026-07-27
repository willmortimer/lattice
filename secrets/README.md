# Secrets (sops + age)

Encrypted secrets for the **public Lattice client** live under `secrets/` and are
**committed**. Ciphertext is safe to publish; only the age **private** key decrypts it.

Cloudflare Pages deploy secrets moved to the private
[`lattice-ecosystem`](https://github.com/willmortimer/lattice-ecosystem) repository.

Do **not** gitignore `secrets/*.env`. Gitignore plaintext dumps and the age
private key only (see root `.gitignore`).

## Layout

| Path | Purpose |
| --- | --- |
| [`.sops.yaml`](../.sops.yaml) | Encryption rules + age recipient |
| [`secrets/apple.env`](./apple.env) | Apple ID / app-specific password / team + signing identity |
| `~/.config/sops/age/keys.txt` | **Private** age key (never commit) |

## Apple Developer (signing / notarization)

```sh
sops secrets/apple.env
```

See [docs/dev/nix-workflows.md](../docs/dev/nix-workflows.md) and
[docs/dev/environment.md](../docs/dev/environment.md).

```sh
sops exec-env secrets/apple.env -- nix run .#desktop-release
```

## What not to do

- Do not put API tokens or Apple passwords in `.env`.
- Do not gitignore encrypted `secrets/*.env` — ciphertext belongs in git.
- Do not commit `secrets/*.decrypted`, `*.plain`, or age private keys.
