import { useEffect } from "react";
import { useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";

import { inBrowser } from "../demo";
import { getVoiceStatus, listenVoiceEvents, type VoiceStatus } from "../lib/voice";
import { DEFAULT_VOICE_PROVIDER_ID, queryKeys } from "./keys";

export function voiceStatusQueryOptions(providerId: string = DEFAULT_VOICE_PROVIDER_ID) {
  return {
    queryKey: queryKeys.voiceStatus(providerId),
    queryFn: getVoiceStatus,
    enabled: !inBrowser,
  } as const;
}

export function useVoiceStatusQuery(options?: { enabled?: boolean; providerId?: string }) {
  const providerId = options?.providerId ?? DEFAULT_VOICE_PROVIDER_ID;
  const queryEnabled = (options?.enabled ?? true) && !inBrowser;
  const queryClient = useQueryClient();

  const query = useQuery({
    ...voiceStatusQueryOptions(providerId),
    enabled: queryEnabled,
  });

  useEffect(() => {
    if (!queryEnabled) return;
    let unlisten: (() => void) | undefined;
    void listenVoiceEvents((event) => {
      if (event.type !== "status" && event.type !== "failed") return;
      queryClient.setQueryData<VoiceStatus>(queryKeys.voiceStatus(providerId), (prev) => {
        if (event.type === "failed") {
          return prev ?? {
            available: false,
            prepared: false,
            preparing: false,
            listening: false,
            nativeCapture: false,
            platform: "macos",
            message: event.message,
          };
        }
        const preparing = event.state === "preparing";
        const prepared = event.state === "ready" || prev?.prepared === true;
        return {
          available: prev?.available ?? true,
          prepared,
          preparing,
          listening: event.state === "listening",
          nativeCapture: prev?.nativeCapture ?? false,
          platform: prev?.platform ?? "macos",
          message: event.message,
        };
      });
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [providerId, queryClient, queryEnabled]);

  return query;
}

export function setVoiceStatusCache(
  queryClient: QueryClient,
  status: VoiceStatus,
  providerId: string = DEFAULT_VOICE_PROVIDER_ID,
): void {
  queryClient.setQueryData(queryKeys.voiceStatus(providerId), status);
}

export function invalidateVoiceStatus(
  queryClient: QueryClient,
  providerId: string = DEFAULT_VOICE_PROVIDER_ID,
): Promise<void> {
  return queryClient.invalidateQueries({ queryKey: queryKeys.voiceStatus(providerId) });
}
