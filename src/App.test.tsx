import { describe, expect, it, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "@/App";
import { ThemeProvider } from "@/themes/ThemeProvider";
import { THEME_STORAGE_KEY } from "@/themes/theme";

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
  it("renders the primitives scaffold heading", () => {
    renderApp();
    expect(screen.getByText("Design-system primitives")).toBeInTheDocument();
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

  it("validates the field and clears the error on input", async () => {
    const user = userEvent.setup();
    renderApp();

    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Name is required");

    await user.type(screen.getByLabelText("Library name"), "Shelf");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("opens and closes the dialog", async () => {
    const user = userEvent.setup();
    renderApp();

    await user.click(screen.getByRole("button", { name: "Open dialog" }));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByText("Edit entry")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("shows a success toast", async () => {
    const user = userEvent.setup();
    renderApp();

    await user.click(screen.getByRole("button", { name: "Success toast" }));
    expect(await screen.findByText("Saved")).toBeInTheDocument();
  });
});
