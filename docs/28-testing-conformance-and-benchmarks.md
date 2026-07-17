# Testing, Conformance, and Benchmarks

## Testing layers

### Unit tests

- Parsers and serializers.
- Commands and validators.
- Path and ID resolution.
- Permission checks.
- Query planning.
- Workflow expressions.
- Storage providers.

### Round-trip tests

For every canonical format:

```text
parse → semantic model → serialize → parse
```

Verify preservation of unknown fields and stable formatting policy.

### Golden fixtures

Phase 1 resource-runtime conformance fixtures live under
`test/fixtures/resource-runtime/` and are exercised by
`lattice-core` integration tests (profiles, diagnostics, containment, and
read budgets). Add cases there when introducing or tightening format
profiles.

Maintain real example workspaces covering:

- Markdown edge cases.
- JSON Canvas.
- Data apps.
- Parquet datasets.
- Jupyter notebooks.
- Ink resources.
- Artifacts and Apps.
- Documentation projects.
- Plugins and workflows.

### Integration tests

- External file edits.
- Crash recovery.
- SQLite WAL behavior.
- Connector cancellation.
- Arrow streaming.
- App and artifact sandboxing.
- Jupyter kernel lifecycle.
- Sync retries and conflicts.
- Daemon/desktop handoff.

### Security tests

- Capability bypass attempts.
- Path traversal.
- Malicious manifests.
- Untrusted WebView IPC.
- Secret leakage.
- SQL injection.
- Plugin resource exhaustion.
- Content in telemetry.

## Conformance suite

Publish conformance commands:

```bash
lattice conformance format
lattice conformance plugin
lattice conformance canvas
lattice conformance app
lattice conformance sync
```

Alternative implementations can run the same fixtures.

## Portability tests

- Delete `.lattice/` and rebuild.
- Open pages in generic Markdown tooling.
- Open canvas in generic JSON Canvas viewer.
- Query SQLite with `sqlite3`.
- Query Parquet with DuckDB or another engine.
- Open notebooks in Jupyter.
- Render artifact source independently.
- Export ink preview without platform cache.

## Performance benchmark corpus

### Quick note

- Cold and warm launch.
- First input latency.
- Save and close.
- Memory footprint.

### Pages

- 100,000 small pages.
- Extremely long page.
- Many embeds.
- Large code blocks.

### Canvas

- Thousands of nodes.
- Large edge graph.
- Many previews.
- Active rich embeds.
- Ink-heavy canvas.

### Data

- Million-row SQLite app.
- Multi-gigabyte Parquet.
- Remote query with latency.
- Arrow transfer throughput.
- Pivot and cross-filter dashboard.

### Compute

- Pyodide startup.
- Native Jupyter startup.
- Notebook with large outputs.
- Nix environment realization.

### Extension

- Many installed plugins.
- Malfunctioning plugin.
- Multiple artifact WebViews.
- Capability pack activation.

## Continuous profiling

Track:

- Launch phases.
- Keystroke latency.
- Canvas frame time.
- Worker queue depth.
- Query bytes scanned.
- Arrow serialization/copying.
- WebView count and memory.
- Indexing throughput.
- Sync backlog.
- Plugin CPU/memory.

## Failure injection

Test:

- Process kill during save.
- Disk full.
- Permission revoked.
- Corrupt SQLite/Parquet/manifest.
- Network loss during sync.
- Duplicate operations.
- Object-store partial failure.
- Worker crash.
- Kernel hang.
- Plugin timeout.

## Accessibility testing

- Keyboard navigation.
- Screen reader linear canvas order.
- High contrast.
- Reduced motion.
- Chart data fallbacks.
- Ink alternatives and recognition.
- Published docs accessibility.

## Compatibility policy

Every release should report:

- Supported format versions.
- Migration behavior.
- Deprecated APIs.
- Plugin compatibility.
- Tested platforms.
- Known round-trip limitations.
