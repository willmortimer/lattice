/** Lon/lat column detection and point extraction for dataset Map panels. */

const LON_ALIASES = new Set(["lon", "lng", "long", "longitude"]);
const LAT_ALIASES = new Set(["lat", "latitude"]);
const LABEL_ALIASES = ["name", "title", "label", "place", "place_id", "id"];

export interface LonLatColumns {
  lon: string;
  lat: string;
  /** Optional display label column when present in the schema. */
  label?: string;
}

export interface GeoPoint {
  lon: number;
  lat: number;
  label?: string;
}

/**
 * Detect WGS84 lon/lat column names case-insensitively.
 * Accepts lon/lng/long/longitude and lat/latitude.
 */
export function detectLonLatColumns(columnNames: Iterable<string>): LonLatColumns | null {
  const names = [...columnNames];
  let lon: string | undefined;
  let lat: string | undefined;

  for (const name of names) {
    const key = name.trim().toLowerCase();
    if (!lon && LON_ALIASES.has(key)) lon = name;
    if (!lat && LAT_ALIASES.has(key)) lat = name;
  }

  if (!lon || !lat) return null;

  const label = LABEL_ALIASES.map((alias) =>
    names.find((name) => name.trim().toLowerCase() === alias),
  ).find((name): name is string => Boolean(name) && name !== lon && name !== lat);

  return label ? { lon, lat, label } : { lon, lat };
}

/** Coerce a cell to a finite WGS84 coordinate, or null if unusable. */
export function coerceGeoCoordinate(value: unknown): number | null {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "bigint") {
    const asNumber = Number(value);
    return Number.isFinite(asNumber) ? asNumber : null;
  }
  if (typeof value === "string" && value.trim() !== "") {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
}

/** Extract plottable points from row objects using detected lon/lat columns. */
export function extractGeoPoints(
  rows: ReadonlyArray<Record<string, unknown>>,
  columns: LonLatColumns,
): GeoPoint[] {
  const points: GeoPoint[] = [];
  for (const row of rows) {
    const lon = coerceGeoCoordinate(row[columns.lon]);
    const lat = coerceGeoCoordinate(row[columns.lat]);
    if (lon === null || lat === null) continue;
    if (lon < -180 || lon > 180 || lat < -90 || lat > 90) continue;
    const rawLabel = columns.label !== undefined ? row[columns.label] : undefined;
    const label =
      rawLabel === null || rawLabel === undefined
        ? undefined
        : typeof rawLabel === "string"
          ? rawLabel
          : String(rawLabel);
    points.push(label !== undefined && label !== "" ? { lon, lat, label } : { lon, lat });
  }
  return points;
}
