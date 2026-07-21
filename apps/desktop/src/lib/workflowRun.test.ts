import { describe, expect, it } from "vitest";

import type { WorkflowRunRecordDto } from "./workflowRun";
import { toExecutionResult } from "./workflowRun";

describe("workflowRun adapters", () => {
  it("maps run records into the shared ExecutionResult contract", () => {
    const raw: WorkflowRunRecordDto = {
      workflowPath: "Simple.workflow.yaml",
      trigger: "manual",
      execution: {
        id: "exec-1",
        status: "succeeded",
        stdout: "ok\n",
        stderr: "",
        startedAt: "2026-07-21T16:00:00Z",
        finishedAt: "2026-07-21T16:00:01Z",
        outputs: [],
        proposalId: "prop-1",
      },
      steps: [
        {
          id: "propose",
          action: "proposal.create",
          status: "succeeded",
          log: "created proposal prop-1\n",
          proposalId: "prop-1",
        },
      ],
    };
    const mapped = toExecutionResult(raw);
    expect(mapped.status).toBe("succeeded");
    expect(mapped.proposalId).toBe("prop-1");
    expect(raw.steps[0]?.action).toBe("proposal.create");
  });
});
