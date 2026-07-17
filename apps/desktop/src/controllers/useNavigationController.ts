import { useCallback, useRef, useState, type Dispatch, type SetStateAction } from "react";
import type { Resource } from "../types";

export type ActivityArea = "home" | "files" | "search" | "quick-note" | "settings";

export interface NavigationState {
  paths: string[];
  index: number;
}

export interface NavigationSessionState {
  tabs: Resource[];
  activity: ActivityArea;
}

export interface NavigationControllerOptions {
  maxOpenTabs: number;
  getResource: (path: string) => Resource | null;
  getSelectedPath: () => string | null;
  canCloseTab: (path: string) => boolean;
  onSelect: (resource: Resource, options?: { recordHistory?: boolean }) => void | Promise<void>;
  onClearSelection: () => void;
}

export interface NavigationController {
  state: NavigationState;
  canGoBack: boolean;
  canGoForward: boolean;
  activityArea: ActivityArea;
  setActivityArea: Dispatch<SetStateAction<ActivityArea>>;
  openTabs: Resource[];
  record: (path: string) => void;
  go: (delta: -1 | 1) => string | null;
  navigateHistory: (delta: -1 | 1) => void;
  replacePath: (from: string, to: string) => void;
  openTab: (resource: Resource) => void;
  closeTab: (path: string) => void;
  reorderTab: (from: string, to: string) => void;
  replaceTab: (from: string, to: Resource) => void;
  removeTabs: (predicate: (resource: Resource) => boolean) => void;
  restoreSession: (session: Pick<NavigationSessionState, "tabs" | "activity">) => void;
  getSessionState: () => NavigationSessionState;
  reset: () => void;
}

export function createNavigationState(paths: string[] = [], index = paths.length - 1): NavigationState {
  return { paths: [...paths], index };
}

export function recordNavigation(state: NavigationState, path: string, limit = 60): NavigationState {
  if (state.paths[state.index] === path) return state;
  const paths = [...state.paths.slice(0, state.index + 1), path].slice(-limit);
  return { paths, index: paths.length - 1 };
}

export function moveNavigation(state: NavigationState, delta: -1 | 1): NavigationState {
  const index = state.index + delta;
  return index < 0 || index >= state.paths.length ? state : { ...state, index };
}

export function closeTabList(
  tabs: Resource[],
  path: string,
  selectedPath: string | null,
): { tabs: Resource[]; fallback: Resource | null; cleared: boolean } {
  const index = tabs.findIndex((tab) => tab.path === path);
  if (index < 0) return { tabs, fallback: null, cleared: false };
  const next = tabs.filter((tab) => tab.path !== path);
  if (selectedPath !== path) return { tabs: next, fallback: null, cleared: false };
  return {
    tabs: next,
    fallback: next[Math.min(index, next.length - 1)] ?? null,
    cleared: next.length === 0,
  };
}

export function reorderTabList(tabs: Resource[], from: string, to: string): Resource[] {
  if (from === to) return tabs;
  const fromIndex = tabs.findIndex((tab) => tab.path === from);
  const toIndex = tabs.findIndex((tab) => tab.path === to);
  if (fromIndex < 0 || toIndex < 0) return tabs;
  const next = [...tabs];
  const [moved] = next.splice(fromIndex, 1);
  next.splice(toIndex, 0, moved);
  return next;
}

export function useNavigationController(options: NavigationControllerOptions): NavigationController {
  const optionsRef = useRef(options);
  optionsRef.current = options;
  const [state, setState] = useState<NavigationState>(() => createNavigationState());
  const [activityArea, setActivityArea] = useState<ActivityArea>("files");
  const [openTabs, setOpenTabs] = useState<Resource[]>([]);
  const stateRef = useRef(state);
  const tabsRef = useRef(openTabs);
  const activityRef = useRef(activityArea);
  stateRef.current = state;
  tabsRef.current = openTabs;
  activityRef.current = activityArea;

  const record = useCallback((path: string) => {
    setState((current) => recordNavigation(current, path));
  }, []);

  const go = useCallback((delta: -1 | 1) => {
    const next = moveNavigation(stateRef.current, delta);
    if (next === stateRef.current) return null;
    stateRef.current = next;
    setState(next);
    return next.paths[next.index] ?? null;
  }, []);

  const navigateHistory = useCallback((delta: -1 | 1) => {
    const next = moveNavigation(stateRef.current, delta);
    const path = next.paths[next.index];
    if (!path) return;
    const resource = optionsRef.current.getResource(path);
    if (!resource) return;
    go(delta);
    void optionsRef.current.onSelect(resource, { recordHistory: false });
  }, [go]);

  const replacePath = useCallback((from: string, to: string) => {
    setState((current) => ({
      ...current,
      paths: current.paths.map((path) => (path === from ? to : path)),
    }));
  }, []);

  const openTab = useCallback((resource: Resource) => {
    setOpenTabs((tabs) => tabs.some((tab) => tab.path === resource.path)
      ? tabs
      : [...tabs, resource].slice(-optionsRef.current.maxOpenTabs));
  }, []);

  const closeTab = useCallback((path: string) => {
    if (!optionsRef.current.canCloseTab(path)) return;
    const result = closeTabList(tabsRef.current, path, optionsRef.current.getSelectedPath());
    if (result.tabs === tabsRef.current) return;
    setOpenTabs(result.tabs);
    if (result.fallback) {
      void optionsRef.current.onSelect(result.fallback, { recordHistory: false });
    } else if (result.cleared) {
      optionsRef.current.onClearSelection();
    }
  }, []);

  const reorderTab = useCallback((from: string, to: string) => {
    setOpenTabs((tabs) => reorderTabList(tabs, from, to));
  }, []);

  const replaceTab = useCallback((from: string, to: Resource) => {
    setOpenTabs((tabs) => tabs.map((tab) => tab.path === from ? to : tab));
  }, []);

  const removeTabs = useCallback((predicate: (resource: Resource) => boolean) => {
    setOpenTabs((tabs) => tabs.filter((tab) => !predicate(tab)));
  }, []);

  const restoreSession = useCallback((session: Pick<NavigationSessionState, "tabs" | "activity">) => {
    setOpenTabs(session.tabs.slice(-optionsRef.current.maxOpenTabs));
    setActivityArea(session.activity);
  }, []);

  const getSessionState = useCallback((): NavigationSessionState => ({
    tabs: tabsRef.current,
    activity: activityRef.current,
  }), []);

  const reset = useCallback(() => {
    const next = createNavigationState();
    stateRef.current = next;
    setState(next);
    setOpenTabs([]);
    setActivityArea("home");
  }, []);

  return {
    state,
    canGoBack: state.index > 0,
    canGoForward: state.index >= 0 && state.index < state.paths.length - 1,
    activityArea,
    setActivityArea,
    openTabs,
    record,
    go,
    navigateHistory,
    replacePath,
    openTab,
    closeTab,
    reorderTab,
    replaceTab,
    removeTabs,
    restoreSession,
    getSessionState,
    reset,
  };
}
