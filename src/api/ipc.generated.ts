// AUTO-GENERATED — do not edit. Regenerate with `npm run codegen`.
// Source of truth: scripts/ipc-contract.json

import { invoke } from "@tauri-apps/api/core";
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";

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
  auto_track: boolean;
  progress: ProgressSummary | null;
  updated_at: string;
}
export interface ProgressSummary {
  percent: number | null;
  completed: number;
  total: number;
  next_label: string | null;
  next_node_id: string | null;
}
export interface ReviewView {
  media_id: string;
  rating: number | null;
  review: string | null;
  short_review: string | null;
  notes: string | null;
  favorite: boolean;
  is_spoiler: boolean;
  created_at: string;
  updated_at: string;
}
export interface MediaTagView {
  id: string;
  name: string;
  scope: string;
}
export interface NodeProgressNextView {
  media_id: string;
  summary: ProgressSummary;
}
export interface MediaListItem {
  id: string;
  content_type: string;
  title: string;
  pub_status: string;
  release_year: number | null;
  cover_asset_id: string | null;
  updated_at: string;
  progress: ProgressSummary;
}
export interface DashboardSummary {
  continue_watching: MediaListItem[];
  recently_completed: MediaListItem[];
  recently_added: MediaListItem[];
}
export interface ExternalSearchView {
  local: MediaListItem[];
  groups: ExternalProviderGroup[];
  failures: ExternalProviderFailure[];
}
export interface ExternalProviderGroup {
  provider: string;
  name: string;
  hits: ExternalHit[];
}
export interface ExternalHit {
  provider: string;
  provider_id: string;
  title: string;
  content_type: string;
  release_year: number | null;
  cover_url: string | null;
  synopsis: string | null;
  url: string | null;
  identity: ExternalIdentityFlag;
}
export interface ExternalIdentityFlag {
  kind: string;
  media_id: string | null;
  score: number | null;
}
export interface ExternalProviderFailure {
  provider: string;
  message: string;
}
export interface ProviderImportView {
  media_id: string;
  created: boolean;
  identity_kind: string;
  title: string;
  content_type: string;
}
export interface EnrichChange {
  field: string;
  before: string | null;
  after: string | null;
}
export interface EnrichView {
  media_id: string;
  provider: string;
  refreshed_at: string;
  changed: boolean;
  changes: EnrichChange[];
}
export interface AssetView {
  id: string;
  kind: string;
  status: string;
  local_path: string | null;
  remote_url: string | null;
  mime_type: string | null;
}
export interface ProviderSettingsView {
  provider: string;
  name: string;
  enabled: boolean;
  requires_key: boolean;
  has_key: boolean;
}
export interface ProviderTestView {
  ok: boolean;
  message: string;
  results: number;
}
export interface CsvMapping {
  title: string | null;
  title_original: string | null;
  alt_titles: string | null;
  content_type: string | null;
  default_content_type: string | null;
  format: string | null;
  pub_status: string | null;
  start_date: string | null;
  end_date: string | null;
  release_year: string | null;
  language: string | null;
  country: string | null;
  content_rating: string | null;
  pages: string | null;
  duration_min: string | null;
  ep_count: string | null;
  ch_count: string | null;
  synopsis: string | null;
  author: string | null;
  artist: string | null;
  studio: string | null;
  genres: string | null;
  tags: string | null;
  external_id: string | null;
  cover_url: string | null;
  banner_url: string | null;
  delimiter: string;
  separator: string;
}
export interface Issue {
  severity: string;
  field: string;
  message: string;
}
export interface PreviewItem {
  source_row: number;
  title: string | null;
  outcome: string;
  matched_media_id: string | null;
  match_kind: string | null;
  match_score: number | null;
  issues: Issue[];
}
export interface ImportPreview {
  total: number;
  valid: number;
  invalid: number;
  new: number;
  in_library: number;
  duplicates: number;
  items: PreviewItem[];
}
export interface ImportPlan {
  rows: number[];
}
export interface ReportItem {
  source_row: number;
  title: string;
  status: string;
  media_id: string | null;
  message: string | null;
}
export interface ImportReport {
  total: number;
  committed: number;
  skipped: number;
  failed: number;
  items: ReportItem[];
}
export interface ExportReport {
  format: string;
  total: number;
  path: string;
}
export interface TaskSnapshot {
  id: string;
  kind: string;
  title: string;
  state: string;
  progress: number | null;
  message: string | null;
  error: string | null;
  result: unknown | null;
  created_at: string;
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

/** List library entries with optional filters. Resolves with summary rows (each carrying its progress summary for the in-grid quick controls) or rejects with an AppError string. */
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
}): Promise<MediaListItem[]> {
  return invoke<MediaListItem[]>("media_list", args);
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

/** Local full-text search over the library. Resolves with summary rows (each carrying its progress summary) or rejects with an AppError string. */
export function media_search(args: { query: string }): Promise<MediaListItem[]> {
  return invoke<MediaListItem[]>("media_search", args);
}

/** The personal tags linked to one media. Resolves with tag rows (id + name + scope) or rejects with an AppError string. */
export function media_tags(args: { media_id: string }): Promise<MediaTagView[]> {
  return invoke<MediaTagView[]>("media_tags", args);
}

/** Add a personal tag to one media (reused or created as needed). Resolves with the updated personal-tag list or rejects with an AppError string. */
export function media_add_tag(args: { media_id: string; tag: string }): Promise<MediaTagView[]> {
  return invoke<MediaTagView[]>("media_add_tag", args);
}

/** Remove a personal tag from one media (the tag row is kept for other media). Resolves with the updated personal-tag list or rejects with an AppError string. */
export function media_remove_tag(args: {
  media_id: string;
  tag_id: string;
}): Promise<MediaTagView[]> {
  return invoke<MediaTagView[]>("media_remove_tag", args);
}

/** Read a media's review. Resolves with the row or null when the user hasn't reviewed it; rejects with an AppError string. */
export function review_get(args: { media_id: string }): Promise<ReviewView | null> {
  return invoke<ReviewView | null>("review_get", args);
}

/** Save (create or update) a media's review. Resolves with the stored row (an entirely empty review clears the row and resolves with an empty view) or rejects with an AppError string. */
export function review_save(args: {
  media_id: string;
  rating: number | null;
  review: string | null;
  short_review: string | null;
  notes: string | null;
  favorite: boolean;
  is_spoiler: boolean;
}): Promise<ReviewView> {
  return invoke<ReviewView>("review_save", args);
}

/** Delete a media's review row. Resolves or rejects with an AppError string. */
export function review_delete(args: { media_id: string }): Promise<void> {
  return invoke<void>("review_delete", args);
}

/** External (provider) search grouped by provider, with identity flags. `content_type` narrows the fan-out when provided; null searches every enabled provider (domain-agnostic). Resolves with local hits + provider groups + per-provider failures, or rejects with an AppError string. */
export function search_external(args: {
  query: string;
  content_type: string | null;
}): Promise<ExternalSearchView> {
  return invoke<ExternalSearchView>("search_external", args);
}

/** Import one provider title into the library (details → identity check → add). Resolves with the media that owns the title — newly created (created: true) or an existing library row the identity check matched (created: false), or rejects with an AppError string. */
export function import_provider(args: {
  provider: string;
  provider_id: string;
}): Promise<ProviderImportView> {
  return invoke<ProviderImportView>("import_provider", args);
}

/** Sniff a file's import format from its content (MISSION-072): `json` vs `anilist` for JSON files, `csv` vs `goodreads` vs `storygraph` for CSV files. The frontend calls this after reading a file to pick the parser and, for the profile kinds (anilist/goodreads/storygraph), to skip the column-mapping step. Resolves with the kind string or rejects with an AppError string. */
export function import_file_detect(args: { source: string }): Promise<string> {
  return invoke<string>("import_file_detect", args);
}

/** Parse + dedup a file (kind `json` = MyLore JSON format, `csv` = CSV with a column mapping, `anilist` = AniList export, `goodreads` = Goodreads CSV, `storygraph` = StoryGraph CSV) into the per-item preview. `mapping` is required for `csv` and ignored for the other kinds. Read-only. Resolves with the preview (per-row outcomes + issues) or rejects with an AppError string. */
export function import_file_preview(args: {
  kind: string;
  source: string;
  mapping: CsvMapping | null;
}): Promise<ImportPreview> {
  return invoke<ImportPreview>("import_file_preview", args);
}

/** Import a file's rows as a background task (MISSION-070): spawns the commit on the TaskManager and resolves with the initial (queued) snapshot; progress + terminal state stream as `task_changed` events and the task can be cancelled. The commit runs in one transaction, savepoint per row. `plan` selects which source rows to import; null imports every `New` row of the preview. Non-new / invalid / unselected rows are reported as skipped; a row that fails to insert rolls back its own savepoint and is reported as failed. On success the task's `result` is the per-item `ImportReport`. */
export function import_commit(args: {
  kind: string;
  source: string;
  mapping: CsvMapping | null;
  plan: ImportPlan | null;
}): Promise<TaskSnapshot> {
  return invoke<TaskSnapshot>("import_commit", args);
}

/** Export the whole library as a background task (MISSION-071): streams rows to `path` as json / csv / markdown (`format`) and resolves with the initial (queued) snapshot; progress + terminal state stream as `task_changed` events and the task can be cancelled (a cancelled export drops its partial file). The file is written to a `*.partial` sibling and renamed into place on success. On success the task's `result` is the `ExportReport` (`{ format, total, path }`). */
export function export_media(args: { format: string; path: string }): Promise<TaskSnapshot> {
  return invoke<TaskSnapshot>("export_media", args);
}

/** Read the header row of a CSV file for the mapping UI's column pickers. Resolves with the trimmed column names or rejects with an AppError string. */
export function import_csv_headers(args: { source: string; delimiter: string }): Promise<string[]> {
  return invoke<string[]>("import_csv_headers", args);
}

/** Refresh a media's provider-owned metadata from its provider and report what changed (per-field before → after). Never touches user data (tracking, review, collections, personal tags, asset ids). Resolves with the diff view or rejects with an AppError string. */
export function media_enrich(args: { media_id: string }): Promise<EnrichView> {
  return invoke<EnrichView>("media_enrich", args);
}

/** Snapshot every registered provider for the settings UI. Resolves with the rows in registration order, or rejects with an AppError string. */
export function providers_list(): Promise<ProviderSettingsView[]> {
  return invoke<ProviderSettingsView[]>("providers_list");
}

/** Toggle one provider on/off. Persists the flag and takes effect immediately (routing rebuilds the coordinator). Resolves with the updated row or rejects with an AppError string. */
export function provider_set_enabled(args: {
  provider: string;
  enabled: boolean;
}): Promise<ProviderSettingsView> {
  return invoke<ProviderSettingsView>("provider_set_enabled", args);
}

/** Store (or clear, when blank) a provider's API key in the OS keyring. The key is never persisted in settings files and never returned to the webview. Resolves with the updated row or rejects with an AppError string. */
export function provider_set_key(args: {
  provider: string;
  api_key: string;
}): Promise<ProviderSettingsView> {
  return invoke<ProviderSettingsView>("provider_set_key", args);
}

/** Ping one provider with a probe search. Runs even when the provider is disabled so a key can be verified before enabling. Resolves with the test outcome (never rejects for a provider failure) or rejects with an AppError. */
export function provider_test_connection(args: { provider: string }): Promise<ProviderTestView> {
  return invoke<ProviderTestView>("provider_test_connection", args);
}

/** Resolve the dashboard widget lists (continue watching, recently completed, recently added). `limit` is optional and clamped per widget (1..=20). Resolves with the DashboardSummary or rejects with an AppError string. */
export function dashboard_summary(args: { limit: number | null }): Promise<DashboardSummary> {
  return invoke<DashboardSummary>("dashboard_summary", args);
}

/** Resolve one cover/banner asset to a cached local file, downloading per the cache policy when needed. `status` is `cached` (local_path usable via `convertFileSrc`), `failed` (transient, retried after a cooldown) or `missing` (permanent broken URL). Resolves with the asset view or rejects with an AppError string. */
export function asset_resolve(args: { asset_id: string }): Promise<AssetView> {
  return invoke<AssetView>("asset_resolve", args);
}

/** Resolve many cover/banner assets in one call (deduped; unknown ids are skipped). The library grid calls this once per visible page so covers resolve as a batch. Resolves with the resolved asset views or rejects with an AppError string. */
export function assets_resolve(args: { asset_ids: string[] }): Promise<AssetView[]> {
  return invoke<AssetView[]>("assets_resolve", args);
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

/** Mark the next not-yet-consumed countable node of a media done (watched for episodes, read otherwise) and run the auto-status rule. Resolves with the refreshed progress summary, null when nothing is left to mark, or rejects with an AppError string. */
export function node_progress_next(args: {
  media_id: string;
}): Promise<NodeProgressNextView | null> {
  return invoke<NodeProgressNextView | null>("node_progress_next", args);
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

/** Toggle Normal (autoTrack) vs Manual tracking mode for one media. Resolves with the updated row (turning Normal back on re-syncs the status to the current progress) or rejects with an AppError string. */
export function tracking_set_auto_track(args: {
  media_id: string;
  auto_track: boolean;
}): Promise<TrackingView> {
  return invoke<TrackingView>("tracking_set_auto_track", args);
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

/** Every background task snapshot, newest first. Resolves with the list or rejects with an AppError string. */
export function task_list(): Promise<TaskSnapshot[]> {
  return invoke<TaskSnapshot[]>("task_list");
}

/** The current snapshot of one background task. Resolves with the snapshot or rejects with an AppError string when the id is unknown. */
export function task_get(args: { id: string }): Promise<TaskSnapshot> {
  return invoke<TaskSnapshot>("task_get", args);
}

/** Request cancellation of a background task. The runner observes the flag at its next checkpoint (dropping its in-flight transaction). Resolves with the current snapshot or rejects with an AppError string when the id is unknown. */
export function task_cancel(args: { id: string }): Promise<TaskSnapshot> {
  return invoke<TaskSnapshot>("task_cancel", args);
}

export function listenTaskChanged(handler: (payload: TaskSnapshot) => void): Promise<UnlistenFn> {
  return listen<TaskSnapshot>("task-changed", (event) => handler(event.payload));
}

export function emitTaskChanged(payload: TaskSnapshot): Promise<void> {
  return emit("task-changed", payload);
}
