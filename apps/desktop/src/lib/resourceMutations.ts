import { invoke } from "@tauri-apps/api/core";

/**
 * Thin adapters for workspace resource mutations exposed by the desktop shell.
 *
 * Delete, move, duplicate, rename, and folder creation flow through the
 * semantic command core and participate in command history / undo.
 */

export async function deleteResource(root: string, path: string): Promise<void> {
  await deleteResources(root, [path]);
}

/** Delete paths in one semantic transaction (one undo restores all). */
export async function deleteResources(root: string, paths: readonly string[]): Promise<void> {
  await invoke("delete_resources", { root, paths: [...paths] });
}

export async function moveResource(
  root: string,
  from: string,
  toDir: string,
): Promise<void> {
  await moveResources(root, [from], toDir);
}

/** Move paths into `toDir` in one semantic transaction (one undo). */
export async function moveResources(
  root: string,
  fromPaths: readonly string[],
  toDir: string,
): Promise<void> {
  await invoke("move_resources", { root, fromPaths: [...fromPaths], toDir });
}

/** Returns the workspace-relative path of the duplicate. */
export async function duplicateResource(root: string, path: string): Promise<string> {
  return invoke<string>("duplicate_resource", { root, path });
}

export async function createFolder(root: string, path: string): Promise<void> {
  await invoke("create_folder", { root, path });
}
