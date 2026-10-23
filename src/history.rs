use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

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
PRAGMA user_version = 1;
";

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
}

impl History {
    pub fn open(path: &Path) -> Option<History> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let db = Connection::open(path).ok()?;
        let _ = db.busy_timeout(Duration::from_secs(2));
        let _ = db.pragma_update(None, "journal_mode", "WAL");
        let _ = db.pragma_update(None, "synchronous", "NORMAL");
        History::init(db)
    }

    #[cfg(test)]
    pub fn in_memory() -> Option<History> {
        History::init(Connection::open_in_memory().ok()?)
    }

    fn init(db: Connection) -> Option<History> {
        db.execute_batch(SCHEMA).ok()?;
        Some(History {
            db,
            ids: HashMap::new(),
        })
    }

    fn log_id(&self, name: &str) -> rusqlite::Result<i64> {
        self.db
            .execute("INSERT OR IGNORE INTO logs (name) VALUES (?1)", [name])?;
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
                "UPDATE logs SET end_ts = MAX(end_ts, ?2) WHERE id = ?1",
                params![id, end_ts as i64],
            )?;
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
             SELECT name, upto FROM log_ends WHERE true \
             ON CONFLICT (name) DO UPDATE SET n = MAX(n, excluded.n)",
            [],
        )?;
        tx.execute(
            "INSERT INTO usage_cursor (name, n) \
             SELECT '', COALESCE(MAX(at), 0) FROM party_fights WHERE true \
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

    pub fn party_after(&self, at: u64, limit: usize) -> Vec<PartyRecord> {
        let Ok(mut q) = self.db.prepare(
            "SELECT at, boss, progress, secs, down, here FROM party_fights \
             WHERE at > ?1 ORDER BY at LIMIT ?2",
        ) else {
            return Vec::new();
        };
        let heads: Vec<PartyRecord> = q
            .query_map(params![at as i64, limit as i64], party_head)
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default();
        heads
            .into_iter()
            .map(|head| PartyRecord {
                rows: self.party_rows(head.at, "slot"),
                ..head
            })
            .collect()
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
