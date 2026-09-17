/* MISSION-117 — The opt-in gate for reading groups.

   Deliberately a decision surface, not a settings toggle: it names exactly what
   leaves the device and what never does, in the words the code actually
   guarantees. The metadata line is specific because it is the part that is easy
   to get wrong — relays read the event tags in the clear, so they learn which
   group is discussing which work. */

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Users } from "lucide-react";
import { Button, InputField } from "@/components/ui";

export interface PrivacyNoticeProps {
  defaultName: string;
  pending: boolean;
  error: string | null;
  onEnable: (displayName: string) => void;
}

export function PrivacyNotice({ defaultName, pending, error, onEnable }: PrivacyNoticeProps) {
  const { t } = useTranslation();
  const [name, setName] = useState(defaultName);

  return (
    <section aria-label={t("groupsPage.title")} className="px-6 py-5">
      <div className="max-w-3xl">
        <h1 className="flex items-center gap-2 text-lg font-semibold text-text-primary">
          <Users size={20} aria-hidden className="text-text-secondary" />
          {t("groupsPage.title")}
        </h1>
        <p className="mt-1 text-sm text-text-secondary">{t("groupsPage.intro")}</p>

        <div className="mt-5 rounded-md border border-border-subtle bg-bg-surface p-4">
          <h2 className="text-sm font-semibold text-text-primary">
            {t("groupsPage.privacyHeading")}
          </h2>
          <dl className="mt-3 flex flex-col gap-3">
            <div>
              <dt className="text-sm font-medium text-text-primary">
                {t("groupsPage.privacySealedLabel")}
              </dt>
              <dd className="mt-0.5 text-sm text-text-secondary">
                {t("groupsPage.privacySealedValue")}
              </dd>
            </div>
            <div>
              <dt className="text-sm font-medium text-text-primary">
                {t("groupsPage.privacyMetadataLabel")}
              </dt>
              <dd className="mt-0.5 text-sm text-text-secondary">
                {t("groupsPage.privacyMetadataValue")}
              </dd>
            </div>
            <div>
              <dt className="text-sm font-medium text-text-primary">
                {t("groupsPage.privacyStaysLabel")}
              </dt>
              <dd className="mt-0.5 text-sm text-text-secondary">
                {t("groupsPage.privacyStaysValue")}
              </dd>
            </div>
          </dl>
          <p className="mt-4 text-sm text-text-tertiary">{t("groupsPage.privacyOffHint")}</p>
        </div>

        <form
          className="mt-5 flex flex-wrap items-end gap-3"
          onSubmit={(event) => {
            event.preventDefault();
            onEnable(name.trim());
          }}
        >
          <div className="w-64">
            <InputField
              label={t("groupsPage.displayNameLabel")}
              value={name}
              placeholder={t("groupsPage.displayNamePlaceholder")}
              maxLength={60}
              onChange={(event) => setName(event.target.value)}
            />
          </div>
          <Button type="submit" disabled={pending}>
            {pending ? t("groupsPage.enabling") : t("groupsPage.enable")}
          </Button>
        </form>
        {error ? <p className="mt-2 text-sm text-danger">{error}</p> : null}
      </div>
    </section>
  );
}
