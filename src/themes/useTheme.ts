import { useContext } from "react";
import { ThemeContext } from "./ThemeContext";

/** Access the applied theme + preference from within a ThemeProvider. */
export function useTheme() {
  const ctx = useContext(ThemeContext);
  if (!ctx) throw new Error("useTheme must be used within a ThemeProvider");
  return ctx;
}
