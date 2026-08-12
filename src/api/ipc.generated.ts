// AUTO-GENERATED — do not edit. Regenerate with `npm run codegen`.
// Source of truth: scripts/ipc-contract.json

import { invoke } from "@tauri-apps/api/core";

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
  pub_status: string | null;
  genre: string | null;
  tag: string | null;
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
