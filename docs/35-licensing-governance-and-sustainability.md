# Licensing, Governance, and Sustainability

## Purpose

Lattice's non-lock-in promise depends on more than readable files. The format specifications, client, server, SDKs, plugin contracts, and conformance tools must be licensed and governed so that compatible implementations, self-hosting, automation, and long-term stewardship remain possible.

## Licensing goals

The licensing structure should:

- guarantee free local use;
- permit independent readers and writers for Lattice formats;
- protect the shared server and core application from closed hosted appropriation where appropriate;
- allow commercial and internal plugins without forcing unrelated application code open;
- encourage broad SDK adoption;
- keep schemas, examples, and conformance fixtures reusable;
- avoid a source-available license marketed as FOSS;
- preserve trademark quality without restricting format compatibility.

## Proposed licensing split

The exact choice requires legal review, but the preferred shape is:

### Format specifications and schemas

Use CC0, Apache-2.0, or another maximally reusable license for:

- schemas;
- examples;
- MIME-type definitions;
- conformance fixtures;
- protocol descriptions;
- generated bindings.

No implementation should need permission to read or write Lattice resources.

### Desktop client and core runtime

Evaluate:

- **MPL-2.0** for file-level copyleft and easier commercial embedding; or
- **AGPL-3.0-or-later** for stronger protection against proprietary network-hosted forks.

This remains an explicit governance decision rather than an accidental default.

### Sync and automation server

AGPL-3.0-or-later is the strongest initial candidate because network copyleft directly addresses modified hosted versions.

### SDKs, client libraries, and examples

Prefer Apache-2.0 or MIT for:

- TypeScript SDK;
- Rust SDK;
- Python SDK;
- plugin interfaces;
- generated clients;
- example applications.

Users should be able to build proprietary internal or commercial Lattice Apps and plugins without ambiguity.

### Bundled capability packs

License according to dependency compatibility while preferring the same open license as the client. Each pack must clearly disclose third-party licenses.

## What must never be plan-gated

Whether hosted services are offered or not, the following remain available locally and for self-hosting:

- file formats and schemas;
- import and export;
- CLI;
- local API;
- MCP;
- plugin SDK;
- local scripting;
- local Jupyter and Python;
- self-hosted server;
- local search;
- local data engines;
- local AI-provider integration;
- workspace backups;
- format migration tools.

A hosted business may charge for operational service, not for access to the user's own local data or automation interfaces.

## Sustainable business model

Reasonable paid hosted services include:

- managed sync;
- storage and bandwidth;
- backups and retention;
- team administration;
- enterprise OIDC and policy;
- audit retention;
- managed connectors;
- server-side automation workers;
- hosted notebook and GPU execution;
- public publishing;
- compliance and support;
- optional hosted model inference.

The project should avoid business incentives that require degrading local mode, withholding formats, or artificially limiting API access.

## Governance

A mature project should maintain:

- a public specification repository;
- architecture decision records;
- semantic versioning;
- a migration policy;
- a deprecation policy;
- security disclosure procedures;
- contributor guidelines;
- a code of conduct;
- release signing;
- reproducible builds where practical;
- an extension compatibility program;
- public conformance suites.

Irreversible format changes require:

- written proposals;
- examples;
- backward-compatibility analysis;
- migration tooling;
- independent implementation feedback;
- a documented acceptance process.

## Trademark and compatibility

The Lattice trademark may protect official distribution quality, but third parties must be free to state:

- “supports Lattice workspace formats”;
- “compatible with Lattice Canvas Profile”;
- “imports and exports Lattice resources.”

Trademark policy should not become a private gate over the format.

## Dependency policy

Dependencies should be evaluated for:

- OSI-approved licensing;
- source availability;
- maintenance health;
- platform support;
- security posture;
- format ownership implications;
- open-core traps;
- ability to replace the dependency.

A capability may integrate a commercial system, but Lattice's core format and runtime should not depend on proprietary libraries or services.

## Fork and continuity readiness

No individual or company should hold the only:

- release keys;
- domain credentials;
- package registry access;
- specification source;
- signing infrastructure;
- server deployment knowledge.

The project should document continuity procedures and maintain multiple trusted maintainers as it grows.
