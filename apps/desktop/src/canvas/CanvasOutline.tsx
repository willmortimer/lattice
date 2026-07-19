import { IconButton } from "@lattice/ui";
import { X } from "@phosphor-icons/react";
import type { CanvasNode } from "./types";

interface CanvasOutlineProps {
  nodes: readonly CanvasNode[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onRemove: (id: string) => void;
  onClose: () => void;
}

function nodeLabel(node: CanvasNode): string {
  if (node.type === "file") return node.file;
  if (node.type === "link") return node.url;
  if (node.type === "group") return node.label || "Group";
  return node.text.replace(/\s+/g, " ").trim().slice(0, 80) || "Text note";
}

/** Small DOM reading order/keyboard surface for the GPU canvas. */
export function CanvasOutline({ nodes, selectedId, onSelect, onRemove, onClose }: CanvasOutlineProps) {
  return (
    <nav className="canvas-outline" aria-label="Canvas outline">
      <header className="canvas-outline-head">
        <p className="canvas-outline-title">Outline</p>
        <IconButton label="Hide outline" onClick={onClose}>
          <X size={14} />
        </IconButton>
      </header>
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
