# Desktop release channels

| Channel | Bundle id | Product name | Default cloud URL | NXR task |
| --- | --- | --- | --- | --- |
| Production | `dev.lattice.desktop` | Lattice | `https://cloud.lattice-notes.com` | `desktop-release` |
| Internal | `dev.lattice.desktop.dev` | Lattice Dev | staging (`LATTICE_CLOUD_URL_DEFAULT`) | `desktop-release-internal` |

Same Apple Team + Developer ID signing identity. Apps install **side-by-side**.

## Internal channel

```sh
# validate Apple env without notarize:
LATTICE_RELEASE_VALIDATE_ONLY=1 nxr task release-env-validate

# build internal .app (bakes staging cloud URL via lattice-cloud-client build.rs)
nxr task desktop-release-internal
```

Live notarize is **not** required in CI for the internal channel.

### Apple Developer console

1. Register App ID `dev.lattice.desktop.dev` (same Team as production).
2. Group it with primary `dev.lattice.desktop` for Sign in with Apple.
3. Do not block code merges on console clicks — document and apply when shipping.

### Security

Never bake `PIONEER_API_KEY` / `OPENAI_API_KEY` into DMGs. AI keys stay in NXR
contexts / sops / `exec-with-ai-env.sh`.
