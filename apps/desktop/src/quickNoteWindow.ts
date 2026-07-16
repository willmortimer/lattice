import { emitTo } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

export const QUICK_NOTE_SHORTCUT = "CommandOrControl+Shift+Space";

export async function showQuickNote(workspaceRoot?: string | null): Promise<void> {
  const quickNote = await WebviewWindow.getByLabel("quick-note");
  if (!quickNote) throw new Error("Quick Note window is unavailable.");
  await quickNote.show();
  await quickNote.setFocus();
  await emitTo("quick-note", "quick-note-open", { root: workspaceRoot ?? null });
}
