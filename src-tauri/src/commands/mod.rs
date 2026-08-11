//! Thin IPC command handlers. Commands carry no business logic (spec §83);
//! they validate input and delegate to `application` services.

use tauri::command;

/// Placeholder greeting command from the create-tauri-app scaffold.
#[command]
pub fn greet(name: &str) -> String {
    format!("Hello, {name}! You've been greeted from Rust!")
}
