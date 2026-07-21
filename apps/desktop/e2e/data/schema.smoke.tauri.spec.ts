/**
 * Native schema / tabular-import smoke (Tauri WebView).
 *
 * Covers First Look Home.md items 10 + 12: CRM Add column (text) and
 * Data/sample.csv → Create table from CSV… → type-review → Import.
 *
 * Local run (not a CI gate):
 *
 *   pnpm --filter @lattice/desktop test:schema:tauri
 *
 * Or two terminals with a reset First Look seed:
 *
 *   pnpm --filter @lattice/desktop tauri:dev:e2e
 *   pnpm --filter @lattice/desktop exec playwright test --project=tauri e2e/data/schema.smoke.tauri.spec.ts
 *
 * Requires `LATTICE_DEV_HOME` + `LATTICE_DEV_RESET_DEMO=1` (tauri:dev:e2e /
 * the wrapper) so CRM + Data/sample.csv match the demo template.
 */
import {
  openTreePage,
  schemaSmoke,
  scrollTreeUntilLabel,
  treeLabelMounted,
  waitForShellChrome,
} from "../perf/helpers";
import { expect, test } from "../fixtures";

const SCHEMA_TIMEOUT_MS = 45_000;

test.describe("Schema / import smoke (tauri)", () => {
  test.beforeEach(async ({ tauriPage }) => {
    tauriPage.setDefaultTimeout(SCHEMA_TIMEOUT_MS);
    await waitForShellChrome(tauriPage);
  });

  test("adds a text column on CRM via Add column designer", async ({ tauriPage }) => {
    await openTreePage(tauriPage, schemaSmoke.crmTreeLabel);

    const title = tauriPage.locator(".data-table-title");
    await title.waitFor(SCHEMA_TIMEOUT_MS);
    await expect(title, "CRM.data should paint the data-table title").toHaveText("CRM");

    const openDesigner = tauriPage.getByRole("button", { name: "Add column" });
    await openDesigner.waitFor(SCHEMA_TIMEOUT_MS);
    await expect(
      openDesigner,
      "Add column toolbar control should be enabled on native",
    ).toBeEnabled();
    await openDesigner.click();

    const panel = tauriPage.locator(schemaSmoke.addColumnPanel);
    await panel.waitFor(SCHEMA_TIMEOUT_MS);

    await tauriPage.fill(schemaSmoke.addColumnNameInput, schemaSmoke.smokeColumnName);
    // Default type is text; set explicitly so the smoke documents the field type.
    await tauriPage.selectOption(schemaSmoke.addColumnTypeSelect, "text");

    await tauriPage.click(schemaSmoke.addColumnSubmit, { timeout: SCHEMA_TIMEOUT_MS });

    // Successful add_data_columns closes the designer (`onClose`).
    await expect
      .poll(
        async () =>
          Boolean(
            await tauriPage.evaluate(
              `!!document.querySelector(${JSON.stringify(schemaSmoke.addColumnPanel)})`,
            ),
          ),
        { timeout: SCHEMA_TIMEOUT_MS },
      )
      .toBe(false);

    // Prove the column landed in the snapshot: reopen designer and re-submit.
    await openDesigner.click();
    await panel.waitFor(SCHEMA_TIMEOUT_MS);
    await tauriPage.fill(schemaSmoke.addColumnNameInput, schemaSmoke.smokeColumnName);
    await tauriPage.click(schemaSmoke.addColumnSubmit, { timeout: SCHEMA_TIMEOUT_MS });

    const duplicateError = panel.locator(".error-text");
    await duplicateError.waitFor(SCHEMA_TIMEOUT_MS);
    await expect(
      duplicateError,
      "Re-adding smoke_notes should hit validateColumnName duplicate guard",
    ).toHaveText(`Column "${schemaSmoke.smokeColumnName}" already exists.`);
  });

  test("promotes Data/sample.csv through type-review import", async ({ tauriPage }) => {
    // Package-name prompt for handlePromoteWorkspaceCsv.
    await tauriPage.installDialogHandler({
      defaultConfirm: true,
      defaultPromptText: schemaSmoke.smokeImportPackage,
    });

    await openTreePage(tauriPage, schemaSmoke.sampleCsvLabel);

    const promote = tauriPage.getByRole("button", { name: schemaSmoke.createTableFromCsv });
    await promote.waitFor(SCHEMA_TIMEOUT_MS);
    await expect(
      promote,
      "Create table from CSV… should be available on native sample.csv",
    ).toBeEnabled();
    await promote.click();

    const reviewTitle = tauriPage.locator(schemaSmoke.importReviewTitle);
    await reviewTitle.waitFor(SCHEMA_TIMEOUT_MS);
    await expect(
      reviewTitle,
      "Promote should open the CSV type-review dialog",
    ).toHaveText(schemaSmoke.importReviewTitleText);

    const reviewPanel = tauriPage.locator(schemaSmoke.importReviewPanel);
    await expect(
      reviewPanel.getByText(`${schemaSmoke.smokeImportPackage}.data`),
      "Review copy should name the destination package",
    ).toBeVisible();

    // Touch one type control so the smoke exercises the review surface.
    const nameType = tauriPage.getByLabel("Type for name");
    await nameType.waitFor(SCHEMA_TIMEOUT_MS);
    await nameType.selectOption("text");

    await reviewPanel.getByRole("button", { name: "Import" }).click();

    const importedTitle = tauriPage.locator(".data-table-title");
    await importedTitle.waitFor(SCHEMA_TIMEOUT_MS);
    await expect(
      importedTitle,
      "commit_tabular_import should open the new data app",
    ).toHaveText(schemaSmoke.smokeImportTitle);

    await scrollTreeUntilLabel(tauriPage, schemaSmoke.smokeImportTreeLabel);
    await expect
      .poll(
        async () => treeLabelMounted(tauriPage, schemaSmoke.smokeImportTreeLabel),
        { timeout: SCHEMA_TIMEOUT_MS },
      )
      .toBe(true);
  });
});
