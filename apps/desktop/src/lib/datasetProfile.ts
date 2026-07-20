import { invoke } from "@tauri-apps/api/core";

export interface ColumnProfile {
  name: string;
  dataType: string;
  rowCount?: number;
  nullPercentage?: number;
  approxDistinct?: number;
  min?: string;
  max?: string;
  avg?: number;
  std?: number;
  q25?: string;
  q50?: string;
  q75?: string;
}

export interface RelationProfile {
  rowCount: number;
  columns: ColumnProfile[];
  relationSql: string;
}

export interface ProfileDatasetRequest {
  sql?: string;
}

export async function profileDataset(
  root: string,
  relPath: string,
  request: ProfileDatasetRequest = {},
): Promise<RelationProfile> {
  return invoke<RelationProfile>("profile_dataset", {
    root,
    relPath,
    request,
  });
}

export function formatPercent(value: number | undefined): string {
  if (value === undefined || Number.isNaN(value)) return "—";
  return `${value.toFixed(1)}%`;
}

export function formatDistinct(value: number | undefined): string {
  if (value === undefined) return "—";
  return `~${value.toLocaleString()}`;
}

export function formatProfileSummary(profile: RelationProfile): string {
  const columnCount = profile.columns.length;
  const rowLabel = profile.rowCount.toLocaleString();
  const columnLabel = columnCount === 1 ? "column" : "columns";
  return `${rowLabel} rows · ${columnCount} ${columnLabel}`;
}
