---
title: Architecture
tags: [diagram]
---

# Architecture

Lattice keeps the workspace honest: files on disk stay canonical; Rust owns
mutations; the shell never becomes a privileged writer.

## Core loop

```mermaid
flowchart LR
  Files[Workspace files] --> Core[Rust command core]
  Core --> UI[Desktop shell]
  Core --> Index[Search index]
  UI -->|semantic commands| Core
```

## With latticed (warm local runtime)

```mermaid
flowchart LR
  Files[Workspace directory] --> Daemon[latticed]
  Daemon --> Index[FTS + chunks + vectors]
  Daemon --> EmbedHost[embed-host]
  Daemon --> VoiceHost[voice-host]
  Desktop[Desktop / CLI] -->|UDS| Daemon
  Desktop -->|native capture| VoiceHost
```

Details and try-paths: [[Research/Local Runtime]].

Related: [[Product/Vision]] and [[Home]].
