import { describe, expect, it } from "vitest";

import { viewNameFromCanvasSubpath } from "./dataViewSubpath";

describe("viewNameFromCanvasSubpath", () => {
  it("returns null for empty or missing subpaths", () => {
    expect(viewNameFromCanvasSubpath(undefined)).toBeNull();
    expect(viewNameFromCanvasSubpath("")).toBeNull();
    expect(viewNameFromCanvasSubpath("   ")).toBeNull();
  });

  it("maps views/Board and views/Board.yaml to the view name", () => {
    expect(viewNameFromCanvasSubpath("views/Board")).toBe("Board");
    expect(viewNameFromCanvasSubpath("views/Board.yaml")).toBe("Board");
    expect(viewNameFromCanvasSubpath("/views/Board.yaml")).toBe("Board");
  });

  it("maps views/Gallery variants", () => {
    expect(viewNameFromCanvasSubpath("views/Gallery")).toBe("Gallery");
    expect(viewNameFromCanvasSubpath("views/Gallery.view.yaml")).toBe("Gallery");
  });

  it("returns null for unrelated subpaths", () => {
    expect(viewNameFromCanvasSubpath("database.sqlite")).toBeNull();
    expect(viewNameFromCanvasSubpath("views/nested/Board.yaml")).toBeNull();
  });
});
