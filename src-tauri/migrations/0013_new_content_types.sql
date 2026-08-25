-- MISSION-109: widen content_type CHECK via writable_schema.
-- The old text is replaced inside sqlite_master; no table rebuild needed.

PRAGMA writable_schema = ON;

UPDATE sqlite_master SET sql =
  replace(
    replace(
      replace(
        replace(
          sql,
          ',''movie'',''other''',
          ',''movie'',''game'',''podcast'',''music'',''comic'',''other'''
        ),
        'x',
        'x'
      ),
      'x',
      'x'
    ),
    'x',
    'x'
  )
WHERE type = 'table'
  AND name = 'media';

PRAGMA writable_schema = OFF;

-- Force schema reload
SELECT count(*) FROM sqlite_master;
