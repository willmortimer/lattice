import { NodeViewWrapper } from "@tiptap/react";
import type { NodeViewProps } from "@tiptap/react";
import { Cube } from "@phosphor-icons/react";

/**
 * Read-view card for `:::lattice-embed` blocks. Shows the referenced resource
 * path; fancy previews (PDF, images, data views) arrive in later slices.
 */
export function LatticeEmbedView({ node }: NodeViewProps) {
  const resource = (node.attrs.resource as string) || "";
  const view = (node.attrs.view as string | null) ?? undefined;
  const height = (node.attrs.height as string | null) ?? undefined;
  const missing = resource.trim().length === 0;

  return (
    <NodeViewWrapper className="page-embed-lattice">
      <div
        className={`page-embed-lattice-card${missing ? " page-embed-lattice-card--degraded" : ""}`}
        role="figure"
        aria-label={missing ? "Embed missing resource" : `Embed: ${resource}`}
      >
        <div className="page-embed-lattice-header">
          <Cube size={14} aria-hidden="true" />
          <span className="page-embed-lattice-label">Embed</span>
        </div>
        {missing ? (
          <p className="page-embed-lattice-degraded">Missing required resource path.</p>
        ) : (
          <p className="page-embed-lattice-resource">{resource}</p>
        )}
        {(view || height) && (
          <dl className="page-embed-lattice-meta">
            {view && (
              <>
                <dt>View</dt>
                <dd>{view}</dd>
              </>
            )}
            {height && (
              <>
                <dt>Height</dt>
                <dd>{height}</dd>
              </>
            )}
          </dl>
        )}
      </div>
    </NodeViewWrapper>
  );
}
