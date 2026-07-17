import type { Resource } from "../../types";
import type { ResourceRendererContext } from "../../renderers/RendererContext";
import "./media.css";

export function MediaDegraded({
  context,
  resource,
  title,
  message,
  detail,
}: {
  context: ResourceRendererContext;
  resource: Resource;
  title: string;
  message: string;
  detail?: string;
}) {
  return (
    <div className="media-viewer media-degraded" role="alert">
      <div className="media-degraded-card">
        <h2 className="media-degraded-title">{title}</h2>
        <p className="media-degraded-copy">{message}</p>
        {detail && <p className="media-degraded-detail">{detail}</p>}
        {context.callbacks.onOpenExternally && (
          <button className="media-button" type="button" onClick={() => context.callbacks.onOpenExternally?.(resource)}>
            Open externally
          </button>
        )}
      </div>
    </div>
  );
}
