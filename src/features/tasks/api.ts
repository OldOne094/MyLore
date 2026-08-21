/* MISSION-070/071 — Shared background-task layer. `useTask` subscribes to one
   task's `task-changed` stream and exposes its live snapshot, falling back to
   `task_get` if an event is missed. `onSuccess` (typically invalidating
   library queries) runs when the task reaches the success terminal state.
   `useTaskCancel` requests cancellation; a cancelled task drops any partial
   output file. */

import { useEffect, useRef } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { listenTaskChanged, task_cancel, task_get, type TaskSnapshot } from "@/api";
import { queryKeys } from "@/api";

export interface UseTaskOptions {
  onSuccess?: (snapshot: TaskSnapshot) => void;
}

/** Live snapshot of one background task; `null` while no task is set. */
export function useTask(taskId: string | null, options: UseTaskOptions = {}) {
  const queryClient = useQueryClient();
  const onSuccessRef = useRef(options.onSuccess);

  useEffect(() => {
    onSuccessRef.current = options.onSuccess;
  }, [options.onSuccess]);

  useEffect(() => {
    if (!taskId) return;
    let disposed = false;
    let unlisten: UnlistenFn | undefined;

    const sync = (snapshot: TaskSnapshot) => {
      queryClient.setQueryData(queryKeys.task.detail(taskId), snapshot);
      if (snapshot.state === "success") onSuccessRef.current?.(snapshot);
    };

    void listenTaskChanged((snapshot) => {
      if (snapshot.id !== taskId) return;
      sync(snapshot);
    }).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [taskId, queryClient]);

  return useQuery({
    queryKey: queryKeys.task.detail(taskId ?? ""),
    queryFn: () => task_get({ id: taskId! }),
    // `!=` catches both null and undefined — an undefined id must never hit
    // the wire and must never collide on the empty-key cache slot.
    enabled: taskId != null,
  });
}

/** Request cancellation of a background task (MISSION-070). */
export function useTaskCancel() {
  return useMutation({
    mutationFn: (id: string) => task_cancel({ id }),
  });
}
