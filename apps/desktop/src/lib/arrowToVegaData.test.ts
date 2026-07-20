import { describe, expect, it } from "vitest";

import { sampleRowsToValues } from "./arrowToVegaData";

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
});
