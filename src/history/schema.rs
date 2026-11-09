pub(super) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS logs (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    end_ts INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS msgs (
    id INTEGER PRIMARY KEY,
    text TEXT NOT NULL UNIQUE
);
CREATE TABLE IF NOT EXISTS lines (
    log INTEGER NOT NULL,
    n INTEGER NOT NULL,
    ts INTEGER NOT NULL,
    msg INTEGER NOT NULL,
    PRIMARY KEY (log, n)
) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS party_fights (
    at INTEGER PRIMARY KEY,
    boss TEXT NOT NULL,
    progress REAL NOT NULL,
    secs INTEGER NOT NULL,
    down INTEGER NOT NULL,
    here INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS party_rows (
    at INTEGER NOT NULL,
    slot INTEGER NOT NULL,
    name TEXT NOT NULL,
    you INTEGER NOT NULL,
    dealt INTEGER NOT NULL,
    taken INTEGER NOT NULL,
    PRIMARY KEY (at, slot)
) WITHOUT ROWID;
CREATE VIEW IF NOT EXISTS log_ends AS
    SELECT name, COALESCE((SELECT MAX(n) + 1 FROM lines WHERE lines.log = logs.id), 0) AS upto
    FROM logs;
CREATE TABLE IF NOT EXISTS usage_cursor (
    name TEXT PRIMARY KEY,
    n INTEGER NOT NULL
) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS party_saves (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
) WITHOUT ROWID;
";

pub(super) const TO_V2: &str = "
INSERT OR REPLACE INTO meta (key, value) SELECT 'party_floor', n FROM usage_cursor WHERE name = '';
INSERT INTO party_saves (at) SELECT at FROM party_fights ORDER BY at;
UPDATE usage_cursor SET n = COALESCE((SELECT MAX(seq) FROM party_saves WHERE at <= usage_cursor.n), 0)
    WHERE name = '';
UPDATE usage_cursor SET n = n + 1
    WHERE name <> '' AND name < (SELECT MAX(name) FROM logs)
    AND n >= COALESCE((SELECT upto FROM log_ends WHERE log_ends.name = usage_cursor.name), 0);
";

pub(super) const TO_V3: &str = "
ALTER TABLE logs ADD COLUMN skew INTEGER;
CREATE TABLE IF NOT EXISTS usage_floor (
    name TEXT PRIMARY KEY,
    n INTEGER NOT NULL
) WITHOUT ROWID;
INSERT OR REPLACE INTO usage_floor (name, n) SELECT name, n FROM usage_cursor WHERE name <> '';
INSERT OR IGNORE INTO meta (key, value)
    SELECT 'party_floor', COALESCE((SELECT at FROM party_saves WHERE seq = usage_cursor.n), 0)
    FROM usage_cursor WHERE name = '';
INSERT OR REPLACE INTO usage_cursor (name, n) SELECT '#party', n FROM usage_cursor WHERE name = '';
UPDATE usage_cursor
    SET n = COALESCE((SELECT CAST(value AS INTEGER) FROM meta WHERE key = 'party_floor'), 0)
    WHERE name = '';
";

pub(super) const REOPENED: &str = "
UPDATE logs SET skew = NULL;
INSERT INTO party_saves (at)
    SELECT at FROM party_fights WHERE at NOT IN (SELECT at FROM party_saves) ORDER BY at;
UPDATE usage_cursor SET n = n + 1
    WHERE name NOT IN ('', '#party') AND name < (SELECT MAX(name) FROM logs)
    AND n >= COALESCE((SELECT upto FROM log_ends WHERE log_ends.name = usage_cursor.name), 0)
    AND NOT EXISTS (SELECT 1 FROM usage_cursor WHERE name = '#party');
INSERT OR IGNORE INTO usage_floor (name, n)
    SELECT name, n FROM usage_cursor WHERE name NOT IN ('', '#party');
INSERT OR IGNORE INTO usage_cursor (name, n)
    SELECT '#party', COALESCE((SELECT MAX(seq) FROM party_saves WHERE at <= usage_cursor.n), 0)
    FROM usage_cursor WHERE name = '';
INSERT OR REPLACE INTO meta (key, value)
    SELECT 'party_floor',
        MAX(n, COALESCE((SELECT CAST(value AS INTEGER) FROM meta WHERE key = 'party_floor'), 0))
    FROM usage_cursor WHERE name = '';
";
