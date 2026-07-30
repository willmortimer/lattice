import { hasTauri, invoke } from "./ipc";

/** Whether a BYO OpenAI API key is stored (never returns the secret). */
export async function hasOpenaiApiKey(): Promise<boolean> {
  if (!hasTauri) return false;
  return invoke<boolean>("has_openai_api_key");
}

/** Store a BYO OpenAI API key in the OS keychain. */
export async function setOpenaiApiKey(key: string): Promise<void> {
  if (!hasTauri) {
    throw new Error("OpenAI API key requires the native desktop shell");
  }
  await invoke("set_openai_api_key", { key });
}

/** Remove the BYO OpenAI API key from the OS keychain. */
export async function clearOpenaiApiKey(): Promise<void> {
  if (!hasTauri) {
    throw new Error("OpenAI API key requires the native desktop shell");
  }
  await invoke("clear_openai_api_key");
}
