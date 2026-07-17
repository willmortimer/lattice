---
title: Incident Response
---

# Incident response

## Triage

1. Create a debug journal entry from [[Templates/Debug Session]].
2. Link the active issue in `Issues.data`.
3. Capture hypotheses and repro steps before changing production.

## Stabilize

- Roll back the last deploy if error rates spike.
- Record the rollback command and outcome in the journal entry.
- File a follow-up ADR in [[Decisions/]] if the fix changes architecture.

## Close out

- Move resolved notes to [[Archive/]] when the incident is complete.
- Update [[Architecture/Component Map]] if the system diagram changed.
