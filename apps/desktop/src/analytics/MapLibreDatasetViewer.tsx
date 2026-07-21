import { useEffect, useMemo, useRef, useState } from "react";
import type { Map as MapLibreMap, StyleSpecification } from "maplibre-gl";

import {
  detectLonLatColumns,
  extractGeoPoints,
  type GeoPoint,
} from "../lib/geoColumns";
import "./maplibre.css";

/**
 * MapLibre GL JS (BSD-3-Clause; ~3–4 MB min+gzip JS, larger with CSS/workers).
 * Loaded lazily so Preview/Chart/Profile paths do not pay the map cost.
 * Offline-first style: solid `--lt-*` background, no remote basemap tiles.
 */

export interface MapLibreDatasetViewerProps {
  /** Plain row objects (from bounded Arrow decode / sample). */
  rows: ReadonlyArray<Record<string, unknown>>;
  /** Schema column names used for lon/lat detection. */
  columnNames: readonly string[];
  /** Bump to force a remount after re-query. */
  loadKey?: string | number;
  onError?: (message: string) => void;
}

function readCssToken(name: string, fallback: string): string {
  if (typeof document === "undefined") return fallback;
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return value || fallback;
}

function prefersReducedMotion(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return false;
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function buildOfflineStyle(points: GeoPoint[]): StyleSpecification {
  const background = readCssToken("--lt-panel", "#1a1a1a");
  const circle = readCssToken("--lt-accent", "#f5a623");
  const stroke = readCssToken("--lt-bg", "#0d0d0d");

  return {
    version: 8,
    sources: {
      places: {
        type: "geojson",
        data: {
          type: "FeatureCollection",
          features: points.map((point, index) => ({
            type: "Feature",
            id: index,
            properties: { label: point.label ?? "" },
            geometry: {
              type: "Point",
              coordinates: [point.lon, point.lat],
            },
          })),
        },
      },
    },
    layers: [
      {
        id: "background",
        type: "background",
        paint: { "background-color": background },
      },
      {
        id: "places-circle",
        type: "circle",
        source: "places",
        paint: {
          "circle-radius": 7,
          "circle-color": circle,
          "circle-stroke-width": 1.5,
          "circle-stroke-color": stroke,
          "circle-opacity": 0.92,
        },
      },
    ],
  };
}

function fitPoints(map: MapLibreMap, points: GeoPoint[]): void {
  if (points.length === 0) return;
  const reduceMotion = prefersReducedMotion();
  if (points.length === 1) {
    const only = points[0]!;
    const camera = { center: [only.lon, only.lat] as [number, number], zoom: 4 };
    if (reduceMotion) {
      map.jumpTo(camera);
    } else {
      map.flyTo({ ...camera, essential: true, duration: 600 });
    }
    return;
  }

  let minLon = Infinity;
  let minLat = Infinity;
  let maxLon = -Infinity;
  let maxLat = -Infinity;
  for (const point of points) {
    minLon = Math.min(minLon, point.lon);
    minLat = Math.min(minLat, point.lat);
    maxLon = Math.max(maxLon, point.lon);
    maxLat = Math.max(maxLat, point.lat);
  }

  map.fitBounds(
    [
      [minLon, minLat],
      [maxLon, maxLat],
    ],
    {
      padding: 48,
      maxZoom: 8,
      animate: !reduceMotion,
      duration: reduceMotion ? 0 : 700,
    },
  );
}

/**
 * Lazy MapLibre map for datasets with lon/lat (or latitude/longitude) columns.
 * Honest empty state when geo columns are absent.
 */
export function MapLibreDatasetViewer({
  rows,
  columnNames,
  loadKey = 0,
  onError,
}: MapLibreDatasetViewerProps) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const mapRef = useRef<MapLibreMap | null>(null);
  const onErrorRef = useRef(onError);
  const [status, setStatus] = useState<"loading" | "ready" | "error">("loading");

  onErrorRef.current = onError;

  const columns = useMemo(() => detectLonLatColumns(columnNames), [columnNames]);

  const points = useMemo(
    () => (columns ? extractGeoPoints(rows, columns) : []),
    [rows, columns],
  );

  useEffect(() => {
    if (!columns || points.length === 0) return;

    let cancelled = false;
    const host = hostRef.current;
    if (!host) return;

    setStatus("loading");

    void (async () => {
      try {
        const maplibre = await import("maplibre-gl");
        await import("maplibre-gl/dist/maplibre-gl.css");
        if (cancelled || !hostRef.current) return;

        const MapCtor = maplibre.Map;
        host.replaceChildren();
        const container = document.createElement("div");
        container.className = "maplibre-dataset-viewer-host";
        host.append(container);

        const map = new MapCtor({
          container,
          style: buildOfflineStyle(points),
          center: [points[0]!.lon, points[0]!.lat],
          zoom: 1.5,
          attributionControl: { compact: true },
        });
        mapRef.current = map;

        map.on("load", () => {
          if (cancelled) return;
          fitPoints(map, points);
          setStatus("ready");
        });

        map.on("error", (event) => {
          if (cancelled) return;
          const message = event.error?.message ?? "MapLibre failed to render.";
          setStatus("error");
          onErrorRef.current?.(message);
        });
      } catch (err: unknown) {
        if (cancelled) return;
        const message = err instanceof Error ? err.message : String(err);
        setStatus("error");
        onErrorRef.current?.(message);
      }
    })();

    return () => {
      cancelled = true;
      const map = mapRef.current;
      mapRef.current = null;
      if (map) {
        map.remove();
      }
      host.replaceChildren();
    };
  }, [loadKey, columns, points]);

  if (!columns) {
    return (
      <div className="maplibre-dataset-viewer-empty" role="status">
        No lon/lat columns found. Add lon and lat (or longitude / latitude) to plot points.
      </div>
    );
  }

  if (points.length === 0) {
    return (
      <div className="maplibre-dataset-viewer-empty" role="status">
        No valid WGS84 points in this bounded sample.
      </div>
    );
  }

  return (
    <div className="maplibre-dataset-viewer" data-status={status}>
      {status === "loading" ? (
        <p className="maplibre-dataset-viewer-status" aria-live="polite">
          Loading map…
        </p>
      ) : null}
      {status === "error" ? (
        <p className="maplibre-dataset-viewer-status" role="alert">
          Map failed to load.
        </p>
      ) : null}
      <div ref={hostRef} className="maplibre-dataset-viewer-host" />
    </div>
  );
}
