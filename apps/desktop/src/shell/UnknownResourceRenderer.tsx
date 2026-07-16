import { Button } from "@lattice/ui";
import { KindMark, KIND_LABELS } from "../KindMark";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererContext } from "../renderers/RendererContext";

export function UnknownResourceRenderer({ context, session }: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  const resource = session.resource;
  return (
    <div className="placeholder">
      <span className="placeholder-mark">
        <KindMark kind={resource.kind} size={36} />
      </span>
      <p className="placeholder-copy">
        No {KIND_LABELS[resource.kind].toLowerCase()} viewer yet.
      </p>
      <p className="placeholder-sub">
        The file stays yours — open <code>{resource.path}</code> in any tool.
      </p>
      {context.callbacks.onOpenExternally && (
        <Button variant="secondary" onClick={() => context.callbacks.onOpenExternally?.(resource)}>
          Open externally
        </Button>
      )}
    </div>
  );
}

export function CapabilityFallbackRenderer({ context, session }: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  return (
    <div className="placeholder">
      <span className="placeholder-mark"><KindMark kind={session.resource.kind} size={36} /></span>
      <p className="placeholder-copy">This capability is not enabled.</p>
      <p className="placeholder-sub">
        Enable {context.missingCapabilities?.join(", ") || "the required capability"} to open <code>{session.resource.path}</code>.
      </p>
    </div>
  );
}
