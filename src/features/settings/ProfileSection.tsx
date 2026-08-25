import { useState } from "react";
import { useTranslation } from "react-i18next";
import { AVATAR_COLORS } from "@/profile/profile";
import { Avatar } from "@/profile/Avatar";
import { useProfile } from "@/profile/ProfileContext";
import { Button, InputField } from "@/components/ui";
import { cn } from "@/lib/cn";

/* MISSION-132 — Profile editor: display name + avatar color picker.
   Stored client-side; purely cosmetic. */

export function ProfileSection() {
  const { t } = useTranslation();
  const { profile, setProfile } = useProfile();
  const [name, setName] = useState(profile.displayName);

  const handleSave = () => {
    setProfile({ displayName: name.trim() });
  };

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center gap-4">
        <Avatar displayName={name || profile.displayName} color={profile.avatarColor} size={56} />
        <div className="flex flex-col gap-1.5 flex-1">
          <InputField
            label={t("settings.profileNameLabel")}
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={t("settings.profileNamePlaceholder")}
            maxLength={40}
          />
          {name.trim() !== profile.displayName ? (
            <Button variant="secondary" size="sm" onClick={handleSave} className="self-start">
              {t("settings.profileSave")}
            </Button>
          ) : null}
        </div>
      </div>

      <div className="flex flex-col gap-2">
        <p className="text-sm font-medium text-text-secondary">{t("settings.profileColorLabel")}</p>
        <div className="flex items-center gap-2">
          {AVATAR_COLORS.map((color) => (
            <button
              key={color}
              type="button"
              aria-label={color}
              aria-pressed={profile.avatarColor === color}
              onClick={() => setProfile({ avatarColor: color })}
              style={{ backgroundColor: color }}
              className={cn(
                "size-8 rounded-full border transition-transform duration-150 ease-out hover:scale-110",
                profile.avatarColor === color
                  ? "border-transparent ring-2 ring-accent ring-offset-2 ring-offset-bg-surface"
                  : "border-border-strong",
              )}
            />
          ))}
        </div>
      </div>
    </div>
  );
}
