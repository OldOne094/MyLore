import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {
  Badge,
  Button,
  Dialog,
  DialogClose,
  DialogContent,
  DialogTitle,
  DialogTrigger,
  InputField,
  Popover,
  PopoverContent,
  PopoverTrigger,
  Skeleton,
} from "./index";

describe("Button", () => {
  it("renders with the primary variant by default", () => {
    render(<Button>Go</Button>);
    const button = screen.getByRole("button", { name: "Go" });
    expect(button).toHaveClass("bg-accent");
  });

  it("applies ghost variant", () => {
    render(<Button variant="ghost">Ghost</Button>);
    expect(screen.getByRole("button", { name: "Ghost" })).toHaveClass("bg-transparent");
  });

  it("defaults to type=button", () => {
    render(<Button>Safe</Button>);
    expect(screen.getByRole("button", { name: "Safe" })).toHaveAttribute("type", "button");
  });
});

describe("InputField", () => {
  it("associates the label and reports validation errors", async () => {
    const user = userEvent.setup();
    render(<InputField label="Title" error="Required" />);
    expect(screen.getByLabelText("Title")).toHaveAttribute("aria-invalid", "true");
    expect(screen.getByRole("alert")).toHaveTextContent("Required");

    await user.type(screen.getByLabelText("Title"), "One Piece");
    expect(screen.getByLabelText("Title")).toHaveValue("One Piece");
  });

  it("renders without an error slot", () => {
    render(<InputField label="Title" />);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});

describe("Badge", () => {
  it("applies the status variant token", () => {
    const { rerender } = render(<Badge variant="completed">Done</Badge>);
    expect(screen.getByText("Done")).toHaveClass("text-status-completed");
    rerender(<Badge>Neutral</Badge>);
    expect(screen.getByText("Neutral")).toHaveClass("text-text-secondary");
  });
});

describe("Dialog", () => {
  it("opens and focuses a dialog with title", async () => {
    const user = userEvent.setup();
    render(
      <Dialog>
        <DialogTrigger asChild>
          <Button>Open</Button>
        </DialogTrigger>
        <DialogContent>
          <DialogTitle>Details</DialogTitle>
        </DialogContent>
      </Dialog>,
    );

    await user.click(screen.getByRole("button", { name: "Open" }));
    const dialog = screen.getByRole("dialog");
    expect(dialog).toBeInTheDocument();
    expect(screen.getByText("Details")).toBeInTheDocument();
  });

  it("closes via DialogClose", async () => {
    const user = userEvent.setup();
    render(
      <Dialog>
        <DialogTrigger asChild>
          <Button>Open</Button>
        </DialogTrigger>
        <DialogContent>
          <DialogTitle>Details</DialogTitle>
          <DialogClose asChild>
            <Button>Done</Button>
          </DialogClose>
        </DialogContent>
      </Dialog>,
    );

    await user.click(screen.getByRole("button", { name: "Open" }));
    await user.click(screen.getByRole("button", { name: "Done" }));
    const { waitFor } = await import("@testing-library/react");
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });
});

describe("Popover", () => {
  it("opens on trigger click", async () => {
    const user = userEvent.setup();
    render(
      <Popover>
        <PopoverTrigger asChild>
          <Button>Menu</Button>
        </PopoverTrigger>
        <PopoverContent>
          <span>Quick actions</span>
        </PopoverContent>
      </Popover>,
    );
    await user.click(screen.getByRole("button", { name: "Menu" }));
    expect(await screen.findByText("Quick actions")).toBeInTheDocument();
  });
});

describe("Skeleton", () => {
  it("renders a hidden placeholder", () => {
    render(<Skeleton className="h-4 w-24" />);
    expect(document.querySelector('[aria-hidden="true"]')).toHaveClass("animate-pulse");
  });
});
