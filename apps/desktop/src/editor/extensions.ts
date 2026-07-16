import type { Extensions } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import { TableKit } from "@tiptap/extension-table";
import Image from "@tiptap/extension-image";

/**
 * The single source of truth for the editor's node/mark set. Both the live
 * Tiptap editor (`PageEditor`) and the standalone markdown codec
 * (`markdown.ts`) build their schema from this list, so the two always
 * agree on what a Lattice page document can contain.
 *
 * Underline is dropped: it has no CommonMark/GFM syntax, and v0's markdown
 * dialect (docs/07) only commits to what round-trips losslessly.
 *
 * `Image` is inline (not StarterKit's implicit block grouping default)
 * because CommonMark images are inline content — `![alt](src)` sits
 * alongside text within a paragraph, exactly like `markdown-it`'s
 * `image` token appears as a child of the paragraph's inline stream
 * (see `markdown.ts`'s parser/serializer entries for it).
 */
export const editorExtensions: Extensions = [
  StarterKit.configure({ underline: false }),
  TableKit.configure({ table: { resizable: false } }),
  Image.configure({ inline: true, allowBase64: false }),
];
