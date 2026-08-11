//! Infrastructure layer: persistence, providers, fs, keyring, logging.
//!
//! `db`, `migrations`, `repositories`, `providers`, `image_cache`,
//! `backup`, `keyring` land with M2/M7.

pub mod logging;

#[cfg(test)]
pub mod test_support;

/// Placeholder to keep the crate skeleton compiling until M2/M7 land.
pub struct Placeholder;
