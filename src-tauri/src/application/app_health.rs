//! Application health flags (MISSION-088).
//!
//! When the database fails its startup integrity check the app still launches
//! — the recovery screen needs to run — and this flag tells the UI which mode
//! it is in. It flips back to healthy only after a successful recovery plus an
//! app restart.

use std::sync::atomic::{AtomicBool, Ordering};

/// Startup health of the local database.
#[derive(Debug)]
pub struct AppHealth {
    database_ok: AtomicBool,
}

impl AppHealth {
    pub fn new(database_ok: bool) -> Self {
        Self {
            database_ok: AtomicBool::new(database_ok),
        }
    }

    pub fn database_ok(&self) -> bool {
        self.database_ok.load(Ordering::Relaxed)
    }
}
