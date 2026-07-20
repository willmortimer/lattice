import { useEffect, useState } from "react";
import { KindMark } from "../KindMark";
import type { ArrowTransportDump } from "../lib/arrowIpc";
import { loadDatasetArrowDump } from "../lib/datasetQuery";
import {
  formatDistinct,
  formatPercent,
  formatProfileSummary,
  profileDataset,
  type RelationProfile,
} from "../lib/datasetProfile";
import type { OpenResourceSession } from "../resourceSession";
import type { ResourceRendererProps } from "../resourceRendererRegistry";
import type { ResourceRendererContext } from "./RendererContext";

type DatasetPanel = "query" | "profile";

export function DatasetResourceRenderer({
  context,
  session,
}: ResourceRendererProps<ResourceRendererContext, OpenResourceSession>) {
  const isDataset = session.kind === "dataset";
  const root = context.workspaceRoot;
  const path = isDataset ? session.resource.path : "";
  const [panel, setPanel] = useState<DatasetPanel>("query");
  const [dump, setDump] = useState<ArrowTransportDump | null>(null);
  const [summary, setSummary] = useState<string | null>(null);
  const [profile, setProfile] = useState<RelationProfile | null>(null);
  const [profileSummary, setProfileSummary] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!isDataset || !root) {
      setDump(null);
      setSummary(null);
      setProfile(null);
      setProfileSummary(null);
      setError(null);
      return;
    }
    let cancelled = false;
    setBusy(true);
    setError(null);

    const load = async () => {
      try {
        if (panel === "query") {
          const { dump: nextDump, summary: nextSummary } = await loadDatasetArrowDump(root, path);
          if (cancelled) return;
          setDump(nextDump);
          setSummary(nextSummary);
          setProfile(null);
          setProfileSummary(null);
        } else {
          const nextProfile = await profileDataset(root, path);
          if (cancelled) return;
          setProfile(nextProfile);
          setProfileSummary(formatProfileSummary(nextProfile));
          setDump(null);
          setSummary(null);
        }
      } catch (err: unknown) {
        if (cancelled) return;
        setDump(null);
        setSummary(null);
        setProfile(null);
        setProfileSummary(null);
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        if (!cancelled) setBusy(false);
      }
    };

    void load();
    return () => {
      cancelled = true;
    };
  }, [isDataset, root, path, context.reloadToken, panel]);

  if (!isDataset) return null;

  return (
    <div className="placeholder">
      <span className="placeholder-mark">
        <KindMark kind="dataset" size={36} />
      </span>
      <p className="placeholder-copy">Dataset</p>
      <p className="placeholder-sub">
        <code>{path}</code>
      </p>
      <div className="placeholder-sub" style={{ display: "flex", gap: "0.5rem", justifyContent: "center" }}>
        <button
          type="button"
          aria-pressed={panel === "query"}
          onClick={() => setPanel("query")}
          disabled={!root || busy}
        >
          Query
        </button>
        <button
          type="button"
          aria-pressed={panel === "profile"}
          onClick={() => setPanel("profile")}
          disabled={!root || busy}
        >
          Profile
        </button>
      </div>
      {!root ? (
        <p className="placeholder-sub">Open a native workspace to run DuckDB queries and profiling.</p>
      ) : busy ? (
        <p className="placeholder-sub">{panel === "query" ? "Running bounded query…" : "Profiling relation…"}</p>
      ) : error ? (
        <p className="placeholder-sub" role="alert">
          {error}
        </p>
      ) : panel === "query" && summary ? (
        <>
          <p className="placeholder-sub">{summary}</p>
          {dump && dump.sampleRows.length > 0 ? (
            <pre className="placeholder-sub" style={{ textAlign: "left", overflow: "auto" }}>
              {JSON.stringify(
                {
                  schema: dump.schema,
                  sampleRows: dump.sampleRows,
                  ipcBytes: dump.ipcByteLength,
                },
                null,
                2,
              )}
            </pre>
          ) : null}
        </>
      ) : panel === "profile" && profile && profileSummary ? (
        <>
          <p className="placeholder-sub">{profileSummary}</p>
          {profile.columns.length > 0 ? (
            <div className="placeholder-sub" style={{ overflow: "auto", textAlign: "left" }}>
              <table>
                <thead>
                  <tr>
                    <th scope="col">Column</th>
                    <th scope="col">Type</th>
                    <th scope="col">Null %</th>
                    <th scope="col">Distinct</th>
                    <th scope="col">Min</th>
                    <th scope="col">Max</th>
                    <th scope="col">Q50</th>
                  </tr>
                </thead>
                <tbody>
                  {profile.columns.map((column) => (
                    <tr key={column.name}>
                      <th scope="row">{column.name}</th>
                      <td>{column.dataType}</td>
                      <td>{formatPercent(column.nullPercentage)}</td>
                      <td>{formatDistinct(column.approxDistinct)}</td>
                      <td>{column.min ?? "—"}</td>
                      <td>{column.max ?? "—"}</td>
                      <td>{column.q50 ?? "—"}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : (
            <p className="placeholder-sub">No columns to profile.</p>
          )}
        </>
      ) : null}
    </div>
  );
}
