import { beforeEach, describe, expect, it, vi } from "vitest";

import { DEMO_PACKAGE_FORMS } from "../data/forms";
import { submitInterfaceFormRecord } from "./interfaceFormSubmit";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("interface form submit refresh", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("reloads package snapshot through open_data_app", async () => {
    const { openPackageSnapshot } = await import("../data/packageSnapshot");
    const snapshot = {
      title: "CRM",
      default_table: "contacts",
      package_revision: "rev:3",
      columns: [],
      rows: [],
      row_offset: 0,
      row_limit: 50,
      row_total: 0,
      has_more: false,
      available_views: ["Board"],
      active_view: "Board",
      filters: [],
    };
    invokeMock.mockResolvedValue(snapshot);

    const loaded = await openPackageSnapshot("/tmp/ws", "CRM.data");

    expect(loaded).toEqual(snapshot);
    expect(invokeMock).toHaveBeenCalledWith("open_data_app", {
      root: "/tmp/ws",
      relPath: "CRM.data",
      viewName: null,
      limit: null,
      offset: null,
    });
  });

  it("invokes package snapshot refresh after successful form submit", async () => {
    const form = DEMO_PACKAGE_FORMS[0]!;
    const refresh = vi.fn();
    invokeMock.mockResolvedValue({ id: "rec_1", revision: "rev:2" });

    const result = await submitInterfaceFormRecord({
      root: "/tmp/ws",
      relPath: "CRM.data",
      form,
      values: { name: { Text: "Ada" } },
      onPackageSnapshotRefresh: refresh,
    });

    expect(result).toEqual({ id: "rec_1" });
    expect(invokeMock).toHaveBeenCalledWith("insert_record", {
      root: "/tmp/ws",
      relPath: "CRM.data",
      table: form.table,
      values: { name: { Text: "Ada" } },
      formName: "ContactIntake",
    });
    expect(refresh).toHaveBeenCalledTimes(1);
  });
});
