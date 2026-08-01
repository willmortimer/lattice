import { useQuery, type QueryClient } from "@tanstack/react-query";

import { inBrowser } from "../demo";
import { getCloudSessionStatus, type CloudSessionStatus } from "../lib/cloud";
import { queryKeys } from "./keys";

export function cloudSessionQueryOptions() {
  return {
    queryKey: queryKeys.cloudSession(),
    queryFn: getCloudSessionStatus,
    enabled: !inBrowser,
  } as const;
}

export function useCloudSessionQuery() {
  return useQuery(cloudSessionQueryOptions());
}

export function setCloudSessionCache(
  queryClient: QueryClient,
  status: CloudSessionStatus,
): void {
  queryClient.setQueryData(queryKeys.cloudSession(), status);
}

export function invalidateCloudSession(queryClient: QueryClient): Promise<void> {
  return queryClient.invalidateQueries({ queryKey: queryKeys.cloudSession() });
}
