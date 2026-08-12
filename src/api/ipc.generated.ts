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
