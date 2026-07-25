import { loadConfig } from "./config.js";
import { RunManager } from "./runner.js";

const events: unknown[] = [];
const config = loadConfig({
  LATTICE_AGENT_FAKE: "1",
  LATTICE_API_BASE_URL: process.env.LATTICE_API_BASE_URL,
  LATTICE_AUTH_TOKEN: process.env.LATTICE_AUTH_TOKEN,
});
const runs = new RunManager(config, (event) => {
  events.push(event);
});

await runs.start({
  type: "start_run",
  threadId: "t",
  runId: "r-test",
  provider: "fake",
  model: "fake-model",
  workspaceRoot: process.env.ROOT,
  messages: [
    {
      id: "m1",
      role: "user",
      content: "tell me about Roadmap",
      parts: [{ type: "text", text: "tell me about Roadmap" }],
    },
  ],
});

console.log(
  "events",
  events.map((event) => (event as { type: string }).type),
);
