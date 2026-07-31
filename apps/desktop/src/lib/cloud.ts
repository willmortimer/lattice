import { invoke } from "./ipc";

export interface CloudUser {
  id: string;
  username: string;
  display_name: string;
  email: string | null;
  created_at: number;
}

export type AiAccess = "none" | "allowlisted" | "paid";

export interface CloudEntitlements {
  ai_access: AiAccess;
  ai_daily_request_budget: number;
  ai_daily_requests_used: number;
}

export interface CloudPreferences {
  ai_audit_enabled: boolean;
  anonymous_telemetry_enabled: boolean;
}

export interface CloudSessionStatus {
  signedIn: boolean;
  cloudUrl: string;
  user?: CloudUser;
  entitlements?: CloudEntitlements;
  preferences?: CloudPreferences;
  error?: string;
}

/** True when Lattice paid AI can run for this cloud session. */
export function isCloudAiEntitled(status: Pick<CloudSessionStatus, "signedIn" | "entitlements">): boolean {
  if (!status.signedIn) {
    return false;
  }
  const access = status.entitlements?.ai_access;
  if (access === undefined) {
    // Legacy `/v1/me` without entitlements: signed-in is enough.
    return true;
  }
  return access === "allowlisted" || access === "paid";
}

export async function getCloudSessionStatus(): Promise<CloudSessionStatus> {
  return invoke<CloudSessionStatus>("cloud_session_status");
}

export async function cloudSignIn(email: string, password: string): Promise<CloudSessionStatus> {
  return invoke<CloudSessionStatus>("cloud_sign_in", { email, password });
}

export async function cloudSignInApple(): Promise<CloudSessionStatus> {
  return invoke<CloudSessionStatus>("cloud_sign_in_apple");
}

export async function cloudSignOut(): Promise<CloudSessionStatus> {
  return invoke<CloudSessionStatus>("cloud_sign_out");
}

export async function cloudUpdatePreferences(input: {
  aiAuditEnabled?: boolean;
  anonymousTelemetryEnabled?: boolean;
}): Promise<CloudPreferences> {
  return invoke<CloudPreferences>("cloud_update_preferences", {
    aiAuditEnabled: input.aiAuditEnabled ?? null,
    anonymousTelemetryEnabled: input.anonymousTelemetryEnabled ?? null,
  });
}

export async function emitProductTelemetry(
  name: string,
  properties?: Record<string, string | number | boolean | null>,
): Promise<void> {
  await invoke("product_telemetry_emit", { name, properties: properties ?? null });
}
