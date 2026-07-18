import { useEffect, useMemo, useState } from "react";

import { isAbsoluteSrc } from "../editor/assets";
import { loadImageAsset } from "../viewers/media/imageSource";
import type { RelationLabelIndex } from "./relationDisplay";
import { formatCellForColumnName } from "./relationDisplay";
import { cellValueToDisplay, type DataColumn, type DataRow } from "./types";
import {
  isImageCoverValue,
  resolveGalleryCoverColumn,
  resolveListPrimaryColumn,
  resolveListSubtitleColumn,
} from "./viewLayout";

interface DataGalleryViewProps {
  root: string;
  rows: DataRow[];
  columns: DataColumn[];
  relationLabelIndex: RelationLabelIndex;
  coverField?: string | null;
  selectedRowId?: string | null;
  onRowOpen: (row: DataRow) => void;
}

interface GalleryCoverProps {
  root: string;
  value: string;
  fallback: string;
}

function GalleryCover({ root, value, fallback }: GalleryCoverProps) {
  const [imageUrl, setImageUrl] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    const trimmed = value.trim();
    if (!trimmed || !isImageCoverValue(trimmed)) {
      setImageUrl(null);
      setFailed(false);
      return;
    }

    if (isAbsoluteSrc(trimmed)) {
      setImageUrl(trimmed);
      setFailed(false);
      return;
    }

    if (!root) {
      setImageUrl(null);
      setFailed(true);
      return;
    }

    const controller = new AbortController();
    let revoke: (() => void) | undefined;

    void loadImageAsset({ root, path: trimmed.replace(/\\/g, "/") }, controller.signal)
      .then((asset) => {
        revoke = asset.lease.revoke;
        setImageUrl(asset.lease.url);
        setFailed(false);
      })
      .catch(() => {
        setImageUrl(null);
        setFailed(true);
      });

    return () => {
      controller.abort();
      revoke?.();
      setImageUrl(null);
    };
  }, [root, value]);

  if (imageUrl && !failed) {
    return (
      <img
        className="data-gallery-cover-image"
        src={imageUrl}
        alt=""
        loading="lazy"
        decoding="async"
      />
    );
  }

  return <span className="data-gallery-cover-fallback">{fallback || "—"}</span>;
}

export function DataGalleryView({
  root,
  rows,
  columns,
  relationLabelIndex,
  coverField,
  selectedRowId,
  onRowOpen,
}: DataGalleryViewProps) {
  const coverColumn = useMemo(
    () => resolveGalleryCoverColumn(columns, coverField),
    [columns, coverField],
  );
  const primaryColumn = useMemo(() => resolveListPrimaryColumn(columns), [columns]);
  const subtitleColumn = useMemo(
    () => resolveListSubtitleColumn(columns, primaryColumn),
    [columns, primaryColumn],
  );

  return (
    <div className="data-gallery-view" role="list">
      {rows.map((row) => {
        const primary = primaryColumn
          ? formatCellForColumnName(row, primaryColumn, columns, relationLabelIndex)
          : row.id;
        const subtitle = subtitleColumn
          ? formatCellForColumnName(row, subtitleColumn, columns, relationLabelIndex)
          : "";
        const coverValue = coverColumn ? cellValueToDisplay(row.values[coverColumn]) : "";
        const selected = selectedRowId === row.id;

        return (
          <button
            key={row.id}
            type="button"
            role="listitem"
            className={`data-gallery-card${selected ? " data-gallery-card--selected" : ""}`}
            onClick={() => onRowOpen(row)}
            aria-current={selected ? "true" : undefined}
          >
            <span className="data-gallery-cover" aria-hidden="true">
              <GalleryCover root={root} value={coverValue} fallback={primary || row.id} />
            </span>
            <span className="data-gallery-card-body">
              <span className="data-gallery-card-primary">{primary || row.id}</span>
              {subtitle && <span className="data-gallery-card-subtitle">{subtitle}</span>}
            </span>
          </button>
        );
      })}
    </div>
  );
}
