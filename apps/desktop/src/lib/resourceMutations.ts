import { invoke } from "@tauri-apps/api/core";

/**
 * Thin adapters for workspace resource mutations exposed by the desktop shell.
 *
 * Delete, move, duplicate, rename, and folder creation flow through the
 * semantic command core and participate in command history / undo.
 */

export async function deleteResource(root: string, path: string): Promise<void> {
  await invoke("delete_resource", { root, path });
}

export async function moveResource(
  root: string,
  from: string,
  toDir: string,
): Promise<void> {
  await invoke("move_resource", { root, from, toDir });
}

/** Returns the workspace-relative path of the duplicate. */
export async function duplicateResource(root: string, path: string): Promise<string> {
  return invoke<string>("duplicate_resource", { root, path });
}

export async function createFolder(root: string, path: string): Promise<void> {
  await invoke("create_folder", { root, path });
}
