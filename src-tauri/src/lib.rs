//! MyLore — Tauri application (library crate).
//!
//! Layers (spec §83): `commands` (thin IPC) → `application` (services) →
//! `domain` (pure) ⇐ `infrastructure` (sqlx repos, providers, fs, keyring).

pub mod application;
pub mod commands;
pub mod domain;
pub mod infrastructure;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![commands::greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
