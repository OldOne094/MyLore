import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ShieldCheck, ShieldOff } from "lucide-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
  InputField,
  useToast,
} from "@/components/ui";
import { db_disable_encryption, db_enable_encryption, db_security_status, queryKeys } from "@/api";

/* MISSION-112 — At-rest encryption card (Settings). Shows the live posture
   and drives the in-place SQLCipher rekey both ways. The backend stores the
   passphrase in the secret pipeline; a restart applies it cleanly on the
   next open. */

type Phase = "idle" | "enable" | "disable";

export function SecuritySection() {
  const { t } = useTranslation();
  const toast = useToast();
  const queryClient = useQueryClient();
  const [phase, setPhase] = useState<Phase>("idle");
  const [pass1, setPass1] = useState("");
  const [pass2, setPass2] = useState("");
  const [error, setError] = useState<string | null>(null);

  const status = useQuery({
    queryKey: queryKeys.settings.dbSecurity(),
    queryFn: db_security_status,
  });

  const openEnable = () => {
    setPass1("");
    setPass2("");
    setError(null);
    setPhase("enable");
  };

  const enable = useMutation({
    mutationFn: () => db_enable_encryption({ passphrase: pass1 }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.settings.dbSecurity() });
      toast.success({ title: t("settings.encryptionEnabledToast") });
    },
    onError: (e) => setError(e instanceof Error ? e.message : String(e)),
  });

  const disable = useMutation({
    mutationFn: () => db_disable_encryption(),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.settings.dbSecurity() });
      toast.info({ title: t("settings.encryptionDisabledToast") });
    },
    onError: (e) => setError(e instanceof Error ? e.message : String(e)),
  });

  const busy = enable.isPending || disable.isPending;
  const data = status.data;

  const submitEnable = () => {
    setError(null);
    if (pass1.length < 8) {
      setError(t("settings.encryptionPassTooShort"));
      return;
    }
    if (pass1 !== pass2) {
      setError(t("settings.encryptionPassMismatch"));
      return;
    }
    enable.mutate();
  };

  return (
    <div className="flex flex-col gap-3">
      {status.isPending ? (
        <p className="text-sm text-text-secondary">{t("settings.encryptionChecking")}</p>
      ) : status.isError ? (
        <p className="text-sm text-danger">{t("settings.encryptionStatusError")}</p>
      ) : data?.encrypted ? (
        <p className="flex items-center gap-2 text-sm text-status-completed">
          <ShieldCheck size={16} aria-hidden="true" />
          {t("settings.encryptionOn")}
        </p>
      ) : (
        <p className="flex items-center gap-2 text-sm text-text-secondary">
          <ShieldOff size={16} aria-hidden="true" />
          {t("settings.encryptionOff")}
        </p>
      )}

      {!data ? null : !data.available ? (
        <p className="text-xs text-text-tertiary">{t("settings.encryptionUnavailable")}</p>
      ) : data.encrypted ? (
        <Button variant="secondary" size="sm" disabled={busy} onClick={() => setPhase("disable")}>
          {t("settings.encryptionDisable")}
        </Button>
      ) : (
        <Button variant="primary" size="sm" disabled={busy} onClick={openEnable}>
          {t("settings.encryptionEnable")}
        </Button>
      )}

      <Dialog open={phase === "enable"} onOpenChange={(open) => !open && setPhase("idle")}>
        <DialogContent closeLabel={t("a11y.close")}>
          <DialogTitle>{t("settings.encryptionEnableTitle")}</DialogTitle>
          <DialogDescription>{t("settings.encryptionEnableHint")}</DialogDescription>

          <div className="mt-4 flex flex-col gap-3">
            <InputField
              label={t("settings.encryptionPassLabel")}
              type="password"
              autoComplete="new-password"
              value={pass1}
              onChange={(e) => setPass1(e.target.value)}
            />
            <InputField
              label={t("settings.encryptionPassConfirmLabel")}
              type="password"
              autoComplete="new-password"
              value={pass2}
              onChange={(e) => setPass2(e.target.value)}
              error={error ?? undefined}
            />
            <p className="text-xs font-medium text-warn">{t("settings.encryptionRestartNote")}</p>
            <div className="flex justify-end gap-2">
              <Button variant="ghost" size="sm" onClick={() => setPhase("idle")}>
                {t("a11y.close")}
              </Button>
              <Button size="sm" disabled={busy || pass1.length === 0} onClick={submitEnable}>
                {busy ? t("settings.encryptionWorking") : t("settings.encryptionEnable")}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>

      <Dialog open={phase === "disable"} onOpenChange={(open) => !open && setPhase("idle")}>
        <DialogContent closeLabel={t("a11y.close")}>
          <DialogTitle>{t("settings.encryptionDisableTitle")}</DialogTitle>
          <DialogDescription>{t("settings.encryptionDisableHint")}</DialogDescription>
          <div className="mt-4 flex justify-end gap-2">
            <Button variant="ghost" size="sm" onClick={() => setPhase("idle")}>
              {t("a11y.close")}
            </Button>
            <Button
              variant="danger"
              size="sm"
              disabled={busy}
              onClick={() => {
                disable.mutate();
                setPhase("idle");
              }}
            >
              {busy ? t("settings.encryptionWorking") : t("settings.encryptionDisableConfirm")}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}
