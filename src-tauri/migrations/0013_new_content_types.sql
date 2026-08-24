-- MISSION-109: widen the content_type CHECK to include game, podcast, music,
-- comic. Uses PRAGMA writable_schema because a table rebuild would cascade
-- ON DELETE CASCADE through every child table (sqlx wraps migrations in
-- transactions where PRAGMA foreign_keys is a no-op).
--
-- This only modifies the SQL text stored in sqlite_master; existing rows are
-- untouched and already satisfy the wider constraint.
--
-- IMPORTANT: sqlx must NOT wrap this in a transaction (writable_schema is
-- a no-op inside one). The `-- no-transaction` directive below tells sqlx
-- 0.8+ to skip the implicit transaction for this migration.

-- no-transaction

PRAGMA writable_schema = ON;

UPDATE sqlite_master
SET sql = REPLACE(
  sql,
  '''book'',''novel'',''web_novel'',''manga'',''manhwa'',''manhua'',''anime'',''tv'',''movie'',''other''',
  '''book'',''novel'',''web_novel'',''manga'',''manhwa'',''manhua'',''anime'',''tv'',''movie'',''game'',''podcast'',''music'',''comic'',''other'''
)
WHERE type = 'table'
  AND name = 'media'
  AND sql LIKE '%content_type%';

PRAGMA writable_schema = OFF;

-- Verify: the schema now includes the new types.
SELECT COUNT(*) FROM sqlite_master
WHERE type = 'table'
  AND name = 'media'
  AND sql LIKE '%game%';
