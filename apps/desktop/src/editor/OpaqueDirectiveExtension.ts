import { Node, mergeAttributes } from "@tiptap/core";

/**
 * Verbatim preservation for supported-directive-shaped blocks Lattice does not
 * model yet (e.g. `:::lattice-code`) or other `:::name` fences.
 */
export const OpaqueDirective = Node.create({
  name: "opaqueDirective",
  group: "block",
  atom: true,
  selectable: true,
  draggable: false,

  addAttributes() {
    return {
      raw: { default: "" },
    };
  },

  parseHTML() {
    return [{ tag: 'div[data-type="opaque-directive"]' }];
  },

  renderHTML({ HTMLAttributes }) {
    return ["div", mergeAttributes(HTMLAttributes, { "data-type": "opaque-directive" })];
  },
});
