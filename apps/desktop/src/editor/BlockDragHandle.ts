import { Extension } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";

const key = new PluginKey("latticeBlockDragHandle");
export const BLOCK_DRAG_MIME = "application/x-lattice-block-pos";

/**
 * Pointer drag handles for top-level blocks. Keyboard Alt+↑/↓ move commands
 * from StarterKit remain available; this only adds a mouse affordance.
 *
 * HTML5 drag from inside `contenteditable` only works when the handle is
 * marked `contentEditable=false` and mousedown does not hand focus back to
 * ProseMirror (which cancels the drag in WebKit/Chromium).
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
                    handle.title = "Drag to reorder block";
                    handle.setAttribute("aria-label", "Drag to reorder block");
                    handle.contentEditable = "false";
                    handle.draggable = true;
                    handle.dataset.blockPos = String(offset);
                    handle.addEventListener("mousedown", (event) => {
                      // Keep ProseMirror from taking selection/focus before dragstart.
                      event.preventDefault();
                    });
                    handle.addEventListener("dragstart", (event) => {
                      event.stopPropagation();
                      const pos = handle.dataset.blockPos ?? String(offset);
                      event.dataTransfer?.setData(BLOCK_DRAG_MIME, pos);
                      event.dataTransfer?.setData("text/plain", pos);
                      if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
                      handle.classList.add("block-drag-handle-active");
                    });
                    handle.addEventListener("dragend", () => {
                      handle.classList.remove("block-drag-handle-active");
                    });
                    return handle;
                  },
                  { side: -1, key: `block-drag-${offset}` },
                ),
              );
            });
            return DecorationSet.create(state.doc, decorations);
          },
          handleDOMEvents: {
            dragover(_view, event) {
              if (!event.dataTransfer?.types.includes(BLOCK_DRAG_MIME)) {
                return false;
              }
              event.preventDefault();
              if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
              return true;
            },
            drop(view, event) {
              const raw = event.dataTransfer?.getData(BLOCK_DRAG_MIME);
              if (!raw) return false;
              event.preventDefault();
              const fromPos = Number(raw);
              const coords = view.posAtCoords({ left: event.clientX, top: event.clientY });
              if (!coords) return true;
              const $to = view.state.doc.resolve(coords.pos);
              let toPos = 0;
              for (let depth = $to.depth; depth > 0; depth -= 1) {
                if ($to.node(depth - 1).type.name === "doc") {
                  toPos = $to.before(depth);
                  break;
                }
              }
              const node = view.state.doc.nodeAt(fromPos);
              if (!node) return true;
              if (toPos === fromPos || toPos === fromPos + node.nodeSize) return true;

              let tr = view.state.tr.delete(fromPos, fromPos + node.nodeSize);
              const mappedTo = tr.mapping.map(toPos, -1);
              tr = tr.insert(mappedTo, node);
              view.dispatch(tr.scrollIntoView());
              return true;
            },
          },
        },
      }),
    ];
  },
});
