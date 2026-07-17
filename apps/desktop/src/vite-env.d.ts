/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** lattice-bridge base URL for container / headless web dev. */
  readonly VITE_LATTICE_BRIDGE_URL?: string;
  /** Workspace root to open when bridge mode has no native folder dialog. */
  readonly VITE_LATTICE_WORKSPACE?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
