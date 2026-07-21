import { Extension } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";

import {
  isBlockDragArmed,
  reorderBlockTransaction,
  resolveBlockDragFromPos,
  topLevelBlockPos,
} from "./blockReorder";

const key = new PluginKey("latticeBlockDragHandle");
export const BLOCK_DRAG_MIME = "application/x-lattice-block-pos";

/**
 * Module-level drag source. WKWebView often omits custom MIME types from
 * `dataTransfer.types` during `dragover`, so drop arming cannot rely on them.
 */
let activeDragFromPos: number | null = null;

/** Read the armed drag source (tests / drop fallback). */
export function getActiveBlockDragFromPos(): number | null {
  return activeDragFromPos;
}

/** Arm or clear the module-level drag source (dragstart / tests). */
export function setActiveBlockDragFromPos(pos: number | null): void {
  activeDragFromPos = pos;
}

/**
 * Pointer drag handles for top-level blocks. Keyboard Alt+↑/↓ move commands
 * from StarterKit remain available; this only adds a mouse affordance.
 *
 * HTML5 drag from inside `contenteditable` only works when the handle is
 * marked `contentEditable=false`. Avoid `mousedown` `preventDefault` — that
 * cancels `dragstart` in WebKit (Tauri macOS).
 */
export const BlockDragHandle = Extension.create({
  name: "blockDragHandle",

  addProseMirrorPlugins() {
    return [
      new Plugin({
        key,
        props: {
          decorations(state) {
            const decorations: Decoration[] = [];
            state.doc.forEach((node, offset) => {
              if (!node.isBlock) return;
              decorations.push(
                Decoration.widget(
                  offset,
                  () => {
                    const handle = document.createElement("button");
                    handle.type = "button";
                    handle.className = "block-drag-handle";
                    handle.title = "Drag to reorder block (or Alt+↑/↓)";
                    handle.setAttribute("aria-label", "Drag to reorder block");
                    handle.contentEditable = "false";
                    handle.draggable = true;
                    handle.dataset.blockPos = String(offset);
                    handle.addEventListener("dragstart", (event) => {
                      event.stopPropagation();
                      const pos = Number(handle.dataset.blockPos ?? offset);
                      activeDragFromPos = Number.isFinite(pos) ? pos : offset;
                      event.dataTransfer?.setData(BLOCK_DRAG_MIME, String(activeDragFromPos));
                      event.dataTransfer?.setData("text/plain", String(activeDragFromPos));
                      if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
                      handle.classList.add("block-drag-handle-active");
                    });
                    handle.addEventListener("dragend", () => {
                      activeDragFromPos = null;
                      handle.classList.remove("block-drag-handle-active");
                    });
                    return handle;
                  },
                  {
                    side: -1,
                    key: `block-drag-${offset}`,
                    stopEvent: () => true,
                    ignoreSelection: true,
                  },
                ),
              );
            });
            return DecorationSet.create(state.doc, decorations);
          },
          handleDOMEvents: {
            dragover(_view, event) {
              // Arm via module-level pos — do not require MIME in types.
              if (!isBlockDragArmed(activeDragFromPos)) return false;
              event.preventDefault();
              if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
              return true;
            },
            drop(view, event) {
              const fromPos = resolveBlockDragFromPos(
                event.dataTransfer?.getData(BLOCK_DRAG_MIME),
                event.dataTransfer?.getData("text/plain"),
                activeDragFromPos,
              );
              activeDragFromPos = null;
              if (fromPos === null) return false;
              event.preventDefault();
              const coords = view.posAtCoords({ left: event.clientX, top: event.clientY });
              if (!coords) return true;
              const toPos = topLevelBlockPos(view.state.doc, coords.pos);
              const tr = reorderBlockTransaction(view.state, fromPos, toPos);
              if (tr) view.dispatch(tr);
              return true;
            },
          },
        },
      }),
    ];
  },
});
