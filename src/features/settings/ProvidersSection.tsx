import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, KeyRound, PlugZap, X } from "lucide-react";
import { Badge, Button, Switch, useToast } from "@/components/ui";
import { cn } from "@/lib/cn";
import {
  useAnilistConnect,
  useAnilistOauthEvent,
  useProvidersQuery,
  useSetProviderEnabled,
  useSetProviderKey,
  useTestConnection,
  type ProviderSettingsRow,
} from "./providers";

/* MISSION-063 / MISSION-130 — Provider settings, redesigned as self-contained
   cards. One card per provider: identity + live state chips on the header, a
   proper Switch on the trailing edge, then the credential controls (uniform
   control height, grouped) and a test probe with its outcome as a chip.
   Contracts kept from MISSION-063/098/130: role=switch labels ("Enable/
   Disable {{name}}"), key field label ("{{name}} API key"), "Save key"
   button, inline "Key saved" indicator — all covered by tests. */

function StateChip({ enabled }: { enabled: boolean }) {
  const { t } = useTranslation();
  return (
    <Badge variant={enabled ? "inprogress" : "neutral"}>
      {t(enabled ? "settings.providersStateOn" : "settings.providersStateOff")}
    </Badge>
  );
}

function ProviderRow({
  row,
  connect,
  connecting,
}: {
  row: ProviderSettingsRow;
  connect?: ReturnType<typeof useAnilistConnect>;
  connecting?: boolean;
}) {
  const { t } = useTranslation();
  const toast = useToast();
  const toggle = useSetProviderEnabled();
  const setKey = useSetProviderKey();
  const test = useTestConnection();
  const [keyValue, setKeyValue] = useState("");
  const [keyError, setKeyError] = useState(false);

  const hasKeyInput = keyValue.trim().length > 0;
  const keyBusy = setKey.isPending;
  const testResult = test.data;

  const handleSaveKey = () => {
    if (!hasKeyInput) return;
    setKeyValue("");
    setKey.mutate(
      { provider: row.provider, apiKey: keyValue },
      {
        onSuccess: () => {
          setKeyError(false);
        },
        onError: (error) => {
          setKeyError(true);
          toast.error({
            title: t("settings.providersKeySaveFailed"),
            description: error instanceof Error ? error.message : String(error),
          });
        },
      },
    );
  };

  return (
    <li
      className={cn(
        "flex flex-col gap-3 rounded-md border bg-bg-surface p-4",
        row.enabled ? "border-border-subtle" : "border-border-subtle opacity-80",
      )}
    >
      {/* Identity + state */}
      <div className="flex items-center justify-between gap-4">
        <div className="flex min-w-0 flex-wrap items-center gap-2">
          <p className="text-sm font-semibold text-text-primary">{row.name}</p>
          <StateChip enabled={row.enabled} />
          {row.requires_key ? (
            <Badge variant="neutral">
              <KeyRound size={11} aria-hidden="true" className="-ms-0.5" />
              {t("settings.providersKeyRequired")}
            </Badge>
          ) : null}
        </div>
        <Switch
          checked={row.enabled}
          disabled={toggle.isPending}
          aria-label={t(row.enabled ? "settings.providersDisable" : "settings.providersEnable", {
            name: row.name,
          })}
          onCheckedChange={(enabled) => toggle.mutate({ provider: row.provider, enabled })}
        />
      </div>

      {/* Credentials */}
      {row.requires_key ? (
        <div className="flex flex-wrap items-center gap-2">
          {row.provider === "anilist" ? (
            <Button
              variant={row.has_key ? "secondary" : "primary"}
              size="sm"
              disabled={connecting}
              onClick={() => connect?.mutate()}
            >
              <PlugZap size={14} aria-hidden="true" />
              {connecting
                ? t("settings.providersAnilistConnecting")
                : t("settings.providersAnilistConnect")}
            </Button>
          ) : null}
          <input
            type="password"
            autoComplete="off"
            spellCheck={false}
            value={keyValue}
            disabled={keyBusy}
            onChange={(event) => setKeyValue(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") handleSaveKey();
            }}
            placeholder={row.has_key ? "••••••••" : t("settings.providersKeyPlaceholder")}
            aria-label={t("settings.providersKeyField", { name: row.name })}
            className={cn(
              "h-[var(--control-height-compact)] min-w-48 flex-1 rounded-sm border bg-bg-base px-3 text-sm text-text-primary",
              "placeholder:text-text-tertiary transition-colors duration-150 ease-out",
              "hover:border-accent focus-visible:outline-none",
            )}
          />
          <Button
            variant="secondary"
            size="sm"
            disabled={!hasKeyInput || keyBusy}
            onClick={handleSaveKey}
          >
            {keyBusy ? t("settings.providersKeySaving") : t("settings.providersKeySave")}
          </Button>
          {keyError ? (
            <Badge variant="dropped">
              <X size={11} aria-hidden="true" className="-ms-0.5" />
              {t("settings.providersKeySaveFailed")}
            </Badge>
          ) : row.has_key ? (
            <Badge variant="completed">
              <Check size={11} aria-hidden="true" className="-ms-0.5" />
              {t("settings.providersKeySaved")}
            </Badge>
          ) : null}
        </div>
      ) : null}

      {/* Probe */}
      <div className="flex flex-wrap items-center gap-2">
        <Button
          variant="secondary"
          size="sm"
          disabled={test.isPending}
          aria-label={t("settings.providersTestAria", { name: row.name })}
          onClick={() => test.mutate({ provider: row.provider })}
        >
          <PlugZap size={14} aria-hidden="true" className="rtl:-scale-x-100" />
          {test.isPending
            ? t("settings.providersTesting")
            : t("settings.providersTest", { name: row.name })}
        </Button>
        {testResult ? (
          testResult.ok ? (
            <Badge variant="completed">
              {t("settings.providersTestOk", { count: testResult.results })}
            </Badge>
          ) : (
            <Badge variant="dropped">
              {t("settings.providersTestFailed", { message: testResult.message })}
            </Badge>
          )
        ) : null}
      </div>
    </li>
  );
}

export function ProvidersSection() {
  const { t } = useTranslation();
  const toast = useToast();
  const { data, isLoading, isError, refetch } = useProvidersQuery();
  const connect = useAnilistConnect();
  useAnilistOauthEvent((ok, message) => {
    if (ok) toast.success({ title: t("settings.providersOauthOk") });
    else
      toast.error({
        title: t("settings.providersOauthFailed", { message: message ?? "" }),
      });
  });

  if (isLoading) {
    return (
      <div
        className="flex flex-col gap-4"
        role="status"
        aria-label={t("settings.providersLoading")}
      >
        {[0, 1, 2].map((i) => (
          <div key={i} className="h-20 animate-pulse rounded-md bg-bg-raised" />
        ))}
      </div>
    );
  }

  if (isError || !data) {
    return (
      <div className="flex flex-col items-start gap-2">
        <p className="text-sm text-text-secondary">{t("settings.providersErrorHint")}</p>
        <Button variant="secondary" size="sm" onClick={() => void refetch()}>
          {t("settings.retry")}
        </Button>
      </div>
    );
  }

  if (data.length === 0) {
    return <p className="text-sm text-text-secondary">{t("settings.providersEmpty")}</p>;
  }

  return (
    <ul className="flex flex-col gap-3">
      {data.map((row) => (
        <ProviderRow
          key={row.provider}
          row={row}
          connect={row.provider === "anilist" ? connect : undefined}
          connecting={connect.isPending}
        />
      ))}
    </ul>
  );
}
