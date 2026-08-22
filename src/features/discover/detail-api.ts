import { useQuery } from "@tanstack/react-query";

/* MISSION-127 — External-hit detail data layer. Fetches full ProviderMedia
   metadata from the provider for the detail screen. The result is a
   free-form JSON object (shape varies slightly by provider) rendered as a
   rich card. */

export interface ExternalHitDetail {
  provider: string;
  provider_id: string;
  title_main: string;
  title_original: string | null;
  alt_titles: string[];
  content_type: string;
  format: string | null;
  pub_status: string;
  synopsis: string | null;
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
  cover_url: string | null;
  banner_url: string | null;
  url: string | null;
  people: { role: string; name: string }[];
  genres: string[];
  tags: string[];
}

export function useProviderGetDetails(provider: string, id: string, enabled: boolean) {
  return useQuery({
    queryKey: ["provider", "details", provider, id],
    queryFn: async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      return invoke<ExternalHitDetail>("provider_get_details", { provider, id });
    },
    enabled,
    staleTime: Number.POSITIVE_INFINITY,
    retry: false,
  });
}
