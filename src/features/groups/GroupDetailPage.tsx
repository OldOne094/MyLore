/* MISSION-117 — One group.

   The page is an alignment matrix (works × members), because the point of a
   reading group is where everyone is in the *same* work; a per-member list would
   hide exactly that. Selecting a row opens its thread beside the matrix, and the
   thread is progress-gated: a member's note stays out of the DOM while they are
   further along than you are. */

import { useState } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { useTranslation } from "react-i18next";
import { ChevronLeft, RefreshCw, Users } from "lucide-react";
import {
  Badge,
  Button,
  Dialog,
  DialogContent,
  DialogTitle,
  EmptyState,
  Skeleton,
  TextareaField,
  useToast,
} from "@/components/ui";
import { cn } from "@/lib/cn";
import type { GroupInviteView } from "@/api";
import {
  isP2pBuild,
  syncReportOf,
  useComposeThreadNote,
  useCreateInvite,
  useDeleteGroup,
  useGroupKeyStatusQuery,
  useGroupPrefsQuery,
  useGroupQuery,
  useGroupRelaysQuery,
  useGroupShelfQuery,
  useGroupSyncTask,
  useSyncGroup,
  useThreadQuery,
} from "@/features/groups/api";
import {
  buildAlignment,
  memberName,
  maySpoil,
  progressOf,
  type AlignmentWork,
} from "@/features/groups/alignment";
import { AddWorkDialog } from "@/features/groups/AddWorkDialog";
import { GroupMembers } from "@/features/groups/GroupMembers";
import { GroupSharing } from "@/features/groups/GroupSharing";
import { InviteDialog } from "@/features/groups/InviteDialog";
import { SpoileredNote } from "@/features/groups/SpoileredNote";

function GroupSkeleton() {
  return (
    <div role="status" aria-label="Loading group" className="px-6 py-5">
      <Skeleton className="h-5 w-48" />
      <Skeleton className="mt-3 h-3 w-32" />
      <Skeleton className="mt-6 h-40 w-full" />
    </div>
  );
}

export function GroupDetailPage() {
  const { groupId = "" } = useParams<{ groupId: string }>();
  const { t } = useTranslation();
  const toast = useToast();
  const navigate = useNavigate();

  const groupQuery = useGroupQuery(groupId);
  const prefsQuery = useGroupPrefsQuery();
  const shelfQuery = useGroupShelfQuery(groupId, groupId.length > 0);
  const keyStatus = useGroupKeyStatusQuery(groupId);
  const relaysQuery = useGroupRelaysQuery(groupId, isP2pBuild(keyStatus));
  const sync = useSyncGroup();
  const deleteGroup = useDeleteGroup();
  const createInvite = useCreateInvite();

  const [taskId, setTaskId] = useState<string | null>(null);
  const task = useGroupSyncTask(taskId, groupId);

  const [selectedWork, setSelectedWork] = useState<string | null>(null);
  const threadQuery = useThreadQuery(groupId, selectedWork ?? "", selectedWork !== null);
  const compose = useComposeThreadNote(groupId, selectedWork ?? "");

  const [body, setBody] = useState("");
  const [inviteOpen, setInviteOpen] = useState(false);
  const [invite, setInvite] = useState<GroupInviteView | null>(null);
  const [inviteError, setInviteError] = useState<string | null>(null);
  const [addWorkOpen, setAddWorkOpen] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);

  if (groupQuery.isLoading || prefsQuery.isLoading) return <GroupSkeleton />;

  if (groupQuery.isError) {
    return (
      <EmptyState
        icon={Users}
        title={t("groupsPage.errorTitle")}
        hint={t("groupsPage.errorHint")}
        action={
          <Button variant="secondary" onClick={() => void groupQuery.refetch()}>
            {t("groupsPage.retry")}
          </Button>
        }
      />
    );
  }

  const group = groupQuery.data;
  if (!group) return <GroupSkeleton />;

  const me = prefsQuery.data?.member_id ?? "";
  const p2p = isP2pBuild(keyStatus);
  const isOwner = group.owner_id === me;
  const relays = relaysQuery.data?.relays ?? [];
  const works = buildAlignment(shelfQuery.data ?? [], group.members);
  const selected = works.find((work) => work.work_key === selectedWork) ?? null;
  const notes = threadQuery.data?.notes ?? [];
  const synced = threadQuery.data?.synced ?? false;
  const report = syncReportOf(task.data);
  const running = sync.isPending || task.data?.state === "running" || task.data?.state === "queued";

  const onPost = () => {
    const text = body.trim();
    if (text.length === 0 || !selectedWork) return;
    compose.mutate(
      { authorId: me, body: text, synced },
      {
        onSuccess: () => setBody(""),
        onError: () => toast.error({ title: t("groupsPage.postErrorToast") }),
      },
    );
  };

  const onInvite = () => {
    setInvite(null);
    setInviteError(null);
    setInviteOpen(true);
    createInvite.mutate(
      { groupId, relays },
      { onSuccess: setInvite, onError: (cause) => setInviteError(String(cause)) },
    );
  };

  return (
    <section aria-label={group.name} className="px-6 py-5">
      <Link
        to="/groups"
        className="inline-flex items-center gap-1 text-sm text-text-secondary hover:text-text-primary"
      >
        <ChevronLeft size={16} aria-hidden />
        {t("groupsPage.backToGroups")}
      </Link>

      <div className="mt-3 flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <h1 className="truncate text-lg font-semibold text-text-primary">{group.name}</h1>
            {p2p ? (
              <Badge variant={keyStatus.data?.has_key ? "accent" : "neutral"}>
                {keyStatus.data?.has_key ? t("groupsPage.badgeSealed") : t("groupsPage.badgeNoKey")}
              </Badge>
            ) : (
              <Badge variant="neutral">{t("groupsPage.badgeLocalOnly")}</Badge>
            )}
            {p2p && relays.length > 0 ? (
              <Badge variant="neutral">
                {t("groupsPage.relayCount", { count: relays.length })}
              </Badge>
            ) : null}
          </div>
          <p className="mt-1 text-sm text-text-secondary">
            {t("groupsPage.memberCount", { count: group.members.length })} ·{" "}
            {t("groupsPage.workCount", { count: works.length })}
          </p>
          {running ? (
            <p className="mt-1 text-xs text-text-tertiary">
              {task.data?.message ?? t("groupsPage.syncing")}
              {typeof task.data?.progress === "number" ? ` · ${task.data.progress}%` : ""}
            </p>
          ) : report ? (
            <p className="mt-1 text-xs text-text-tertiary">
              {t("groupsPage.syncReport", {
                published: report.published,
                merged: report.merged,
                skipped: report.skipped,
                pending: report.pending,
              })}
            </p>
          ) : null}
        </div>

        <div className="flex flex-wrap items-center gap-2">
          {p2p ? (
            <>
              <Button variant="secondary" onClick={onInvite}>
                {t("groupsPage.invite")}
              </Button>
              <Button
                disabled={running}
                onClick={() =>
                  sync.mutate(groupId, {
                    onSuccess: (snapshot) => setTaskId(snapshot.id),
                    onError: () => toast.error({ title: t("groupsPage.syncErrorToast") }),
                  })
                }
              >
                <RefreshCw size={16} aria-hidden className={cn(running && "animate-spin")} />
                {running ? t("groupsPage.syncing") : t("groupsPage.syncNow")}
              </Button>
            </>
          ) : null}
        </div>
      </div>

      <div className="mt-5 grid items-start gap-4 lg:grid-cols-[minmax(0,1fr)_24rem]">
        <div className="flex flex-col gap-4">
          <section
            aria-label={t("groupsPage.shelfHeading")}
            className="rounded-md border border-border-subtle bg-bg-surface p-4"
          >
            <div className="flex flex-wrap items-center justify-between gap-2">
              <h2 className="text-sm font-semibold text-text-primary">
                {t("groupsPage.shelfHeading")}
              </h2>
              <Button size="sm" variant="secondary" onClick={() => setAddWorkOpen(true)}>
                {t("groupsPage.addWork")}
              </Button>
            </div>

            {shelfQuery.isLoading ? (
              <div role="status" aria-label={t("groupsPage.shelfHeading")} className="mt-3">
                <Skeleton className="h-4 w-full" />
                <Skeleton className="mt-2 h-4 w-2/3" />
              </div>
            ) : shelfQuery.isError ? (
              <p className="mt-3 text-sm text-text-secondary">
                {t("groupsPage.shelfError")}{" "}
                <button
                  type="button"
                  className="text-accent underline-offset-2 hover:underline"
                  onClick={() => void shelfQuery.refetch()}
                >
                  {t("groupsPage.retry")}
                </button>
              </p>
            ) : works.length === 0 ? (
              <p className="mt-3 text-sm text-text-tertiary">{t("groupsPage.shelfEmpty")}</p>
            ) : (
              <div className="mt-3 overflow-x-auto">
                <table className="w-full border-collapse">
                  <caption className="sr-only">{t("groupsPage.shelfCaption")}</caption>
                  <thead>
                    <tr className="text-xs text-text-tertiary">
                      <th scope="col" className="pb-2 text-start font-medium">
                        {t("groupsPage.workColumn")}
                      </th>
                      {group.members.map((member) => (
                        <th
                          key={member.member_id}
                          scope="col"
                          className="pb-2 text-start font-medium"
                        >
                          {member.display_name}
                        </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {works.map((work) => (
                      <ShelfRow
                        key={work.work_key}
                        work={work}
                        members={group.members.map((member) => member.member_id)}
                        selected={work.work_key === selectedWork}
                        onSelect={() => setSelectedWork(work.work_key)}
                      />
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </section>

          <GroupMembers
            groupId={groupId}
            members={group.members}
            myMemberId={me}
            myName={prefsQuery.data?.display_name ?? ""}
          />

          <GroupSharing groupId={groupId} groupName={group.name} p2p={p2p} isOwner={isOwner} />
        </div>

        <section
          aria-label={t("groupsPage.threadHeading")}
          className="rounded-md border border-border-subtle bg-bg-surface p-4 lg:sticky lg:top-0"
        >
          <h2 className="truncate text-sm font-semibold text-text-primary">
            {selected ? selected.title : t("groupsPage.threadHeading")}
          </h2>

          {!selected ? (
            <p className="mt-2 text-sm text-text-tertiary">{t("groupsPage.threadPickWork")}</p>
          ) : threadQuery.isLoading ? (
            <div role="status" aria-label={t("groupsPage.threadHeading")} className="mt-3">
              <Skeleton className="h-4 w-3/4" />
              <Skeleton className="mt-2 h-4 w-1/2" />
            </div>
          ) : threadQuery.isError ? (
            <p className="mt-3 text-sm text-text-secondary">
              {t("groupsPage.threadError")}{" "}
              <button
                type="button"
                className="text-accent underline-offset-2 hover:underline"
                onClick={() => void threadQuery.refetch()}
              >
                {t("groupsPage.retry")}
              </button>
            </p>
          ) : notes.length === 0 ? (
            <p className="mt-2 text-sm text-text-tertiary">{t("groupsPage.threadEmpty")}</p>
          ) : (
            <ul className="mt-3 flex max-h-[60vh] flex-col gap-3 overflow-y-auto">
              {notes.map((note) => {
                const author = memberName(group.members, note.author_id);
                return (
                  <SpoileredNote
                    key={note.id}
                    note={note}
                    authorName={author}
                    authorProgress={selected ? progressOf(selected, note.author_id) : 0}
                    gated={selected ? maySpoil(selected, note.author_id, me) : false}
                  />
                );
              })}
            </ul>
          )}

          {selected ? (
            <form
              className="mt-4 flex flex-col gap-2 border-t border-border-subtle pt-4"
              onSubmit={(event) => {
                event.preventDefault();
                onPost();
              }}
            >
              <TextareaField
                label={t("groupsPage.noteLabel")}
                value={body}
                rows={3}
                onChange={(event) => setBody(event.target.value)}
              />
              <div className="flex items-center justify-between gap-2">
                <span className="text-xs text-text-tertiary">
                  {synced ? t("groupsPage.notesSyncedHint") : t("groupsPage.notesLocalHint")}
                </span>
                <Button
                  type="submit"
                  size="sm"
                  disabled={body.trim().length === 0 || compose.isPending}
                >
                  {t("groupsPage.post")}
                </Button>
              </div>
            </form>
          ) : null}
        </section>
      </div>

      <div className="mt-6 flex flex-wrap items-center justify-between gap-3 border-t border-border-subtle pt-4">
        <p className="text-sm text-text-tertiary">{t("groupsPage.deleteHint")}</p>
        <Button variant="ghost" onClick={() => setConfirmDelete(true)}>
          {t("groupsPage.deleteGroup")}
        </Button>
      </div>

      <InviteDialog
        open={inviteOpen}
        onOpenChange={setInviteOpen}
        invite={invite}
        error={inviteError}
      />
      <AddWorkDialog
        groupId={groupId}
        memberId={me}
        open={addWorkOpen}
        onOpenChange={setAddWorkOpen}
      />

      <Dialog open={confirmDelete} onOpenChange={setConfirmDelete}>
        <DialogContent>
          <DialogTitle>{t("groupsPage.deleteTitle")}</DialogTitle>
          <p className="mt-2 text-sm text-text-secondary">{t("groupsPage.deleteBody")}</p>
          <div className="mt-6 flex justify-end gap-2">
            <Button variant="secondary" onClick={() => setConfirmDelete(false)}>
              {t("groupsPage.cancel")}
            </Button>
            <Button
              variant="danger"
              disabled={deleteGroup.isPending}
              onClick={() =>
                deleteGroup.mutate(groupId, {
                  onSuccess: () => {
                    toast.success({ title: t("groupsPage.deleteToast") });
                    void navigate("/groups");
                  },
                  onError: () => toast.error({ title: t("groupsPage.deleteErrorToast") }),
                })
              }
            >
              {t("groupsPage.confirmDelete")}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </section>
  );
}

interface ShelfRowProps {
  work: AlignmentWork;
  members: string[];
  selected: boolean;
  onSelect: () => void;
}

function ShelfRow({ work, members, selected, onSelect }: ShelfRowProps) {
  const { t } = useTranslation();
  return (
    <tr className="border-t border-border-subtle align-top">
      <th scope="row" className="py-2 pe-3 text-start font-normal">
        <button
          type="button"
          aria-current={selected ? "true" : undefined}
          className={cn(
            "text-start text-sm transition-colors duration-150 ease-out",
            selected ? "font-medium text-accent" : "text-text-primary hover:text-accent",
          )}
          onClick={onSelect}
        >
          {work.title}
        </button>
      </th>
      {members.map((memberId) => {
        const entry = work.by_member[memberId];
        return (
          <td key={memberId} className="py-2 pe-3">
            {entry ? (
              <span className="flex flex-col">
                <span className="text-sm tabular-nums text-text-primary">{entry.progress}</span>
                <span className="text-2xs text-text-tertiary">
                  {t(`coreStatus.${entry.status}`)}
                </span>
              </span>
            ) : (
              <span className="text-sm text-text-tertiary" aria-label={t("groupsPage.notShelved")}>
                —
              </span>
            )}
          </td>
        );
      })}
    </tr>
  );
}
