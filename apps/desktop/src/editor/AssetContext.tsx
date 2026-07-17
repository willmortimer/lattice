import { createContext, useContext } from "react";

export interface AssetContextValue {
  /**
   * The open workspace's root, or `null` when there is nothing real on
   * disk to resolve relative embeds against (the in-browser demo shell,
   * or no workspace open yet).
   */
  root: string | null;
  /** The open page's path, relative to `root` — embeds resolve relative
   * to this file's own directory, not the workspace root. */
  pagePath: string;
  /** Open an embedded resource when the user activates a lattice-embed card. */
  onOpenEmbed?: (path: string) => void;
}

const DEFAULT_VALUE: AssetContextValue = { root: null, pagePath: "" };

/**
 * Threads the open workspace's root and page path down to Tiptap node
 * views (`ImageView`) that render read-view embeds. `PageEditor` renders
 * inside whatever provider its caller wraps it in — a plain React
 * context works across Tiptap's `ReactNodeViewRenderer` portals, which
 * preserve their surrounding component tree for context purposes even
 * though they render into a DOM node ProseMirror owns directly.
 */
const AssetContext = createContext<AssetContextValue>(DEFAULT_VALUE);

export const AssetContextProvider = AssetContext.Provider;

export function useAssetContext(): AssetContextValue {
  return useContext(AssetContext);
}
