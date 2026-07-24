# Friday demo rehearsal checklist

Recorded results for the Friday First Look closeout on `friday/integration` (F6).
F0 (embedded form submit → sibling data-view refresh) is merged at integration tip.

## Snapshot

| Field | Value |
| --- | --- |
| Branch / ref | `friday/integration` (detached worktree) |
| Commit | `4f1e986cd97af6935f29d0467b4e72636dfdea50` |
| Recorded | 2026-07-24 |
| Worktree | F6 isolated worktree (`f6-friday-demo-*`) |

## Automated pre-flight

| Check | Command | Result | Notes |
| --- | --- | --- | --- |
| First Look seed | `nxr prepare-first-look` | **PASS** | Events + Places Parquet, `compile-templates`, 8 artifact paths |
| Governed loop | `cargo test -p lattice-commands --test governed_loop_smoke -- --nocapture` | **PASS** | ~2.7s test; form → workflow → proposal → derived rebuild → undo |

## Fixture changes (F6)

| Change | Rationale |
| --- | --- |
| Pre-seed `AgentDigest` interface in demo template | Skip approve for fast Agent digest walkthrough; task/MCP path remains optional |
| OpsDashboard description + Home/Research copy | Documents F0 embedded form → Board refresh rehearsal |
| Attachment column seed | **Skipped** — `template.rs` rejects attachment/enum/multi_enum in template seeds |

## Manual rehearsal (native desktop)

Run after `nxr prepare-first-look`. Use `nxr desktop-dev` or a **new** First Look
workspace (existing install folders are sticky).

| # | Step | Status | Notes |
| --- | --- | --- | --- |
| 1 | Launch / open seeded workspace | **Manual** | `LATTICE_DEV_RESET_DEMO=1` for dev-home; or new template workspace |
| 2 | OpsDashboard: open interface | **Manual** | CRM → Interfaces → Ops dashboard |
| 3 | Embedded form submit → Board refresh | **Manual** | F0 — verify new contact card without reload |
| 4 | Agent digest (pre-seeded) | **Manual** | Interfaces → Agent digest — two metrics |
| 5 | AgentFirstLook task → approve (optional) | **Manual** | Rich proposal review path; same interface YAML |
| 6 | ContactBrief derived rebuild | **Manual** | Stale → Rebuild; edit `Derived/input.txt` → stale again |

## Honest shipped boundaries (Friday)

- **No peer sharing** — local single-user workspace only.
- **Cron deferred** — interval schedules in open session only; cron parses, does not run.
- **Closed-desktop schedules** — durable registry may still land in F5; tray merge is open-session only.
- **Attachments** — native staged upload; not template-seedable yet.
- **MCP live validation** — sample transcript only; no external client gate in F6.

## Related docs

- Linear rehearsal script: [first-look-demo.md](./first-look-demo.md#friday-demo-rehearsal-2026-07-24)
- Thursday baseline + Gate C: [thursday-baseline.md](./thursday-baseline.md)
- MCP transcript: [first-look-agent-mcp.md](./first-look-agent-mcp.md)

## Re-run

```sh
nxr prepare-first-look
cargo test -p lattice-commands --test governed_loop_smoke -- --nocapture
nxr desktop-dev
```
