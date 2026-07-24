import { useEffect, useMemo, useRef, useState } from "react";
import type {
  Map as MapLibreMap,
  Marker as MapLibreMarker,
  Popup as MapLibrePopup,
  StyleSpecification,
} from "maplibre-gl";

import { observeThemeChange, readToken } from "../canvas/colors";
import {
  detectLonLatColumns,
  extractGeoPoints,
  type GeoPoint,
} from "../lib/geoColumns";
import { worldLand } from "./worldLand";
import "./maplibre.css";

/**
 * MapLibre GL JS (BSD-3-Clause; ~3–4 MB min+gzip JS, larger with CSS/workers).
 * Loaded lazily so Preview/Chart/Profile paths do not pay the map cost.
 *
 * Offline-first: no remote tiles. Geography is a vendored Natural Earth 110m
 * land layer (public domain, see worldLand.ts) plus a generated 30° graticule.
 * Labels/tooltips are DOM (Popup/Marker) because symbol layers would need a
 * glyphs endpoint we do not ship.
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

/** Permanent DOM label chips are only rendered at or below this point count. */
const MAX_PERMANENT_LABELS = 30;

interface MapPalette {
  /** Ocean / empty space. */
  background: string;
  landFill: string;
  landLine: string;
  graticule: string;
  circle: string;
  circleStroke: string;
  halo: string;
}

/**
 * Resolve a CSS color expression (including color-mix()) to a concrete color
 * MapLibre's parser accepts, via a computed-style probe element.
 */
function resolveCssColor(expression: string, fallback: string): string {
  if (typeof document === "undefined" || expression.trim() === "") return fallback;
  const probe = document.createElement("span");
  probe.style.display = "none";
  probe.style.color = expression;
  document.body.append(probe);
  const resolved = getComputedStyle(probe).color;
  probe.remove();
  return resolved && resolved.trim() !== "" ? resolved : fallback;
}

/** Resolve `--lt-*` token → concrete color, tolerating color-mix() values. */
function tokenColor(name: string, fallback: string): string {
  return resolveCssColor(readToken(name, fallback), fallback);
}

/** Re-alpha a resolved rgb()/rgba() color; falls back to the input on parse miss. */
function withAlpha(color: string, alpha: number): string {
  const m = /^rgba?\(\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)/.exec(color);
  if (!m) return color;
  return `rgba(${m[1]}, ${m[2]}, ${m[3]}, ${alpha})`;
}

function readMapPalette(): MapPalette {
  const slate = tokenColor("--lt-slate", "rgb(140, 162, 196)");
  return {
    background: tokenColor("--lt-bg", "#0d0d0d"),
    landFill: withAlpha(slate, 0.14),
    landLine: withAlpha(slate, 0.28),
    graticule: withAlpha(slate, 0.1),
    circle: tokenColor("--lt-accent", "#f5a623"),
    circleStroke: tokenColor("--lt-bg", "#0d0d0d"),
    halo: withAlpha(tokenColor("--lt-accent", "rgb(245, 166, 35)"), 0.18),
  };
}

/** 30° graticule as one MultiLineString (thin, low-alpha world grid). */
function buildGraticule(): { type: "MultiLineString"; coordinates: number[][][] } {
  const lines: number[][][] = [];
  for (let lon = -180; lon <= 180; lon += 30) {
    const meridian: number[][] = [];
    for (let lat = -85; lat <= 85; lat += 5) meridian.push([lon, lat]);
    lines.push(meridian);
  }
  for (let lat = -60; lat <= 60; lat += 30) {
    const parallel: number[][] = [];
    for (let lon = -180; lon <= 180; lon += 5) parallel.push([lon, lat]);
    lines.push(parallel);
  }
  return { type: "MultiLineString", coordinates: lines };
}

function buildOfflineStyle(points: GeoPoint[], palette: MapPalette): StyleSpecification {
  return {
    version: 8,
    sources: {
      land: {
        type: "geojson",
        data: worldLand,
        attribution: "Natural Earth",
      },
      graticule: {
        type: "geojson",
        data: {
          type: "Feature",
          properties: {},
          geometry: buildGraticule(),
        },
      },
      places: {
        type: "geojson",
        data: {
          type: "FeatureCollection",
          features: points.map((point, index) => ({
            type: "Feature",
            id: index,
            properties: {
              label: point.label ?? "",
              lon: point.lon,
              lat: point.lat,
            },
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
        paint: { "background-color": palette.background },
      },
      {
        id: "land-fill",
        type: "fill",
        source: "land",
        paint: { "fill-color": palette.landFill },
      },
      {
        id: "land-line",
        type: "line",
        source: "land",
        paint: { "line-color": palette.landLine, "line-width": 0.75 },
      },
      {
        id: "graticule",
        type: "line",
        source: "graticule",
        paint: { "line-color": palette.graticule, "line-width": 0.5 },
      },
      {
        // Subtle halo beneath each dot so points read on any land/ocean color.
        id: "places-halo",
        type: "circle",
        source: "places",
        paint: {
          "circle-radius": 12,
          "circle-color": palette.halo,
          "circle-blur": 0.6,
        },
      },
      {
        id: "places-circle",
        type: "circle",
        source: "places",
        paint: {
          "circle-radius": 6,
          "circle-color": palette.circle,
          "circle-stroke-width": 1.5,
          "circle-stroke-color": palette.circleStroke,
          "circle-opacity": 0.92,
        },
      },
    ],
  };
}

function applyPalette(map: MapLibreMap, palette: MapPalette): void {
  map.setPaintProperty("background", "background-color", palette.background);
  map.setPaintProperty("land-fill", "fill-color", palette.landFill);
  map.setPaintProperty("land-line", "line-color", palette.landLine);
  map.setPaintProperty("graticule", "line-color", palette.graticule);
  map.setPaintProperty("places-halo", "circle-color", palette.halo);
  map.setPaintProperty("places-circle", "circle-color", palette.circle);
  map.setPaintProperty("places-circle", "circle-stroke-color", palette.circleStroke);
}

function prefersReducedMotion(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return false;
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function formatCoordinate(lon: number, lat: number): string {
  const ns = lat >= 0 ? "N" : "S";
  const ew = lon >= 0 ? "E" : "W";
  return `${Math.abs(lat).toFixed(3)}°${ns} ${Math.abs(lon).toFixed(3)}°${ew}`;
}

/** Tooltip body built via DOM APIs — never string HTML from row data. */
function buildPopupContent(label: string, lon: number, lat: number): HTMLElement {
  const wrap = document.createElement("div");
  wrap.className = "maplibre-dataset-popup-body";
  if (label !== "") {
    const title = document.createElement("p");
    title.className = "maplibre-dataset-popup-label";
    title.textContent = label;
    wrap.append(title);
  }
  const coords = document.createElement("p");
  coords.className = "maplibre-dataset-popup-coords";
  coords.textContent = formatCoordinate(lon, lat);
  wrap.append(coords);
  return wrap;
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
    const markers: MapLibreMarker[] = [];
    let popup: MapLibrePopup | null = null;
    let unobserveTheme: (() => void) | null = null;

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
          style: buildOfflineStyle(points, readMapPalette()),
          center: [points[0]!.lon, points[0]!.lat],
          zoom: 1.2,
          minZoom: 0.4,
          renderWorldCopies: false,
          maxBounds: [
            [-185, -86],
            [185, 86],
          ],
          attributionControl: { compact: true },
        });
        mapRef.current = map;

        map.addControl(
          new maplibre.NavigationControl({ showCompass: false }),
          "top-right",
        );

        // Hover tooltip (DOM popup — no glyph atlas required).
        popup = new maplibre.Popup({
          closeButton: false,
          closeOnClick: false,
          className: "maplibre-dataset-popup",
          offset: 12,
          maxWidth: "260px",
        });

        map.on("mousemove", "places-circle", (event) => {
          const feature = event.features?.[0];
          if (!feature || feature.geometry.type !== "Point") return;
          map.getCanvas().style.cursor = "pointer";
          const [lon, lat] = feature.geometry.coordinates as [number, number];
          const label = String(feature.properties?.label ?? "");
          popup
            ?.setLngLat([lon, lat])
            .setDOMContent(buildPopupContent(label, lon, lat))
            .addTo(map);
        });
        map.on("mouseleave", "places-circle", () => {
          map.getCanvas().style.cursor = "";
          popup?.remove();
        });

        // Permanent label chips for small point sets.
        if (points.length <= MAX_PERMANENT_LABELS) {
          for (const point of points) {
            if (!point.label) continue;
            const chip = document.createElement("div");
            chip.className = "maplibre-dataset-marker-label";
            chip.textContent = point.label;
            const marker = new maplibre.Marker({
              element: chip,
              anchor: "left",
              offset: [10, 0],
            })
              .setLngLat([point.lon, point.lat])
              .addTo(map);
            markers.push(marker);
          }
        }

        // Live theme swaps: repaint colors in place, no remount.
        unobserveTheme = observeThemeChange(() => {
          if (!mapRef.current) return;
          try {
            applyPalette(mapRef.current, readMapPalette());
          } catch {
            /* style may be mid-load during teardown */
          }
        });

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
      unobserveTheme?.();
      popup?.remove();
      for (const marker of markers) marker.remove();
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
