import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "@/App";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue("Hello, MyLore! You've been greeted from Rust!"),
}));

import { invoke } from "@tauri-apps/api/core";

describe("App", () => {
  it("renders the scaffold heading", () => {
    render(<App />);
    expect(screen.getByText("Welcome to Tauri + React")).toBeInTheDocument();
  });

  it("greets the user through the Tauri command", async () => {
    const user = userEvent.setup();
    render(<App />);

    const input = screen.getByPlaceholderText("Enter a name...");
    await user.type(input, "MyLore");
    await user.click(screen.getByRole("button", { name: "Greet" }));

    expect(await screen.findByText(/You've been greeted from Rust!/)).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("greet", { name: "MyLore" });
  });
});
