import { useQuery, type QueryClient } from "@tanstack/react-query";

import { inBrowser } from "../demo";
import { getRemoteAccessStatus, type RemoteAccessStatus } from "../lib/remoteAccess";
import { queryKeys } from "./keys";

export function remoteAccessQueryOptions() {
  return {
    queryKey: queryKeys.remoteAccess(),
    queryFn: getRemoteAccessStatus,
    enabled: !inBrowser,
  } as const;
}

export function useRemoteAccessQuery() {
  return useQuery(remoteAccessQueryOptions());
}

export function setRemoteAccessCache(
  queryClient: QueryClient,
  status: RemoteAccessStatus,
): void {
  queryClient.setQueryData(queryKeys.remoteAccess(), status);
}

export function invalidateRemoteAccess(queryClient: QueryClient): Promise<void> {
  return queryClient.invalidateQueries({ queryKey: queryKeys.remoteAccess() });
}
