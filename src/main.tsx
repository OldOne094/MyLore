import React from "react";
import ReactDOM from "react-dom/client";
import { RouterProvider } from "react-router";
import { QueryClientProvider } from "@tanstack/react-query";
import { initTheme } from "@/themes/theme";
import { ThemeProvider } from "@/themes/ThemeProvider";
import { ToastProvider } from "@/components/ui";
import { PreferencesProvider } from "@/preferences/PreferencesProvider";
import { ProfileProvider } from "@/profile/ProfileContext";
import { queryClient } from "@/api/queryClient";
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
    <QueryClientProvider client={queryClient}>
      <ThemeProvider>
        <PreferencesProvider>
          <ProfileProvider>
            <ToastProvider>
              <RouterProvider router={router} />
            </ToastProvider>
          </ProfileProvider>
        </PreferencesProvider>
      </ThemeProvider>
    </QueryClientProvider>
  </React.StrictMode>,
);
