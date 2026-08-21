/* MISSION-088 — Recovery data layer. The app-health query gates the whole
   shell; the two recovery actions swap files under a closed pool and always
   end with "restart required". */

import { useMutation, useQuery } from "@tanstack/react-query";
import {
  app_health,
  recover_restore,
  recover_start_fresh,
  type HealthStatus,
  type RecoveryOutcome,
} from "@/api";
import { queryKeys } from "@/api";

/** Startup database health — false means recovery mode. */
export function useAppHealth() {
  return useQuery({
    queryKey: queryKeys.health.app(),
    queryFn: () => app_health(),
    staleTime: Number.POSITIVE_INFINITY,
  });
}

/** Restore a `.mylore` archive over a corrupt database. Restart afterwards. */
export function useRecoverRestore() {
  return useMutation({
    mutationFn: (path: string): Promise<RecoveryOutcome> => recover_restore({ path }),
  });
}

/** Move the corrupt database aside; next startup starts fresh. */
export function useRecoverStartFresh() {
  return useMutation({
    mutationFn: (): Promise<RecoveryOutcome> => recover_start_fresh(),
  });
}

export type { HealthStatus, RecoveryOutcome };
