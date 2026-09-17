/* MISSION-117 — Copy-to-clipboard for the two values a group shares out of band
   (the invite link and your member id). */

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, Copy } from "lucide-react";
import { Button } from "@/components/ui";

export interface CopyButtonProps {
  value: string;
  label?: string;
}

export function CopyButton({ value, label }: CopyButtonProps) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);

  return (
    <Button
      size="sm"
      variant="secondary"
      onClick={() => {
        void navigator.clipboard?.writeText(value).catch(() => undefined);
        setCopied(true);
      }}
    >
      {copied ? <Check size={14} aria-hidden /> : <Copy size={14} aria-hidden />}
      {label ?? (copied ? t("groupsPage.copied") : t("groupsPage.copy"))}
    </Button>
  );
}
