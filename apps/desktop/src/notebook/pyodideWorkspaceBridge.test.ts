import { describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  DEFAULT_BRIDGED_WORKSPACE_PATHS,
  normalizeWorkspaceRelPath,
  packagesForNotebookCode,
  prepareWorkspaceBridge,
  PYODIDE_WORKSPACE_ROOT,
  pyodideMountPath,
} from "./pyodideWorkspaceBridge";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("pyodideWorkspaceBridge", () => {
  it("maps workspace-relative paths under the Pyodide workspace root", () => {
    expect(pyodideMountPath("Data/Orders.dataset/sources/orders.csv")).toBe(
      `${PYODIDE_WORKSPACE_ROOT}/Data/Orders.dataset/sources/orders.csv`,
    );
    expect(normalizeWorkspaceRelPath("/Data/sample.csv")).toBe("Data/sample.csv");
    expect(DEFAULT_BRIDGED_WORKSPACE_PATHS).toContain(
      "Data/Orders.dataset/sources/orders.csv",
    );
  });

  it("rejects path traversal", () => {
    expect(() => normalizeWorkspaceRelPath("../secrets.csv")).toThrow(/must not contain/);
    expect(() => normalizeWorkspaceRelPath("Data/../../etc/passwd")).toThrow(/must not contain/);
  });

  it("infers pandas and matplotlib packages from cell source", () => {
    expect(packagesForNotebookCode("print(1)")).toEqual([]);
    expect(packagesForNotebookCode("import pandas as pd\npd.read_csv('x')")).toEqual([
      "pandas",
    ]);
    expect(
      packagesForNotebookCode("import matplotlib.pyplot as plt\nplt.plot([1, 2])"),
    ).toEqual(["matplotlib"]);
    expect(
      packagesForNotebookCode(
        "import pandas as pd\nimport matplotlib.pyplot as plt\nplt.bar(pd.Series([1]))",
      ),
    ).toEqual(["pandas", "matplotlib"]);
  });

  it("returns an honest unavailable result in the browser demo", async () => {
    const result = await prepareWorkspaceBridge({
      root: "/tmp/ws",
      inBrowser: true,
    });
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.reason).toBe("browser-demo");
    expect(result.message).toMatch(/native desktop/i);
  });

  it("returns unavailable when no workspace root is open", async () => {
    const result = await prepareWorkspaceBridge({
      root: null,
      inBrowser: false,
    });
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.reason).toBe("no-root");
    expect(result.message).toMatch(/open a workspace/i);
  });

  it("falls back to the compiled First Look template when disk read fails", async () => {
    vi.mocked(invoke).mockRejectedValueOnce(
      new Error('cannot resolve "Data/Orders.dataset/sources/orders.csv"'),
    );

    const result = await prepareWorkspaceBridge({
      root: "/tmp/sticky-first-look",
      inBrowser: false,
    });
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.files).toHaveLength(1);
    expect(result.files[0]?.source).toBe("demo-template");
    expect(result.files[0]?.bytes.byteLength).toBeGreaterThan(0);
    expect(result.notice).toMatch(/template copy/i);
  });
});
