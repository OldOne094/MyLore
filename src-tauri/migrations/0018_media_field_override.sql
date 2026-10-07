-- MISSION-161: which provider-owned fields the user has edited by hand.
--
-- `EnrichService` refreshes a title by writing the provider's values over the
-- `media` row, and only *reports* the diff afterwards. Without this table a
-- hand-corrected title, synopsis or year is silently replaced by the next
-- refresh — the edit is lost with no way to tell it happened.
--
-- A row here pins one field to the user's value: enrich keeps it and says so,
-- until the user releases it (`unpin`) or edits the field back. Keys are the
-- same field names the enrich diff uses (`title_main`, `synopsis`,
-- `release_year`, `pub_status`, `format`, `genres`, …), which is what lets one
-- list drive both the pin and the skip.
--
-- Cascades with the media row, so a deleted title takes its pins with it.

CREATE TABLE media_field_override (
  media_id   TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  field      TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (media_id, field)
);
