/**
 * Native analytics smoke (Tauri WebView via tauri-plugin-playwright).
 *
 * Covers dataset Preview (Perspective), Orders Vega chart, and canvas Fit.
 *
 * Local run (not a CI gate):
 *
 *   pnpm --filter @lattice/desktop test:analytics:tauri
 *
 * Or two terminals with a reset First Look seed:
 *
 *   pnpm --filter @lattice/desktop tauri:dev:e2e
 *   pnpm --filter @lattice/desktop exec playwright test --project=tauri e2e/data/analytics.smoke.tauri.spec.ts
 *
 * Requires `LATTICE_DEV_HOME` + `LATTICE_DEV_RESET_DEMO=1` (tauri:dev:e2e /
 * the wrapper) so Orders.dataset + Dashboards match the demo template.
 */
import { openTreePage, waitForShellChrome } from "../perf/helpers";
import { expect, test } from "../fixtures";

const ORDERS_DATASET = "Dataset: Data/Orders.dataset";
const REVENUE_BY_DAY = "File: Dashboards/Revenue by day.vl.json";
const PRODUCT_STRATEGY = "Canvas: Canvases/Product Strategy.canvas";

/** DuckDB → Arrow → Perspective / Vega can exceed the CRM UI smoke budget. */
const ANALYTICS_TIMEOUT_MS = 60_000;

test.describe("Analytics smoke (tauri)", () => {
  test.beforeEach(async ({ tauriPage }) => {
    tauriPage.setDefaultTimeout(ANALYTICS_TIMEOUT_MS);
    await waitForShellChrome(tauriPage);
  });

  test("opens Orders.dataset Preview with Perspective host", async ({ tauriPage }) => {
    await openTreePage(tauriPage, ORDERS_DATASET);

    const title = tauriPage.locator(".dataset-surface-title");
    await title.waitFor(ANALYTICS_TIMEOUT_MS);
    await expect(title, "Orders.dataset should paint the Dataset surface title").toHaveText(
      "Dataset",
    );

    const previewTab = tauriPage.getByRole("tab", { name: "Preview" });
    await previewTab.waitFor(ANALYTICS_TIMEOUT_MS);
    await expect(previewTab, "Preview tab should be selected by default").toHaveAttribute(
      "aria-selected",
      "true",
    );

    const perspective = tauriPage.locator(".perspective-dataset-viewer");
    await perspective.waitFor(ANALYTICS_TIMEOUT_MS);
    await tauriPage.locator('.perspective-dataset-viewer[data-status="ready"]').waitFor(
      ANALYTICS_TIMEOUT_MS,
    );

    await expect(
      tauriPage.locator(".perspective-dataset-viewer-host perspective-viewer"),
      "Perspective host should mount <perspective-viewer> (not schema-only fallback)",
    ).toHaveCount(1);

    await expect(
      tauriPage.getByRole("alert").filter({ hasText: /Perspective unavailable|schema preview/i }),
      "Orders Preview must not fall back to schema-only alert",
    ).toHaveCount(0);
  });

  test("opens Revenue by day Vega-Lite chart", async ({ tauriPage }) => {
    await openTreePage(tauriPage, REVENUE_BY_DAY);

    const heading = tauriPage.locator(".placeholder-copy");
    await heading.waitFor(ANALYTICS_TIMEOUT_MS);
    await expect(heading, "Chart resource should identify as Vega-Lite chart").toHaveText(
      "Vega-Lite chart",
    );

    const path = tauriPage.locator(".placeholder-sub code");
    await expect(
      path,
      "Opened chart should be Dashboards/Revenue by day.vl.json",
    ).toHaveText("Dashboards/Revenue by day.vl.json");

    const chartSvg = tauriPage.locator(".vega-lite-chart-canvas svg");
    await chartSvg.waitFor(ANALYTICS_TIMEOUT_MS);
    await expect(
      chartSvg,
      "Revenue by day should embed a Vega SVG (vega-embed renderer)",
    ).toBeVisible();

    await expect(
      tauriPage.getByRole("alert"),
      "Chart surface should not show a load/query alert",
    ).toHaveCount(0);
  });

  test("opens Product Strategy canvas and Fits", async ({ tauriPage }) => {
    await openTreePage(tauriPage, PRODUCT_STRATEGY);

    const toolbar = tauriPage.locator('[aria-label="Canvas editing actions"]');
    await toolbar.waitFor(ANALYTICS_TIMEOUT_MS);

    const scene = tauriPage.locator('[aria-label="Canvas scene"]');
    await scene.waitFor(ANALYTICS_TIMEOUT_MS);
    await expect(scene, "Product Strategy canvas should expose the Pixi scene").toBeVisible();

    const fit = toolbar.getByRole("button", { name: "Fit" });
    await fit.waitFor(ANALYTICS_TIMEOUT_MS);
    await fit.click();

    // Outline defaults open (localStorage); only toggle if a prior session collapsed it.
    const outlineMounted = await tauriPage.evaluate(
      `!!document.querySelector('[aria-label="Canvas outline"]')`,
    );
    if (!outlineMounted) {
      await toolbar.getByRole("button", { name: "Outline" }).click();
    }
    const outline = tauriPage.locator('[aria-label="Canvas outline"]');
    await outline.waitFor(ANALYTICS_TIMEOUT_MS);
    await expect(
      outline.getByRole("button", { name: "Product/Vision.md" }),
      "Canvas outline should list Product/Vision.md after Fit",
    ).toBeVisible();
  });
});
