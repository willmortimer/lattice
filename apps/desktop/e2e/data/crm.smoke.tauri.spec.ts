/**
 * Native CRM Wave 2 smoke (Tauri WebView via tauri-plugin-playwright).
 *
 * Local run (not a CI gate this sprint):
 *
 *   pnpm --filter @lattice/desktop test:crm:tauri
 *
 * Or two terminals with a reset First Look seed:
 *
 *   pnpm --filter @lattice/desktop tauri:dev:e2e
 *   pnpm --filter @lattice/desktop exec playwright test --project=tauri e2e/data/crm.smoke.tauri.spec.ts
 *
 * Requires `LATTICE_DEV_HOME` + `LATTICE_DEV_RESET_DEMO=1` (tauri:dev:e2e /
 * the wrapper) so CRM.data forms/actions match the demo template.
 */
import { openTreePage, waitForShellChrome } from "../perf/helpers";
import { expect, test } from "../fixtures";

const CRM_TREE_LABEL = "Data app: CRM.data";

test.describe("CRM Wave 2 smoke (tauri)", () => {
  test.beforeEach(async ({ tauriPage }) => {
    tauriPage.setDefaultTimeout(30_000);
    await waitForShellChrome(tauriPage);
  });

  test("opens CRM and exercises Save view, Actions, and FormSave", async ({
    tauriPage,
  }) => {
    await openTreePage(tauriPage, CRM_TREE_LABEL);

    const title = tauriPage.locator(".data-table-title");
    await title.waitFor(30_000);
    await expect(title, "CRM.data should paint the data-table title").toHaveText(
      "CRM",
    );

    // Native-only control: enabled when not browser demo / not busy.
    const saveView = tauriPage.getByRole("button", { name: "Save view" });
    await saveView.waitFor(30_000);
    await expect(
      saveView,
      "Save view must be enabled on native (disabled only in browser demo or while busy)",
    ).toBeEnabled();

    const formsButton = tauriPage.getByRole("button", { name: "Forms" });
    await expect(formsButton, "Forms toolbar control should be present").toBeEnabled();

    // Actions → Contact intake opens the package form fill surface.
    const actions = tauriPage.getByRole("button", { name: "Actions" });
    await actions.waitFor(30_000);
    await actions.click();
    const contactIntakeAction = tauriPage.getByRole("menuitem", {
      name: "Contact intake",
    });
    await contactIntakeAction.waitFor(30_000);
    await contactIntakeAction.click();

    const formsPanel = tauriPage.locator('[aria-label="Package forms"]');
    await formsPanel.waitFor(30_000);
    await expect(
      formsPanel.locator(".package-form-title"),
      "Actions → Contact intake should open the Contact intake form",
    ).toHaveText("Contact intake");

    // FormSave designer: Edit form → Save form control on native.
    const editForm = tauriPage.getByRole("button", { name: "Edit form" });
    await editForm.waitFor(30_000);
    await editForm.click();
    const saveForm = tauriPage.getByRole("button", { name: "Save form" });
    await saveForm.waitFor(30_000);
    await expect(
      saveForm,
      "FormSave designer should expose Save form on native CRM",
    ).toBeEnabled();
    await expect(
      formsPanel.locator(".package-form-title"),
      "FormSave designer title should reflect the ContactIntake form name",
    ).toHaveText("Edit ContactIntake");
  });
});
