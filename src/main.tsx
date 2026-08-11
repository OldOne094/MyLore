import React from "react";
import ReactDOM from "react-dom/client";
import { RouterProvider } from "react-router";
import { initTheme } from "@/themes/theme";
import { ThemeProvider } from "@/themes/ThemeProvider";
import { ToastProvider } from "@/components/ui";
import { router } from "@/router";
import "@/i18n";
import { initI18n } from "@/i18n";
import "@/styles/tailwind.css";
import "@/design-tokens/tokens.css";
import "@/styles/global.css";

// Apply persisted theme + locale before first paint to avoid a flash.
initTheme();
initI18n();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ThemeProvider>
      <ToastProvider>
        <RouterProvider router={router} />
      </ToastProvider>
    </ThemeProvider>
  </React.StrictMode>,
);
