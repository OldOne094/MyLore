/* MISSION-117 — Reading groups list.

   Rows, not cards: a group is one line of quiet information (name + how many
   members), and the interesting work happens one level down on the group page.
   When the feature is off this page *is* the opt-in gate. */

import { useState } from "react";
import { Link, useNavigate } from "react-router";
import { useTranslation } from "react-i18next";
import { ChevronRight, Link2, Users } from "lucide-react";
import {
  Button,
  Dialog,
  DialogContent,
  DialogTitle,
  EmptyState,
  InputField,
  Skeleton,
  TextareaField,
  useToast,
} from "@/components/ui";
import {
  useAcceptInvite,
  useCreateGroup,
  useGroupPrefsQuery,
  useGroupsQuery,
  useSetGroupPrefs,
} from "@/features/groups/api";
import { PrivacyNotice } from "@/features/groups/PrivacyNotice";

function GroupsSkeleton() {
  return (
    <div role="status" aria-label="Loading groups" className="px-6 py-5">
      {Array.from({ length: 3 }, (_, index) => (
        <div key={index} className="mb-2 flex items-center gap-3 rounded-md px-3 py-2">
          <Skeleton className="h-4 w-40" />
          <Skeleton className="h-3 w-20" />
        </div>
      ))}
    </div>
  );
}

export function GroupsPage() {
  const { t } = useTranslation();
  const toast = useToast();
  const navigate = useNavigate();

  const prefsQuery = useGroupPrefsQuery();
  const groupsQuery = useGroupsQuery();
  const setPrefs = useSetGroupPrefs();
  const createGroup = useCreateGroup();
  const acceptInvite = useAcceptInvite();

  const [creating, setCreating] = useState(false);
  const [joining, setJoining] = useState(false);
  const [name, setName] = useState("");
  const [link, setLink] = useState("");
  const [formError, setFormError] = useState<string | null>(null);

  if (prefsQuery.isLoading) return <GroupsSkeleton />;

  if (prefsQuery.isError) {
    return (
      <EmptyState
        icon={Users}
        title={t("groupsPage.errorTitle")}
        hint={t("groupsPage.errorHint")}
        action={
          <Button variant="secondary" onClick={() => void prefsQuery.refetch()}>
            {t("groupsPage.retry")}
          </Button>
        }
      />
    );
  }

  const prefs = prefsQuery.data;
  if (!prefs) return <GroupsSkeleton />;

  if (!prefs.enabled) {
    return (
      <PrivacyNotice
        defaultName={prefs.display_name}
        pending={setPrefs.isPending}
        error={formError}
        onEnable={(displayName) => {
          setFormError(null);
          setPrefs.mutate(
            { enabled: true, displayName },
            {
              onSuccess: () => toast.success({ title: t("groupsPage.enabledToast") }),
              onError: (error) => setFormError(String(error)),
            },
          );
        }}
      />
    );
  }

  const groups = groupsQuery.data ?? [];

  const onCreate = () => {
    setFormError(null);
    createGroup.mutate(name.trim(), {
      onSuccess: (group) => {
        setCreating(false);
        setName("");
        toast.success({ title: t("groupsPage.createToast", { name: group.name }) });
        void navigate(`/groups/${group.id}`);
      },
      onError: (error) => setFormError(String(error)),
    });
  };

  const onJoin = () => {
    setFormError(null);
    acceptInvite.mutate(link.trim(), {
      onSuccess: (groupId) => {
        setJoining(false);
        setLink("");
        toast.success({ title: t("groupsPage.joinToast") });
        void navigate(`/groups/${groupId}`);
      },
      onError: (error) => setFormError(String(error)),
    });
  };

  return (
    <section aria-label={t("groupsPage.title")} className="px-6 py-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-lg font-semibold text-text-primary">{t("groupsPage.title")}</h1>
        <div className="flex items-center gap-2">
          <Button
            variant="secondary"
            onClick={() => {
              setFormError(null);
              setJoining(true);
            }}
          >
            <Link2 size={16} aria-hidden />
            {t("groupsPage.joinGroup")}
          </Button>
          <Button
            onClick={() => {
              setFormError(null);
              setCreating(true);
            }}
          >
            {t("groupsPage.newGroup")}
          </Button>
        </div>
      </div>

      {groupsQuery.isLoading ? (
        <div role="status" aria-label="Loading groups" className="mt-4">
          {Array.from({ length: 3 }, (_, index) => (
            <div key={index} className="mb-2 flex items-center gap-3 px-3 py-2">
              <Skeleton className="h-4 w-40" />
            </div>
          ))}
        </div>
      ) : groupsQuery.isError ? (
        <EmptyState
          icon={Users}
          title={t("groupsPage.errorTitle")}
          hint={t("groupsPage.errorHint")}
          action={
            <Button variant="secondary" onClick={() => void groupsQuery.refetch()}>
              {t("groupsPage.retry")}
            </Button>
          }
        />
      ) : groups.length === 0 ? (
        <EmptyState
          icon={Users}
          title={t("groupsPage.emptyTitle")}
          hint={t("groupsPage.emptyHint")}
          action={<Button onClick={() => setCreating(true)}>{t("groupsPage.newGroup")}</Button>}
        />
      ) : (
        <ul className="mt-4 flex flex-col gap-1">
          {groups.map((group) => (
            <li key={group.id}>
              <Link
                to={`/groups/${group.id}`}
                className="flex items-center justify-between gap-3 rounded-md border border-transparent bg-bg-surface px-3 py-2 transition-colors duration-150 ease-out hover:border-border-subtle hover:bg-bg-hover"
              >
                <span className="flex min-w-0 flex-col">
                  <span className="truncate text-sm font-medium text-text-primary">
                    {group.name}
                  </span>
                  <span className="text-xs text-text-tertiary">
                    {t("groupsPage.memberCount", { count: group.members.length })}
                  </span>
                </span>
                <ChevronRight size={16} aria-hidden className="shrink-0 text-text-tertiary" />
              </Link>
            </li>
          ))}
        </ul>
      )}

      <Dialog open={creating} onOpenChange={setCreating}>
        <DialogContent>
          <DialogTitle>{t("groupsPage.newGroup")}</DialogTitle>
          <form
            className="mt-4 flex flex-col gap-4"
            onSubmit={(event) => {
              event.preventDefault();
              onCreate();
            }}
          >
            <InputField
              label={t("groupsPage.groupNameLabel")}
              value={name}
              autoFocus
              maxLength={120}
              placeholder={t("groupsPage.groupNamePlaceholder")}
              onChange={(event) => setName(event.target.value)}
            />
            {formError ? <p className="text-sm text-danger">{formError}</p> : null}
            <div className="flex justify-end gap-2">
              <Button variant="secondary" type="button" onClick={() => setCreating(false)}>
                {t("groupsPage.cancel")}
              </Button>
              <Button type="submit" disabled={name.trim().length === 0 || createGroup.isPending}>
                {createGroup.isPending ? t("groupsPage.creating") : t("groupsPage.create")}
              </Button>
            </div>
          </form>
        </DialogContent>
      </Dialog>

      <Dialog open={joining} onOpenChange={setJoining}>
        <DialogContent>
          <DialogTitle>{t("groupsPage.joinTitle")}</DialogTitle>
          <form
            className="mt-4 flex flex-col gap-4"
            onSubmit={(event) => {
              event.preventDefault();
              onJoin();
            }}
          >
            <TextareaField
              label={t("groupsPage.joinLinkLabel")}
              value={link}
              rows={3}
              placeholder="mylore://group-invite#…"
              onChange={(event) => setLink(event.target.value)}
            />
            <p className="text-sm text-text-tertiary">{t("groupsPage.joinHint")}</p>
            {formError ? <p className="text-sm text-danger">{formError}</p> : null}
            <div className="flex justify-end gap-2">
              <Button variant="secondary" type="button" onClick={() => setJoining(false)}>
                {t("groupsPage.cancel")}
              </Button>
              <Button type="submit" disabled={link.trim().length === 0 || acceptInvite.isPending}>
                {acceptInvite.isPending ? t("groupsPage.joining") : t("groupsPage.join")}
              </Button>
            </div>
          </form>
        </DialogContent>
      </Dialog>
    </section>
  );
}
