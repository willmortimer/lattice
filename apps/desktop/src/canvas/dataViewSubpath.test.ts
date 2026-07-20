import { describe, expect, it } from "vitest";

import {
  interfaceNameFromCanvasSubpath,
  viewNameFromCanvasSubpath,
  viewNameFromInterfaceBindings,
} from "./dataViewSubpath";

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
    expect(viewNameFromCanvasSubpath("interfaces/ContactOps")).toBeNull();
  });
});

describe("interfaceNameFromCanvasSubpath", () => {
  it("returns null for empty or missing subpaths", () => {
    expect(interfaceNameFromCanvasSubpath(undefined)).toBeNull();
    expect(interfaceNameFromCanvasSubpath("")).toBeNull();
    expect(interfaceNameFromCanvasSubpath("   ")).toBeNull();
  });

  it("maps interfaces/ContactOps variants", () => {
    expect(interfaceNameFromCanvasSubpath("interfaces/ContactOps")).toBe("ContactOps");
    expect(interfaceNameFromCanvasSubpath("interfaces/ContactOps.interface.yaml")).toBe(
      "ContactOps",
    );
    expect(interfaceNameFromCanvasSubpath("/interfaces/ContactOps.interface.yaml")).toBe(
      "ContactOps",
    );
  });

  it("returns null for view or nested subpaths", () => {
    expect(interfaceNameFromCanvasSubpath("views/Board")).toBeNull();
    expect(interfaceNameFromCanvasSubpath("interfaces/nested/ContactOps")).toBeNull();
    expect(interfaceNameFromCanvasSubpath("interfaces/ContactOps.yaml")).toBeNull();
  });
});

describe("viewNameFromInterfaceBindings", () => {
  it("prefers the first bound view for canvas open", () => {
    expect(
      viewNameFromInterfaceBindings({
        views: ["Board", "Gallery"],
        forms: ["ContactIntake"],
      }),
    ).toBe("Board");
  });

  it("returns null for form-only interfaces", () => {
    expect(
      viewNameFromInterfaceBindings({
        views: [],
        forms: ["ContactIntake"],
      }),
    ).toBeNull();
  });
});
