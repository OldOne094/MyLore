//! Thin IPC command handlers. Commands carry no business logic (spec §83);
//! they validate input and delegate to `application` services.

pub mod backup;
pub mod bulk;
pub mod calendar;
pub mod collection;
pub mod dashboard;
pub mod db_security;
pub mod discover;
pub mod enrich;
pub mod export;
pub mod images;
pub mod import;
pub mod media;
pub mod merge;
pub mod node;
pub mod providers;
pub mod reading;
pub mod reading_group;
pub mod recap;
pub mod recovery;
pub mod review;
pub mod stats;
pub mod tasks;
pub mod tracking;
pub mod trash;
