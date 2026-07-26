/** System-browser AuthPresenter for OAuth authorize URLs. */
import { openUrl } from "@tauri-apps/plugin-opener";

/**
 * Present an OAuth authorize URL to the user.
 * Default strategy: system browser (not an embedded webview).
 */
export async function presentAuthorizeUrl(authorizeUrl: string): Promise<void> {
  await openUrl(authorizeUrl);
}
