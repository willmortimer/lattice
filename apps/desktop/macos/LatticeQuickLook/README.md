# Lattice Quick Look appex

Bundle ID: `dev.lattice.desktop.quicklook`  
App Group: `group.dev.lattice.shared`

Spacebar Quick Look renders Markdown and `dev.lattice.page` as formatted HTML
(headings, lists, links, code, emphasis). PDF and images stay on the system
preview — this appex does not claim those types.

## Build / embed

```sh
# From lattice repo root:
bash scripts/macos/test-quicklook-markdown.sh
bash scripts/macos/build-quicklook-appex.sh
# → target/macos/LatticeQuickLook.appex

# Release assemble embeds under Contents/PlugIns/:
nxr task assemble-app   # part of desktop-release
```

`codesign-app` signs the appex with `LatticeQuickLook.entitlements` before the host app.
