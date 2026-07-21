# Artifacts, Lattice Apps, and the UI Kit

## Layered application model

Lattice supports several levels of custom interaction:

### Level 1: built-in blocks and views

Text, tables, charts, forms, metrics, buttons, record details, filters, and standard dashboards.

### Level 2: declarative interface/dashboard

Readable YAML/JSON composition over built-in components. Preferred for most AI-generated dashboards because it is inspectable and maintainable.

### Level 3: HTML artifact

A focused sandboxed HTML/CSS/JavaScript package.

### Level 4: Lattice App

A complete source-backed web project using React, Svelte, Solid, Vue, or any build system producing browser assets.

### Level 5: external web application

An independently hosted app embedded or connected through APIs.

## Artifact package

```text
Market Map.artifact/
├── README.md
├── artifact.yaml
├── index.html
├── app.js
├── styles.css
└── assets/
```

```yaml
format: lattice-artifact
version: 1
title: Market map
entrypoint: ./index.html
bindings:
  companies:
    type: sqlite-query
    resource: ../../Research/Companies.data
    sql: SELECT * FROM companies LIMIT 100
    limit: 100
permissions:
  network: []
  workspace_write: []
fallback:
  file: ./README.md
```

Artifacts run in isolated sandboxed iframes (`sandbox="allow-scripts"`, no
`allow-same-origin`, no ambient Tauri). The host injects `--lt-*` theme tokens
via postMessage, resolves only named read-only `BindingSpec` bindings declared
in the manifest, and can open linked resources. Off-screen artifacts suspend
via IntersectionObserver. Network is deny-by-default (`permissions.network`
must be empty in v1).

Desktop session kind `artifact` mounts `ArtifactResourceRenderer` on the
`main` and `embed` renderer surfaces. Use `:::lattice-embed` with
`mode: interactive` to mount the same sandbox inline on a page.

## Lattice App package

```text
Customer Portal.app/
├── README.md
├── lattice-app.yaml
├── package.json
├── pnpm-lock.yaml
├── src/
├── public/
└── dist/
```

```yaml
format: lattice-app
version: 1
title: Customer portal
entrypoint: ./dist/index.html
source: ./src
framework:
  name: react
  version: 19
routes:
  - /
  - /customers/:id
build:
  task: ./Build.task.yaml
bindings:
  customers:
    type: sqlite-query
    database: ../../Data/CRM.data/database.sqlite
    query: SELECT * FROM customers
capabilities:
  network: []
  workspace_read: [../../Assets/**]
  workspace_write: []
publishing:
  static: false
```

React is the blessed default because of ecosystem breadth and model familiarity, not a format requirement.

## UI kit

`@lattice/ui` should provide:

- Theme tokens and CSS variables.
- Typography.
- Buttons and inputs.
- Tabs, panels, dialogs, and menus.
- Data tables and record cards.
- Layout primitives.
- Charts and empty states.
- Responsive patterns.
- Command palette components.
- Accessible focus behavior.

Apps can match the host without copying private shell code.

## App SDK

`@lattice/app-sdk` exposes approved host behavior:

```ts
const rows = await lattice.data.query({
  resource: "lattice://...",
  sql: "SELECT * FROM customers LIMIT 100"
});

await lattice.commands.propose({
  command: "page.create",
  input: { title: "Customer review" }
});
```

Capabilities:

- Resource reads.
- Bounded data queries.
- Proposed transactions.
- Selection and parameter state.
- Theme and locale.
- File picker.
- Notifications.
- Deep links.
- Export and publish.
- Capability inspection.

The SDK never exposes raw Tauri APIs.

## Data bindings

Bindings declare:

- Source.
- Query or table.
- Read or proposed-write access.
- Refresh behavior.
- Parameter inputs.
- Result limits.

An artifact or app cannot open arbitrary workspace paths merely because JavaScript can express it.

## Publishing modes

### Static

Produce HTML/CSS/JS plus data snapshots. Suitable for landing pages, reports, docs, portfolios, and public dashboards.

### Connected

Published app uses scoped OAuth/token access to Lattice server resources.

### Standalone export

Bundle snapshots and source so the app can run independently from Lattice.

### Embedded

Render app inside a canvas, page, documentation site, or another Lattice App.

## Build and lineage

Apps declare build tasks and dependencies. Lattice tracks:

- Inputs.
- Lockfile.
- Build command.
- Output hash.
- Last successful build.
- Staleness.
- Dependency vulnerabilities where tooling exists.
- Generated versus human-authored source.

## External web embeds

A generic web embed resource supports Grafana, internal applications, and ordinary websites.

Embedding is a compatibility adapter. Native connectors are preferred when Lattice needs offline snapshots, cross-filtering, data export, or AI access.

## Security

- Separate origin or isolated WebView.
- Strict CSP.
- No ambient Tauri IPC.
- Explicit hosts and workspace bindings.
- CPU/memory/lifecycle controls.
- Inspect source, manifest, dependencies, and permissions.
- Suspend offscreen.
- Destroy untrusted WebView when closed where practical.
