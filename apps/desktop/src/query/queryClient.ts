import { QueryClient } from "@tanstack/react-query";

/** Shared defaults: daemon events drive freshness, not focus polling. */
export function createDesktopQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: {
        staleTime: 30_000,
        refetchOnWindowFocus: false,
        retry: false,
      },
    },
  });
}

export const queryClient = createDesktopQueryClient();
