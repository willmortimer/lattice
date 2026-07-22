import { describe, expect, it } from "vitest";

import {
  ruleMatchesDisplay,
  themeOverrideForCell,
  type ConditionalFormatRule,
} from "./conditionalFormat";

const activeRule: ConditionalFormatRule = {
  field: "status",
  operator: "equals",
  value: "Active",
  style: { bg: "accent-wash", text: "accent" },
};

describe("conditionalFormat", () => {
  it("matches equals and contains case-insensitively", () => {
    expect(ruleMatchesDisplay(activeRule, "Active")).toBe(true);
    expect(ruleMatchesDisplay(activeRule, "active")).toBe(true);
    expect(ruleMatchesDisplay(activeRule, "Done")).toBe(false);
    expect(
      ruleMatchesDisplay(
        { ...activeRule, operator: "contains", value: "act" },
        "Inactive",
      ),
    ).toBe(true);
  });

  it("builds a theme override for the first matching field rule", () => {
    const override = themeOverrideForCell("status", "Active", [
      activeRule,
      {
        field: "status",
        operator: "equals",
        value: "Active",
        style: { bg: "danger" },
      },
    ]);
    expect(override?.bgCell).toBeTruthy();
    expect(override?.textDark).toBeTruthy();
    expect(themeOverrideForCell("status", "Done", [activeRule])).toBeUndefined();
    expect(themeOverrideForCell("name", "Active", [activeRule])).toBeUndefined();
  });
});
