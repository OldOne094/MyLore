//! Thin IPC command handlers. Commands carry no business logic (spec §83);
//! they validate input and delegate to `application` services.

use tauri::command;
use tracing::info;

use crate::error::AppError;

/// Placeholder greeting command from the create-tauri-app scaffold.
#[command]
pub fn greet(name: &str) -> Result<String, AppError> {
    info!(name, "greet command invoked");
    Ok(format!("Hello, {name}! You've been greeted from Rust!"))
}
