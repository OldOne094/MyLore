import { afterEach, describe, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router";
import { ThemeProvider } from "@/themes/ThemeProvider";
import { ToastProvider } from "@/components/ui";
import { PreferencesProvider } from "@/preferences/PreferencesProvider";
import { appRoutes } from "@/routes";
import "@/i18n";
import i18n from "@/i18n";

function renderApp(initialEntry = "/library") {
  const router = createMemoryRouter(appRoutes, { initialEntries: [initialEntry] });
  return render(
    <ThemeProvider>
      <PreferencesProvider>
        <ToastProvider>
          <RouterProvider router={router} />
        </ToastProvider>
      </PreferencesProvider>
    </ThemeProvider>,
  );
}

afterEach(async () => {
  localStorage.clear();
  document.documentElement.removeAttribute("data-theme");
  document.documentElement.removeAttribute("dir");
  document.documentElement.removeAttribute("lang");
  await i18n.changeLanguage("en");
});

describe("semantic shell", () => {
  it("exposes the standard landmarks", () => {
    renderApp();
    expect(screen.getByRole("navigation", { name: "Primary navigation" })).toBeInTheDocument();
    expect(screen.getByRole("banner")).toBeInTheDocument();
    expect(screen.getByRole("main")).toHaveAttribute("id", "main-content");
    expect(screen.getByRole("contentinfo")).toBeInTheDocument();
  });

  it("provides a skip-to-content link as the first tab stop", async () => {
    const user = userEvent.setup();
    renderApp();
    const skip = screen.getByRole("link", { name: "Skip to content" });
    expect(skip).toHaveAttribute("href", "#main-content");

    await user.tab();
    expect(document.activeElement).toBe(skip);
  });

  it("focusing the skip link moves focus to the content landmark", async () => {
    const user = userEvent.setup();
    renderApp();
    const skip = screen.getByRole("link", { name: "Skip to content" });
    skip.focus();
    await user.keyboard("{Enter}");
    expect(document.activeElement).toBe(screen.getByRole("main"));
  });

  it("keeps nav sections distinguishable by heading", () => {
    renderApp("/settings");
    expect(screen.getByRole("heading", { name: "Settings", level: 1 })).toBeInTheDocument();
  });
});

describe("screen-reader labels", () => {
  it("names the theme and language controls", () => {
    renderApp();
    expect(screen.getByRole("button", { name: "Light" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "EN" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "ع" })).toBeInTheDocument();
    expect(screen.getByLabelText("Language")).toBeInTheDocument();
    expect(screen.getByLabelText("Theme")).toBeInTheDocument();
  });

  it("names the palette combobox and its close button", () => {
    renderApp();
    fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    expect(screen.getByRole("combobox", { name: "Type a command or search…" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Close" })).toBeInTheDocument();
  });

  it("translates landmarks when the language changes", async () => {
    renderApp();
    fireEvent.click(screen.getByRole("button", { name: "ع" }));
    expect(await screen.findByRole("navigation", { name: "التنقل الرئيسي" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "تخطَّ إلى المحتوى" })).toBeInTheDocument();
  });
});
