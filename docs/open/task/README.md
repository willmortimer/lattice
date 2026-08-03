# Task package

Repeatable local work with an explicit manifest and entry script.

## Example

```text
Tasks/WeeklyDigest.task/
├── task.yaml
└── main.py
```

## Agent notes

- Tasks declare runtime needs explicitly (for example Python via `uv`).
- Prefer reading the manifest before executing anything.
