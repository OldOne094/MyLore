import { useTranslation } from "react-i18next";
import { Badge, Button, Popover, PopoverContent, PopoverTrigger } from "@/components/ui";
import { isTaskTerminal, useTaskCancel, useTaskList } from "@/features/tasks/api";
import type { BadgeVariant } from "@/components/ui";
import type { TaskSnapshot } from "@/api";

/* MISSION-155 — the task centre. Background work used to be visible only from
   the dialog that started it: close the dialog and a running import/backup/sync
   became invisible, while `task_list` sat unused on the Rust side. The status bar
   now carries the live count and a panel with every task this session spawned. */

const STATE_VARIANTS: Record<string, BadgeVariant> = {
  queued: "neutral",
  running: "inprogress",
  success: "completed",
  failed: "dropped",
  cancelled: "neutral",
};

export function TaskCenter() {
  const { t } = useTranslation();
  const list = useTaskList();
  const cancel = useTaskCancel();

  // `useTaskList` normalises the reply, so anything the backend answers with
  // arrives here as an array.
  const tasks: TaskSnapshot[] = list.data ?? [];
  const live = tasks.filter((task) => !isTaskTerminal(task)).length;

  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button size="sm" variant="ghost" className="h-6 px-2 text-xs">
          {t("shell.tasks.label")}
          {live > 0 ? (
            <span
              className="ms-1.5 tabular-nums text-accent"
              aria-label={t("shell.tasks.liveAria", { count: live })}
            >
              {live}
            </span>
          ) : null}
        </Button>
      </PopoverTrigger>
      <PopoverContent align="start" className="w-96">
        <h2 className="text-sm font-semibold text-text-primary">{t("shell.tasks.heading")}</h2>
        {tasks.length === 0 ? (
          <p className="mt-2 text-sm text-text-secondary">{t("shell.tasks.empty")}</p>
        ) : (
          <ul className="mt-3 flex max-h-80 flex-col gap-2 overflow-y-auto">
            {tasks.map((task) => (
              <li
                key={task.id}
                className="flex items-start justify-between gap-3 border-b border-border-subtle pb-2 last:border-b-0"
              >
                <div className="min-w-0">
                  <p className="truncate text-sm text-text-primary">{task.title}</p>
                  <p className="mt-0.5 flex items-center gap-2 text-xs text-text-tertiary">
                    <Badge variant={STATE_VARIANTS[task.state] ?? "neutral"}>
                      {t(`shell.tasks.state.${task.state}`, { defaultValue: task.state })}
                    </Badge>
                    {typeof task.progress === "number" ? (
                      <span className="tabular-nums">
                        {t("shell.tasks.percent", { percent: task.progress })}
                      </span>
                    ) : null}
                  </p>
                  {task.message ? (
                    <p className="mt-0.5 truncate text-xs text-text-secondary">{task.message}</p>
                  ) : null}
                  {task.error ? (
                    <p className="mt-0.5 text-xs text-text-secondary">{task.error}</p>
                  ) : null}
                </div>
                {isTaskTerminal(task) ? null : (
                  <Button
                    size="sm"
                    variant="secondary"
                    disabled={cancel.isPending}
                    onClick={() => cancel.mutate(task.id)}
                  >
                    {t("shell.tasks.cancel")}
                  </Button>
                )}
              </li>
            ))}
          </ul>
        )}
      </PopoverContent>
    </Popover>
  );
}
