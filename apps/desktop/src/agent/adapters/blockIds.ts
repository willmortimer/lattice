import type { Node as ProseMirrorNode } from "@tiptap/pm/model";

export type StructuralBlockKind = "heading" | "paragraph" | "code" | "table" | "list";

export interface BlockRange {
  blockId: string;
  from: number;
  to: number;
}

interface HeadingFrame {
  level: number;
  text: string;
}

function structuralBlockKind(node: ProseMirrorNode): StructuralBlockKind {
  switch (node.type.name) {
    case "heading":
      return "heading";
    case "paragraph":
      return "paragraph";
    case "codeBlock":
      return "code";
    case "table":
      return "table";
    case "bulletList":
    case "orderedList":
      return "list";
    default:
      return "paragraph";
  }
}

function updateHeadingStack(stack: HeadingFrame[], node: ProseMirrorNode): void {
  const level = Number(node.attrs.level);
  if (!Number.isFinite(level)) return;
  while (stack.length > 0 && stack[stack.length - 1]!.level >= level) {
    stack.pop();
  }
  stack.push({ level, text: node.textContent.trim() });
}

export function structuralBlockId(
  headingPath: readonly string[],
  kind: StructuralBlockKind,
  occurrenceCounts: Map<string, number>,
): string {
  const path = headingPath.length === 0 ? "root" : headingPath.join("/");
  const key = `${path}|${kind}`;
  const occurrence = occurrenceCounts.get(key) ?? 0;
  occurrenceCounts.set(key, occurrence + 1);
  return `${path}|${kind}#${occurrence}`;
}

/** Walk top-level blocks and compute indexer-stable structural ids. */
export function blockRangesForDocument(doc: ProseMirrorNode): BlockRange[] {
  const ranges: BlockRange[] = [];
  const headingStack: HeadingFrame[] = [];
  const occurrenceCounts = new Map<string, number>();

  doc.forEach((node, offset) => {
    if (!node.isBlock) return;
    if (node.type.name === "heading") {
      updateHeadingStack(headingStack, node);
    }
    const headingPath = headingStack.map((frame) => frame.text);
    const kind = structuralBlockKind(node);
    const blockId = structuralBlockId(headingPath, kind, occurrenceCounts);
    ranges.push({
      blockId,
      from: offset,
      to: offset + node.nodeSize,
    });
  });

  return ranges;
}

export function resolveBlockRange(
  ranges: readonly BlockRange[],
  blockId: string,
): BlockRange | undefined {
  const exact = ranges.find((range) => range.blockId === blockId);
  if (exact) return exact;

  if (blockId.includes("#")) {
    return undefined;
  }

  const path = blockId === "root" ? "root" : blockId;
  const prefix = `${path}|`;
  const headingMatch = ranges.find((range) => range.blockId.startsWith(`${prefix}heading#`));
  if (headingMatch) return headingMatch;
  return ranges.find((range) => range.blockId.startsWith(prefix));
}
