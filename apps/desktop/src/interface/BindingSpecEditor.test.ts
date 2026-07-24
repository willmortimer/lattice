import { describe, expect, it } from "vitest";

import { validateComponentBinding } from "./BindingSpecEditor";

describe("validateComponentBinding", () => {
  it("requires a saved-view for data-view tiles", () => {
    expect(
      validateComponentBinding({
        type: "data-view",
        binding: { type: "resource", resource: "." },
      }),
    ).toMatch(/saved-view/);
    expect(
      validateComponentBinding({
        type: "data-view",
        binding: { type: "saved-view", resource: ".", view: "Board" },
      }),
    ).toBeNull();
  });

  it("requires form name or package forms for form tiles", () => {
    expect(validateComponentBinding({ type: "form" })).toMatch(/form name/);
    expect(
      validateComponentBinding({
        type: "form",
        form: "ContactIntake",
        binding: { type: "resource", resource: "." },
      }),
    ).toBeNull();
  });

  it("requires duckdb SQL for chart tiles", () => {
    expect(
      validateComponentBinding({
        type: "chart",
        binding: {
          type: "duckdb-query",
          resources: ["Data/Orders.dataset"],
          sql: "",
          limit: 10,
        },
      }),
    ).toMatch(/duckdb-query/);
  });
});
