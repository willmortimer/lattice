# ADR 0018: Extensions receive explicit capabilities and prefer proposed writes

## Status
Accepted

## Context
Unlimited scripting, generated applications, remote connectors, and MCP access create a serious security boundary.

## Decision
Plugins, apps, artifacts, scripts, workflows, connectors, and MCP clients declare scoped access to resources, datasets, network destinations, processes, secrets, and schema mutations. Untrusted components prefer returning proposed transactions for validation and approval. Direct write access is separately granted.

## Consequences
- Security behavior is understandable and auditable.
- Permission prompts and manifests require careful UX.
- Safe mode can disable all non-core execution while preserving file access.
