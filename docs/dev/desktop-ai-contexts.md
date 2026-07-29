# Desktop AI / cloud NXR contexts

Repeatable launch profiles without baking provider keys into DMGs.

| Context | Cloud URL | AI policy | Notes |
| --- | --- | --- | --- |
| `dev-local-ai` | `http://127.0.0.1:8788` | `local` | AI keys from env / `exec-with-ai-env.sh` |
| `dev-cloud-ai` | `https://cloud.lattice-notes.com` | `cloud` | Same secret delivery; production cloud |

```sh
# from lattice/
nxr context list
nxr context run dev-local-ai -- task desktop-dev
nxr context run dev-cloud-ai -- task agentd
```

**Finder / Dock launches** do not inherit shell env. Use a baked channel build
([`desktop-channels.md`](desktop-channels.md)) for internal DMGs. Ecosystem-private
`exec-for-dev.sh` remains for demo/YC wrappers only — repeatable defaults live here.
