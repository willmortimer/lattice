/**
 * Per-window warm cache for workspace UI sessions.
 *
 * Closed or background workspaces stay in profile storage; the active workspace
 * and recently visited ids are kept here so quick switches do not round-trip SQLite.
 */
import { create } from "zustand";

import {
  defaultWorkspaceUiSession,
  normalizeWorkspaceUiSession,
  type WorkspaceUiSession,
} from "../lib/workspaceUiSession";

type WorkspaceUiSessionStore = {
  activeWorkspaceId: string | null;
  warmSessions: Record<string, WorkspaceUiSession>;
  setActiveWorkspaceId: (workspaceId: string | null) => void;
  getWarmSession: (workspaceId: string) => WorkspaceUiSession | undefined;
  setWarmSession: (session: WorkspaceUiSession) => void;
  clearWarmSession: (workspaceId: string) => void;
};

export const useWorkspaceUiSessionStore = create<WorkspaceUiSessionStore>((set, get) => ({
  activeWorkspaceId: null,
  warmSessions: {},
  setActiveWorkspaceId: (workspaceId) => set({ activeWorkspaceId: workspaceId }),
  getWarmSession: (workspaceId) => get().warmSessions[workspaceId],
  setWarmSession: (session) => {
    const normalized = normalizeWorkspaceUiSession(session.workspaceId, session);
    set((state) => ({
      warmSessions: { ...state.warmSessions, [normalized.workspaceId]: normalized },
    }));
  },
  clearWarmSession: (workspaceId) =>
    set((state) => {
      if (!(workspaceId in state.warmSessions)) return state;
      const { [workspaceId]: _removed, ...rest } = state.warmSessions;
      return { warmSessions: rest };
    }),
}));

export function warmWorkspaceUiSession(workspaceId: string): WorkspaceUiSession {
  return (
    useWorkspaceUiSessionStore.getState().getWarmSession(workspaceId) ??
    defaultWorkspaceUiSession(workspaceId)
  );
}
