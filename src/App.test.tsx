import { describe, expect, it, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router";
import { ThemeProvider } from "@/themes/ThemeProvider";
import { ToastProvider } from "@/components/ui";
import { appRoutes } from "@/routes";

function renderApp(initialEntry = "/") {
  const router = createMemoryRouter(appRoutes, { initialEntries: [initialEntry] });
  return render(
    <ThemeProvider>
      <ToastProvider>
        <RouterProvider router={router} />
      </ToastProvider>
    </ThemeProvider>,
  );
}

const LIBRARY_HINT = "Your tracked titles appear here as you add them.";
const STATS_HINT = "Time watched, pages read and your ratings distribution.";

afterEach(() => {
  localStorage.clear();
  document.documentElement.removeAttribute("data-theme");
});

describe("App shell", () => {
  it("redirects the root to the first nav section", async () => {
    renderApp("/");
    expect(await screen.findByText(LIBRARY_HINT)).toBeInTheDocument();
  });

  it("navigates between sections via the nav rail", async () => {
    const user = userEvent.setup();
    renderApp("/library");
    await user.click(screen.getByRole("link", { name: "Stats" }));
    expect(await screen.findByText(STATS_HINT)).toBeInTheDocument();
  });

  it("highlights the active nav item", async () => {
    renderApp("/library");
    expect(screen.getByRole("link", { name: "Library" })).toHaveClass("bg-accent-soft");
  });

  it("shows the status bar", () => {
    renderApp("/library");
    expect(screen.getByText("0 titles")).toBeInTheDocument();
  });

  it("switches the theme from the top bar", async () => {
    const user = userEvent.setup();
    renderApp("/library");
    await user.click(screen.getByRole("button", { name: "dark" }));
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
  });
});
