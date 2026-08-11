-- Baseline (MISSION-012): app-level metadata table.
-- Key/value metadata about the app/schema (e.g. schema version snapshot used by
-- backup meta.json). User data lives in later aggregates (MISSION-013+).
CREATE TABLE app_meta (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
