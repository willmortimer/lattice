import { useCallback, useState } from "react";

export interface NavigationState {
  paths: string[];
  index: number;
}

export interface NavigationController {
  state: NavigationState;
  canGoBack: boolean;
  canGoForward: boolean;
  record: (path: string) => void;
  go: (delta: -1 | 1) => string | null;
  replacePath: (from: string, to: string) => void;
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

export function useNavigationController(): NavigationController {
  const [state, setState] = useState<NavigationState>(() => createNavigationState());
  const record = useCallback((path: string) => setState((current) => recordNavigation(current, path)), []);
  const go = useCallback((delta: -1 | 1) => {
    let path: string | null = null;
    setState((current) => {
      const next = moveNavigation(current, delta);
      path = next === current ? null : next.paths[next.index] ?? null;
      return next;
    });
    return path;
  }, []);
  const replacePath = useCallback((from: string, to: string) => {
    setState((current) => ({
      ...current,
      paths: current.paths.map((path) => (path === from ? to : path)),
    }));
  }, []);
  const reset = useCallback(() => setState(createNavigationState()), []);
  return {
    state,
    canGoBack: state.index > 0,
    canGoForward: state.index >= 0 && state.index < state.paths.length - 1,
    record,
    go,
    replacePath,
    reset,
  };
}
