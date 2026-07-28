import { invoke } from "./ipc";

export interface CloudUser {
  id: string;
  username: string;
  display_name: string;
  email: string | null;
  created_at: number;
}

export interface CloudSessionStatus {
  signedIn: boolean;
  cloudUrl: string;
  user?: CloudUser;
  error?: string;
}

export async function getCloudSessionStatus(): Promise<CloudSessionStatus> {
  return invoke<CloudSessionStatus>("cloud_session_status");
}

export async function cloudSignIn(email: string, password: string): Promise<CloudSessionStatus> {
  return invoke<CloudSessionStatus>("cloud_sign_in", { email, password });
}

export async function cloudSignOut(): Promise<CloudSessionStatus> {
  return invoke<CloudSessionStatus>("cloud_sign_out");
}
