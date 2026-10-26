use crate::game::event::Event;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

const BUSY_WAIT: Duration = Duration::from_secs(2);

const WAL_LIMIT: i64 = 1 << 20;

const SCHEMA: &str = "
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

const TO_V2: &str = "
INSERT OR REPLACE INTO meta (key, value) SELECT 'party_floor', n FROM usage_cursor WHERE name = '';
INSERT INTO party_saves (at) SELECT at FROM party_fights ORDER BY at;
UPDATE usage_cursor SET n = COALESCE((SELECT MAX(seq) FROM party_saves WHERE at <= usage_cursor.n), 0)
    WHERE name = '';
UPDATE usage_cursor SET n = n + 1
    WHERE name <> '' AND name < (SELECT MAX(name) FROM logs)
    AND n >= COALESCE((SELECT upto FROM log_ends WHERE log_ends.name = usage_cursor.name), 0);
";

const TO_V3: &str = "
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

const REOPENED: &str = "
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

fn align(stored: &[(u64, String)], kept: &[(u64, String)]) -> u64 {
    let mut j = 0;
    for (i, row) in kept.iter().enumerate() {
        if j == stored.len() {
            return i as u64;
        }
        if *row == stored[j] {
            j += 1;
        }
    }
    if j == stored.len() {
        return kept.len() as u64;
    }
    let last = stored.last().map_or(0, |r| r.0);
    kept.iter().take_while(|r| r.0 <= last).count() as u64
}

#[derive(Debug, Clone, PartialEq)]
pub struct PartyRecord {
    pub at: u64,
    pub boss: String,
    pub progress: f32,
    pub secs: u64,
    pub down: bool,
    pub here: usize,
    pub rows: Vec<SavedRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedRow {
    pub name: String,
    pub you: bool,
    pub dealt: u64,
    pub taken: u64,
}

pub struct History {
    db: Connection,
    ids: HashMap<String, i64>,
    unsure: Option<(String, u64, u64)>,
}

impl Drop for History {
    fn drop(&mut self) {
        self.checkpoint();
    }
}

impl History {
    pub fn open(path: &Path) -> Option<History> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let db = Connection::open(path).ok()?;
        let _ = db.busy_timeout(BUSY_WAIT);
        let _ = db.pragma_update(None, "journal_mode", "WAL");
        let _ = db.pragma_update(None, "synchronous", "NORMAL");
        let _ = db.pragma_update(None, "journal_size_limit", WAL_LIMIT);
        History::init(db)
    }

    pub fn checkpoint(&self) -> bool {
        let _ = self.db.busy_timeout(Duration::ZERO);
        let done = self
            .db
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| {
                r.get::<_, i64>(0)
            })
            .is_ok_and(|busy| busy == 0);
        let _ = self.db.busy_timeout(BUSY_WAIT);
        done
    }

    #[cfg(test)]
    pub fn in_memory() -> Option<History> {
        History::init(Connection::open_in_memory().ok()?)
    }

    fn init(db: Connection) -> Option<History> {
        let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0)).ok()?;
        db.execute_batch(SCHEMA).ok()?;
        let marked: i64 = db
            .query_row(
                "SELECT COALESCE((SELECT CAST(value AS INTEGER) FROM meta WHERE key = 'schema'), 0)",
                [],
                |r| r.get(0),
            )
            .ok()?;
        if marked >= 3 && version < 3 {
            db.execute_batch(&format!(
                "BEGIN; {REOPENED} PRAGMA user_version = 3; COMMIT;"
            ))
            .ok()?;
        }
        if marked < 3 && version < 2 {
            db.execute_batch(&format!("BEGIN; {TO_V2} PRAGMA user_version = 2; COMMIT;"))
                .ok()?;
        }
        if marked < 3 && version < 3 {
            db.execute_batch(&format!("BEGIN; {TO_V3} PRAGMA user_version = 3; COMMIT;"))
                .ok()?;
        }
        db.execute(
            "INSERT OR IGNORE INTO meta (key, value) VALUES ('schema', '3')",
            [],
        )
        .ok()?;
        Some(History {
            db,
            ids: HashMap::new(),
            unsure: None,
        })
    }

    fn log_id(&self, name: &str) -> rusqlite::Result<i64> {
        self.db.execute(
            "INSERT OR IGNORE INTO logs (name, skew) VALUES (?1, 0)",
            [name],
        )?;
        self.db
            .query_row("SELECT id FROM logs WHERE name = ?1", [name], |r| r.get(0))
    }

    pub fn stored(&self, name: &str) -> u64 {
        self.db
            .query_row(
                "SELECT COUNT(*) FROM lines JOIN logs ON logs.id = lines.log WHERE logs.name = ?1",
                [name],
                |r| r.get::<_, i64>(0),
            )
            .map_or(0, |n| n as u64)
    }

    pub fn resume(&mut self, name: &str, kept: &[(u64, String)]) -> (u64, u64) {
        let stored = self.stored(name);
        let skew = self
            .db
            .query_row("SELECT skew FROM logs WHERE name = ?1", [name], |r| {
                r.get::<_, Option<i64>>(0)
            })
            .optional()
            .ok()
            .flatten();
        let skew = match skew {
            Some(None) => {
                let mut rows = Vec::new();
                let seen = self.replay(name, |ts, msg| rows.push((ts, msg.to_string())));
                let skew = align(&rows, kept).saturating_sub(stored);
                let have = ((stored + skew) as usize).min(kept.len());
                let unsure = kept[have..].iter().take_while(|r| r.0 <= seen).count() as u64;
                let upto = stored + (kept.len() - have) as u64;
                if self.try_realign(name, skew, (stored, upto)).is_ok() && unsure > 0 {
                    self.unsure = Some((name.to_string(), stored, stored + unsure));
                }
                skew
            }
            Some(Some(n)) => n.max(0) as u64,
            None => 0,
        };
        (stored + skew, skew)
    }

    fn try_realign(
        &mut self,
        name: &str,
        skew: u64,
        (old, upto): (u64, u64),
    ) -> rusqlite::Result<()> {
        let tx = self.db.transaction()?;
        tx.execute(
            "UPDATE logs SET skew = ?2 WHERE name = ?1",
            params![name, skew as i64],
        )?;
        for table in ["usage_cursor", "usage_floor"] {
            tx.execute(
                &format!("UPDATE {table} SET n = ?2 + 1 WHERE name = ?1 AND n > ?3"),
                params![name, upto as i64, old as i64],
            )?;
        }
        tx.commit()
    }

    #[cfg(test)]
    pub fn forget_skew(&self) {
        let _ = self.db.execute("UPDATE logs SET skew = NULL", []);
    }

    pub fn archive(&mut self, name: &str, first: u64, lines: &[(u64, String)], end_ts: u64) {
        let _ = self.try_archive(name, first, lines, end_ts);
    }

    fn try_archive(
        &mut self,
        name: &str,
        first: u64,
        lines: &[(u64, String)],
        end_ts: u64,
    ) -> rusqlite::Result<()> {
        let id = self.log_id(name)?;
        let ids = &mut self.ids;
        let tx = self.db.transaction()?;
        let count = |tx: &rusqlite::Transaction| {
            tx.query_row("SELECT COUNT(*) FROM lines WHERE log = ?1", [id], |r| {
                r.get::<_, i64>(0)
            })
        };
        let before = count(&tx)?;
        {
            let mut put = tx.prepare_cached(
                "INSERT OR IGNORE INTO lines (log, n, ts, msg) VALUES (?1, ?2, ?3, ?4)",
            )?;
            let mut add = tx.prepare_cached("INSERT OR IGNORE INTO msgs (text) VALUES (?1)")?;
            let mut find = tx.prepare_cached("SELECT id FROM msgs WHERE text = ?1")?;
            for (i, (ts, msg)) in lines.iter().enumerate() {
                let m = match ids.get(msg) {
                    Some(&m) => m,
                    None => {
                        add.execute([msg])?;
                        let m: i64 = find.query_row([msg], |r| r.get(0))?;
                        ids.insert(msg.clone(), m);
                        m
                    }
                };
                put.execute(params![id, (first + i as u64) as i64, *ts as i64, m])?;
            }
            tx.execute(
                "UPDATE logs SET end_ts = MAX(end_ts, ?2) WHERE id = ?1 AND skew IS NOT NULL",
                params![id, end_ts as i64],
            )?;
        }
        let after = count(&tx)?;
        if after > before {
            tx.execute(
                "UPDATE usage_cursor SET n = ?3 + 1 WHERE name = ?1 AND n = ?2 + 1 \
                 AND EXISTS (SELECT 1 FROM usage_floor WHERE name = ?1 AND n = ?2 + 1)",
                params![name, before, after],
            )?;
            tx.execute(
                "UPDATE usage_floor SET n = ?3 + 1 WHERE name = ?1 AND n = ?2 + 1",
                params![name, before, after],
            )?;
            tx.execute(
                "UPDATE usage_cursor SET n = ?2 WHERE name = ?1 AND n = ?2 + 1",
                params![name, before],
            )?;
        }
        if let Some((_, old, upto)) = self.unsure.take_if(|u| u.0 == name) {
            for table in ["usage_cursor", "usage_floor"] {
                tx.execute(
                    &format!("UPDATE {table} SET n = ?2 WHERE name = ?1 AND n = ?3"),
                    params![name, upto as i64, old as i64],
                )?;
            }
        }
        let done = tx.commit();
        if done.is_err() {
            self.ids.clear();
        }
        done
    }

    pub fn logs(&self) -> Vec<String> {
        let Ok(mut q) = self.db.prepare("SELECT name FROM logs ORDER BY name") else {
            return Vec::new();
        };
        q.query_map([], |r| r.get(0))
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default()
    }

    pub fn replay(&self, name: &str, mut f: impl FnMut(u64, &str)) -> u64 {
        let Ok(mut q) = self.db.prepare(
            "SELECT lines.ts, msgs.text FROM lines \
             JOIN logs ON logs.id = lines.log JOIN msgs ON msgs.id = lines.msg \
             WHERE logs.name = ?1 ORDER BY lines.n",
        ) else {
            return 0;
        };
        if let Ok(mut rows) = q.query([name]) {
            while let Ok(Some(row)) = rows.next() {
                let ts: i64 = row.get(0).unwrap_or(0);
                if let Ok(msg) = row.get_ref(1).and_then(|v| v.as_str().map_err(Into::into)) {
                    f(ts as u64, msg);
                }
            }
        }
        self.db
            .query_row("SELECT end_ts FROM logs WHERE name = ?1", [name], |r| {
                r.get::<_, i64>(0)
            })
            .map_or(0, |t| t as u64)
    }

    pub fn save_party(&mut self, rec: &PartyRecord) {
        let _ = self.try_save_party(rec);
    }

    fn try_save_party(&mut self, rec: &PartyRecord) -> rusqlite::Result<()> {
        let tx = self.db.transaction()?;
        let at = rec.at as i64;
        tx.execute(
            "INSERT OR REPLACE INTO party_fights (at, boss, progress, secs, down, here) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                at,
                rec.boss,
                rec.progress as f64,
                rec.secs as i64,
                rec.down,
                rec.here as i64
            ],
        )?;
        tx.execute("INSERT INTO party_saves (at) VALUES (?1)", [at])?;
        tx.execute("DELETE FROM party_rows WHERE at = ?1", [at])?;
        {
            let mut put = tx.prepare_cached(
                "INSERT INTO party_rows (at, slot, name, you, dealt, taken) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for (slot, r) in rec.rows.iter().enumerate() {
                put.execute(params![
                    at,
                    slot as i64,
                    r.name,
                    r.you,
                    r.dealt as i64,
                    r.taken as i64
                ])?;
            }
        }
        tx.commit()
    }

    pub fn usage_start(&mut self) {
        let _ = self.try_usage_start();
    }

    fn try_usage_start(&mut self) -> rusqlite::Result<()> {
        let tx = self.db.transaction()?;
        tx.execute(
            "INSERT INTO usage_cursor (name, n) \
             SELECT name, upto + (name < (SELECT MAX(name) FROM logs)) FROM log_ends WHERE true \
             ON CONFLICT (name) DO UPDATE SET n = MAX(n, excluded.n)",
            [],
        )?;
        tx.execute(
            "INSERT INTO usage_cursor (name, n) \
             SELECT '#party', COALESCE(MAX(seq), 0) FROM party_saves WHERE true \
             ON CONFLICT (name) DO UPDATE SET n = MAX(n, excluded.n)",
            [],
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO usage_floor (name, n) \
             SELECT name, n FROM usage_cursor WHERE name NOT IN ('', '#party')",
            [],
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO meta (key, value) \
             SELECT 'party_floor', COALESCE(MAX(end_ts), 0) FROM logs",
            [],
        )?;
        tx.execute(
            "INSERT INTO usage_cursor (name, n) \
             SELECT '', MAX(COALESCE((SELECT MAX(at) FROM party_fights), 0), \
                 COALESCE((SELECT MAX(end_ts) FROM logs), 0)) WHERE true \
             ON CONFLICT (name) DO UPDATE SET n = MAX(n, excluded.n)",
            [],
        )?;
        tx.commit()
    }

    pub fn log_ends(&self) -> Vec<(String, u64)> {
        let Ok(mut q) = self
            .db
            .prepare("SELECT name, upto FROM log_ends ORDER BY name")
        else {
            return Vec::new();
        };
        q.query_map([], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as u64)))
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default()
    }

    pub fn usage_cursors(&self) -> HashMap<String, u64> {
        let Ok(mut q) = self.db.prepare("SELECT name, n FROM usage_cursor") else {
            return HashMap::new();
        };
        q.query_map([], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as u64)))
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default()
    }

    pub fn usage_floors(&self) -> HashMap<String, u64> {
        let Ok(mut q) = self.db.prepare("SELECT name, n FROM usage_floor") else {
            return HashMap::new();
        };
        q.query_map([], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as u64)))
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default()
    }

    pub fn snapshot<T>(&self, f: impl FnOnce() -> T) -> T {
        let began = self.db.execute_batch("BEGIN").is_ok();
        let out = f();
        if began && self.db.execute_batch("COMMIT").is_err() {
            let _ = self.db.execute_batch("ROLLBACK");
        }
        out
    }

    pub fn usage_commit(&mut self, cursors: &[(String, u64)]) -> bool {
        self.try_usage_commit(cursors).is_ok()
    }

    fn try_usage_commit(&mut self, cursors: &[(String, u64)]) -> rusqlite::Result<()> {
        let tx = self.db.transaction()?;
        {
            let mut put = tx.prepare_cached(
                "INSERT INTO usage_cursor (name, n) VALUES (?1, ?2) \
                 ON CONFLICT (name) DO UPDATE SET n = MAX(n, excluded.n)",
            )?;
            for (name, n) in cursors {
                put.execute(params![name, *n as i64])?;
            }
        }
        tx.commit()
    }

    pub fn party_saved_after(&self, seq: u64, limit: usize) -> Vec<(u64, PartyRecord)> {
        let Ok(mut q) = self.db.prepare(
            "SELECT party_fights.at, boss, progress, secs, down, here, last FROM party_fights \
             JOIN (SELECT at, MAX(seq) AS last FROM party_saves GROUP BY at) AS s \
             ON s.at = party_fights.at WHERE last > ?1 AND party_fights.at > COALESCE( \
             (SELECT CAST(value AS INTEGER) FROM meta WHERE key = 'party_floor'), 0) \
             ORDER BY last LIMIT ?2",
        ) else {
            return Vec::new();
        };
        let heads: Vec<(u64, PartyRecord)> = q
            .query_map(params![seq as i64, limit as i64], |r| {
                Ok((r.get::<_, i64>(6)? as u64, party_head(r)?))
            })
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default();
        heads
            .into_iter()
            .map(|(seq, head)| {
                let rows = self.party_rows(head.at, "slot");
                (seq, PartyRecord { rows, ..head })
            })
            .collect()
    }

    pub fn known_names(&self) -> (Vec<String>, Vec<String>) {
        let mut me = Vec::new();
        let mut names = Vec::new();
        if let Ok(mut q) = self.db.prepare(
            "SELECT text FROM msgs WHERE text LIKE 'User Authenticated: %' \
             OR text LIKE '[Behaviour] OnPlayerJoined %' OR text LIKE '[Behaviour] OnPlayerLeft %' \
             OR text LIKE 'ownership of % transferred to %'",
        ) {
            if let Ok(rows) = q.query_map([], |r| r.get::<_, String>(0)) {
                for msg in rows.flatten() {
                    match crate::game::event::parse_msg(&msg) {
                        Some(Event::SelfName { name, .. }) => me.push(name),
                        Some(Event::PlayerJoined { name, .. })
                        | Some(Event::PlayerLeft { name, .. })
                        | Some(Event::Ownership { player: name, .. }) => names.push(name),
                        _ => {}
                    }
                }
            }
        }
        if let Ok(mut q) = self.db.prepare("SELECT DISTINCT name, you FROM party_rows") {
            if let Ok(rows) =
                q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?)))
            {
                for (name, you) in rows.flatten() {
                    if you {
                        me.push(name);
                    } else {
                        names.push(name);
                    }
                }
            }
        }
        me.sort();
        me.dedup();
        names.sort();
        names.dedup();
        names.retain(|n| me.binary_search(n).is_err());
        (me, names)
    }

    pub fn salt(&self) -> String {
        let found = self
            .db
            .query_row("SELECT value FROM meta WHERE key = 'salt'", [], |r| {
                r.get::<_, String>(0)
            })
            .ok();
        if let Some(salt) = found {
            return salt;
        }
        let salt = format!("{:016x}{:016x}", crate::relay::rand(), crate::relay::rand());
        let _ = self.db.execute(
            "INSERT OR IGNORE INTO meta (key, value) VALUES ('salt', ?1)",
            [&salt],
        );
        self.db
            .query_row("SELECT value FROM meta WHERE key = 'salt'", [], |r| {
                r.get(0)
            })
            .unwrap_or(salt)
    }

    fn party_rows(&self, at: u64, order: &str) -> Vec<SavedRow> {
        let Ok(mut q) = self.db.prepare(&format!(
            "SELECT name, you, dealt, taken FROM party_rows WHERE at = ?1 ORDER BY {order}"
        )) else {
            return Vec::new();
        };
        q.query_map([at as i64], |r| {
            Ok(SavedRow {
                name: r.get(0)?,
                you: r.get(1)?,
                dealt: r.get::<_, i64>(2)? as u64,
                taken: r.get::<_, i64>(3)? as u64,
            })
        })
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
    }

    pub fn party_count(&self, skip: u64) -> usize {
        self.db
            .query_row(
                "SELECT COUNT(*) FROM party_fights WHERE at != ?1",
                [skip as i64],
                |r| r.get::<_, i64>(0),
            )
            .map_or(0, |n| n as usize)
    }

    pub fn party_page(&self, skip: u64, page: usize) -> Option<PartyRecord> {
        let head = self
            .db
            .query_row(
                "SELECT at, boss, progress, secs, down, here FROM party_fights \
                 WHERE at != ?1 ORDER BY at DESC LIMIT 1 OFFSET ?2",
                params![skip as i64, page as i64],
                party_head,
            )
            .optional()
            .ok()??;
        Some(PartyRecord {
            rows: self.party_rows(head.at, "dealt DESC, slot"),
            ..head
        })
    }
}

fn party_head(r: &rusqlite::Row) -> rusqlite::Result<PartyRecord> {
    Ok(PartyRecord {
        at: r.get::<_, i64>(0)? as u64,
        boss: r.get(1)?,
        progress: r.get::<_, f64>(2)? as f32,
        secs: r.get::<_, i64>(3)? as u64,
        down: r.get(4)?,
        here: r.get::<_, i64>(5)? as usize,
        rows: Vec::new(),
    })
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
