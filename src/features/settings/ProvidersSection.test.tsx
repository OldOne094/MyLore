import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ToastProvider } from "@/components/ui";
import "@/i18n";
import i18n from "@/i18n";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { ProvidersSection } from "./ProvidersSection";
import type { ProviderSettingsRow } from "./providers";

let current: ProviderSettingsRow[];

const PROVIDERS: ProviderSettingsRow[] = [
  { provider: "tmdb", name: "TMDB", enabled: false, requires_key: true, has_key: false },
  { provider: "anilist", name: "AniList", enabled: true, requires_key: false, has_key: false },
];

function renderSection() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ToastProvider>
        <ProvidersSection />
      </ToastProvider>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  current = PROVIDERS.map((row) => ({ ...row }));
  vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
    const input = (args ?? {}) as Record<string, unknown>;
    switch (cmd) {
      case "providers_list":
        return current.map((row) => ({ ...row }));
      case "provider_set_enabled": {
        const provider = String(input.provider);
        const enabled = Boolean(input.enabled);
        const row = current.find((r) => r.provider === provider);
        if (row) row.enabled = enabled;
        return { ...(row ?? { provider: "" }) };
      }
      case "provider_set_key": {
        const provider = String(input.provider);
        const apiKey = String(input.api_key);
        const row = current.find((r) => r.provider === provider);
        if (row) row.has_key = apiKey.trim().length > 0;
        return { ...(row ?? { provider: "" }) };
      }
      case "provider_test_connection":
        return { ok: true, message: "", results: 3 };
      default:
        throw new Error(`unexpected command ${cmd}`);
    }
  });
});

afterEach(async () => {
  vi.mocked(invoke).mockReset();
  await i18n.changeLanguage("en");
});

describe("ProvidersSection", () => {
  it("renders one row per provider with switches and key fields only where required", async () => {
    renderSection();
    expect(await screen.findByText("TMDB")).toBeInTheDocument();
    expect(screen.getByText("AniList")).toBeInTheDocument();

    const switches = screen.getAllByRole("switch");
    expect(switches).toHaveLength(2);
    expect(screen.getByRole("switch", { name: "Enable TMDB" })).toHaveAttribute(
      "aria-checked",
      "false",
    );
    expect(screen.getByRole("switch", { name: "Disable AniList" })).toHaveAttribute(
      "aria-checked",
      "true",
    );

    expect(screen.getByLabelText("TMDB API key")).toBeInTheDocument();
    expect(screen.queryByLabelText("AniList API key")).not.toBeInTheDocument();
  });

  it("toggles a provider on and refetches the snapshot", async () => {
    const user = userEvent.setup();
    renderSection();

    const tmdbSwitch = await screen.findByRole("switch", { name: "Enable TMDB" });
    await user.click(tmdbSwitch);

    await waitFor(() => {
      expect(screen.getByRole("switch", { name: "Disable TMDB" })).toHaveAttribute(
        "aria-checked",
        "true",
      );
    });
    expect(invoke).toHaveBeenCalledWith("provider_set_enabled", {
      provider: "tmdb",
      enabled: true,
    });
  });

  it("saves an API key, clears the field and shows the saved indicator", async () => {
    const user = userEvent.setup();
    renderSection();

    const keyField = await screen.findByLabelText("TMDB API key");
    await user.type(keyField, "abc-123");
    await user.click(screen.getByRole("button", { name: "Save key" }));

    expect(invoke).toHaveBeenCalledWith("provider_set_key", {
      provider: "tmdb",
      api_key: "abc-123",
    });
    await waitFor(() => {
      expect(screen.getByText("Key saved")).toBeInTheDocument();
    });
    expect(keyField).toHaveValue("");
  });

  it("disables the save button until a key is typed", async () => {
    renderSection();
    const saveButton = await screen.findByRole("button", { name: "Save key" });
    expect(saveButton).toBeDisabled();
  });

  it("reports a successful connection test with the result count", async () => {
    const user = userEvent.setup();
    renderSection();

    await user.click(await screen.findByRole("button", { name: "Test TMDB connection" }));

    await waitFor(() => {
      expect(screen.getByText("Connected — 3 results")).toBeInTheDocument();
    });
    expect(invoke).toHaveBeenCalledWith("provider_test_connection", { provider: "tmdb" });
  });

  it("surfaces a failed connection test", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "providers_list") return PROVIDERS.map((row) => ({ ...row }));
      if (cmd === "provider_test_connection") {
        return { ok: false, message: "tmdb requires authentication", results: 0 };
      }
      throw new Error(`unexpected command ${cmd}`);
    });
    const user = userEvent.setup();
    renderSection();

    await user.click(await screen.findByRole("button", { name: "Test TMDB connection" }));

    await waitFor(() => {
      expect(
        screen.getByText("Couldn't connect: tmdb requires authentication"),
      ).toBeInTheDocument();
    });
  });

  it("renders an empty state when no providers are registered", async () => {
    vi.mocked(invoke).mockResolvedValue([]);
    renderSection();
    expect(await screen.findByText("No providers registered.")).toBeInTheDocument();
  });

  it("shows the error state with retry when the snapshot fails", async () => {
    vi.mocked(invoke).mockRejectedValue("internal error: boom");
    const user = userEvent.setup();
    renderSection();

    expect(
      await screen.findByText("Something went wrong while reading your provider settings."),
    ).toBeInTheDocument();

    vi.mocked(invoke).mockResolvedValue(PROVIDERS.map((row) => ({ ...row })));
    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByText("TMDB")).toBeInTheDocument();
  });
});
