#!/usr/bin/env node
/**
 * Starts Lattice with `e2e-testing` + First Look reset, runs the CRM Wave 2
 * Tauri smoke (`e2e/data/crm.smoke.tauri.spec.ts`), then tears the app down.
 *
 * Set `LATTICE_PERF_REUSE_TAURI=1` to attach to an already-running
 * `pnpm tauri:dev:e2e` instead of spawning one.
 *
 * Not a CI gate this sprint — local / optional confidence only.
 */
import { spawn } from "node:child_process";
import { existsSync, unlinkSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const desktopRoot = resolve(__dirname, "..");
const repoRoot = resolve(desktopRoot, "../..");
const socketPath = process.env.TAURI_PLAYWRIGHT_SOCKET ?? "/tmp/tauri-playwright.sock";
const startTimeoutMs = Number(process.env.LATTICE_TAURI_PERF_START_MS ?? 180_000);
const smokeSpec = "e2e/data/crm.smoke.tauri.spec.ts";

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

async function waitForSocket(timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (existsSync(socketPath)) return;
    await sleep(250);
  }
  throw new Error(`Tauri Playwright socket did not appear: ${socketPath}`);
}

function runPlaywright() {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(
      "pnpm",
      ["exec", "playwright", "test", "--project=tauri", smokeSpec],
      {
        cwd: desktopRoot,
        stdio: "inherit",
        env: {
          ...process.env,
          TAURI_PLAYWRIGHT_SOCKET: socketPath,
          // Tauri `beforeDevCommand` already starts Vite; don't double-bind :5173.
          LATTICE_PERF_SKIP_WEBSERVER: "1",
        },
      },
    );
    child.on("error", reject);
    child.on("exit", (code) => {
      if (code === 0) resolvePromise();
      else reject(new Error(`playwright exited with code ${code ?? "null"}`));
    });
  });
}

function stopProcessTree(child) {
  if (!child?.pid) return;
  try {
    process.kill(-child.pid, "SIGTERM");
  } catch {
    try {
      child.kill("SIGTERM");
    } catch {
      // ignore
    }
  }
  setTimeout(() => {
    try {
      process.kill(-child.pid, "SIGKILL");
    } catch {
      try {
        child.kill("SIGKILL");
      } catch {
        // ignore
      }
    }
  }, 5_000).unref();
}

async function main() {
  let app = null;
  const forceReuse = process.env.LATTICE_PERF_REUSE_TAURI === "1";
  let reused = forceReuse && existsSync(socketPath);

  if (!reused && existsSync(socketPath)) {
    try {
      unlinkSync(socketPath);
    } catch {
      // ignore
    }
  }

  if (!reused) {
    const devHome = process.env.LATTICE_DEV_HOME ?? resolve(repoRoot, "target/dev-home");
    app = spawn("pnpm", ["tauri", "dev", "--features", "e2e-testing"], {
      cwd: desktopRoot,
      stdio: ["ignore", "pipe", "pipe"],
      detached: true,
      env: {
        ...process.env,
        LATTICE_DEV_HOME: devHome,
        // Match `pnpm tauri:dev:e2e` so CRM forms/actions match the demo seed.
        LATTICE_DEV_RESET_DEMO: process.env.LATTICE_DEV_RESET_DEMO ?? "1",
        TAURI_PLAYWRIGHT_SOCKET: socketPath,
      },
    });

    const onChunk = (buf) => {
      const text = buf.toString();
      if (process.env.LATTICE_TAURI_PERF_VERBOSE === "1") {
        process.stderr.write(text);
      }
    };
    app.stdout?.on("data", onChunk);
    app.stderr?.on("data", onChunk);
    app.on("exit", (code) => {
      if (code !== null && code !== 0 && code !== 143 && code !== 137) {
        console.error(`tauri:dev:e2e exited early with code ${code}`);
      }
    });

    try {
      await waitForSocket(startTimeoutMs);
      // Let First Look ensure_home + shell paint before the first fixture connect.
      await sleep(1_500);
    } catch (error) {
      stopProcessTree(app);
      throw error;
    }
  } else {
    console.error(`Reusing existing Playwright socket at ${socketPath}`);
  }

  try {
    await runPlaywright();
  } finally {
    if (app) stopProcessTree(app);
    if (!reused) {
      try {
        unlinkSync(socketPath);
      } catch {
        // ignore
      }
    }
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
});
