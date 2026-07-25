import { loadConfig } from "../src/config.js";
import { RunManager } from "../src/runner.js";

const events: Array<{ type: string; chunk?: { type?: string }; message?: string }> =
  [];
const config = loadConfig({
  PIONEER_API_KEY: process.env.PIONEER_API_KEY,
  LATTICE_AGENT_PROVIDER: "pioneer",
  LATTICE_AGENT_MODEL: process.env.LATTICE_AGENT_MODEL || "claude-sonnet-4-5",
  LATTICE_API_BASE_URL: process.env.LATTICE_API_BASE_URL,
  LATTICE_AUTH_TOKEN: process.env.LATTICE_AUTH_TOKEN,
  LATTICE_AGENT_FAKE: undefined,
});

console.error("cfg", {
  provider: config.defaultProvider,
  model: config.defaultModel,
  hasClient: Boolean(config.latticeClient),
});

const deadline = Date.now() + 60_000;
const runs = new RunManager(config, (event) => {
  events.push(event as (typeof events)[number]);
  const line =
    event.type === "message_chunk"
      ? `message_chunk ${(event as { chunk?: { type?: string } }).chunk?.type}`
      : event.type === "run_failed"
        ? `run_failed ${(event as { message?: string }).message}`
        : event.type;
  console.log(line);
});

const timer = setInterval(() => {
  if (Date.now() > deadline) {
    console.error("SMOKE TIMEOUT");
    process.exit(2);
  }
}, 1000);

await runs.start({
  type: "start_run",
  threadId: "t",
  runId: "smoke-tools",
  provider: "pioneer",
  model: config.defaultModel,
  workspaceRoot: process.env.ROOT,
  prompt: "Use the search tool for query roadmap, then briefly summarize the top hit path.",
});

clearInterval(timer);
console.error(
  "done",
  events.map((e) => e.type),
);
