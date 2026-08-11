// Typed IPC + data boundary (MISSION-009, MISSION-035).
//
// Re-exports the generated command/event wrappers plus the React Query layer:
// the `api` object, domain query hooks, and the typed query-key factory. These
// are the only sanctioned ways to cross the IPC boundary or read/write domain
// cache entries — components never call `invoke` or construct keys directly.
//
// Error contract: every command resolves with its declared return type or
// rejects with the serialized AppError string (see src-tauri/src/error.rs).
// Treat rejects as `unknown` until a shared error type lands.

export * from "./ipc.generated";
export * from "./api";
export * from "./queryKeys";
export { createQueryClient, queryClient } from "./queryClient";
