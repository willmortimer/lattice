import navigationJson from "../../../../docs/help/navigation.json";

import canvasMd from "../../../../docs/help/canvas.md?raw";
import firstWorkspaceMd from "../../../../docs/help/first-workspace.md?raw";
import findAndJumpMd from "../../../../docs/help/find-and-jump.md?raw";
import importCsvMd from "../../../../docs/help/import-csv.md?raw";
import inspectMd from "../../../../docs/help/inspect.md?raw";
import offlineMd from "../../../../docs/help/offline.md?raw";
import pagesMd from "../../../../docs/help/pages.md?raw";
import somethingWrongMd from "../../../../docs/help/something-wrong.md?raw";
import welcomeMd from "../../../../docs/help/welcome.md?raw";
import whatToClickMd from "../../../../docs/help/what-to-click.md?raw";

import type { HelpNavSection } from "./helpCorpus";

export const HELP_NAVIGATION = navigationJson as HelpNavSection[];

/** Raw markdown keyed by navigation `file` (e.g. `welcome.md`). */
export const HELP_RAW_BY_FILE: Record<string, string> = {
  "welcome.md": welcomeMd,
  "first-workspace.md": firstWorkspaceMd,
  "what-to-click.md": whatToClickMd,
  "pages.md": pagesMd,
  "find-and-jump.md": findAndJumpMd,
  "import-csv.md": importCsvMd,
  "canvas.md": canvasMd,
  "something-wrong.md": somethingWrongMd,
  "inspect.md": inspectMd,
  "offline.md": offlineMd,
};
