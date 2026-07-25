import { describe, expect, it, vi } from "vitest";
import { RunContext } from "@openai/agents";

import type { AgentEvent } from "@lattice/agent-protocol";

import { LatticeApiError, LatticeToolClient } from "./lattice-client.js";
import {
  createLatticeTools,
  emitOverlayShowSequence,
  type LatticeRunContext,
} from "./tools.js";

describe("LatticeToolClient", () => {
  it("posts JSON with bearer auth", async () => {
    const fetchImpl = vi.fn(
      async (_url: string | URL | Request, _init?: RequestInit) => {
        return new Response(JSON.stringify({ hits: [] }), {
          status: 200,
          headers: { "content-type": "application/json" },
        });
      },
    );
    const client = new LatticeToolClient({
      baseUrl: "http://127.0.0.1:18787",
      authToken: "tok",
      fetchImpl: fetchImpl as unknown as typeof fetch,
    });
    const result = await client.search({ query: "Events", workspaceId: "ws" });
    expect(result).toEqual({ hits: [] });
    expect(fetchImpl).toHaveBeenCalledOnce();
    const call = fetchImpl.mock.calls[0];
    expect(call?.[0]).toBe("http://127.0.0.1:18787/v1/search");
    expect(call?.[1]?.headers).toMatchObject({
      authorization: "Bearer tok",
    });
  });

  it("maps API errors", async () => {
    const fetchImpl = vi.fn(async () => {
      return new Response(
        JSON.stringify({
          error: { code: "not_found", message: "missing workspace" },
        }),
        { status: 404, headers: { "content-type": "application/json" } },
      );
    });
    const client = new LatticeToolClient({
      baseUrl: "http://127.0.0.1:18787/",
      authToken: "tok",
      fetchImpl: fetchImpl as unknown as typeof fetch,
    });
    await expect(client.read({ path: "x.md", root: "/tmp/ws" })).rejects.toMatchObject({
      name: "LatticeApiError",
      status: 404,
      code: "not_found",
    } satisfies Partial<LatticeApiError>);
  });
});

describe("createLatticeTools", () => {
  it("registers MCP-parity tool names plus get_current_context", () => {
    const names = createLatticeTools().map((t) => t.name).sort();
    expect(names).toEqual(
      [
        "build_context",
        "create_proposal",
        "focus_anchor",
        "get_current_context",
        "get_dataset_schema",
        "get_proposal",
        "highlight_anchors",
        "list_proposals",
        "profile_dataset",
        "propose_artifact",
        "propose_interface",
        "propose_page",
        "propose_resource",
        "propose_workflow",
        "read",
        "related",
        "search",
      ].sort(),
    );
  });

  it("search uses run context workspace binding", async () => {
    const fetchImpl = vi.fn(
      async (_url: string | URL | Request, _init?: RequestInit) => {
        return new Response(JSON.stringify({ ok: true }), {
          status: 200,
          headers: { "content-type": "application/json" },
        });
      },
    );
    const client = new LatticeToolClient({
      baseUrl: "http://127.0.0.1:18787",
      authToken: "tok",
      fetchImpl: fetchImpl as unknown as typeof fetch,
    });
    const search = createLatticeTools().find((t) => t.name === "search");
    expect(search).toBeDefined();
    const ctx: LatticeRunContext = {
      client,
      workspaceId: "ws-from-run",
    };
    const out = await search!.invoke(
      new RunContext(ctx),
      JSON.stringify({
        query: "Events",
        workspaceId: null,
        root: null,
        limit: null,
        mode: null,
      }),
    );
    expect(JSON.parse(String(out))).toEqual({ ok: true });
    const body = JSON.parse(String(fetchImpl.mock.calls[0]?.[1]?.body));
    expect(body).toMatchObject({
      query: "Events",
      workspaceId: "ws-from-run",
    });
  });

  it("focus_anchor emits navigation then overlay event sequence", async () => {
    const events: AgentEvent[] = [];
    const focus = createLatticeTools().find((t) => t.name === "focus_anchor");
    expect(focus).toBeDefined();

    const ctx: LatticeRunContext = {
      client: null,
      runId: "run-spatial-1",
      emitEvent: (event) => {
        events.push(event);
      },
    };

    const anchor = {
      kind: "markdown-block",
      resourceId: "page:notes",
      blockId: "blk-2",
    };
    const out = await focus!.invoke(
      new RunContext(ctx),
      JSON.stringify({
        anchorJson: JSON.stringify(anchor),
        commentary: "See this block",
      }),
    );
    const parsed = JSON.parse(String(out)) as { ok: boolean; overlayId: string };
    expect(parsed.ok).toBe(true);
    expect(parsed.overlayId).toMatch(/^overlay-/);

    expect(events.map((e) => e.type)).toEqual([
      "step_started",
      "step_completed",
      "step_started",
      "overlay_show",
      "step_completed",
    ]);
    expect(events[0]).toMatchObject({
      type: "step_started",
      runId: "run-spatial-1",
      kind: "navigation",
    });
    const overlay = events.find((e) => e.type === "overlay_show");
    expect(overlay).toMatchObject({
      type: "overlay_show",
      runId: "run-spatial-1",
      overlayId: parsed.overlayId,
      purpose: "attention",
      commentary: "See this block",
      anchors: [anchor],
    });
  });

  it("highlight_anchors emits overlay event sequence", async () => {
    const events: AgentEvent[] = [];
    const highlight = createLatticeTools().find(
      (t) => t.name === "highlight_anchors",
    );
    expect(highlight).toBeDefined();

    const ctx: LatticeRunContext = {
      client: null,
      runId: "run-spatial-2",
      emitEvent: (event) => {
        events.push(event);
      },
    };

    const anchors = [
      {
        kind: "dataset-region",
        resourceId: "table:inventory",
        rowKeys: ["row-7"],
        columns: ["sku"],
      },
    ];
    const out = await highlight!.invoke(
      new RunContext(ctx),
      JSON.stringify({
        anchorsJson: JSON.stringify(anchors),
        purpose: "evidence",
        commentary: null,
      }),
    );
    const parsed = JSON.parse(String(out)) as { ok: boolean; overlayId: string };
    expect(parsed.ok).toBe(true);

    expect(events.map((e) => e.type)).toEqual([
      "step_started",
      "overlay_show",
      "step_completed",
    ]);
    expect(events[1]).toMatchObject({
      type: "overlay_show",
      runId: "run-spatial-2",
      overlayId: parsed.overlayId,
      purpose: "evidence",
      anchors,
    });
  });

  it("highlight_anchors rejects more than MAX_OVERLAY_ANCHORS", async () => {
    const highlight = createLatticeTools().find(
      (t) => t.name === "highlight_anchors",
    );
    expect(highlight).toBeDefined();

    const ctx: LatticeRunContext = {
      client: null,
      runId: "run-spatial-3",
      emitEvent: () => {},
    };
    const tooMany = Array.from({ length: 21 }, (_, index) => ({
      kind: "markdown-block",
      resourceId: `page:${index}`,
      blockId: `blk-${index}`,
    }));

    const out = await highlight!.invoke(
      new RunContext(ctx),
      JSON.stringify({
        anchorsJson: JSON.stringify(tooMany),
        purpose: "attention",
        commentary: null,
      }),
    );
    expect(String(out)).toMatch(/at most 20 anchors/);
  });
});

describe("emitOverlayShowSequence", () => {
  it("emits step_started, overlay_show, step_completed in order", () => {
    const events: AgentEvent[] = [];
    const anchor = {
      kind: "markdown-block" as const,
      resourceId: "page:notes",
      blockId: "blk-1",
    };

    const result = emitOverlayShowSequence("run-1", (event) => {
      events.push(event);
    }, {
      anchors: [anchor],
      purpose: "attention",
      label: "Highlight anchors",
    });

    expect(result).toEqual({ ok: true, overlayId: result.overlayId });
    expect(events.map((e) => e.type)).toEqual([
      "step_started",
      "overlay_show",
      "step_completed",
    ]);
    expect(events[0]?.type === "step_started" && events[0].stepId).toBeTruthy();
    expect(
      events[2]?.type === "step_completed" && events[2].stepId,
    ).toBe(events[0]?.type === "step_started" ? events[0].stepId : undefined);
  });
});
