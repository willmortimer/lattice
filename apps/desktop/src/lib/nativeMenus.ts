import { Menu } from "@tauri-apps/api/menu";

import { inBrowser } from "../demo";

let activeMenu: Menu | null = null;

async function popup(menu: Menu): Promise<void> {
  if (activeMenu) await activeMenu.close();
  activeMenu = menu;
  await menu.popup();
}

export function isEditableTarget(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLElement &&
    Boolean(target.closest("input, textarea, [contenteditable='true']"))
  );
}

export async function showNativeEditMenu(): Promise<void> {
  if (inBrowser) return;
  const menu = await Menu.new({
    items: [
      { item: "Undo" },
      { item: "Redo" },
      { item: "Separator" },
      { item: "Cut" },
      { item: "Copy" },
      { item: "Paste" },
      { item: "Separator" },
      { item: "SelectAll" },
    ],
  });
  await popup(menu);
}

export interface TreeResourceMenuActions {
  open: () => void;
  inspect: () => void;
  openExternally?: () => void;
  copyPath: () => void;
  rename: () => void;
  duplicate: () => void;
  delete: () => void;
}

export interface TreeFolderMenuActions {
  newPage: () => void;
  newFolder: () => void;
  copyPath: () => void;
}

/** @deprecated Use `showNativeTreeResourceMenu` for sidebar tree rows. */
export async function showNativeResourceMenu(actions: {
  open: () => void;
  inspect: () => void;
  openExternally?: () => void;
}): Promise<void> {
  await showNativeTreeResourceMenu({
    ...actions,
    copyPath: () => undefined,
    rename: () => undefined,
    duplicate: () => undefined,
    delete: () => undefined,
  });
}

export async function showNativeTreeResourceMenu(actions: TreeResourceMenuActions): Promise<void> {
  if (inBrowser) return;
  const menu = await Menu.new({
    items: [
      { text: "Open", action: actions.open },
      { text: "Inspect", action: actions.inspect },
      ...(actions.openExternally
        ? [
            { item: "Separator" as const },
            { text: "Open Externally", action: actions.openExternally },
          ]
        : []),
      { item: "Separator" },
      { text: "Copy Path", action: actions.copyPath },
      { item: "Separator" },
      { text: "Rename", action: actions.rename },
      { text: "Duplicate", action: actions.duplicate },
      { item: "Separator" },
      { text: "Delete", action: actions.delete },
    ],
  });
  await popup(menu);
}

export async function showNativeTreeFolderMenu(actions: TreeFolderMenuActions): Promise<void> {
  if (inBrowser) return;
  const menu = await Menu.new({
    items: [
      { text: "New Page", action: actions.newPage },
      { text: "New Folder", action: actions.newFolder },
      { item: "Separator" },
      { text: "Copy Path", action: actions.copyPath },
    ],
  });
  await popup(menu);
}

export function installNativeContextMenus(enabled: () => boolean): () => void {
  const onContextMenu = (event: MouseEvent) => {
    event.preventDefault();
    if (enabled() && isEditableTarget(event.target)) {
      void showNativeEditMenu();
    }
  };
  window.addEventListener("contextmenu", onContextMenu, { capture: true });
  return () => window.removeEventListener("contextmenu", onContextMenu, { capture: true });
}
