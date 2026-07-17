import { Node, mergeAttributes } from "@tiptap/core";

/** Block atom for `:::lattice-embed` resource references (docs/07). */
export const LatticeEmbed = Node.create({
  name: "latticeEmbed",
  group: "block",
  atom: true,
  selectable: true,
  draggable: false,

  addAttributes() {
    return {
      resource: { default: "" },
      view: { default: null },
      height: { default: null },
      lines: { default: null },
      fallback: { default: null },
      extraFields: { default: {} },
      extraFieldKeys: { default: [] },
    };
  },

  parseHTML() {
    return [{ tag: 'div[data-type="lattice-embed"]' }];
  },

  renderHTML({ HTMLAttributes }) {
    return ["div", mergeAttributes(HTMLAttributes, { "data-type": "lattice-embed" })];
  },
});
