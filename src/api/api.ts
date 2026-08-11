import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { greet as greetCommand } from "./ipc.generated";
import { queryKeys } from "./queryKeys";

/* MISSION-035 — Typed command wrappers (`api.ts`). IPC crossing stays inside
   the generated wrappers; this layer adds React Query hooks so features read
   and write domain data without touching invoke or cache keys directly. */

export const api = {
  greet: greetCommand,
};

export function useGreetQuery(name: string) {
  return useQuery({
    queryKey: queryKeys.system.greeting(name),
    queryFn: () => api.greet({ name }),
  });
}

export function useGreetMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ name }: { name: string }) => api.greet({ name }),
    onSuccess: (greeting, { name }) => {
      queryClient.setQueryData(queryKeys.system.greeting(name), greeting);
    },
  });
}
