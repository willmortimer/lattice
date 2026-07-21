---
title: Install and run Lattice
description: Current distribution status and the supported source-build path for the native desktop app.
---

Lattice is currently available as an open-source native build. Signed,
notarized public installers are being prepared; the repository does not yet
publish a general-audience download.

## Run the native app from source

The supported development environment uses Nix on macOS or Linux.

```sh
git clone https://github.com/willmortimer/lattice.git
cd lattice
nix run .#desktop-dev
```

This opens the native Tauri app and creates an isolated First Look workspace
under the repository's development state. It does not take over your normal
`~/Lattice` profile.

To build the release binary without installing an application bundle:

```sh
nix run .#desktop-build
```

For the complete contributor environment, local signing, and troubleshooting,
read the [Nix workflow guide](https://github.com/willmortimer/lattice/blob/main/docs/dev/nix-workflows.md).

## Try the CLI

Inside the development shell:

```sh
nix develop
cargo run -p lattice-cli -- --help
```

Continue with [Getting started](/docs/getting-started/) for the desktop tour or
the [CLI guide](/docs/cli/) for headless workflows.
