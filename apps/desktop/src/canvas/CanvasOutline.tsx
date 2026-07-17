import type { CanvasNode } from "./types";

interface CanvasOutlineProps {
  nodes: readonly CanvasNode[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onRemove: (id: string) => void;
}

function nodeLabel(node: CanvasNode): string {
  if (node.type === "file") return node.file;
  if (node.type === "link") return node.url;
  if (node.type === "group") return node.label || "Group";
  return node.text.replace(/\s+/g, " ").trim().slice(0, 80) || "Text note";
}

/** Small DOM reading order/keyboard surface for the GPU canvas. */
export function CanvasOutline({ nodes, selectedId, onSelect, onRemove }: CanvasOutlineProps) {
  return (
    <nav className="canvas-outline" aria-label="Canvas outline">
      <p className="canvas-outline-title">Outline</p>
      <ol>
        {nodes.map((node) => (
          <li key={node.id}>
            <button
              type="button"
              className={selectedId === node.id ? "is-selected" : undefined}
              aria-current={selectedId === node.id ? "true" : undefined}
              onClick={() => onSelect(node.id)}
            >
              <span className="canvas-outline-preview" aria-hidden="true">
                {node.type === "text" ? node.text.slice(0, 2) : node.type.slice(0, 2).toUpperCase()}
              </span>
              <span>{nodeLabel(node)}</span>
            </button>
            {selectedId === node.id && (
              <button type="button" className="canvas-outline-remove" onClick={() => onRemove(node.id)}>
                Remove
              </button>
            )}
          </li>
        ))}
      </ol>
    </nav>
  );
}
