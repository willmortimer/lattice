import { describe, expect, it, vi } from "vitest";
import { RunContext } from "@openai/agents";

import { LatticeApiError, LatticeToolClient } from "./lattice-client.js";
import { createLatticeTools, type LatticeRunContext } from "./tools.js";

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
        "get_current_context",
        "get_dataset_schema",
        "get_proposal",
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
});
