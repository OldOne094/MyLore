import { describe, expect, it, vi, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "@/App";
import { ThemeProvider } from "@/themes/ThemeProvider";
import { THEME_STORAGE_KEY } from "@/themes/theme";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue("Hello, MyLore! You've been greeted from Rust!"),
}));

import { invoke } from "@tauri-apps/api/core";

function renderApp() {
  return render(
    <ThemeProvider>
      <App />
    </ThemeProvider>,
  );
}

afterEach(() => {
  localStorage.clear();
  document.documentElement.removeAttribute("data-theme");
});

describe("App", () => {
  it("renders the scaffold heading", () => {
    renderApp();
    expect(screen.getByText("Welcome to MyLore")).toBeInTheDocument();
  });

  it("greets the user through the Tauri command", async () => {
    const user = userEvent.setup();
    renderApp();

    const input = screen.getByPlaceholderText("Enter a name...");
    await user.type(input, "MyLore");
    await user.click(screen.getByRole("button", { name: "Greet" }));

    expect(await screen.findByText(/You've been greeted from Rust!/)).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("greet", { name: "MyLore" });
  });

  it("switches the applied theme and persists the choice", async () => {
    const user = userEvent.setup();
    renderApp();

    expect(document.documentElement.getAttribute("data-theme")).toBe("light");

    await user.click(screen.getByRole("button", { name: "dark" }));
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe("dark");

    await user.click(screen.getByRole("button", { name: "light" }));
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
  });
});
