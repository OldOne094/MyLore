import { afterEach, describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "@/i18n";
import i18n from "@/i18n";
import { SpoileredNote } from "./SpoileredNote";

const NOTE = {
  id: "n:1:m-nour:uuid",
  author_id: "m-nour",
  body: "The ending reveals everything.",
  at: "2026-03-04T05:06:07Z",
};

afterEach(async () => {
  await i18n.changeLanguage("en");
});

describe("SpoileredNote", () => {
  it("keeps a gated note out of the DOM entirely", () => {
    render(
      <ul>
        <SpoileredNote note={NOTE} authorName="Nour" authorProgress={40} gated />
      </ul>,
    );

    // The body must not merely be blurred — it must not be readable at all.
    expect(screen.queryByText("The ending reveals everything.")).not.toBeInTheDocument();
    expect(screen.getByText("Nour is at 40 — further than you")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Show anyway" })).toBeInTheDocument();
  });

  it("reveals a gated note on request", async () => {
    render(
      <ul>
        <SpoileredNote note={NOTE} authorName="Nour" authorProgress={40} gated />
      </ul>,
    );

    await userEvent.click(screen.getByRole("button", { name: "Show anyway" }));
    expect(screen.getByText("The ending reveals everything.")).toBeInTheDocument();
    expect(screen.queryByText("Nour is at 40 — further than you")).not.toBeInTheDocument();
  });

  it("shows an ungated note plainly", () => {
    render(
      <ul>
        <SpoileredNote note={NOTE} authorName="Nour" authorProgress={2} gated={false} />
      </ul>,
    );

    expect(screen.getByText("The ending reveals everything.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Show anyway" })).not.toBeInTheDocument();
  });

  it("labels a note whose author cannot be identified", () => {
    render(
      <ul>
        <SpoileredNote
          note={{ ...NOTE, author_id: "", at: null }}
          authorName=""
          authorProgress={0}
          gated={false}
        />
      </ul>,
    );
    expect(screen.getByText("Unknown")).toBeInTheDocument();
  });
});
