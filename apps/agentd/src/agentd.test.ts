import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { PassThrough } from "node:stream";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import {
  PROTOCOL_VERSION,
  parseEvent,
  serializeCommand,
  type AgentEvent,
} from "@lattice/agent-protocol";

import { loadConfig } from "./config.js";
import { runAgentdLoop } from "./index.js";
import { RunManager } from "./runner.js";

const here = path.dirname(fileURLToPath(import.meta.url));
const entry = path.join(here, "index.ts");
const tsxCli = path.resolve(here, "../node_modules/tsx/dist/cli.mjs");

function collectEvents(): {
  events: AgentEvent[];
  sink: (event: AgentEvent) => void;
} {
  const events: AgentEvent[] = [];
  return {
    events,
    sink: (event) => {
      events.push(event);
    },
  };
}

describe("handshake", () => {
  it("responds to hello with hello_ack PROTOCOL_VERSION", async () => {
    const events: AgentEvent[] = [];
    const input = new PassThrough();

    const loop = runAgentdLoop({
      input,
      config: loadConfig({ LATTICE_AGENT_FAKE: "1" }),
      onEventLine: (line) => {
        events.push(parseEvent(line));
      },
    });

    input.write(
      `${serializeCommand({ type: "hello", protocolVersion: PROTOCOL_VERSION })}\n`,
    );
    input.write(`${serializeCommand({ type: "shutdown" })}\n`);
    input.end();

    await loop;

    expect(events).toEqual([
      { type: "hello_ack", protocolVersion: PROTOCOL_VERSION },
    ]);
  });
});

describe("fake start_run", () => {
  it("emits run_started, message_chunks, and run_completed", async () => {
    const { events, sink } = collectEvents();
    const config = loadConfig({ LATTICE_AGENT_FAKE: "1" });
    const runs = new RunManager(config, sink);

    await runs.start({
      type: "start_run",
      threadId: "thread-1",
      runId: "run-1",
      provider: "fake",
      model: "fake-model",
      prompt: "Hello lattice",
    });

    expect(events[0]).toEqual({
      type: "run_started",
      runId: "run-1",
      threadId: "thread-1",
    });
    const chunks = events.filter((e) => e.type === "message_chunk");
    expect(chunks.length).toBeGreaterThanOrEqual(2);
    for (const chunk of chunks) {
      expect(chunk.runId).toBe("run-1");
    }
    expect(events.at(-1)).toEqual({ type: "run_completed", runId: "run-1" });
  });
});

describe("cancel_run", () => {
  it("aborts an in-flight fake run without hanging", async () => {
    const { events, sink } = collectEvents();
    const config = loadConfig({ LATTICE_AGENT_FAKE: "1" });
    const runs = new RunManager(config, sink, { chunkDelayMs: 50 });

    const started = runs.start({
      type: "start_run",
      threadId: "thread-2",
      runId: "run-2",
      provider: "fake",
      model: "fake-model",
      prompt: "Slow echo that we will cancel mid-stream",
    });

    // Wait until the run is registered and has started streaming.
    await new Promise<void>((resolve) => {
      const timer = setInterval(() => {
        if (
          runs.activeRunId === "run-2" ||
          events.some((e) => e.type === "run_started")
        ) {
          clearInterval(timer);
          resolve();
        }
      }, 5);
    });

    runs.cancel("run-2");
    await started;

    expect(events.some((e) => e.type === "run_started")).toBe(true);
    const terminal = events.at(-1);
    expect(terminal?.type).toBe("run_failed");
    if (terminal?.type === "run_failed") {
      expect(terminal.message).toMatch(/cancel/i);
      expect(terminal.retryable).toBe(false);
    }
    expect(runs.activeRunId).toBeNull();
  });
});

describe("health + child process JSONL", () => {
  it("spawns tsx entry and completes hello + fake run + shutdown", async () => {
    const child = spawn(process.execPath, [tsxCli, entry], {
      env: {
        ...process.env,
        LATTICE_AGENT_FAKE: "1",
      },
      stdio: ["pipe", "pipe", "pipe"],
    });

    const events: AgentEvent[] = [];
    const rl = createInterface({ input: child.stdout! });
    rl.on("line", (line) => {
      const trimmed = line.trim();
      if (trimmed.length === 0) {
        return;
      }
      events.push(parseEvent(trimmed));
    });

    const stderr: string[] = [];
    child.stderr?.on("data", (buf: Buffer) => {
      stderr.push(buf.toString("utf8"));
    });

    child.stdin!.write(
      `${serializeCommand({ type: "hello", protocolVersion: PROTOCOL_VERSION })}\n`,
    );
    child.stdin!.write(
      `${serializeCommand({
        type: "start_run",
        threadId: "t-child",
        runId: "r-child",
        provider: "fake",
        model: "fake-model",
        prompt: "from child",
      })}\n`,
    );

    // Wait for run_completed then shut down.
    await new Promise<void>((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(
          new Error(
            `timed out; events=${JSON.stringify(events)} stderr=${stderr.join("")}`,
          ),
        );
      }, 15_000);
      const check = setInterval(() => {
        if (
          events.some((e) => e.type === "run_completed" && e.runId === "r-child")
        ) {
          clearInterval(check);
          clearTimeout(timeout);
          resolve();
        }
      }, 20);
    });

    child.stdin!.write(`${serializeCommand({ type: "shutdown" })}\n`);
    child.stdin!.end();

    const exitCode = await new Promise<number | null>((resolve) => {
      child.on("close", (code) => resolve(code));
    });

    expect(exitCode).toBe(0);
    expect(events[0]).toEqual({
      type: "hello_ack",
      protocolVersion: PROTOCOL_VERSION,
    });
    expect(events.some((e) => e.type === "run_started")).toBe(true);
    expect(events.some((e) => e.type === "message_chunk")).toBe(true);
    expect(events.some((e) => e.type === "run_completed")).toBe(true);
  }, 20_000);
});
