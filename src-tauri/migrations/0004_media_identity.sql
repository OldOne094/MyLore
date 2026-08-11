-- MISSION-015: external identity + media relationships.
--
-- media_external_id: exact identity (provider, ext_id) is globally unique; a
-- media may hold at most one id per provider (dedup / REQ-MEDIA-005).
-- media_relation: directed relationship; self-relations and unknown relation
-- values are rejected by CHECK constraints.

CREATE TABLE media_external_id (
  media_id  TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  provider  TEXT NOT NULL,
  ext_id    TEXT NOT NULL,
  url       TEXT,
  PRIMARY KEY (provider, ext_id),
  UNIQUE (media_id, provider)
);

CREATE TABLE media_relation (
  from_id   TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  to_id     TEXT NOT NULL REFERENCES media(id) ON DELETE CASCADE,
  relation  TEXT NOT NULL CHECK (relation IN
              ('sequel','prequel','adaptation','same_universe','spin_off','other')),
  PRIMARY KEY (from_id, to_id, relation),
  CHECK (from_id <> to_id)
);
