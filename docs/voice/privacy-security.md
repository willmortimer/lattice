# Privacy and Security

## Scope

Make local-only voice processing explicit and enforceable. Aligns with
invariant #20 ([docs/02](../02-principles-and-invariants.md)) and
[docs/20](../20-security-permissions-secrets-and-trust.md).

## Privacy guarantees

Default behavior:

- Audio remains on the local device.
- Raw audio is held only long enough to finish the active utterance.
- Raw audio is deleted after final decoding.
- Only final transcript text enters document storage.
- Provisional transcript events are not persisted.
- Voice telemetry does not include audio or transcript content.
- Cloud sync sees final document edits, not audio.
- No Lattice cloud account is required for dictation.

## Optional retention

Any future recording feature **must** be separate from dictation and explicitly
enabled. It **must** define:

- Storage location
- Encryption
- Retention period
- Sync behavior
- Export behavior
- Deletion behavior

## Threat model

| Threat | Mitigation |
|--------|------------|
| Untrusted local processes connecting to latticed | Local auth + socket permissions ([daemon-protocol.md](./daemon-protocol.md)) |
| Malicious audio attempting to trigger commands | Deterministic grammar + modes ([voice-commands.md](./voice-commands.md)) |
| Prompt-like content in spoken prose | No implicit NL command execution in v1 |
| Compromised model downloads | Hash (and optional signature) verification |
| Tampered bridge libraries | Code signing + ABI version checks |
| Plugin-defined voice commands | Capability grants; no ambient authority |
| Accidental destructive commands | Risk class + confirmation |
| Sensitive transcripts in logs | Never log content by default |
| Crash dumps containing audio buffers | Minimize residency; scrub where feasible |

## Security requirements

- Hash-verify model artifacts.
- Restrict IPC access.
- Zero or release audio buffers promptly.
- Never log transcript content by default.
- Require confirmation for destructive or external commands.
- Expose voice commands only through registered capabilities.
- Prevent voice from bypassing plugin permissions.
- Separate model files and executable code in distribution metadata.
- Do not send audio over public HTTP/MCP APIs.

## Testing requirements

- Auth rejection integration test
- Buffer release after finalize (instrumented test or sanitizer-assisted)
- Log redaction tests
- Destructive command confirmation gating
- Model hash mismatch

## Open questions

- Crash-dump scrubbing practicality on Apple platforms
- Strength of client executable identity attestation

## Acceptance criteria

- [ ] Default path never uploads audio or transcripts
- [ ] Privacy copy in Settings matches implemented behavior
- [ ] Threat mitigations above have corresponding tests or documented residual risk
