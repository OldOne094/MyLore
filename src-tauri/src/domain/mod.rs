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
//!   - `progress`     — derived progress templates + aggregation engine
//!   - `status`       — status engine: transitions, custom statuses, auto-rules
//!   - `normalize`    — title fold (case/unicode/diacritics, script-aware)
//!   - `identity`     — exact + fuzzy identity matching, candidate ranking
//!   - `stats`        — dashboard statistics (counts, hours, completion, rating)
//!   - `value_objects`/`enums` — immutable values and `CHECK`-aligned enums

pub mod content_node;
pub mod enums;
pub mod error;
pub mod identity;
pub mod media;
pub mod normalize;
pub mod progress;
pub mod review;
pub mod stats;
pub mod status;
pub mod tracking;
pub mod value_objects;

pub use error::DomainError;
