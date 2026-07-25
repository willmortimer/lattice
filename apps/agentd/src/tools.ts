/**
 * OpenAI Agents SDK tools mirroring latticed MCP / localhost HTTP.
 */

import { randomUUID } from "node:crypto";

import {
  MAX_OVERLAY_ANCHORS,
  overlayPurposeSchema,
  workspaceAnchorSchema,
  type AgentEvent,
  type OverlayPurpose,
  type WorkspaceAnchor,
} from "@lattice/agent-protocol";
import { tool, type FunctionTool } from "@openai/agents";
import { z } from "zod";

import type { LatticeToolClient } from "./lattice-client.js";
import type { EventSink } from "./runner.js";

/** Per-run context injected via `run(..., { context })`. */
export type LatticeRunContext = {
  client: LatticeToolClient | null;
  workspaceId?: string;
  workspaceRoot?: string;
  runId?: string;
  emitEvent?: EventSink;
};

type WorkspaceBinding = {
  workspaceId?: string;
  root?: string;
};

function runCtx(
  runContext: { context?: unknown } | undefined,
): LatticeRunContext | undefined {
  return runContext?.context as LatticeRunContext | undefined;
}

function requireClient(ctx: LatticeRunContext | undefined): LatticeToolClient {
  const client = ctx?.client;
  if (!client) {
    throw new Error(
      "Lattice tools are unavailable (set LATTICE_API_BASE_URL and LATTICE_AUTH_TOKEN)",
    );
  }
  return client;
}

function bindWorkspace(
  ctx: LatticeRunContext | undefined,
  args: { workspaceId?: string | null; root?: string | null },
): WorkspaceBinding {
  const workspaceId =
    args.workspaceId?.trim() || ctx?.workspaceId?.trim() || undefined;
  const root = args.root?.trim() || ctx?.workspaceRoot?.trim() || undefined;
  if (!workspaceId && !root) {
    throw new Error(
      "workspace binding required: pass workspaceId or root, or start_run with workspaceId/workspaceRoot",
    );
  }
  return { workspaceId, root };
}

function withWorkspace(
  ctx: LatticeRunContext | undefined,
  args: { workspaceId?: string | null; root?: string | null },
  rest: Record<string, unknown>,
): Record<string, unknown> {
  const binding = bindWorkspace(ctx, args);
  const body: Record<string, unknown> = { ...rest };
  if (binding.workspaceId) {
    body.workspaceId = binding.workspaceId;
  }
  if (binding.root) {
    body.root = binding.root;
  }
  return body;
}

function asToolJson(value: unknown): string {
  return JSON.stringify(value);
}

function newSpatialId(prefix: string): string {
  return `${prefix}-${randomUUID()}`;
}

function requireSpatialContext(
  ctx: LatticeRunContext | undefined,
): { runId: string; emitEvent: EventSink } {
  const runId = ctx?.runId?.trim();
  const emitEvent = ctx?.emitEvent;
  if (!runId || !emitEvent) {
    throw new Error(
      "Spatial tools require an active agent run (runId and event sink missing)",
    );
  }
  return { runId, emitEvent };
}

function parseAnchorJson(anchorJson: string): WorkspaceAnchor {
  let value: unknown;
  try {
    value = JSON.parse(anchorJson);
  } catch {
    throw new Error("anchorJson must be a JSON object");
  }
  return workspaceAnchorSchema.parse(value);
}

function parseAnchorsJson(anchorsJson: string): WorkspaceAnchor[] {
  let value: unknown;
  try {
    value = JSON.parse(anchorsJson);
  } catch {
    throw new Error("anchorsJson must be a JSON array");
  }
  if (!Array.isArray(value)) {
    throw new Error("anchorsJson must be a JSON array");
  }
  if (value.length < 1) {
    throw new Error("anchorsJson must contain at least one anchor");
  }
  if (value.length > MAX_OVERLAY_ANCHORS) {
    throw new Error(`anchorsJson may contain at most ${MAX_OVERLAY_ANCHORS} anchors`);
  }
  return value.map((anchor, index) => {
    try {
      return workspaceAnchorSchema.parse(anchor);
    } catch {
      throw new Error(`anchorsJson[${index}] is not a valid workspace anchor`);
    }
  });
}

function emitNavigationStep(
  runId: string,
  emitEvent: EventSink,
  label: string,
): void {
  const stepId = newSpatialId("step");
  const startedAt = Date.now();
  emitEvent({
    type: "step_started",
    runId,
    stepId,
    kind: "navigation",
    label,
  } satisfies AgentEvent);
  emitEvent({
    type: "step_completed",
    runId,
    stepId,
    durationMs: Date.now() - startedAt,
  } satisfies AgentEvent);
}

/** Emit step_started → overlay_show → step_completed on the agent event sink. */
export function emitOverlayShowSequence(
  runId: string,
  emitEvent: EventSink,
  params: {
    anchors: WorkspaceAnchor[];
    purpose: OverlayPurpose;
    commentary?: string;
    label: string;
  },
): { ok: true; overlayId: string } {
  const overlayId = newSpatialId("overlay");
  const stepId = newSpatialId("step");
  const startedAt = Date.now();

  emitEvent({
    type: "step_started",
    runId,
    stepId,
    kind: "tool",
    label: params.label,
  } satisfies AgentEvent);
  emitEvent({
    type: "overlay_show",
    runId,
    overlayId,
    anchors: params.anchors,
    purpose: params.purpose,
    ...(params.commentary !== undefined ? { commentary: params.commentary } : {}),
  } satisfies AgentEvent);
  emitEvent({
    type: "step_completed",
    runId,
    stepId,
    durationMs: Date.now() - startedAt,
  } satisfies AgentEvent);

  return { ok: true, overlayId };
}

/** Strict-schema optional string (OpenAI requires nullable, not plain optional). */
const optStr = z.string().nullable();
const optInt = z.number().int().nullable();

/**
 * Build the Phase B Lattice tool set (MCP name parity).
 */
export function createLatticeTools(): FunctionTool<LatticeRunContext, any, string>[] {
  const getCurrentContext = tool({
    name: "get_current_context",
    description:
      "Return the active Lattice workspace binding for this agent run (session id and/or root path).",
    parameters: z.object({}),
    execute: async (_args, runContext) => {
      const ctx = runCtx(runContext);
      return asToolJson({
        workspaceId: ctx?.workspaceId ?? null,
        workspaceRoot: ctx?.workspaceRoot ?? null,
        latticeApiConfigured: Boolean(ctx?.client),
      });
    },
  });

  const search = tool({
    name: "search",
    description:
      "Hybrid or FTS search over the open Lattice workspace. Returns provenance and export-policy flags.",
    parameters: z.object({
      workspaceId: optStr,
      root: optStr,
      query: z.string(),
      limit: optInt,
      mode: z.enum(["hybrid", "fts"]).nullable(),
    }),
    execute: async (args, runContext) => {
      const ctx = runCtx(runContext);
      const client = requireClient(ctx);
      // Force FTS for Phase B: hybrid/embeddings can block for seconds while
      // IndexProgress floods the shared event bus and starves the UI stream.
      const body = withWorkspace(ctx, args, {
        query: args.query,
        ...(args.limit != null ? { limit: args.limit } : {}),
        mode: "fts",
      });
      return asToolJson(await client.search(body));
    },
  });

  const read = tool({
    name: "read",
    description: "Read a bounded byte range from a workspace page/resource.",
    parameters: z.object({
      workspaceId: optStr,
      root: optStr,
      path: z.string(),
      startByte: optInt,
      endByte: optInt,
      maxBytes: optInt,
    }),
    execute: async (args, runContext) => {
      const ctx = runCtx(runContext);
      const client = requireClient(ctx);
      const body = withWorkspace(ctx, args, {
        path: args.path,
        ...(args.startByte != null ? { startByte: args.startByte } : {}),
        ...(args.endByte != null ? { endByte: args.endByte } : {}),
        ...(args.maxBytes != null ? { maxBytes: args.maxBytes } : {}),
      });
      return asToolJson(await client.read(body));
    },
  });

  const related = tool({
    name: "related",
    description: "Find related resources via backlinks and FTS.",
    parameters: z.object({
      workspaceId: optStr,
      root: optStr,
      path: z.string(),
      limit: optInt,
    }),
    execute: async (args, runContext) => {
      const ctx = runCtx(runContext);
      const client = requireClient(ctx);
      const body = withWorkspace(ctx, args, {
        path: args.path,
        ...(args.limit != null ? { limit: args.limit } : {}),
      });
      return asToolJson(await client.related(body));
    },
  });

  const buildContext = tool({
    name: "build_context",
    description:
      "Assemble bounded context excerpts for a query. Respects export_policy.",
    parameters: z.object({
      workspaceId: optStr,
      root: optStr,
      query: z.string(),
      limit: optInt,
      maxBytes: optInt,
    }),
    execute: async (args, runContext) => {
      const ctx = runCtx(runContext);
      const client = requireClient(ctx);
      const body = withWorkspace(ctx, args, {
        query: args.query,
        ...(args.limit != null ? { limit: args.limit } : {}),
        ...(args.maxBytes != null ? { maxBytes: args.maxBytes } : {}),
      });
      return asToolJson(await client.buildContext(body));
    },
  });

  const getDatasetSchema = tool({
    name: "get_dataset_schema",
    description:
      "Return column names/types for a .dataset package via a bounded LIMIT 0 describe.",
    parameters: z.object({
      workspaceId: optStr,
      root: optStr,
      path: z.string(),
      sql: optStr,
    }),
    execute: async (args, runContext) => {
      const ctx = runCtx(runContext);
      const client = requireClient(ctx);
      const body = withWorkspace(ctx, args, {
        path: args.path,
        ...(args.sql != null ? { sql: args.sql } : {}),
      });
      return asToolJson(await client.getDatasetSchema(body));
    },
  });

  const profileDataset = tool({
    name: "profile_dataset",
    description:
      "Bounded DuckDB SUMMARIZE profile for a .dataset package (optional sample-row cap).",
    parameters: z.object({
      workspaceId: optStr,
      root: optStr,
      path: z.string(),
      sql: optStr,
      maxSampleRows: optInt,
    }),
    execute: async (args, runContext) => {
      const ctx = runCtx(runContext);
      const client = requireClient(ctx);
      const body = withWorkspace(ctx, args, {
        path: args.path,
        ...(args.sql != null ? { sql: args.sql } : {}),
        ...(args.maxSampleRows != null
          ? { maxSampleRows: args.maxSampleRows }
          : {}),
      });
      return asToolJson(await client.profileDataset(body));
    },
  });

  const createProposal = tool({
    name: "create_proposal",
    description:
      "Create a reviewable transaction proposal from semantic commands. Does not apply mutations. Pass commandsJson as a JSON array string of command objects.",
    parameters: z.object({
      workspaceId: optStr,
      root: optStr,
      summary: z.string(),
      commandsJson: z
        .string()
        .describe("JSON array of semantic command objects"),
      affectedPathsJson: optStr.describe(
        "Optional JSON array of affected workspace paths",
      ),
      warningsJson: optStr.describe("Optional JSON array of warning strings"),
      sourceResource: optStr,
    }),
    execute: async (args, runContext) => {
      const ctx = runCtx(runContext);
      const client = requireClient(ctx);
      let commands: unknown;
      try {
        commands = JSON.parse(args.commandsJson);
      } catch {
        throw new Error("commandsJson must be a JSON array");
      }
      if (!Array.isArray(commands)) {
        throw new Error("commandsJson must be a JSON array");
      }
      const body = withWorkspace(ctx, args, {
        summary: args.summary,
        commands,
      });
      if (args.affectedPathsJson != null) {
        body.affectedPaths = JSON.parse(args.affectedPathsJson);
      }
      if (args.warningsJson != null) {
        body.warnings = JSON.parse(args.warningsJson);
      }
      if (args.sourceResource != null) {
        body.sourceResource = args.sourceResource;
      }
      return asToolJson(await client.createProposal(body));
    },
  });

  const listProposals = tool({
    name: "list_proposals",
    description: "List pending transaction proposals in the workspace inbox.",
    parameters: z.object({
      workspaceId: optStr,
      root: optStr,
    }),
    execute: async (args, runContext) => {
      const ctx = runCtx(runContext);
      const client = requireClient(ctx);
      const body = withWorkspace(ctx, args, {});
      return asToolJson(await client.listProposals(body));
    },
  });

  const getProposal = tool({
    name: "get_proposal",
    description: "Load one pending transaction proposal by id.",
    parameters: z.object({
      workspaceId: optStr,
      root: optStr,
      proposalId: z.string(),
    }),
    execute: async (args, runContext) => {
      const ctx = runCtx(runContext);
      const client = requireClient(ctx);
      const body = withWorkspace(ctx, args, { proposalId: args.proposalId });
      return asToolJson(await client.getProposal(body));
    },
  });

  const proposePage = tool({
    name: "propose_page",
    description: "Propose creating or updating a page. Does not write the page directly.",
    parameters: z.object({
      workspaceId: optStr,
      root: optStr,
      path: z.string(),
      content: optStr,
      title: optStr,
    }),
    execute: async (args, runContext) => {
      const ctx = runCtx(runContext);
      const client = requireClient(ctx);
      const body = withWorkspace(ctx, args, {
        path: args.path,
        ...(args.content != null ? { content: args.content } : {}),
        ...(args.title != null ? { title: args.title } : {}),
      });
      return asToolJson(await client.proposePage(body));
    },
  });

  const proposeResource = tool({
    name: "propose_resource",
    description: "Propose creating a text resource. Does not apply.",
    parameters: z.object({
      workspaceId: optStr,
      root: optStr,
      path: z.string(),
      content: z.string(),
      summary: optStr,
    }),
    execute: async (args, runContext) => {
      const ctx = runCtx(runContext);
      const client = requireClient(ctx);
      const body = withWorkspace(ctx, args, {
        path: args.path,
        content: args.content,
        ...(args.summary != null ? { summary: args.summary } : {}),
      });
      return asToolJson(await client.proposeResource(body));
    },
  });

  const proposeWorkflow = tool({
    name: "propose_workflow",
    description: "Validate workflow YAML and propose creating it. Does not apply.",
    parameters: z.object({
      workspaceId: optStr,
      root: optStr,
      path: z.string(),
      content: z.string(),
      summary: optStr,
    }),
    execute: async (args, runContext) => {
      const ctx = runCtx(runContext);
      const client = requireClient(ctx);
      const body = withWorkspace(ctx, args, {
        path: args.path,
        content: args.content,
        ...(args.summary != null ? { summary: args.summary } : {}),
      });
      return asToolJson(await client.proposeWorkflow(body));
    },
  });

  const proposeInterface = tool({
    name: "propose_interface",
    description: "Validate interface YAML and propose creating it. Does not apply.",
    parameters: z.object({
      workspaceId: optStr,
      root: optStr,
      path: z.string(),
      content: z.string(),
      summary: optStr,
    }),
    execute: async (args, runContext) => {
      const ctx = runCtx(runContext);
      const client = requireClient(ctx);
      const body = withWorkspace(ctx, args, {
        path: args.path,
        content: args.content,
        ...(args.summary != null ? { summary: args.summary } : {}),
      });
      return asToolJson(await client.proposeInterface(body));
    },
  });

  const focusAnchor = tool({
    name: "focus_anchor",
    description:
      "Request the shell to open and highlight a single workspace anchor (markdown block or dataset region). Does not mutate workspace content.",
    parameters: z.object({
      anchorJson: z
        .string()
        .describe(
          "JSON object matching WorkspaceAnchor (markdown-block or dataset-region)",
        ),
      commentary: optStr.describe("Optional short label for the overlay"),
    }),
    execute: async (args, runContext) => {
      const ctx = runCtx(runContext);
      const { runId, emitEvent } = requireSpatialContext(ctx);
      const anchor = parseAnchorJson(args.anchorJson);
      const commentary = args.commentary?.trim() || undefined;

      emitNavigationStep(runId, emitEvent, "Open anchored resource");
      const result = emitOverlayShowSequence(runId, emitEvent, {
        anchors: [anchor],
        purpose: "attention",
        commentary,
        label: "Focus anchor",
      });
      return asToolJson(result);
    },
  });

  const highlightAnchors = tool({
    name: "highlight_anchors",
    description:
      "Highlight one or more workspace anchors without changing the active resource. Up to 20 anchors per call.",
    parameters: z.object({
      anchorsJson: z
        .string()
        .describe(
          "JSON array of WorkspaceAnchor objects (markdown-block or dataset-region)",
        ),
      purpose: overlayPurposeSchema,
      commentary: optStr.describe("Optional short label for the overlay"),
    }),
    execute: async (args, runContext) => {
      const ctx = runCtx(runContext);
      const { runId, emitEvent } = requireSpatialContext(ctx);
      const anchors = parseAnchorsJson(args.anchorsJson);
      const commentary = args.commentary?.trim() || undefined;

      const result = emitOverlayShowSequence(runId, emitEvent, {
        anchors,
        purpose: args.purpose,
        commentary,
        label: "Highlight anchors",
      });
      return asToolJson(result);
    },
  });

  const proposeArtifact = tool({
    name: "propose_artifact",
    description: "Validate artifact.yaml and propose creating the manifest. Does not apply.",
    parameters: z.object({
      workspaceId: optStr,
      root: optStr,
      path: z.string(),
      content: z.string(),
      summary: optStr,
    }),
    execute: async (args, runContext) => {
      const ctx = runCtx(runContext);
      const client = requireClient(ctx);
      const body = withWorkspace(ctx, args, {
        path: args.path,
        content: args.content,
        ...(args.summary != null ? { summary: args.summary } : {}),
      });
      return asToolJson(await client.proposeArtifact(body));
    },
  });

  return [
    getCurrentContext,
    focusAnchor,
    highlightAnchors,
    search,
    read,
    related,
    buildContext,
    getDatasetSchema,
    profileDataset,
    createProposal,
    listProposals,
    getProposal,
    proposePage,
    proposeResource,
    proposeWorkflow,
    proposeInterface,
    proposeArtifact,
  ];
}
