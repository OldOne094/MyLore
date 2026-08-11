import React from "react";
import ReactDOM from "react-dom/client";
import App from "@/App";
import { initTheme } from "@/themes/theme";
import { ThemeProvider } from "@/themes/ThemeProvider";
import "@/design-tokens/tokens.css";
import "@/styles/global.css";

// Apply the persisted (or system) theme before first paint to avoid a flash.
initTheme();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ThemeProvider>
      <App />
    </ThemeProvider>
  </React.StrictMode>,
);
