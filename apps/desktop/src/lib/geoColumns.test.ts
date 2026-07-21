import { describe, expect, it } from "vitest";

import {
  coerceGeoCoordinate,
  detectLonLatColumns,
  extractGeoPoints,
} from "./geoColumns";

describe("detectLonLatColumns", () => {
  it("detects lon/lat case-insensitively", () => {
    expect(detectLonLatColumns(["name", "LON", "Lat"])).toEqual({
      lon: "LON",
      lat: "Lat",
      label: "name",
    });
  });

  it("accepts longitude/latitude aliases", () => {
    expect(detectLonLatColumns(["longitude", "latitude"])).toEqual({
      lon: "longitude",
      lat: "latitude",
    });
  });

  it("accepts lng alias", () => {
    expect(detectLonLatColumns(["lng", "lat"])).toEqual({
      lon: "lng",
      lat: "lat",
    });
  });

  it("returns null when either axis is missing", () => {
    expect(detectLonLatColumns(["lon", "name"])).toBeNull();
    expect(detectLonLatColumns(["lat"])).toBeNull();
    expect(detectLonLatColumns([])).toBeNull();
  });

  it("prefers name over place_id for labels", () => {
    expect(detectLonLatColumns(["place_id", "name", "lon", "lat"])).toEqual({
      lon: "lon",
      lat: "lat",
      label: "name",
    });
  });
});

describe("coerceGeoCoordinate", () => {
  it("accepts finite numbers and numeric strings", () => {
    expect(coerceGeoCoordinate(-122.4)).toBe(-122.4);
    expect(coerceGeoCoordinate("37.77")).toBe(37.77);
    expect(coerceGeoCoordinate(12n)).toBe(12);
  });

  it("rejects non-numeric values", () => {
    expect(coerceGeoCoordinate(null)).toBeNull();
    expect(coerceGeoCoordinate("")).toBeNull();
    expect(coerceGeoCoordinate(Number.NaN)).toBeNull();
    expect(coerceGeoCoordinate({})).toBeNull();
  });
});

describe("extractGeoPoints", () => {
  it("keeps valid WGS84 points and drops out-of-range rows", () => {
    const points = extractGeoPoints(
      [
        { name: "SF", lon: -122.4194, lat: 37.7749 },
        { name: "bad", lon: 200, lat: 10 },
        { name: "nullish", lon: null, lat: 1 },
      ],
      { lon: "lon", lat: "lat", label: "name" },
    );
    expect(points).toEqual([{ lon: -122.4194, lat: 37.7749, label: "SF" }]);
  });
});
