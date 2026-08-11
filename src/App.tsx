import { useState, type FormEvent } from "react";
import { useTheme } from "@/themes/useTheme";
import type { ThemePreference } from "@/themes/theme";
import {
  Badge,
  Button,
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogTitle,
  DialogTrigger,
  InputField,
  Popover,
  PopoverContent,
  PopoverTrigger,
  Skeleton,
  ToastProvider,
  useToast,
} from "@/components/ui";
import "./App.css";

const THEME_CHOICES: ThemePreference[] = ["light", "dark", "system"];

function ThemeSwitcher() {
  const { preference, setPreference } = useTheme();
  return (
    <div className="theme-switcher" aria-label="Theme">
      {THEME_CHOICES.map((choice) => (
        <button
          key={choice}
          type="button"
          className={`theme-switcher__option${preference === choice ? " is-active" : ""}`}
          aria-pressed={preference === choice}
          onClick={() => setPreference(choice)}
        >
          {choice}
        </button>
      ))}
    </div>
  );
}

function FieldDemo() {
  const [value, setValue] = useState("");
  const [error, setError] = useState<string | undefined>();

  function onSubmit(event: FormEvent) {
    event.preventDefault();
    setError(value.trim() ? undefined : "Name is required");
  }

  return (
    <form className="stack" onSubmit={onSubmit}>
      <InputField
        label="Library name"
        placeholder="e.g. My shelf"
        value={value}
        error={error}
        onChange={(e) => {
          setValue(e.currentTarget.value);
          if (error) setError(undefined);
        }}
      />
      <Button type="submit">Save</Button>
    </form>
  );
}

function ToastDemo() {
  const toast = useToast();
  return (
    <div className="row">
      <Button
        variant="secondary"
        onClick={() => toast.success({ title: "Saved", description: "The record was updated." })}
      >
        Success toast
      </Button>
      <Button
        variant="danger"
        onClick={() => toast.error({ title: "Import failed", description: "No new records." })}
      >
        Error toast
      </Button>
      <Button
        variant="secondary"
        onClick={() =>
          toast.info({ title: "Merged 2 entries", action: { label: "Undo", onClick: () => {} } })
        }
      >
        Undo toast
      </Button>
    </div>
  );
}

function App() {
  return (
    <ToastProvider>
      <main className="shell">
        <header className="shell__header">
          <span className="shell__brand">MyLore</span>
          <ThemeSwitcher />
        </header>

        <section className="card" aria-labelledby="demo-heading">
          <h1 id="demo-heading" className="card__title">
            Design-system primitives
          </h1>
          <p className="card__hint">Token-driven UI on Radix (MISSION-031).</p>

          <div className="stack">
            <FieldDemo />
          </div>

          <div className="row">
            <Button variant="primary">Primary</Button>
            <Button variant="secondary">Secondary</Button>
            <Button variant="ghost">Ghost</Button>
            <Button variant="danger">Danger</Button>
            <Button variant="secondary" size="sm">
              Small
            </Button>
          </div>

          <div className="row">
            <Badge variant="planned">Planned</Badge>
            <Badge variant="inprogress">In progress</Badge>
            <Badge variant="completed">Completed</Badge>
            <Badge variant="onhold">On hold</Badge>
            <Badge variant="dropped">Dropped</Badge>
            <Badge variant="repeat">Repeat</Badge>
            <Badge>Neutral</Badge>
          </div>

          <div className="row">
            <Dialog>
              <DialogTrigger asChild>
                <Button variant="secondary">Open dialog</Button>
              </DialogTrigger>
              <DialogContent aria-describedby={undefined}>
                <DialogTitle>Edit entry</DialogTitle>
                <DialogDescription>
                  Make changes to the entry. Press Esc to close.
                </DialogDescription>
                <div className="row row--padded">
                  <DialogClose asChild>
                    <Button variant="secondary">Cancel</Button>
                  </DialogClose>
                  <DialogClose asChild>
                    <Button>Save</Button>
                  </DialogClose>
                </div>
              </DialogContent>
            </Dialog>

            <Popover>
              <PopoverTrigger asChild>
                <Button variant="secondary">Quick actions</Button>
              </PopoverTrigger>
              <PopoverContent align="start">
                <p className="text-sm text-text-secondary">Mark as watched, rate or move.</p>
              </PopoverContent>
            </Popover>
          </div>

          <div className="stack">
            <ToastDemo />
          </div>

          <div className="skeleton-row" aria-label="Loading placeholder">
            <Skeleton className="size-24" />
            <div className="stack">
              <Skeleton className="h-4 w-40" />
              <Skeleton className="h-4 w-24" />
            </div>
          </div>
        </section>
      </main>
    </ToastProvider>
  );
}

export default App;
