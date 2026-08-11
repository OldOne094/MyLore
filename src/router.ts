import { createHashRouter } from "react-router";
import { appRoutes } from "@/routes";

/* Hash router: safe for the Tauri webview custom scheme (no server rewrites
   needed on reload). Created once and shared with main.tsx. */

export const router = createHashRouter(appRoutes);
