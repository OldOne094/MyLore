-- MISSION-052: per-media tracking mode — Normal (autoTrack) vs Manual.
--
-- `auto_track` gates the auto-status rule: in Normal mode (the default) every
-- progress write re-derives the status from the aggregate (planned → in
-- progress → completed); in Manual mode the user owns the status and progress
-- marks never change it. NovelUpdates "Normal vs Manual", adapted to the status
-- engine (DOMAIN_MODEL §2.3, UX_RESEARCH §3.10).
ALTER TABLE tracking ADD COLUMN auto_track INTEGER NOT NULL DEFAULT 1;
