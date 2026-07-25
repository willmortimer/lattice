import { loadConfig } from "../src/config.ts";
import { RunManager } from "../src/runner.ts";

const model = process.env.LATTICE_AGENT_MODEL ?? "MiniMaxAI/MiniMax-M3";
const root = process.env.ROOT;
const events: Array<{ type: string; message?: string; chunk?: { type?: string } }> =
  [];

const config = loadConfig();
const mgr = new RunManager(config, (event) => {
  events.push(event as { type: string; message?: string; chunk?: { type?: string } });
});

await mgr.start({
  type: "start_run",
  runId: `probe-${Date.now()}`,
  threadId: "probe-thread",
  provider: "pioneer",
  model,
  workspaceRoot: root,
  prompt:
    "Use get_current_context once, then reply with only the workspaceRoot field from the tool result.",
} as never);

const failed = events.find((event) => event.type === "run_failed");
const completed = events.find((event) => event.type === "run_completed");
const toolChunks = events
  .filter((event) => event.type === "message_chunk")
  .map((event) => event.chunk?.type)
  .filter(Boolean);

if (failed) {
  console.log(`FAIL\t${model}\t${failed.message}`);
} else if (completed) {
  console.log(`OK\t${model}\t${toolChunks.join(",")}`);
} else {
  console.log(`UNKNOWN\t${model}\t${events.at(-1)?.type ?? "none"}`);
}
