-- MISSION-160: provenance for a work copied out of a reading group.
--
-- ADR-007 keeps the group aggregate out of the personal ones, and it still
-- holds: this table is not group *content* — it is the one-way seam a copy
-- leaves behind, so pressing "add to my library" twice cannot create two
-- titles. Group rows never join into media/tracking/review; only this mapping
-- crosses, and only in the direction "I took a copy of that work".
--
-- `media_id` cascades: deleting the copy frees the key, so the work can be
-- copied again (and a deleted title must not leave a ghost "already added"
-- badge behind).
--
-- A group is keyed by `work_key` (stable across devices) while a copy is a
-- device-local media UUID, which is why the pair lives here and not on `media`.

CREATE TABLE group_work_origin (
  group_id   TEXT NOT NULL,
  work_key   TEXT NOT NULL,
  media_id   TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  created_at TEXT NOT NULL,
  PRIMARY KEY (group_id, work_key)
);

CREATE INDEX idx_group_work_origin_media ON group_work_origin(media_id);
