import { useState } from "react";
import reactLogo from "./assets/react.svg";
import { greet } from "@/api";
import { useTheme } from "@/themes/useTheme";
import type { ThemePreference } from "@/themes/theme";
import "./App.css";

const THEME_CHOICES: ThemePreference[] = ["light", "dark", "system"];

function App() {
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");
  const { theme, preference, setPreference } = useTheme();

  async function onGreet() {
    setGreetMsg(await greet({ name }));
  }

  return (
    <main className="shell">
      <header className="shell__header">
        <span className="shell__brand">MyLore</span>
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
      </header>

      <section className="card">
        <h1 className="card__title">Welcome to MyLore</h1>
        <p className="card__hint">
          Current theme: <strong className="card__theme">{theme}</strong>
        </p>

        <div className="row">
          <a href="https://vite.dev" target="_blank" rel="noreferrer">
            <img src="/vite.svg" className="logo logo--vite" alt="Vite logo" />
          </a>
          <a href="https://tauri.app" target="_blank" rel="noreferrer">
            <img src="/tauri.svg" className="logo logo--tauri" alt="Tauri logo" />
          </a>
          <a href="https://react.dev" target="_blank" rel="noreferrer">
            <img src={reactLogo} className="logo logo--react" alt="React logo" />
          </a>
        </div>

        <form
          className="row row--form"
          onSubmit={(e) => {
            e.preventDefault();
            void onGreet();
          }}
        >
          <input
            id="greet-input"
            className="field"
            onChange={(e) => setName(e.currentTarget.value)}
            placeholder="Enter a name..."
          />
          <button type="submit" className="button button--primary">
            Greet
          </button>
        </form>
        <p className="card__msg" aria-live="polite">
          {greetMsg}
        </p>
      </section>
    </main>
  );
}

export default App;
