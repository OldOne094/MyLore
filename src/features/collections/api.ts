/* MISSION-076 — Collections feature data layer. Wraps the collection IPC
   commands behind typed hooks: list/create/rename/delete over the collections
   plus ordered membership (bulk add, single remove, drag/drop reorder). List
   and member queries share the collection fan-out so mutations stay coherent.
   MISSION-077 adds smart collections: create from a saved filter and replace
   that filter later — membership is computed server-side, so the same members
   query key works for both manual and smart collections. */

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  collection_bulk_add,
  collection_create,
  collection_create_smart,
  collection_delete,
  collection_list,
  collection_members,
  collection_remove_member,
  collection_rename,
  collection_reorder,
  collection_update_smart,
} from "@/api";
import { queryKeys } from "@/api";
import type { SmartFilter } from "@/api";

/** Read every collection with member counts (Collections page + picker). */
export function useCollectionsQuery() {
  return useQuery({
    queryKey: queryKeys.collection.lists(),
    queryFn: () => collection_list(),
  });
}

/** Read one collection's members in display order. */
export function useCollectionMembersQuery(collectionId: string) {
  return useQuery({
    queryKey: queryKeys.collection.members(collectionId),
    queryFn: () => collection_members({ collectionId: collectionId }),
  });
}

function useCollectionWrite() {
  const queryClient = useQueryClient();
  return {
    /** Drop stale collection list caches after any mutation. */
    onAny: async () => {
      await queryClient.invalidateQueries({ queryKey: queryKeys.collection.all() });
    },
  };
}

export function useCreateCollection() {
  const { onAny } = useCollectionWrite();
  return useMutation({
    mutationFn: (name: string) => collection_create({ name }),
    onSuccess: onAny,
  });
}

/** Create a smart collection from a saved filter (MISSION-077). */
export function useCreateSmartCollection() {
  const { onAny } = useCollectionWrite();
  return useMutation({
    mutationFn: ({ name, filter }: { name: string; filter: SmartFilter }) =>
      collection_create_smart({ name, filter }),
    onSuccess: onAny,
  });
}

/** Replace a smart collection's filter; membership recomputes live (MISSION-077). */
export function useUpdateSmartFilter() {
  const { onAny } = useCollectionWrite();
  return useMutation({
    mutationFn: ({ collectionId, filter }: { collectionId: string; filter: SmartFilter }) =>
      collection_update_smart({ collectionId: collectionId, filter }),
    onSuccess: onAny,
  });
}

export function useRenameCollection() {
  const { onAny } = useCollectionWrite();
  return useMutation({
    mutationFn: ({ collectionId, name }: { collectionId: string; name: string }) =>
      collection_rename({ collectionId: collectionId, name }),
    onSuccess: onAny,
  });
}

export function useDeleteCollection() {
  const { onAny } = useCollectionWrite();
  return useMutation({
    mutationFn: (collectionId: string) => collection_delete({ collectionId: collectionId }),
    onSuccess: onAny,
  });
}

export function useAddMembers() {
  const { onAny } = useCollectionWrite();
  return useMutation({
    mutationFn: ({ collectionId, mediaIds }: { collectionId: string; mediaIds: string[] }) =>
      collection_bulk_add({ collectionId: collectionId, mediaIds: mediaIds, filter: null }),
    onSuccess: onAny,
  });
}

export function useRemoveMember() {
  const { onAny } = useCollectionWrite();
  return useMutation({
    mutationFn: ({ collectionId, mediaId }: { collectionId: string; mediaId: string }) =>
      collection_remove_member({ collectionId: collectionId, mediaId: mediaId }),
    onSuccess: onAny,
  });
}

export function useReorderMembers() {
  const { onAny } = useCollectionWrite();
  return useMutation({
    mutationFn: ({ collectionId, mediaIds }: { collectionId: string; mediaIds: string[] }) =>
      collection_reorder({ collectionId: collectionId, mediaIds: mediaIds }),
    onSuccess: onAny,
  });
}
