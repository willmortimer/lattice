export type HealthResponse = {
  status: "ok" | "degraded";
  version: string;
};

export async function handleHealth(version: string): Promise<HealthResponse> {
  return { status: "ok", version };
}
