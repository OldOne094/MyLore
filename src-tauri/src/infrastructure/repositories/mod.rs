//! Typed persistence repositories (MISSION-019).
//!
//! Each module owns the SQL for one aggregate and exposes async, type-checked
//! operations over the shared `SqlitePool`. Repositories stay free of domain
//! logic:
//!   - cross-row tree invariants are delegated to the validators in
//!     `infrastructure::content_node` (node repo);
//!   - the FTS index needs no maintenance here — triggers (0007) refresh it;
//!   - timestamps are supplied by callers so repositories remain clock-free.

pub mod activity;
pub mod asset;
pub mod calendar;
pub mod collection;
pub mod media;
pub mod node;
pub mod review;
pub mod tracking;
pub mod trash;
