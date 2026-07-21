import { describe, expect, it } from "vitest";

import { sampleRowsToValues, coerceVegaCell } from "./arrowToVegaData";

describe("arrowToVegaData", () => {
  it("maps preview rows to named Vega records", () => {
    const values = sampleRowsToValues(
      [
        [1, "Ada"],
        [2, "Grace"],
      ],
      [
        { name: "id", dataType: "int64", nullable: false },
        { name: "name", dataType: "utf8", nullable: true },
      ],
    );

    expect(values).toEqual([
      { id: 1, name: "Ada" },
      { id: 2, name: "Grace" },
    ]);
  });

  it("coerces bigint cells to numbers for Vega encodings", () => {
    expect(coerceVegaCell(42n)).toBe(42);
    expect(coerceVegaCell("ok")).toBe("ok");
  });
});
