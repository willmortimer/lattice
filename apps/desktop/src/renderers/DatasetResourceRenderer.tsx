import { KindMark } from "../KindMark";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { ResourceRendererContext } from "./RendererContext";

export function DatasetResourceRenderer({
  session,
}: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  if (session.kind !== "dataset") return null;
  return (
    <div className="placeholder">
      <span className="placeholder-mark">
        <KindMark kind="dataset" size={36} />
      </span>
      <p className="placeholder-copy">Analytical viewer coming soon.</p>
      <p className="placeholder-sub">
        Dataset packages open here once Perspective and DuckDB land in Phase 3. For now, inspect{" "}
        <code>{session.resource.path}</code> on disk.
      </p>
    </div>
  );
}
