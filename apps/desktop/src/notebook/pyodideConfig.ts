/**
 * Pyodide load strategy for N3 notebook execution.
 *
 * Lean approach: no npm package. A module Web Worker dynamically imports
 * `pyodide.mjs` from jsDelivr on first Run, then fetches the runtime assets
 * from the same index URL.
 *
 * Approximate first-load transfer (not bundled into the desktop app):
 * - ~6–8 MB compressed JS/WASM for the core interpreter
 * - ~10–15 MB uncompressed on disk in the browser cache
 * Subsequent Runs reuse the warm worker until cancel terminates it.
 */
export const PYODIDE_VERSION = "0.27.7";

export const PYODIDE_INDEX_URL =
  `https://cdn.jsdelivr.net/pyodide/v${PYODIDE_VERSION}/full/`;

/** Cap each stdout/stderr/repr/traceback string written into `.ipynb` outputs. */
export const MAX_NOTEBOOK_OUTPUT_CHARS = 200_000;
