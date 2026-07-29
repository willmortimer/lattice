# Lattice Quick Look appex

Bundle ID: `dev.lattice.desktop.quicklook`  
App Group: `group.dev.lattice.shared`

## Build / embed

```sh
# From lattice repo root:
bash scripts/macos/build-quicklook-appex.sh
# → target/macos/LatticeQuickLook.appex

# Release assemble embeds under Contents/PlugIns/:
nxr task assemble-app   # part of desktop-release
```

`codesign-app` signs the appex with `LatticeQuickLook.entitlements` before the host app.
