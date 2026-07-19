# Platforms, Accessibility, Localization, and Mobile

## Platform goals

Lattice begins as a desktop-first application because its richest workflows involve files, databases, multiple windows, canvases, notebooks, external tools, and local execution. The architecture should nevertheless support:

- macOS;
- Windows;
- Linux;
- iPadOS;
- iOS;
- Android;
- browser-based access;
- self-hosted web clients;
- headless CLI and daemon environments.

Feature parity is not required on day one. Resource compatibility is.

## Desktop

Desktop is the reference environment for:

- full workspace editing;
- native filesystem ownership;
- external-editor interoperability;
- local databases;
- Jupyter kernels;
- Python, Nix, and container execution;
- plugin development;
- multi-window workflows;
- remote database administration;
- large analytical datasets.

## iPadOS

The iPad roadmap should prioritize:

- fast notebooks and pages;
- full canvas navigation;
- native PencilKit ink;
- PDF and image annotation;
- quick capture;
- database views and forms;
- notebook viewing and selected execution;
- external keyboard and trackpad;
- Files app integration;
- share-sheet imports;
- offline workspace mirrors.

Native iPad integrations should use Tauri mobile plugins and Swift where browser APIs are insufficient.

## Phones

Phone clients should initially focus on:

- reading;
- search;
- inbox capture;
- voice notes;
- photos and scans;
- forms;
- record updates;
- notifications;
- lightweight page editing;
- approval of proposed transactions;
- sync status.

A phone should not be forced to mount a desktop-scale application shell.

## Browser

A browser client may use OPFS as its local working store and synchronize or export to user-visible files. It should support:

- remote workspaces;
- published sites and apps;
- local browser scratch workspaces;
- document and canvas editing;
- data views;
- Pyodide notebooks;
- capability-limited plugins.

Browser mode does not redefine the canonical desktop storage model.

## Accessibility

Accessibility is a core architecture requirement.

Lattice must provide:

- semantic DOM for documents and controls;
- keyboard navigation across all core surfaces;
- visible focus;
- screen-reader names and descriptions;
- accessible alternatives to canvas-only layout;
- explicit canvas reading order;
- high-contrast themes;
- scalable typography;
- reduced-motion mode;
- non-color status indicators;
- accessible charts and data tables;
- captions and transcripts for media;
- alt text and validation for published content;
- touch target sizing;
- switch-control compatibility where platforms support it.

GPU-rendered canvas content must have a synchronized semantic representation. A user should be able to traverse canvas resources in reading order without relying on spatial vision.

## Localization

All core UI strings should be externalized from the beginning.

Resources should support:

- Unicode paths and content;
- locale-aware sorting and formatting;
- time-zone-aware schedules;
- right-to-left layout;
- translated metadata;
- documentation-site language variants;
- per-resource language declaration;
- locale-independent canonical numeric and date storage.

Machine-readable manifests should use stable identifiers rather than localized enum values.

## Input methods

The editor must respect:

- IME composition;
- dead keys;
- dictation ([docs/voice/](voice/README.md) for local macOS STT);
- handwriting conversion;
- right-to-left scripts;
- platform clipboard conventions;
- accessibility input devices;
- mouse, touch, trackpad, pen, keyboard, and gamepad where relevant.

Custom renderers must not assume Latin keyboard input or mouse-only interaction.

## Platform-specific features

Lattice may expose platform-specific enhancements without making them canonical requirements:

- Apple Pencil hover, squeeze, and double-tap;
- Spotlight indexing and Quick Look;
- Windows Ink;
- Android stylus APIs;
- native share sheets;
- OS search integration;
- filesystem providers;
- native notifications;
- secure keychains;
- background tasks;
- platform menu and shortcut conventions.

Canonical resources remain portable even when a platform provides a richer editing cache or interaction layer.

## Testing

Platform quality requires:

- automated accessibility audits;
- keyboard-only test suites;
- screen-reader smoke tests;
- RTL visual tests;
- IME integration tests;
- touch and pen latency tests;
- low-memory mobile tests;
- offline and interrupted-sync tests;
- platform-specific file-provider tests.
