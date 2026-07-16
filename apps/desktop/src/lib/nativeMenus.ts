import { Menu } from "@tauri-apps/api/menu";

import { inBrowser } from "../demo";

let activeMenu: Menu | null = null;

async function popup(menu: Menu): Promise<void> {
  if (activeMenu) await activeMenu.close();
  activeMenu = menu;
  await menu.popup();
}

function isEditable(target: EventTarget | null): boolean {
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

export async function showNativeResourceMenu(actions: {
  open: () => void;
  inspect: () => void;
  openExternally?: () => void;
}): Promise<void> {
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
    ],
  });
  await popup(menu);
}

export function installNativeContextMenus(enabled: () => boolean): () => void {
  const onContextMenu = (event: MouseEvent) => {
    event.preventDefault();
    if (enabled() && isEditable(event.target)) {
      void showNativeEditMenu();
    }
  };
  window.addEventListener("contextmenu", onContextMenu, { capture: true });
  return () => window.removeEventListener("contextmenu", onContextMenu, { capture: true });
}
