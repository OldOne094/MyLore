import { QueryClient } from "@tanstack/react-query";

/* MISSION-035 — Query client defaults. Local-first: domain data lives in
   SQLite behind IPC commands, so queries are cheap (short staleTime is safe),
   retries are pointless for domain errors, and refetch-on-window-focus is off. */

export function createQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: {
        staleTime: 60_000,
        gcTime: 5 * 60_000,
        retry: false,
        refetchOnWindowFocus: false,
      },
      mutations: {
        retry: false,
      },
    },
  });
}

export const queryClient = createQueryClient();
