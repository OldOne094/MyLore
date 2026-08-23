import { useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { cn } from "@/lib/cn";
import type { AssetView } from "@/api";
import { TYPE_ICONS } from "./mediaMeta";

/* MISSION-062 — Resolved cover art. When the backend has cached the asset
   (`status === "cached"` with a local path) the cached file is served through
   the Tauri asset protocol (`convertFileSrc`); any other status (including
   `failed`/`missing` and not-yet-resolved) falls back to the content-type
   placeholder icon so broken URLs never show a broken image. The image fades
   in over ~150ms once decoded instead of popping into place. */

export interface CoverImageProps {
  /** Resolved asset view; `undefined`/`null` renders the placeholder icon. */
  asset?: AssetView | null;
  contentType: string;
  alt: string;
  /** Fallback placeholder icon size. */
  iconSize?: number;
  className?: string;
  /** Extra classes for the `<img>` when a cached cover is available. */
  imgClassName?: string;
}

export function CoverImage({
  asset,
  contentType,
  alt,
  iconSize = 28,
  className,
  imgClassName,
}: CoverImageProps) {
  const [loaded, setLoaded] = useState(false);
  const src =
    asset?.status === "cached" && asset.local_path ? convertFileSrc(asset.local_path) : null;

  if (src) {
    return (
      <img
        src={src}
        alt={alt}
        loading="lazy"
        onLoad={() => setLoaded(true)}
        className={cn(
          "cover-art h-full w-full object-cover transition-opacity duration-150 ease-out",
          loaded ? "opacity-100" : "opacity-0",
          imgClassName,
        )}
      />
    );
  }

  const Icon = TYPE_ICONS[contentType] ?? TYPE_ICONS.other;
  return (
    <div className={cn("flex items-center justify-center text-text-tertiary", className)}>
      <Icon size={iconSize} aria-hidden="true" />
    </div>
  );
}
