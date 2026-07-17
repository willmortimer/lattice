import type { StructuredNode } from "./structuredParserCore";

export interface TreeRow {
  id: string;
  label: string;
  level: number;
  node: StructuredNode;
  expandable: boolean;
  expanded: boolean;
}

function preview(node: StructuredNode): string {
  if (node.kind === "value") return node.value === null ? "null" : String(node.value);
  if (node.kind === "alias") return `*${node.name}`;
  return node.kind === "array" ? `[${node.items.length}]` : `{${node.entries.length}}`;
}

function children(node: StructuredNode): Array<{ key: string; value: StructuredNode }> {
  if (node.kind === "object") return node.entries;
  if (node.kind === "array") return node.items.map((value, index) => ({ key: String(index), value }));
  return [];
}

export function defaultExpandedIds(root: StructuredNode): Set<string> {
  const expanded = new Set<string>(["root"]);
  const visit = (node: StructuredNode, id: string, depth: number) => {
    if (depth >= 2) return;
    for (const child of children(node)) {
      const childId = `${id}.${child.key}`;
      if (child.value.kind === "object" || child.value.kind === "array") {
        expanded.add(childId);
        visit(child.value, childId, depth + 1);
      }
    }
  };
  visit(root, "root", 0);
  return expanded;
}

export function flattenVisibleTree(root: StructuredNode, expandedIds: ReadonlySet<string>): TreeRow[] {
  const rows: TreeRow[] = [];
  const visit = (node: StructuredNode, id: string, level: number, label: string) => {
    const childEntries = children(node);
    const expanded = expandedIds.has(id);
    rows.push({ id, label: `${label}: ${preview(node)}`, level, node, expandable: childEntries.length > 0, expanded });
    if (!expanded) return;
    for (const child of childEntries) visit(child.value, `${id}.${child.key}`, level + 1, child.key);
  };
  visit(root, "root", 1, "value");
  return rows;
}
