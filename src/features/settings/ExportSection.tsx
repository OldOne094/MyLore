import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Button,
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogTitle,
  DialogTrigger,
  useToast,
} from "@/components/ui";
import { type ExportReport } from "@/api";
import { cn } from "@/lib/cn";
import {
  EXPORT_FORMATS,
  pickExportPath,
  useExportMedia,
  useExportTask,
  type ExportFormat,
} from "./export";
import { useTaskCancel } from "@/features/tasks/api";

/* MISSION-071 — Export-library section (settings page). Pick a format, choose a
   destination through the native save dialog, then stream the export as a
   background task with live progress; the typed `ExportReport` confirms the
   result. */

function baseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

export function ExportSection() {
  const { t } = useTranslation();
  const toast = useToast();
  const exportMedia = useExportMedia();
  const taskCancel = useTaskCancel();

  const [open, setOpen] = useState(false);
  const [format, setFormat] = useState<ExportFormat>("json");
  const [taskId, setTaskId] = useState<string | null>(null);

  const taskQuery = useExportTask(taskId);
  const task = taskQuery.data;
  const report = task?.state === "success" && task.result ? (task.result as ExportReport) : null;
  const exporting =
    taskId !== null && !report && task?.state !== "failed" && task?.state !== "cancelled";

  const openDialog = (value: boolean) => {
    setOpen(value);
    if (value) {
      setFormat("json");
      setTaskId(null);
    }
  };

  const runExport = () => {
    if (exporting) return;
    void pickExportPath(format, t("export.exportAction")).then((path) => {
      if (!path) return;
      exportMedia.mutate(
        { format, path },
        {
          onSuccess: (snapshot) => setTaskId(snapshot.id),
          onError: () => toast.error({ title: t("export.pickFailed") }),
        },
      );
    });
  };

  const cancelTask = () => {
    if (taskId) {
      taskCancel.mutate(taskId, {
        onError: () => toast.error({ title: t("export.cancelFailed") }),
      });
    }
  };

  return (
    <section className="rounded-md border border-border-subtle bg-bg-surface p-6">
      <h2 className="text-sm font-semibold text-text-primary">{t("settings.export")}</h2>
      <p className="mt-1 text-sm text-text-secondary">{t("settings.exportHint")}</p>
      <div className="mt-4">
        <Dialog open={open} onOpenChange={openDialog}>
          <DialogTrigger asChild>
            <Button variant="secondary">{t("settings.export")}</Button>
          </DialogTrigger>
          <DialogContent closeLabel={t("export.close")}>
            <DialogTitle>{t("export.dialogTitle")}</DialogTitle>
            <DialogDescription>{t("export.dialogHint")}</DialogDescription>

            <div className="mt-5 flex flex-col gap-4">
              <div
                role="group"
                aria-label={t("export.formatAria")}
                className="inline-flex items-center gap-1 rounded-full border border-border-subtle bg-bg-raised p-1"
              >
                {EXPORT_FORMATS.map((value) => (
                  <button
                    key={value}
                    type="button"
                    className={cn(
                      "rounded-full border-none bg-transparent px-3 py-1 text-sm text-text-secondary transition-colors duration-150 ease-out hover:bg-bg-hover hover:text-text-primary",
                      format === value && "bg-accent text-bg-surface hover:bg-accent",
                    )}
                    aria-pressed={format === value}
                    onClick={() => setFormat(value)}
                  >
                    {t(`export.format${value[0].toUpperCase()}${value.slice(1)}`)}
                  </button>
                ))}
              </div>

              {report ? (
                <div className="rounded-sm border border-border-subtle p-3 text-sm">
                  <p className="font-medium text-text-primary">{t("export.exportFinished")}</p>
                  <p className="mt-1 text-text-secondary">
                    {t("export.exportedCount", { count: report.total })} ·{" "}
                    {t("export.savedTo", { name: baseName(report.path) })}
                  </p>
                </div>
              ) : exporting ? (
                <div
                  role="status"
                  aria-live="polite"
                  className="flex flex-col gap-2 rounded-sm border border-border-subtle p-3 text-sm"
                >
                  <p className="font-medium text-text-primary">{t("export.exporting")}</p>
                  {task?.message ? <p className="text-text-secondary">{task.message}</p> : null}
                  <div className="h-1.5 w-full overflow-hidden rounded-full bg-bg-hover">
                    <div
                      className="h-full bg-accent transition-[width] duration-150 ease-out"
                      style={{ width: `${task?.progress ?? 0}%` }}
                    />
                  </div>
                </div>
              ) : task?.state === "failed" ? (
                <div className="rounded-sm border border-border-subtle p-3 text-sm">
                  <p className="font-medium text-text-primary">{t("export.errorTitle")}</p>
                  {task.error ? <p className="mt-1 text-text-secondary">{task.error}</p> : null}
                </div>
              ) : task?.state === "cancelled" ? (
                <div className="rounded-sm border border-border-subtle p-3 text-sm">
                  <p className="font-medium text-text-primary">{t("export.exportCancelled")}</p>
                </div>
              ) : null}

              <div className="mt-2 flex justify-end gap-2">
                {report || task?.state === "failed" || task?.state === "cancelled" ? (
                  <DialogClose asChild>
                    <Button>{t("export.close")}</Button>
                  </DialogClose>
                ) : exporting ? (
                  <>
                    <DialogClose asChild>
                      <Button variant="secondary">{t("export.close")}</Button>
                    </DialogClose>
                    <Button
                      variant="secondary"
                      onClick={cancelTask}
                      disabled={taskCancel.isPending}
                    >
                      {t("export.cancelTask")}
                    </Button>
                  </>
                ) : (
                  <>
                    <DialogClose asChild>
                      <Button variant="secondary">{t("export.close")}</Button>
                    </DialogClose>
                    <Button onClick={runExport} disabled={exportMedia.isPending}>
                      {exportMedia.isPending ? t("export.exporting") : t("export.exportAction")}
                    </Button>
                  </>
                )}
              </div>
            </div>
          </DialogContent>
        </Dialog>
      </div>
    </section>
  );
}
