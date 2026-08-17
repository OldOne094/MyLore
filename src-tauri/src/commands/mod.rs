//! Thin IPC command handlers. Commands carry no business logic (spec §83);
//! they validate input and delegate to `application` services.

use tauri::command;
use tracing::info;

use crate::error::AppError;

pub mod bulk;
pub mod dashboard;
pub mod discover;
pub mod enrich;
pub mod images;
pub mod import;
pub mod media;
pub mod node;
pub mod providers;
pub mod tasks;
pub mod tracking;
pub mod trash;

/// Placeholder greeting command from the create-tauri-app scaffold.
#[command]
pub fn greet(name: &str) -> Result<String, AppError> {
    info!(name, "greet command invoked");
    Ok(format!("Hello, {name}! You've been greeted from Rust!"))
}
