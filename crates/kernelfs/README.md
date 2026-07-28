# kernelfs

This is the public Lattice harness crate for **KernelFS** — the scoped execution projection for WASI and agent runs (`/input`, `/work`, `/output`, `/tmp`). It was adapted from the private ecosystem package at `packages/kernelfs` so `lattice-agentd` and other public crates can depend on it via path. See `docs/architecture/kernelfs-mvp.md` in the ecosystem repo for the full MVP spec.
