// Typed IPC boundary (MISSION-009).
//
// Re-exports the generated command/event wrappers. The wrappers are the only
// sanctioned way to cross the IPC boundary — components never call `invoke`
// directly.
//
// Error contract: every command resolves with its declared return type or
// rejects with the serialized AppError string (see src-tauri/src/error.rs).
// Treat rejects as `unknown` until a shared error type lands.

export * from "./ipc.generated";
