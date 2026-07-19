# ADR 0040: Local voice dictation documentation package

## Status

Accepted.

## Context

Lattice product specs already anticipate dictation and voice notes
([docs/36](../36-platforms-accessibility-localization-and-mobile.md),
[docs/21](../21-search-links-context-and-ai-interoperability.md)) but there was
no implementation-ready design for fully local speech-to-text. Offline-first
and privacy invariants forbid silent cloud ASR
([docs/02](../02-principles-and-invariants.md) #2, #20). Mutations must flow
through the semantic command core ([ADR 0007](0007-semantic-command-transaction-core.md)).
Specialized hot paths must not run through React
([ADR 0006](0006-react-shell-specialized-renderers.md)).

Building FluidAudio / Parakeet integration without a locked architecture would
force repeated revisiting of process boundaries, provisional-vs-final text,
IPC, licensing, and editor persistence.

## Decision

Adopt the **Voice Dictation documentation package** under
[`docs/voice/`](../voice/README.md) as the architectural source of truth for
macOS local dictation, including:

- FluidAudio + Parakeet Unified English Core ML as the initial provider
- Client-owned capture with daemon-compatible voice service ownership
- Streaming provisional transcripts; offline re-decode for final text
- Final text only enters document state via editor transactions / commands
- Deterministic voice commands (no implicit NL agents in v1)
- Download-on-setup model distribution with explicit license attribution

Subsystem ADRs live in [`docs/voice/adr/`](../voice/adr/) (0001–0006) and are
normative for voice implementation. New irreversible cross-cutting choices that
affect the whole product should continue to land in `docs/decisions/` and
update `docs/voice/` in the same change.

Implementation follows
[`docs/voice/implementation-roadmap.md`](../voice/implementation-roadmap.md)
milestones M0–M8. Global Quick Note must not ship before model ownership moves
to `latticed` (M4 before M5).

## Consequences

- Implementers can split work across Tauri, `latticed`, editor, and the Swift
  bridge without re-litigating core boundaries.
- Research questions in the roadmap must be resolved in M0 (and early spikes)
  before locking pins and budgets in code.
- AGPL Lattice code remains distinct from FluidAudio and model-weight licenses;
  Settings must surface attribution.
- Roadmap and open-question register gain an explicit Voice Dictation section.
- Linux/Windows providers remain future work behind the same `SpeechProvider`
  abstraction.
