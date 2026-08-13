// AUTO-GENERATED — do not edit. Regenerate with `npm run codegen`.
// Source of truth: scripts/ipc-contract.json

import { invoke } from "@tauri-apps/api/core";

export interface ContentNode {
  id: string;
  kind: string;
  position: number;
  number: string | null;
  title: string | null;
  release_date: string | null;
  duration_min: number | null;
  page_count: number | null;
  synopsis: string | null;
  is_special: boolean;
  state: string | null;
  children: ContentNode[];
}
export interface TrackingView {
  media_id: string;
  core_status: string;
  custom_status_id: string | null;
  started_at: string | null;
  finished_at: string | null;
  repeat_count: number;
  updated_at: string;
}

/** Placeholder greeting command (create-tauri-app scaffold). Resolves with the greeting or rejects with an AppError string. */
export function greet(args: { name: string }): Promise<string> {
  return invoke<string>("greet", args);
}

/** Create a media entry from manual input. Resolves with the new media id or rejects with an AppError string. */
export function media_create(args: {
  title: string;
  content_type: string;
  format: string | null;
  pub_status: string | null;
  synopsis: string | null;
  release_year: number | null;
  language: string | null;
  country: string | null;
  pages: number | null;
  duration_min: number | null;
  ep_count: number | null;
  ch_count: number | null;
  genres: string[];
}): Promise<string> {
  return invoke<string>("media_create", args);
}

/** List library entries with optional filters. Resolves with summary rows or rejects with an AppError string. */
export function media_list(args: {
  content_type: string | null;
  format: string | null;
  pub_status: string | null;
  genre: string | null;
  tag: string | null;
  year: number | null;
  favorite: boolean | null;
  search: string | null;
  sort: string | null;
  ascending: boolean | null;
  limit: number | null;
  offset: number | null;
}): Promise<
  {
    id: string;
    content_type: string;
    title: string;
    pub_status: string;
    release_year: number | null;
    cover_asset_id: string | null;
    updated_at: string;
  }[]
> {
  return invoke<
    {
      id: string;
      content_type: string;
      title: string;
      pub_status: string;
      release_year: number | null;
      cover_asset_id: string | null;
      updated_at: string;
    }[]
  >("media_list", args);
}

/** Distinct filter values present in the library. Resolves with facet options or rejects with an AppError string. */
export function media_facets(): Promise<{
  formats: string[];
  genres: { id: string; name: string }[];
  tags: { id: string; name: string }[];
  years: number[];
}> {
  return invoke<{
    formats: string[];
    genres: { id: string; name: string }[];
    tags: { id: string; name: string }[];
    years: number[];
  }>("media_facets");
}

/** Read the full aggregate for one media. Resolves with the record or null when not found; rejects with an AppError string. */
export function media_get(args: { id: string }): Promise<{
  id: string;
  content_type: string;
  format: string | null;
  title_main: string;
  title_original: string | null;
  synopsis: string | null;
  pub_status: string;
  start_date: string | null;
  end_date: string | null;
  release_year: number | null;
  language: string | null;
  country: string | null;
  content_rating: string | null;
  pages: number | null;
  duration_min: number | null;
  ep_count: number | null;
  ch_count: number | null;
  cover_asset_id: string | null;
  banner_asset_id: string | null;
  provider: string | null;
  provider_url: string | null;
  metadata_refreshed_at: string | null;
  created_at: string;
  updated_at: string;
  alt_titles: { lang: string; title: string }[];
  people: string[];
  genres: string[];
  tags: string[];
  external_ids: { provider: string; ext_id: string; url: string | null }[];
  relations: { to_id: string; relation: string }[];
} | null> {
  return invoke<{
    id: string;
    content_type: string;
    format: string | null;
    title_main: string;
    title_original: string | null;
    synopsis: string | null;
    pub_status: string;
    start_date: string | null;
    end_date: string | null;
    release_year: number | null;
    language: string | null;
    country: string | null;
    content_rating: string | null;
    pages: number | null;
    duration_min: number | null;
    ep_count: number | null;
    ch_count: number | null;
    cover_asset_id: string | null;
    banner_asset_id: string | null;
    provider: string | null;
    provider_url: string | null;
    metadata_refreshed_at: string | null;
    created_at: string;
    updated_at: string;
    alt_titles: { lang: string; title: string }[];
    people: string[];
    genres: string[];
    tags: string[];
    external_ids: { provider: string; ext_id: string; url: string | null }[];
    relations: { to_id: string; relation: string }[];
  } | null>("media_get", args);
}

/** Local full-text search over the library. Resolves with summary rows or rejects with an AppError string. */
export function media_search(args: { query: string }): Promise<
  {
    id: string;
    content_type: string;
    title: string;
    pub_status: string;
    release_year: number | null;
    cover_asset_id: string | null;
    updated_at: string;
  }[]
> {
  return invoke<
    {
      id: string;
      content_type: string;
      title: string;
      pub_status: string;
      release_year: number | null;
      cover_asset_id: string | null;
      updated_at: string;
    }[]
  >("media_search", args);
}

/** Read the full content tree for one media (seasons→episodes, volumes→chapters) with per-node progress state. Resolves with the nested tree, roots ordered by position, or rejects with an AppError string. */
export function media_nodes(args: { id: string }): Promise<ContentNode[]> {
  return invoke<ContentNode[]>("media_nodes", args);
}

/** Set the progress state of one node (read/watched/skipped/unread). Completed states stamp read_at. Resolves or rejects with an AppError string. */
export function node_progress_set(args: { node_id: string; node_state: string }): Promise<void> {
  return invoke<void>("node_progress_set", args);
}

/** Set the progress state of every node between two nodes in the media's display order. Resolves with the affected node ids (for optimistic UI) or rejects with an AppError string. */
export function node_progress_range(args: {
  media_id: string;
  from_id: string;
  to_id: string;
  node_state: string;
}): Promise<string[]> {
  return invoke<string[]>("node_progress_range", args);
}

/** Read the tracking row for one media. Resolves with the row or null when the media is untracked; rejects with an AppError string. */
export function tracking_get(args: { media_id: string }): Promise<TrackingView | null> {
  return invoke<TrackingView | null>("tracking_get", args);
}

/** Apply a status transition for one media (status engine applies, incl. the Repeat guard and started/finished stamps). Resolves with the updated row or rejects with an AppError string. */
export function tracking_set_status(args: {
  media_id: string;
  core_status: string;
}): Promise<TrackingView> {
  return invoke<TrackingView>("tracking_set_status", args);
}

/** Soft-delete a media: store its before-image in trash, cascade the row away. Resolves with the trash id (accepted by trash_restore for undo) or rejects with an AppError string. */
export function media_delete(args: { id: string }): Promise<string> {
  return invoke<string>("media_delete", args);
}

/** List active (not restored) trash entries. Resolves with trash items or rejects with an AppError string. */
export function trash_list(): Promise<
  { id: string; kind: string; title: string; deleted_at: string }[]
> {
  return invoke<{ id: string; kind: string; title: string; deleted_at: string }[]>("trash_list");
}

/** Restore a soft-deleted aggregate from its trash before-image. Resolves or rejects with an AppError string. */
export function trash_restore(args: { id: string }): Promise<void> {
  return invoke<void>("trash_restore", args);
}

/** Permanently forget a trash entry. Resolves or rejects with an AppError string. */
export function trash_purge(args: { id: string }): Promise<void> {
  return invoke<void>("trash_purge", args);
}

/** Set the tracking status for many media at once (status engine applies). Resolves or rejects with an AppError string. */
export function tracking_bulk_set_status(args: {
  ids: string[];
  core_status: string;
}): Promise<void> {
  return invoke<void>("tracking_bulk_set_status", args);
}

/** Add a personal tag to many media at once (reused or created as needed). Resolves or rejects with an AppError string. */
export function media_bulk_add_tag(args: { ids: string[]; tag: string }): Promise<void> {
  return invoke<void>("media_bulk_add_tag", args);
}

/** Soft-delete many media. Resolves with a trash id per media (for group undo) or rejects with an AppError string. */
export function media_bulk_delete(args: { ids: string[] }): Promise<string[]> {
  return invoke<string[]>("media_bulk_delete", args);
}

/** List collections for the add-to-list picker. Resolves with collection rows or rejects with an AppError string. */
export function collection_list(): Promise<{ id: string; name: string }[]> {
  return invoke<{ id: string; name: string }[]>("collection_list");
}

/** Add many media to one collection. Resolves or rejects with an AppError string. */
export function collection_bulk_add(args: {
  collection_id: string;
  media_ids: string[];
}): Promise<void> {
  return invoke<void>("collection_bulk_add", args);
}
