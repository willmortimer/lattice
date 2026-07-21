import { describe, expect, it } from "vitest";

import type { ExecutionResult } from "./executionContracts";
import { toExecutionResult } from "./taskRun";

describe("taskRun adapters", () => {
  it("maps execution status into the shared ExecutionResult contract", () => {
    const raw: ExecutionResult = {
      id: "exec-1",
      status: "succeeded",
      stdout: "ok\n",
      stderr: "",
      startedAt: "2026-07-21T16:00:00Z",
      finishedAt: "2026-07-21T16:00:01Z",
      outputs: [{ path: "Notes/Out.md", kind: "page" }],
    };
    const mapped = toExecutionResult(raw);
    expect(mapped.status).toBe("succeeded");
    expect(mapped.outputs[0]?.path).toBe("Notes/Out.md");
    expect(mapped.proposalId).toBeUndefined();
  });
});
