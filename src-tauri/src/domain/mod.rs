//! Domain layer: entities, value objects, enums, invariants (MISSION-022).
//!
//! Pure and side-effect-free: no SQL, no I/O, no UI. Persistence happens in
//! `infrastructure`; the application layer maps `DomainError` to `AppError` at
//! the boundary.
//!
//! Entity layout follows `DOMAIN_MODEL.md`:
//!   - `media`        — metadata aggregate (Media, MediaRuntime, PersonCredit, …)
//!   - `content_node` — generic node tree + per-node progress
//!   - `tracking`     — per-media user state
//!   - `review`       — user rating / review / notes / favorite
//!   - `value_objects`/`enums` — immutable values and `CHECK`-aligned enums

pub mod content_node;
pub mod enums;
pub mod error;
pub mod media;
pub mod review;
pub mod tracking;
pub mod value_objects;

pub use error::DomainError;
