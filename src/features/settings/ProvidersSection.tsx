import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Check } from "lucide-react";
import { Button } from "@/components/ui";
import { cn } from "@/lib/cn";
import {
  useProvidersQuery,
  useSetProviderEnabled,
  useSetProviderKey,
  useTestConnection,
  type ProviderSettingsRow,
} from "./providers";

/* MISSION-063 — Provider settings. One row per registered provider: an
   enable/disable switch, an API-key field for key-required providers (keys are
   stored in the OS keyring by the backend and never returned) and a live
   "test connection" probe. */

function ProviderRow({ row }: { row: ProviderSettingsRow }) {
  const { t } = useTranslation();
  const toggle = useSetProviderEnabled();
  const setKey = useSetProviderKey();
  const test = useTestConnection();
  const [keyValue, setKeyValue] = useState("");

  const hasKeyInput = keyValue.trim().length > 0;
  const keyBusy = setKey.isPending;
  const testResult = test.data;

  return (
    <li className="flex flex-col gap-3 border-t border-border-subtle py-4 first:border-t-0">
      <div className="flex items-center justify-between gap-4">
        <div className="min-w-0">
          <p className="text-sm font-medium text-text-primary">{row.name}</p>
          {row.requires_key ? (
            <p className="mt-0.5 text-xs text-text-tertiary">
              {t("settings.providersKeyRequired")}
            </p>
          ) : null}
        </div>
        <button
          type="button"
          role="switch"
          aria-checked={row.enabled}
          aria-label={t(row.enabled ? "settings.providersDisable" : "settings.providersEnable", {
            name: row.name,
          })}
          disabled={toggle.isPending}
          onClick={() => toggle.mutate({ provider: row.provider, enabled: !row.enabled })}
          className={cn(
            "relative inline-flex h-5 w-9 shrink-0 items-center rounded-full border transition-colors duration-150 ease-out",
            "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent",
            "disabled:pointer-events-none disabled:opacity-50",
            row.enabled ? "border-accent bg-accent" : "border-border-strong bg-bg-raised",
          )}
        >
          <span
            aria-hidden="true"
            className={cn(
              "inline-block h-3.5 w-3.5 rounded-full bg-bg-surface transition-transform duration-150 ease-out",
              row.enabled ? "translate-x-[20px]" : "translate-x-0.5",
            )}
          />
        </button>
      </div>

      {row.requires_key ? (
        <div className="flex items-center gap-2">
          <input
            type="password"
            autoComplete="off"
            spellCheck={false}
            value={keyValue}
            disabled={keyBusy}
            onChange={(event) => setKeyValue(event.target.value)}
            placeholder={row.has_key ? "••••••••" : t("settings.providersKeyPlaceholder")}
            aria-label={t("settings.providersKeyField", { name: row.name })}
            className={cn(
              "w-52 rounded-sm border bg-bg-base px-3 py-1.5 text-sm text-text-primary",
              "placeholder:text-text-tertiary transition-colors duration-150 ease-out",
              "hover:border-accent focus-visible:outline-none",
            )}
          />
          <Button
            variant="secondary"
            size="sm"
            disabled={!hasKeyInput || keyBusy}
            onClick={() => {
              setKey.mutate({ provider: row.provider, apiKey: keyValue });
              setKeyValue("");
            }}
          >
            {keyBusy ? t("settings.providersKeySaving") : t("settings.providersKeySave")}
          </Button>
          {row.has_key ? (
            <span className="inline-flex items-center gap-1 text-xs text-status-completed">
              <Check size={12} aria-hidden="true" />
              {t("settings.providersKeySaved")}
            </span>
          ) : null}
        </div>
      ) : null}

      <div className="flex items-center gap-3">
        <Button
          variant="ghost"
          size="sm"
          disabled={test.isPending}
          aria-label={t("settings.providersTestAria", { name: row.name })}
          onClick={() => test.mutate({ provider: row.provider })}
        >
          {test.isPending
            ? t("settings.providersTesting")
            : t("settings.providersTest", { name: row.name })}
        </Button>
        {testResult ? (
          testResult.ok ? (
            <span className="text-xs text-status-completed">
              {t("settings.providersTestOk", { count: testResult.results })}
            </span>
          ) : (
            <span className="text-xs text-danger">
              {t("settings.providersTestFailed", { message: testResult.message })}
            </span>
          )
        ) : null}
      </div>
    </li>
  );
}

export function ProvidersSection() {
  const { t } = useTranslation();
  const { data, isLoading, isError, refetch } = useProvidersQuery();

  if (isLoading) {
    return (
      <div
        className="flex flex-col gap-4"
        role="status"
        aria-label={t("settings.providersLoading")}
      >
        {[0, 1, 2].map((i) => (
          <div key={i} className="h-12 animate-pulse rounded-sm bg-bg-raised" />
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
    <ul className="divide-y-0">
      {data.map((row) => (
        <ProviderRow key={row.provider} row={row} />
      ))}
    </ul>
  );
}
