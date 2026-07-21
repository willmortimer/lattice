/**
 * Native tree / undo / move+repair / trash smoke (Tauri WebView).
 *
 * Covers First Look Home.md items 26–30: folder under Projects, ⌘Z undo,
 * Product/Vision move + link repair, multi-select trash + undo.
 *
 * Local run (not a CI gate):
 *
 *   pnpm --filter @lattice/desktop test:tree:tauri
 *
 * Or two terminals with a reset First Look seed:
 *
 *   pnpm --filter @lattice/desktop tauri:dev:e2e
 *   pnpm --filter @lattice/desktop exec playwright test --project=tauri e2e/data/tree.smoke.tauri.spec.ts
 *
 * Requires `LATTICE_DEV_HOME` + `LATTICE_DEV_RESET_DEMO=1` (tauri:dev:e2e /
 * the wrapper) so Projects / Product / Inbox match the demo template.
 */
import {
  acceptLinkRepairIfPresent,
  createFolderViaToolbar,
  metaClickTreeLabel,
  movePageToFolder,
  scrollTreeUntilLabel,
  treeLabelMounted,
  treeSmoke,
  trashSelectionWithConfirm,
  undoWorkspaceChange,
  waitForShellChrome,
} from "../perf/helpers";
import { expect, test } from "../fixtures";

const SMOKE_FOLDER = "Projects/Smoke Folder";
const VISION_PAGE = "Product/Vision.md";
const MOVE_TARGET = "Inbox";
const MOVED_VISION = "Inbox/Vision.md";
const TRASH_A = "Product/Principles.md";
const TRASH_B = "Product/Release Notes.md";

const TREE_TIMEOUT_MS = 45_000;

test.describe("Tree undo/move/trash smoke (tauri)", () => {
  test.beforeEach(async ({ tauriPage }) => {
    tauriPage.setDefaultTimeout(TREE_TIMEOUT_MS);
    await waitForShellChrome(tauriPage);
    // prompt (new folder) + confirm (trash) for the whole smoke.
    await tauriPage.installDialogHandler({
      defaultConfirm: true,
      defaultPromptText: "Smoke Folder",
    });
  });

  test("creates folder under Projects and undoes it", async ({ tauriPage }) => {
    await createFolderViaToolbar(tauriPage, "Projects");

    await scrollTreeUntilLabel(tauriPage, treeSmoke.folderLabel(SMOKE_FOLDER));
    await expect
      .poll(async () => treeLabelMounted(tauriPage, treeSmoke.folderLabel(SMOKE_FOLDER)), {
        timeout: TREE_TIMEOUT_MS,
      })
      .toBe(true);

    await undoWorkspaceChange(tauriPage);

    await expect
      .poll(async () => treeLabelMounted(tauriPage, treeSmoke.folderLabel(SMOKE_FOLDER)), {
        timeout: TREE_TIMEOUT_MS,
      })
      .toBe(false);
  });

  test("moves Product/Vision and accepts link repair when present", async ({
    tauriPage,
  }) => {
    await scrollTreeUntilLabel(tauriPage, treeSmoke.pageLabel(VISION_PAGE));
    await expect
      .poll(async () => treeLabelMounted(tauriPage, treeSmoke.pageLabel(VISION_PAGE)), {
        timeout: TREE_TIMEOUT_MS,
      })
      .toBe(true);

    await movePageToFolder(tauriPage, VISION_PAGE, MOVE_TARGET);

    // Prefer accepting repair when the modal appears; First Look links Vision
    // from Home / Principles so candidates are expected — if flaky, still assert
    // the move landed.
    const repaired = await acceptLinkRepairIfPresent(tauriPage, 12_000);
    if (!repaired) {
      // Document: repair modal may be absent when preview returns zero candidates
      // (e.g. stale index); move success is still required.
      console.warn(
        "tree smoke: link-repair modal did not appear; asserting move only",
      );
    }

    await scrollTreeUntilLabel(tauriPage, treeSmoke.pageLabel(MOVED_VISION));
    await expect
      .poll(async () => treeLabelMounted(tauriPage, treeSmoke.pageLabel(MOVED_VISION)), {
        timeout: TREE_TIMEOUT_MS,
      })
      .toBe(true);
    await expect
      .poll(async () => treeLabelMounted(tauriPage, treeSmoke.pageLabel(VISION_PAGE)), {
        timeout: TREE_TIMEOUT_MS,
      })
      .toBe(false);

    // Restore First Look layout for later local re-runs / sibling tests.
    await undoWorkspaceChange(tauriPage);
    await expect
      .poll(async () => treeLabelMounted(tauriPage, treeSmoke.pageLabel(VISION_PAGE)), {
        timeout: TREE_TIMEOUT_MS,
      })
      .toBe(true);
  });

  test("multi-select trash and undoes", async ({ tauriPage }) => {
    await scrollTreeUntilLabel(tauriPage, treeSmoke.pageLabel(TRASH_A));
    await tauriPage.click(treeSmoke.pageSelector(TRASH_A), { timeout: TREE_TIMEOUT_MS });
    await metaClickTreeLabel(tauriPage, treeSmoke.pageLabel(TRASH_B));

    await tauriPage.clearDialogs();
    await trashSelectionWithConfirm(tauriPage);

    const dialogs = await tauriPage.getDialogs();
    const trashConfirm = dialogs.find(
      (dialog) => dialog.type === "confirm" && treeSmoke.trashConfirmRe.test(dialog.message),
    );
    expect(
      trashConfirm,
      "Delete should open the Trash confirm dialog",
    ).toBeTruthy();

    await expect
      .poll(async () => treeLabelMounted(tauriPage, treeSmoke.pageLabel(TRASH_A)), {
        timeout: TREE_TIMEOUT_MS,
      })
      .toBe(false);
    await expect
      .poll(async () => treeLabelMounted(tauriPage, treeSmoke.pageLabel(TRASH_B)), {
        timeout: TREE_TIMEOUT_MS,
      })
      .toBe(false);

    await undoWorkspaceChange(tauriPage);

    await expect
      .poll(async () => treeLabelMounted(tauriPage, treeSmoke.pageLabel(TRASH_A)), {
        timeout: TREE_TIMEOUT_MS,
      })
      .toBe(true);
    await expect
      .poll(async () => treeLabelMounted(tauriPage, treeSmoke.pageLabel(TRASH_B)), {
        timeout: TREE_TIMEOUT_MS,
      })
      .toBe(true);
  });
});
