/**
 * Typed client for latticed's authenticated localhost HTTP API.
 * Same semantics as `latticed mcp` tools (Phase B; Phase E switches to MCP HTTP).
 */

export type LatticeToolClientOptions = {
  baseUrl: string;
  authToken: string;
  fetchImpl?: typeof fetch;
};

export class LatticeApiError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = "LatticeApiError";
    this.status = status;
    this.code = code;
  }
}

export class LatticeToolClient {
  readonly baseUrl: string;
  private readonly authToken: string;
  private readonly fetchImpl: typeof fetch;

  constructor(options: LatticeToolClientOptions) {
    this.baseUrl = options.baseUrl.replace(/\/$/, "");
    this.authToken = options.authToken;
    this.fetchImpl = options.fetchImpl ?? fetch;
  }

  async post<T = unknown>(path: string, body: Record<string, unknown>): Promise<T> {
    const url = `${this.baseUrl}${path.startsWith("/") ? path : `/${path}`}`;
    const response = await this.fetchImpl(url, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        authorization: `Bearer ${this.authToken}`,
      },
      body: JSON.stringify(body),
    });

    const text = await response.text();
    let parsed: unknown = undefined;
    if (text.length > 0) {
      try {
        parsed = JSON.parse(text) as unknown;
      } catch {
        parsed = { raw: text };
      }
    }

    if (!response.ok) {
      const errObj =
        typeof parsed === "object" && parsed !== null && "error" in parsed
          ? (parsed as { error?: { code?: string; message?: string } }).error
          : undefined;
      throw new LatticeApiError(
        response.status,
        errObj?.code ?? "http_error",
        errObj?.message ?? `Lattice API ${response.status} for ${path}`,
      );
    }

    return parsed as T;
  }

  search(body: Record<string, unknown>) {
    return this.post("/v1/search", body);
  }

  read(body: Record<string, unknown>) {
    return this.post("/v1/read", body);
  }

  related(body: Record<string, unknown>) {
    return this.post("/v1/related", body);
  }

  buildContext(body: Record<string, unknown>) {
    return this.post("/v1/build_context", body);
  }

  getDatasetSchema(body: Record<string, unknown>) {
    return this.post("/v1/datasets/schema", body);
  }

  profileDataset(body: Record<string, unknown>) {
    return this.post("/v1/datasets/profile", body);
  }

  createProposal(body: Record<string, unknown>) {
    return this.post("/v1/proposals", body);
  }

  listProposals(body: Record<string, unknown>) {
    return this.post("/v1/proposals/list", body);
  }

  getProposal(body: Record<string, unknown>) {
    return this.post("/v1/proposals/get", body);
  }

  proposePage(body: Record<string, unknown>) {
    return this.post("/v1/proposals/propose_page", body);
  }

  proposeResource(body: Record<string, unknown>) {
    return this.post("/v1/proposals/propose_resource", body);
  }

  proposeWorkflow(body: Record<string, unknown>) {
    return this.post("/v1/proposals/propose_workflow", body);
  }

  proposeInterface(body: Record<string, unknown>) {
    return this.post("/v1/proposals/propose_interface", body);
  }

  proposeArtifact(body: Record<string, unknown>) {
    return this.post("/v1/proposals/propose_artifact", body);
  }
}

/** Build a client from process env when Lattice HTTP tools are configured. */
export function latticeClientFromEnv(
  env: NodeJS.ProcessEnv = process.env,
): LatticeToolClient | null {
  const baseUrl = env.LATTICE_API_BASE_URL?.trim();
  const authToken = env.LATTICE_AUTH_TOKEN?.trim();
  if (!baseUrl || !authToken) {
    return null;
  }
  return new LatticeToolClient({ baseUrl, authToken });
}
