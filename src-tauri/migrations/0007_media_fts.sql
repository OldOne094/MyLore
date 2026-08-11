-- MISSION-018: FTS5 search index + triggers + rebuild.
--
-- Two FTS5 tables share one assembled document per media:
--   * media_fts       — unicode61 tokenizer (Latin + Arabic), 9 searchable fields
--   * media_fts_cjk   — trigram tokenizer (CJK substring matching, REQ-SEARCH-003)
--
-- Both store content (not contentless): FTS5's special `'delete'` command on a
-- contentless table either requires the deleted values (which refresh triggers
-- cannot know) or silently leaves orphaned index terms, so a refresh would keep
-- stale matches. Stored content makes plain `DELETE ... WHERE rowid = ?` remove
-- the terms correctly. rowid = media rowid.
--
-- Triggers on `media` and every table whose content feeds the index keep it
-- fresh: a refresh deletes + reinserts the media's document. Cascade deletes
-- re-fire these triggers because connections run with
-- `PRAGMA recursive_triggers = ON` (set in `infrastructure::db::connect`).
--
-- Multilingual tokenization: unicode61 case-folds Latin; Arabic is folded in
-- SQL (Alef variants -> ا, ى -> ي, ة -> ه, diacritics/tanween stripped) both
-- at index time and (app-side) at query time, so queries must apply the same
-- fold. CJK is indexed as 3-grams by `trigram`, giving substring matching.

-- Shared document source (normalized per column; `cjk` = full text).
CREATE VIEW v_media_fts_source AS
WITH src AS (
  SELECT
    m.id,
    m.rowid AS rowid,
    m.title_main,
    m.title_original,
    m.synopsis,
    (SELECT group_concat(title, ' ') FROM media_alt_title WHERE media_id = m.id) AS alt_titles,
    (SELECT group_concat(p.name, ' ') FROM media_person mp
       JOIN person p ON p.id = mp.person_id WHERE mp.media_id = m.id) AS people,
    (SELECT group_concat(g.name, ' ') FROM media_genre mg
       JOIN genre g ON g.id = mg.genre_id WHERE mg.media_id = m.id) AS genres,
    (SELECT group_concat(t.name, ' ') FROM media_tag mt
       JOIN tag t ON t.id = mt.tag_id WHERE mt.media_id = m.id) AS tags,
    (SELECT notes FROM review WHERE media_id = m.id) AS notes,
    (SELECT review FROM review WHERE media_id = m.id) AS review,
    (SELECT group_concat(ext_id, ' ') FROM media_external_id WHERE media_id = m.id) AS external_ids
  FROM media m
)
SELECT
  id,
  rowid,
  replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(
    lower(title_main || ' ' || COALESCE(title_original, '')),
    'أ','ا'),'إ','ا'),'آ','ا'),'ٱ','ا'),'ى','ي'),'ة','ه'),
    'ً',''),'ٌ',''),'ٍ',''),'َ',''),'ُ',''),'ِ',''),'ّ',''),'ْ','') AS title,
  replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(
    lower(COALESCE(alt_titles, '')),
    'أ','ا'),'إ','ا'),'آ','ا'),'ٱ','ا'),'ى','ي'),'ة','ه'),
    'ً',''),'ٌ',''),'ٍ',''),'َ',''),'ُ',''),'ِ',''),'ّ',''),'ْ','') AS alt_titles,
  replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(
    lower(COALESCE(synopsis, '')),
    'أ','ا'),'إ','ا'),'آ','ا'),'ٱ','ا'),'ى','ي'),'ة','ه'),
    'ً',''),'ٌ',''),'ٍ',''),'َ',''),'ُ',''),'ِ',''),'ّ',''),'ْ','') AS synopsis,
  replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(
    lower(COALESCE(people, '')),
    'أ','ا'),'إ','ا'),'آ','ا'),'ٱ','ا'),'ى','ي'),'ة','ه'),
    'ً',''),'ٌ',''),'ٍ',''),'َ',''),'ُ',''),'ِ',''),'ّ',''),'ْ','') AS people,
  replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(
    lower(COALESCE(genres, '')),
    'أ','ا'),'إ','ا'),'آ','ا'),'ٱ','ا'),'ى','ي'),'ة','ه'),
    'ً',''),'ٌ',''),'ٍ',''),'َ',''),'ُ',''),'ِ',''),'ّ',''),'ْ','') AS genres,
  replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(
    lower(COALESCE(tags, '')),
    'أ','ا'),'إ','ا'),'آ','ا'),'ٱ','ا'),'ى','ي'),'ة','ه'),
    'ً',''),'ٌ',''),'ٍ',''),'َ',''),'ُ',''),'ِ',''),'ّ',''),'ْ','') AS tags,
  replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(
    lower(COALESCE(notes, '')),
    'أ','ا'),'إ','ا'),'آ','ا'),'ٱ','ا'),'ى','ي'),'ة','ه'),
    'ً',''),'ٌ',''),'ٍ',''),'َ',''),'ُ',''),'ِ',''),'ّ',''),'ْ','') AS notes,
  replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(
    lower(COALESCE(review, '')),
    'أ','ا'),'إ','ا'),'آ','ا'),'ٱ','ا'),'ى','ي'),'ة','ه'),
    'ً',''),'ٌ',''),'ٍ',''),'َ',''),'ُ',''),'ِ',''),'ّ',''),'ْ','') AS review,
  replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(
    lower(COALESCE(external_ids, '')),
    'أ','ا'),'إ','ا'),'آ','ا'),'ٱ','ا'),'ى','ي'),'ة','ه'),
    'ً',''),'ٌ',''),'ٍ',''),'َ',''),'ُ',''),'ِ',''),'ّ',''),'ْ','') AS external_ids,
  replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(
    lower(COALESCE(title_main,'')||' '||COALESCE(title_original,'')||' '||COALESCE(synopsis,'')
      ||' '||COALESCE(alt_titles,'')||' '||COALESCE(people,'')||' '||COALESCE(genres,'')
      ||' '||COALESCE(tags,'')||' '||COALESCE(notes,'')||' '||COALESCE(review,'')
      ||' '||COALESCE(external_ids,'')),
    'أ','ا'),'إ','ا'),'آ','ا'),'ٱ','ا'),'ى','ي'),'ة','ه'),
    'ً',''),'ٌ',''),'ٍ',''),'َ',''),'ُ',''),'ِ',''),'ّ',''),'ْ','') AS cjk
FROM src;

CREATE VIRTUAL TABLE media_fts USING fts5(
  title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids
);
CREATE VIRTUAL TABLE media_fts_cjk USING fts5(
  cjk,
  tokenize = 'trigram'
);

-- Refresh helper used by every trigger.
CREATE TRIGGER trg_media_fts_insert AFTER INSERT ON media BEGIN
  INSERT INTO media_fts(rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids)
  SELECT rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids FROM v_media_fts_source WHERE id = NEW.id;
  INSERT INTO media_fts_cjk(rowid, cjk)
  SELECT rowid, cjk FROM v_media_fts_source WHERE id = NEW.id;
END;
CREATE TRIGGER trg_media_fts_delete AFTER DELETE ON media BEGIN
  DELETE FROM media_fts WHERE rowid = OLD.rowid;
  DELETE FROM media_fts_cjk WHERE rowid = OLD.rowid;
END;
CREATE TRIGGER trg_media_fts_update AFTER UPDATE ON media BEGIN
  DELETE FROM media_fts WHERE rowid = NEW.rowid;
  DELETE FROM media_fts_cjk WHERE rowid = NEW.rowid;
  INSERT INTO media_fts(rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids)
  SELECT rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids FROM v_media_fts_source WHERE id = NEW.id;
  INSERT INTO media_fts_cjk(rowid, cjk)
  SELECT rowid, cjk FROM v_media_fts_source WHERE id = NEW.id;
END;

CREATE TRIGGER trg_alt_title_fts_ins AFTER INSERT ON media_alt_title BEGIN
  DELETE FROM media_fts WHERE rowid = (SELECT rowid FROM media WHERE id = NEW.media_id);
  INSERT INTO media_fts(rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids)
  SELECT rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids FROM v_media_fts_source WHERE id = NEW.media_id;
  DELETE FROM media_fts_cjk WHERE rowid = (SELECT rowid FROM media WHERE id = NEW.media_id);
  INSERT INTO media_fts_cjk(rowid, cjk)
  SELECT rowid, cjk FROM v_media_fts_source WHERE id = NEW.media_id;
END;
CREATE TRIGGER trg_alt_title_fts_del AFTER DELETE ON media_alt_title BEGIN
  DELETE FROM media_fts WHERE rowid = (SELECT rowid FROM media WHERE id = OLD.media_id);
  INSERT INTO media_fts(rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids)
  SELECT rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids FROM v_media_fts_source WHERE id = OLD.media_id;
  DELETE FROM media_fts_cjk WHERE rowid = (SELECT rowid FROM media WHERE id = OLD.media_id);
  INSERT INTO media_fts_cjk(rowid, cjk)
  SELECT rowid, cjk FROM v_media_fts_source WHERE id = OLD.media_id;
END;
CREATE TRIGGER trg_alt_title_fts_upd AFTER UPDATE ON media_alt_title BEGIN
  DELETE FROM media_fts WHERE rowid = (SELECT rowid FROM media WHERE id = NEW.media_id);
  INSERT INTO media_fts(rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids)
  SELECT rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids FROM v_media_fts_source WHERE id = NEW.media_id;
  DELETE FROM media_fts_cjk WHERE rowid = (SELECT rowid FROM media WHERE id = NEW.media_id);
  INSERT INTO media_fts_cjk(rowid, cjk)
  SELECT rowid, cjk FROM v_media_fts_source WHERE id = NEW.media_id;
END;

CREATE TRIGGER trg_media_person_fts_ins AFTER INSERT ON media_person BEGIN
  DELETE FROM media_fts WHERE rowid = (SELECT rowid FROM media WHERE id = NEW.media_id);
  INSERT INTO media_fts(rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids)
  SELECT rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids FROM v_media_fts_source WHERE id = NEW.media_id;
  DELETE FROM media_fts_cjk WHERE rowid = (SELECT rowid FROM media WHERE id = NEW.media_id);
  INSERT INTO media_fts_cjk(rowid, cjk)
  SELECT rowid, cjk FROM v_media_fts_source WHERE id = NEW.media_id;
END;
CREATE TRIGGER trg_media_person_fts_del AFTER DELETE ON media_person BEGIN
  DELETE FROM media_fts WHERE rowid = (SELECT rowid FROM media WHERE id = OLD.media_id);
  INSERT INTO media_fts(rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids)
  SELECT rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids FROM v_media_fts_source WHERE id = OLD.media_id;
  DELETE FROM media_fts_cjk WHERE rowid = (SELECT rowid FROM media WHERE id = OLD.media_id);
  INSERT INTO media_fts_cjk(rowid, cjk)
  SELECT rowid, cjk FROM v_media_fts_source WHERE id = OLD.media_id;
END;
CREATE TRIGGER trg_media_person_fts_upd AFTER UPDATE ON media_person BEGIN
  DELETE FROM media_fts WHERE rowid = (SELECT rowid FROM media WHERE id = NEW.media_id);
  INSERT INTO media_fts(rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids)
  SELECT rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids FROM v_media_fts_source WHERE id = NEW.media_id;
  DELETE FROM media_fts_cjk WHERE rowid = (SELECT rowid FROM media WHERE id = NEW.media_id);
  INSERT INTO media_fts_cjk(rowid, cjk)
  SELECT rowid, cjk FROM v_media_fts_source WHERE id = NEW.media_id;
END;

CREATE TRIGGER trg_media_genre_fts_ins AFTER INSERT ON media_genre BEGIN
  DELETE FROM media_fts WHERE rowid = (SELECT rowid FROM media WHERE id = NEW.media_id);
  INSERT INTO media_fts(rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids)
  SELECT rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids FROM v_media_fts_source WHERE id = NEW.media_id;
  DELETE FROM media_fts_cjk WHERE rowid = (SELECT rowid FROM media WHERE id = NEW.media_id);
  INSERT INTO media_fts_cjk(rowid, cjk)
  SELECT rowid, cjk FROM v_media_fts_source WHERE id = NEW.media_id;
END;
CREATE TRIGGER trg_media_genre_fts_del AFTER DELETE ON media_genre BEGIN
  DELETE FROM media_fts WHERE rowid = (SELECT rowid FROM media WHERE id = OLD.media_id);
  INSERT INTO media_fts(rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids)
  SELECT rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids FROM v_media_fts_source WHERE id = OLD.media_id;
  DELETE FROM media_fts_cjk WHERE rowid = (SELECT rowid FROM media WHERE id = OLD.media_id);
  INSERT INTO media_fts_cjk(rowid, cjk)
  SELECT rowid, cjk FROM v_media_fts_source WHERE id = OLD.media_id;
END;
CREATE TRIGGER trg_media_genre_fts_upd AFTER UPDATE ON media_genre BEGIN
  DELETE FROM media_fts WHERE rowid = (SELECT rowid FROM media WHERE id = NEW.media_id);
  INSERT INTO media_fts(rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids)
  SELECT rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids FROM v_media_fts_source WHERE id = NEW.media_id;
  DELETE FROM media_fts_cjk WHERE rowid = (SELECT rowid FROM media WHERE id = NEW.media_id);
  INSERT INTO media_fts_cjk(rowid, cjk)
  SELECT rowid, cjk FROM v_media_fts_source WHERE id = NEW.media_id;
END;

CREATE TRIGGER trg_media_tag_fts_ins AFTER INSERT ON media_tag BEGIN
  DELETE FROM media_fts WHERE rowid = (SELECT rowid FROM media WHERE id = NEW.media_id);
  INSERT INTO media_fts(rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids)
  SELECT rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids FROM v_media_fts_source WHERE id = NEW.media_id;
  DELETE FROM media_fts_cjk WHERE rowid = (SELECT rowid FROM media WHERE id = NEW.media_id);
  INSERT INTO media_fts_cjk(rowid, cjk)
  SELECT rowid, cjk FROM v_media_fts_source WHERE id = NEW.media_id;
END;
CREATE TRIGGER trg_media_tag_fts_del AFTER DELETE ON media_tag BEGIN
  DELETE FROM media_fts WHERE rowid = (SELECT rowid FROM media WHERE id = OLD.media_id);
  INSERT INTO media_fts(rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids)
  SELECT rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids FROM v_media_fts_source WHERE id = OLD.media_id;
  DELETE FROM media_fts_cjk WHERE rowid = (SELECT rowid FROM media WHERE id = OLD.media_id);
  INSERT INTO media_fts_cjk(rowid, cjk)
  SELECT rowid, cjk FROM v_media_fts_source WHERE id = OLD.media_id;
END;
CREATE TRIGGER trg_media_tag_fts_upd AFTER UPDATE ON media_tag BEGIN
  DELETE FROM media_fts WHERE rowid = (SELECT rowid FROM media WHERE id = NEW.media_id);
  INSERT INTO media_fts(rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids)
  SELECT rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids FROM v_media_fts_source WHERE id = NEW.media_id;
  DELETE FROM media_fts_cjk WHERE rowid = (SELECT rowid FROM media WHERE id = NEW.media_id);
  INSERT INTO media_fts_cjk(rowid, cjk)
  SELECT rowid, cjk FROM v_media_fts_source WHERE id = NEW.media_id;
END;

CREATE TRIGGER trg_review_fts_ins AFTER INSERT ON review BEGIN
  DELETE FROM media_fts WHERE rowid = (SELECT rowid FROM media WHERE id = NEW.media_id);
  INSERT INTO media_fts(rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids)
  SELECT rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids FROM v_media_fts_source WHERE id = NEW.media_id;
  DELETE FROM media_fts_cjk WHERE rowid = (SELECT rowid FROM media WHERE id = NEW.media_id);
  INSERT INTO media_fts_cjk(rowid, cjk)
  SELECT rowid, cjk FROM v_media_fts_source WHERE id = NEW.media_id;
END;
CREATE TRIGGER trg_review_fts_del AFTER DELETE ON review BEGIN
  DELETE FROM media_fts WHERE rowid = (SELECT rowid FROM media WHERE id = OLD.media_id);
  INSERT INTO media_fts(rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids)
  SELECT rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids FROM v_media_fts_source WHERE id = OLD.media_id;
  DELETE FROM media_fts_cjk WHERE rowid = (SELECT rowid FROM media WHERE id = OLD.media_id);
  INSERT INTO media_fts_cjk(rowid, cjk)
  SELECT rowid, cjk FROM v_media_fts_source WHERE id = OLD.media_id;
END;
CREATE TRIGGER trg_review_fts_upd AFTER UPDATE ON review BEGIN
  DELETE FROM media_fts WHERE rowid = (SELECT rowid FROM media WHERE id = NEW.media_id);
  INSERT INTO media_fts(rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids)
  SELECT rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids FROM v_media_fts_source WHERE id = NEW.media_id;
  DELETE FROM media_fts_cjk WHERE rowid = (SELECT rowid FROM media WHERE id = NEW.media_id);
  INSERT INTO media_fts_cjk(rowid, cjk)
  SELECT rowid, cjk FROM v_media_fts_source WHERE id = NEW.media_id;
END;

CREATE TRIGGER trg_ext_id_fts_ins AFTER INSERT ON media_external_id BEGIN
  DELETE FROM media_fts WHERE rowid = (SELECT rowid FROM media WHERE id = NEW.media_id);
  INSERT INTO media_fts(rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids)
  SELECT rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids FROM v_media_fts_source WHERE id = NEW.media_id;
  DELETE FROM media_fts_cjk WHERE rowid = (SELECT rowid FROM media WHERE id = NEW.media_id);
  INSERT INTO media_fts_cjk(rowid, cjk)
  SELECT rowid, cjk FROM v_media_fts_source WHERE id = NEW.media_id;
END;
CREATE TRIGGER trg_ext_id_fts_del AFTER DELETE ON media_external_id BEGIN
  DELETE FROM media_fts WHERE rowid = (SELECT rowid FROM media WHERE id = OLD.media_id);
  INSERT INTO media_fts(rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids)
  SELECT rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids FROM v_media_fts_source WHERE id = OLD.media_id;
  DELETE FROM media_fts_cjk WHERE rowid = (SELECT rowid FROM media WHERE id = OLD.media_id);
  INSERT INTO media_fts_cjk(rowid, cjk)
  SELECT rowid, cjk FROM v_media_fts_source WHERE id = OLD.media_id;
END;
CREATE TRIGGER trg_ext_id_fts_upd AFTER UPDATE ON media_external_id BEGIN
  DELETE FROM media_fts WHERE rowid = (SELECT rowid FROM media WHERE id = NEW.media_id);
  INSERT INTO media_fts(rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids)
  SELECT rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids FROM v_media_fts_source WHERE id = NEW.media_id;
  DELETE FROM media_fts_cjk WHERE rowid = (SELECT rowid FROM media WHERE id = NEW.media_id);
  INSERT INTO media_fts_cjk(rowid, cjk)
  SELECT rowid, cjk FROM v_media_fts_source WHERE id = NEW.media_id;
END;

-- Backfill existing media on upgrade (no-op on a fresh install).
INSERT INTO media_fts(rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids)
SELECT rowid, title, alt_titles, synopsis, people, genres, tags, notes, review, external_ids FROM v_media_fts_source;
INSERT INTO media_fts_cjk(rowid, cjk)
SELECT rowid, cjk FROM v_media_fts_source;
