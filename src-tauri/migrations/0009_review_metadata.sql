-- MISSION-079: StoryGraph-style review metadata — mood, pace and content
-- warnings as user-owned review fields, with an acknowledgment timestamp for
-- the current content-warning set ("acknowledged-with-timestamp metadata,
-- never forced"). Moods / content warnings are canonical JSON arrays (sorted,
-- deduplicated, validated against a fixed vocabulary by the service); pace is
-- a single fixed-vocabulary value.

ALTER TABLE review ADD COLUMN moods TEXT;
ALTER TABLE review ADD COLUMN pace TEXT CHECK (pace IS NULL OR pace IN ('slow','medium','fast'));
ALTER TABLE review ADD COLUMN content_warnings TEXT;
ALTER TABLE review ADD COLUMN warnings_acknowledged_at TEXT;
