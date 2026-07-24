import { beforeEach, describe, expect, it, vi } from "vitest";

import { DEMO_PACKAGE_FORMS } from "../data/forms";
import { DEMO_OPS_DASHBOARD } from "../data/interfaces";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("interface form component", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("binds ContactIntake on the OpsDashboard fixture", () => {
    const formComponent = DEMO_OPS_DASHBOARD.components?.find((item) => item.type === "form");
    expect(formComponent).toMatchObject({
      id: "intake",
      type: "form",
      form: "ContactIntake",
    });
    expect(formComponent?.binding?.type).toBe("resource");
  });

  it("submits package forms through insert_record with formName", async () => {
    const { submitPackageFormRecord } = await import("../data/forms");
    const form = DEMO_PACKAGE_FORMS[0]!;
    invokeMock.mockResolvedValue({ id: "rec_1", revision: "rev:2" });

    const result = await submitPackageFormRecord({
      root: "/tmp/ws",
      relPath: "CRM.data",
      form,
      values: { name: { Text: "Ada" } },
    });

    expect(result).toEqual({ id: "rec_1", revision: "rev:2" });
    expect(invokeMock).toHaveBeenCalledWith("insert_record", {
      root: "/tmp/ws",
      relPath: "CRM.data",
      table: form.table,
      values: { name: { Text: "Ada" } },
      formName: "ContactIntake",
    });
  });
});
