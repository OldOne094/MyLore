import React from "react";
import ReactDOM from "react-dom/client";
import { RouterProvider } from "react-router";
import { initTheme } from "@/themes/theme";
import { ThemeProvider } from "@/themes/ThemeProvider";
import { ToastProvider } from "@/components/ui";
import { router } from "@/router";
import "@/styles/tailwind.css";
import "@/design-tokens/tokens.css";
import "@/styles/global.css";

// Apply the persisted (or system) theme before first paint to avoid a flash.
initTheme();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ThemeProvider>
      <ToastProvider>
        <RouterProvider router={router} />
      </ToastProvider>
    </ThemeProvider>
  </React.StrictMode>,
);
