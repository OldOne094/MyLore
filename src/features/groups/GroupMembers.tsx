/* MISSION-117 — Group members, and the local-only truth about them.

   The roster lives on this device: an invite shares the group key, not the
   member list, so members are added by id. The panel therefore shows your own
   member id — the one value a friend needs to add you — rather than implying the
   group already knows about everyone. */

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { UserMinus } from "lucide-react";
import { Button, Dialog, DialogContent, DialogTitle, InputField, useToast } from "@/components/ui";
import type { GroupMemberView } from "@/api";
import { useAddGroupMember, useRemoveGroupMember } from "@/features/groups/api";
import { CopyButton } from "@/features/groups/CopyButton";

export interface GroupMembersProps {
  groupId: string;
  members: GroupMemberView[];
  myMemberId: string;
  myName: string;
}

/** The owner of the group on this device. */
function ownerIdOf(members: GroupMemberView[]): string {
  return members.find((member) => member.role === "owner")?.member_id ?? "";
}

export function GroupMembers({ groupId, members, myMemberId, myName }: GroupMembersProps) {
  const { t } = useTranslation();
  const toast = useToast();
  const addMember = useAddGroupMember(groupId);
  const removeMember = useRemoveGroupMember(groupId);

  const [adding, setAdding] = useState(false);
  const [memberId, setMemberId] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [removing, setRemoving] = useState<GroupMemberView | null>(null);

  const ownerId = ownerIdOf(members);

  return (
    <section
      aria-label={t("groupsPage.membersHeading")}
      className="rounded-md border border-border-subtle bg-bg-surface p-4"
    >
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h2 className="text-sm font-semibold text-text-primary">
          {t("groupsPage.membersHeading")}
        </h2>
        <Button size="sm" variant="secondary" onClick={() => setAdding(true)}>
          {t("groupsPage.addMember")}
        </Button>
      </div>

      <div className="mt-3 flex flex-wrap items-center gap-2 text-xs text-text-tertiary">
        <span>
          {t("groupsPage.yourIdentity", { name: myName || t("groupsPage.unknownAuthor") })}
        </span>
        <span className="font-mono break-all text-text-secondary">{myMemberId}</span>
        <CopyButton value={myMemberId} />
      </div>

      <ul className="mt-3 flex flex-col">
        {members.map((member) => (
          <li
            key={member.member_id}
            className="flex items-center justify-between gap-3 border-t border-border-subtle py-2 first:border-t-0"
          >
            <span className="flex min-w-0 flex-col">
              <span className="truncate text-sm text-text-primary">
                {member.display_name}
                {member.member_id === myMemberId ? (
                  <span className="ms-2 text-xs text-text-tertiary">{t("groupsPage.you")}</span>
                ) : null}
              </span>
              <span className="font-mono text-xs break-all text-text-tertiary">
                {member.member_id}
              </span>
            </span>
            {member.member_id === ownerId ? (
              <span className="text-xs text-text-tertiary">{t("groupsPage.roleOwner")}</span>
            ) : (
              <Button
                size="sm"
                variant="ghost"
                aria-label={t("groupsPage.removeMemberAria", { name: member.display_name })}
                onClick={() => setRemoving(member)}
              >
                <UserMinus size={14} aria-hidden />
              </Button>
            )}
          </li>
        ))}
      </ul>

      <p className="mt-3 text-xs text-text-tertiary">{t("groupsPage.membersLocalHint")}</p>

      <Dialog open={adding} onOpenChange={setAdding}>
        <DialogContent>
          <DialogTitle>{t("groupsPage.addMember")}</DialogTitle>
          <form
            className="mt-4 flex flex-col gap-4"
            onSubmit={(event) => {
              event.preventDefault();
              addMember.mutate(
                { memberId: memberId.trim(), displayName: displayName.trim(), role: "member" },
                {
                  onSuccess: () => {
                    setAdding(false);
                    setMemberId("");
                    setDisplayName("");
                    toast.success({ title: t("groupsPage.addMemberToast") });
                  },
                  onError: () => toast.error({ title: t("groupsPage.addMemberErrorToast") }),
                },
              );
            }}
          >
            <InputField
              label={t("groupsPage.memberIdLabel")}
              value={memberId}
              autoFocus
              placeholder="m-…"
              onChange={(event) => setMemberId(event.target.value)}
            />
            <InputField
              label={t("groupsPage.memberNameLabel")}
              value={displayName}
              onChange={(event) => setDisplayName(event.target.value)}
            />
            <div className="flex justify-end gap-2">
              <Button variant="secondary" type="button" onClick={() => setAdding(false)}>
                {t("groupsPage.cancel")}
              </Button>
              <Button
                type="submit"
                disabled={
                  memberId.trim().length === 0 ||
                  displayName.trim().length === 0 ||
                  addMember.isPending
                }
              >
                {t("groupsPage.add")}
              </Button>
            </div>
          </form>
        </DialogContent>
      </Dialog>

      <Dialog open={removing !== null} onOpenChange={(open) => !open && setRemoving(null)}>
        <DialogContent>
          <DialogTitle>
            {t("groupsPage.removeMemberTitle", { name: removing?.display_name ?? "" })}
          </DialogTitle>
          <p className="mt-2 text-sm text-text-secondary">{t("groupsPage.removeMemberBody")}</p>
          <div className="mt-6 flex justify-end gap-2">
            <Button variant="secondary" onClick={() => setRemoving(null)}>
              {t("groupsPage.cancel")}
            </Button>
            <Button
              variant="danger"
              disabled={removeMember.isPending}
              onClick={() => {
                if (!removing) return;
                removeMember.mutate(removing.member_id, {
                  onSuccess: () => {
                    setRemoving(null);
                    toast.success({ title: t("groupsPage.removeMemberToast") });
                  },
                  onError: () => toast.error({ title: t("groupsPage.removeMemberErrorToast") }),
                });
              }}
            >
              {t("groupsPage.removeMember")}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </section>
  );
}
