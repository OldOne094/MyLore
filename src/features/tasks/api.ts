/* MISSION-070/071 — Shared background-task layer. `useTask` subscribes to one
   task's `task-changed` stream and exposes its live snapshot, falling back to
   `task_get` if an event is missed. `onSuccess` (typically invalidating
   library queries) runs when the task reaches the success terminal state.
   `useTaskCancel` requests cancellation; a cancelled task drops any partial
   output file. */

import { useCallback, useEffect, useRef } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { listenTaskChanged, task_cancel, task_get, task_list, type TaskSnapshot } from "@/api";
import { queryKeys } from "@/api";

export interface UseTaskOptions {
  onSuccess?: (snapshot: TaskSnapshot) => void;
}

/** Terminal states, mirroring `TaskState::is_terminal` on the Rust side. */
const TERMINAL_STATES = ["success", "failed", "cancelled"];
/** How often to re-read a task that has not reached a terminal state. */
const POLL_MS = 1000;
/** …and a task list that has nothing live in it. */
const IDLE_POLL_MS = 15000;

export function isTaskTerminal(snapshot: TaskSnapshot | undefined): boolean {
  return snapshot != null && TERMINAL_STATES.includes(snapshot.state);
}

/** Live snapshot of one background task; `null` while no task is set. */
export function useTask(taskId: string | null, options: UseTaskOptions = {}) {
  const queryClient = useQueryClient();
  const onSuccessRef = useRef(options.onSuccess);
  const reportedRef = useRef<string | null>(null);

  useEffect(() => {
    onSuccessRef.current = options.onSuccess;
  }, [options.onSuccess]);

  // Report success once per task, **whichever path delivered it**. The
  // subscription below is installed asynchronously while the spawn already
  // resolved, so a short task can finish before anyone is listening: relying on
  // the event alone left the progress panel spinning and skipped the refresh of
  // everything that depends on the finished task.
  const report = useCallback((snapshot: TaskSnapshot) => {
    if (snapshot.state !== "success" || reportedRef.current === snapshot.id) return;
    reportedRef.current = snapshot.id;
    onSuccessRef.current?.(snapshot);
  }, []);

  useEffect(() => {
    reportedRef.current = null;
  }, [taskId]);

  useEffect(() => {
    if (!taskId) return;
    let disposed = false;
    let unlisten: UnlistenFn | undefined;

    const sync = (snapshot: TaskSnapshot) => {
      queryClient.setQueryData(queryKeys.task.detail(taskId), snapshot);
      report(snapshot);
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
  }, [taskId, queryClient, report]);

  const query = useQuery({
    queryKey: queryKeys.task.detail(taskId ?? ""),
    queryFn: () => task_get({ id: taskId! }),
    // `!=` catches both null and undefined — an undefined id must never hit
    // the wire and must never collide on the empty-key cache slot.
    enabled: taskId != null,
    // Poll until the task ends. A single read was the only backstop for a missed
    // event, and if it returned a *running* snapshot nothing ever read again —
    // a task that had already finished stayed on screen as still running.
    refetchInterval: (query) => (isTaskTerminal(query.state.data) ? false : POLL_MS),
  });

  const { data } = query;
  useEffect(() => {
    if (data) report(data);
  }, [data, report]);

  return query;
}

/** Every task this session has spawned, newest first — the task centre.
    Polls faster while something is live, and quietly otherwise. */
export function useTaskList() {
  return useQuery({
    queryKey: queryKeys.task.all(),
    queryFn: () => task_list(),
    refetchInterval: (query) =>
      (query.state.data ?? []).some((task) => !isTaskTerminal(task)) ? 1500 : IDLE_POLL_MS,
  });
}

/** Request cancellation of a background task (MISSION-070). */
export function useTaskCancel() {
  return useMutation({
    mutationFn: (id: string) => task_cancel({ id }),
  });
}
