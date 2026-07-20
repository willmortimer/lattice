# Plugins

Plugins execute outside the renderer process. Each plugin receives explicit,
revocable filesystem grants and network capability tokens before it can touch
workspace files.

Capability grants are never ambient. A plugin that needs `Inbox/` must request
that path; Lattice records the grant and can revoke it without restarting the
host.
