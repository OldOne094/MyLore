/* MISSION-117 — Reading-groups data layer.

   Two store shapes exist behind one UI. Group CRUD, members, shelves and relay
   settings are plain local tables that every build has. Notes have two homes:
   the conflict-free document (built only with the `p2p` feature, and the store
   that actually syncs) and the plain `group_note` table (every build). The
   thread hooks below prefer the document and fall back to the table when the
   build rejects the p2p commands, so the same controls work in both builds —
   and, crucially, the thread a user reads is the same data a sync moves.

   Note ids carry their author and time (see `alignment.ts`), because the
   document only stores id → body. */

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { save } from "@tauri-apps/plugin-dialog";
import { asList } from "@/lib/asList";
import {
  reading_group_add_member,
  reading_group_add_note,
  reading_group_create,
  reading_group_delete,
  reading_group_export,
  reading_group_import,
  reading_group_invite_accept,
  reading_group_invite_create,
  reading_group_key_rotate,
  reading_group_key_status,
  reading_group_list,
  reading_group_note_edit,
  reading_group_note_state,
  reading_group_notes,
  reading_group_prefs_get,
  reading_group_prefs_set,
  reading_group_relays_get,
  reading_group_relays_set,
  reading_group_remove_member,
  reading_group_rename,
  reading_group_set_shelf,
  reading_group_shelf,
  reading_group_sync_now,
  reading_group_view,
  reading_group_work_key,
  type GroupExportReport,
  type GroupImportReport,
  type GroupKeyStatus,
  type GroupPrefs,
  type GroupRelayView,
  type GroupSyncReport,
  type GroupView,
  type TaskSnapshot,
} from "@/api";
import { queryKeys } from "@/api";
import { useTask } from "@/features/tasks/api";
import {
  encodeNoteId,
  notesFromDocument,
  notesFromTable,
  type ThreadNote,
} from "@/features/groups/alignment";

/** The rejection every p2p command returns in a build without the feature. */
const P2P_UNSUPPORTED = "compiled without reading-groups p2p support";

function isP2pUnsupported(error: unknown): boolean {
  return String(error).includes(P2P_UNSUPPORTED);
}

export interface ThreadView {
  notes: ThreadNote[];
  /** True when the notes came from the synced document. */
  synced: boolean;
}

// ------------------------------------------------------------------- prefs

export function useGroupPrefsQuery() {
  return useQuery({
    queryKey: queryKeys.readingGroup.prefs(),
    queryFn: () => reading_group_prefs_get(),
  });
}

export function useSetGroupPrefs() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { enabled: boolean; displayName: string }): Promise<GroupPrefs> =>
      reading_group_prefs_set(input),
    onSuccess: (prefs) => queryClient.setQueryData(queryKeys.readingGroup.prefs(), prefs),
  });
}

// ------------------------------------------------------------------ groups

export function useGroupsQuery() {
  return useQuery({
    queryKey: queryKeys.readingGroup.list(),
    queryFn: async () => asList(await reading_group_list()),
  });
}

export function useGroupQuery(groupId: string) {
  return useQuery({
    queryKey: queryKeys.readingGroup.detail(groupId),
    queryFn: () => reading_group_view({ groupId }),
    enabled: groupId.length > 0,
  });
}

export function useCreateGroup() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (name: string): Promise<GroupView> => reading_group_create({ name }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.readingGroup.lists() });
    },
  });
}

export function useRenameGroup(groupId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (name: string): Promise<GroupView> => reading_group_rename({ groupId, name }),
    onSuccess: async (group) => {
      queryClient.setQueryData(queryKeys.readingGroup.detail(groupId), group);
      await queryClient.invalidateQueries({ queryKey: queryKeys.readingGroup.lists() });
    },
  });
}

export function useDeleteGroup() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (groupId: string) => reading_group_delete({ groupId }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.readingGroup.all() });
    },
  });
}

// ----------------------------------------------------------------- members

export function useAddGroupMember(groupId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { memberId: string; displayName: string; role: string }) =>
      reading_group_add_member({ groupId, ...input }),
    onSuccess: async (group) => {
      queryClient.setQueryData(queryKeys.readingGroup.detail(groupId), group);
      await queryClient.invalidateQueries({ queryKey: queryKeys.readingGroup.lists() });
    },
  });
}

export function useRemoveGroupMember(groupId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (memberId: string) => reading_group_remove_member({ groupId, memberId }),
    onSuccess: async (group) => {
      queryClient.setQueryData(queryKeys.readingGroup.detail(groupId), group);
      // Removing a member rotates the group key (MISSION-118), so the E2EE badge
      // and the member's shelf rows are both stale now.
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.readingGroup.shelf(groupId, null) }),
        queryClient.invalidateQueries({ queryKey: queryKeys.readingGroup.keyStatus(groupId) }),
      ]);
    },
  });
}

// ------------------------------------------------------------------- shelf

/** The whole group's shelf rows — the source of the alignment matrix. */
export function useGroupShelfQuery(groupId: string, enabled = true) {
  return useQuery({
    queryKey: queryKeys.readingGroup.shelf(groupId, null),
    queryFn: async () => asList(await reading_group_shelf({ groupId, memberId: null })),
    enabled: enabled && groupId.length > 0,
  });
}

export interface ShelfEntryInput {
  memberId: string;
  workKey: string;
  title: string;
  contentType: string;
  status: string;
  progress: number;
}

export function useSetShelfEntry(groupId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: ShelfEntryInput) => reading_group_set_shelf({ groupId, ...input }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: queryKeys.readingGroup.shelf(groupId, null),
      });
    },
  });
}

/** Ask the domain for a work's stable key — never re-derive it in the UI. */
export function resolveWorkKey(input: {
  title: string;
  author?: string | null;
  year?: number | null;
  provider?: string | null;
  externalId?: string | null;
}): Promise<string> {
  return reading_group_work_key({
    title: input.title,
    author: input.author ?? null,
    year: input.year ?? null,
    provider: input.provider ?? null,
    externalId: input.externalId ?? null,
  });
}

// ------------------------------------------------------------------ thread

/** A work's discussion thread: the synced document when available, else the
    local table. */
export function useThreadQuery(groupId: string, workKey: string, enabled = true) {
  return useQuery({
    queryKey: queryKeys.readingGroup.thread(groupId, workKey),
    enabled: enabled && groupId.length > 0 && workKey.length > 0,
    queryFn: async (): Promise<ThreadView> => {
      try {
        const entries = await reading_group_note_state({ groupId, workKey });
        return { notes: notesFromDocument(entries), synced: true };
      } catch (error) {
        if (!isP2pUnsupported(error)) throw error;
        const rows = await reading_group_notes({ groupId, workKey });
        return { notes: notesFromTable(rows), synced: false };
      }
    },
  });
}

/** Append a note to the thread's store. Append-only in both modes: the
    conflict-free document is a map with no delete, so offering one would work
    in only one of the two builds. */
export function useComposeThreadNote(groupId: string, workKey: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { authorId: string; body: string; synced: boolean }): Promise<unknown> => {
      if (input.synced) {
        return reading_group_note_edit({
          groupId,
          workKey,
          noteId: encodeNoteId(input.authorId),
          body: input.body,
        });
      }
      return reading_group_add_note({
        groupId,
        workKey,
        authorId: input.authorId,
        body: input.body,
      });
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: queryKeys.readingGroup.thread(groupId, workKey),
      });
    },
  });
}

// ------------------------------------------------------- p2p: key & relays

/** Shared-key presence (the E2EE badge) — also the build-capability probe: a
    build without `p2p` rejects this call. */
export function useGroupKeyStatusQuery(groupId: string) {
  return useQuery({
    queryKey: queryKeys.readingGroup.keyStatus(groupId),
    queryFn: () => reading_group_key_status({ groupId }),
    enabled: groupId.length > 0,
    retry: false,
  });
}

/** Whether this build can talk to relays at all. */
export function isP2pBuild(status: { isError: boolean; error: unknown }): boolean {
  return !(status.isError && isP2pUnsupported(status.error));
}

/** Replace the group key (owner-only). Everyone who should still read the group
    needs the invite created after this. */
export function useRotateGroupKey(groupId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (): Promise<GroupKeyStatus> => reading_group_key_rotate({ groupId }),
    onSuccess: async (status) => {
      queryClient.setQueryData(queryKeys.readingGroup.keyStatus(groupId), status);
      await queryClient.invalidateQueries({ queryKey: queryKeys.readingGroup.relays(groupId) });
    },
  });
}

export function useGroupRelaysQuery(groupId: string, enabled: boolean) {
  return useQuery({
    queryKey: queryKeys.readingGroup.relays(groupId),
    queryFn: () => reading_group_relays_get({ groupId }),
    enabled: enabled && groupId.length > 0,
    retry: false,
  });
}

export function useSetGroupRelays(groupId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (relays: string[]): Promise<GroupRelayView> =>
      reading_group_relays_set({ groupId, relays }),
    onSuccess: (view) => queryClient.setQueryData(queryKeys.readingGroup.relays(groupId), view),
  });
}

export function useCreateInvite() {
  return useMutation({
    mutationFn: (input: { groupId: string; relays: string[] }) =>
      reading_group_invite_create(input),
  });
}

export function useAcceptInvite() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (link: string) => reading_group_invite_accept({ link }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.readingGroup.all() });
    },
  });
}

// -------------------------------------------------------------------- sync

/** Start a sync pass. Resolves with the queued task snapshot. */
export function useSyncGroup() {
  return useMutation({
    mutationFn: (groupId: string): Promise<TaskSnapshot> => reading_group_sync_now({ groupId }),
  });
}

/** Follow a sync task; the group, its thread and its relay depth refresh when it
    ends. A sync now also carries the other members' announcements, so the member
    list and the shelves matrix are as stale as the thread. */
export function useGroupSyncTask(taskId: string | null, groupId: string) {
  const queryClient = useQueryClient();
  return useTask(taskId, {
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.readingGroup.detail(groupId) });
      void queryClient.invalidateQueries({ queryKey: queryKeys.readingGroup.relays(groupId) });
      void queryClient.invalidateQueries({
        queryKey: queryKeys.readingGroup.shelf(groupId, null),
      });
      void queryClient.invalidateQueries({
        predicate: (query) =>
          query.queryKey[0] === "readingGroup" && query.queryKey[1] === "thread",
      });
    },
  });
}

/** The typed payload of a finished sync task. */
export function syncReportOf(snapshot: TaskSnapshot | undefined): GroupSyncReport | null {
  if (!snapshot || snapshot.state !== "success" || !snapshot.result) return null;
  return snapshot.result as GroupSyncReport;
}

// ------------------------------------------------------------- group file

/** Write `group_state.json` through the native save dialog (MISSION-114's
    transport, reachable from the UI). Resolves `null` when the user cancels. */
export function useExportGroupFile() {
  return useMutation({
    mutationFn: async (input: {
      groupId: string;
      fileName: string;
    }): Promise<GroupExportReport | null> => {
      const path = await save({
        title: "mylore",
        defaultPath: input.fileName,
        filters: [{ name: "MyLore group", extensions: ["json"] }],
      });
      if (!path) return null;
      return reading_group_export({ path, groupId: input.groupId });
    },
  });
}

/** Merge a `group_state.json` payload the webview read (idempotent). */
export function useImportGroupFile() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (source: string): Promise<GroupImportReport> => reading_group_import({ source }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.readingGroup.all() });
    },
  });
}

export type {
  GroupExportReport,
  GroupImportReport,
  GroupKeyStatus,
  GroupPrefs,
  GroupRelayView,
  GroupSyncReport,
  GroupView,
};
