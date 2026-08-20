-- MISSION-082: year-in-review aggregates.
-- The recap service groups activity by kind and window over created_at, so a
-- (kind, created_at) index covers both the genre aggregation and the per-kind
-- grouping in one pass.
CREATE INDEX idx_activity_kind ON activity(kind, created_at);