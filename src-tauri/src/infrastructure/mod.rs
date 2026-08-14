//! Infrastructure layer: persistence, providers, fs, keyring, logging.
//!
//! `migrations`, `repositories`, `providers`, `image_cache`, `backup`,
//! `keyring` land with later M2/M7 missions.

pub mod content_node;
pub mod db;
pub mod fts;
pub mod logging;
pub mod providers;
pub mod repositories;

#[cfg(test)]
mod integration_tests;
#[cfg(test)]
pub mod test_support;

/// Placeholder to keep the crate skeleton compiling until M2/M7 land.
pub struct Placeholder;
