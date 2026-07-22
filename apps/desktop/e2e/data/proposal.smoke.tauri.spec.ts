/**
 * Native proposal inbox accept / undo smoke (Tauri WebView).
 *
 * Seeds via the sidebar **Create demo proposal** control (`create_demo_proposal`
 * IPC), opens the Proposals inbox, approves the page-create, asserts
 * `Proposals/Welcome.md` lands in the tree, then ⌘Z undoes.
 *
 * Local run (not a CI gate):
 *
 *   pnpm --filter @lattice/desktop test:proposal:tauri
 *
 * Or two terminals with a reset First Look seed:
 *
 *   pnpm --filter @lattice/desktop tauri:dev:e2e
 *   pnpm --filter @lattice/desktop exec playwright test --project=tauri e2e/data/proposal.smoke.tauri.spec.ts
 *
 * Requires `LATTICE_DEV_HOME` + `LATTICE_DEV_RESET_DEMO=1` (tauri:dev:e2e /
 * the wrapper) so the native Proposals inbox is available.
 */
import {
  acceptProposalReview,
  openProposalInboxItem,
  proposalSmoke,
  scrollTreeUntilLabel,
  seedDemoProposal,
  treeLabelMounted,
  treeSmoke,
  undoWorkspaceChange,
  waitForShellChrome,
} from "../perf/helpers";
import { expect, test } from "../fixtures";

const PROPOSAL_TIMEOUT_MS = 45_000;
const WELCOME_LABEL = treeSmoke.pageLabel(proposalSmoke.welcomePagePath);

test.describe("Proposal inbox smoke (tauri)", () => {
  test.beforeEach(async ({ tauriPage }) => {
    tauriPage.setDefaultTimeout(PROPOSAL_TIMEOUT_MS);
    await waitForShellChrome(tauriPage);
  });

  test("seeds demo proposal, approves page-create, and undoes", async ({ tauriPage }) => {
    await expect(
      tauriPage.locator(proposalSmoke.inbox),
      "Proposals inbox should render on native desktop",
    ).toBeVisible();

    await seedDemoProposal(tauriPage);

    const inboxItem = tauriPage
      .locator(".proposal-inbox-item")
      .filter({ hasText: proposalSmoke.demoSummary });
    await expect
      .poll(async () => inboxItem.count(), { timeout: PROPOSAL_TIMEOUT_MS })
      .toBeGreaterThan(0);

    await openProposalInboxItem(tauriPage, proposalSmoke.demoSummary);

    await expect(
      tauriPage.locator(proposalSmoke.reviewTitle),
      "Review modal should open for the demo proposal",
    ).toHaveText("Review proposed changes");

    await acceptProposalReview(tauriPage);

    await scrollTreeUntilLabel(tauriPage, WELCOME_LABEL);
    await expect
      .poll(async () => treeLabelMounted(tauriPage, WELCOME_LABEL), {
        timeout: PROPOSAL_TIMEOUT_MS,
      })
      .toBe(true);

    await undoWorkspaceChange(tauriPage);

    await expect
      .poll(async () => treeLabelMounted(tauriPage, WELCOME_LABEL), {
        timeout: PROPOSAL_TIMEOUT_MS,
      })
      .toBe(false);
  });
});
