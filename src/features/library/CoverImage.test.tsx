import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import "@/i18n";
import i18n from "@/i18n";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  convertFileSrc: vi.fn((path: string) => `asset://${encodeURIComponent(path)}`),
}));

import { convertFileSrc } from "@tauri-apps/api/core";
import { CoverImage } from "./CoverImage";
import type { AssetView } from "@/api";

/* MISSION-062 — CoverImage renders a cached cover through the Tauri asset
   protocol or falls back to the content-type placeholder icon. */

function cachedAsset(overrides: Partial<AssetView> = {}): AssetView {
  return {
    id: "a-1",
    kind: "cover",
    status: "cached",
    local_path: "C:/appdata/images/a-1.jpg",
    remote_url: "https://cdn.example/cover.jpg",
    mime_type: "image/jpeg",
    ...overrides,
  };
}

afterEach(async () => {
  vi.mocked(convertFileSrc).mockClear();
  await i18n.changeLanguage("en");
});

describe("CoverImage", () => {
  it("renders the cached file through convertFileSrc when the asset is cached", () => {
    render(
      <CoverImage asset={cachedAsset()} contentType="anime" alt="Steins;Gate" iconSize={28} />,
    );
    const img = screen.getByRole("img", { name: "Steins;Gate" });
    expect(img).toHaveAttribute("src", "asset://C%3A%2Fappdata%2Fimages%2Fa-1.jpg");
    expect(img).toHaveAttribute("loading", "lazy");
    expect(convertFileSrc).toHaveBeenCalledWith("C:/appdata/images/a-1.jpg");
  });

  it("falls back to the placeholder icon for remote/failed/missing assets", () => {
    const cases: Array<Partial<AssetView>> = [
      { status: "remote", local_path: null },
      { status: "failed", local_path: null },
      { status: "missing", local_path: null },
    ];
    for (const overrides of cases) {
      const { unmount } = render(
        <CoverImage asset={cachedAsset(overrides)} contentType="anime" alt="Title" />,
      );
      expect(screen.queryByRole("img")).not.toBeInTheDocument();
      unmount();
    }
  });

  it("falls back to the placeholder icon when the asset is unresolved", () => {
    render(<CoverImage asset={null} contentType="novel" alt="Title" />);
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(convertFileSrc).not.toHaveBeenCalled();
  });

  it("never calls convertFileSrc for non-cached assets", () => {
    render(
      <CoverImage
        asset={cachedAsset({ status: "missing", local_path: null })}
        contentType="anime"
        alt="Title"
      />,
    );
    expect(convertFileSrc).not.toHaveBeenCalled();
  });
});
