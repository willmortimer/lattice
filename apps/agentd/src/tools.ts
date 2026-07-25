/**
 * OpenAI Agents SDK tools mirroring latticed MCP / localhost HTTP.
 */

import { tool, type FunctionTool } from "@openai/agents";
import { z } from "zod";

import type { LatticeToolClient } from "./lattice-client.js";

/** Per-run context injected via `run(..., { context })`. */
export type LatticeRunContext = {
  client: LatticeToolClient | null;
  workspaceId?: string;
  workspaceRoot?: string;
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
      // Prefer FTS by default: hybrid waits on embeddings and can stall the
      // streamed tool turn under Pioneer embedding load / cold index.
      const mode = args.mode ?? "fts";
      const body = withWorkspace(ctx, args, {
        query: args.query,
        ...(args.limit != null ? { limit: args.limit } : {}),
        mode,
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
    description: "Propose creating a page. Does not write the page directly.",
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
