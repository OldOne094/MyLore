/* MISSION-063 — Provider settings data layer. Reads the provider snapshot and
   exposes typed mutations for enable/disable, API keys (stored in the OS
   keyring by the backend — never returned) and test connection. Every mutation
   refreshes the providers list so toggles and key states stay in sync. */

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import {
  anilist_oauth_start,
  listenAnilistOauth,
  providers_list,
  provider_set_enabled,
  provider_set_key,
  provider_test_connection,
} from "@/api";
import { queryKeys } from "@/api";

/** Snapshot of one registered provider for the settings UI. */
export interface ProviderSettingsRow {
  provider: string;
  name: string;
  enabled: boolean;
  requires_key: boolean;
  has_key: boolean;
}

export function useProvidersQuery() {
  return useQuery({
    queryKey: queryKeys.settings.providers(),
    queryFn: providers_list,
    staleTime: 30_000,
  });
}

export function useSetProviderEnabled() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ provider, enabled }: { provider: string; enabled: boolean }) =>
      provider_set_enabled({ provider, enabled }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.settings.providers() });
    },
  });
}

export function useSetProviderKey() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ provider, apiKey }: { provider: string; apiKey: string }) =>
      provider_set_key({ provider: provider, apiKey: apiKey }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.settings.providers() });
    },
  });
}

/** Ping one provider; the outcome (ok/message/results) renders inline. */
export function useTestConnection() {
  return useMutation({
    mutationFn: ({ provider }: { provider: string }) => provider_test_connection({ provider }),
  });
}

/** MISSION-130 - open the system browser at AniList's authorize page and
    wait for the loopback callback. The token lands in the backend secret
    store; completion arrives via the anilist-oauth event (see
    {@link useAnilistOauthEvent}). */
export function useAnilistConnect() {
  return useMutation({ mutationFn: () => anilist_oauth_start() });
}

/** Subscribe once to the AniList OAuth completion event: refreshes the
    providers snapshot and hands the outcome to the caller for toasting. */
export function useAnilistOauthEvent(onDone: (ok: boolean, message?: string) => void) {
  const queryClient = useQueryClient();
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    listenAnilistOauth((payload) => {
      if (payload.ok) {
        void queryClient.invalidateQueries({ queryKey: queryKeys.settings.providers() });
      }
      onDone(payload.ok, payload.message);
    }).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}
