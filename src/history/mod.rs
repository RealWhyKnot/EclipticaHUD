use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

mod party;
mod usage;

mod schema;

use schema::{REOPENED, SCHEMA, TO_V2, TO_V3};

const BUSY_WAIT: Duration = Duration::from_secs(2);

const WAL_LIMIT: i64 = 1 << 20;

fn align(stored: &[(u64, String)], kept: &[(u64, String)]) -> u64 {
    let mut j = 0;
    for (i, row) in kept.iter().enumerate() {
        if j == stored.len() {
            return crate::num::to_u64(i);
        }
        if *row == stored[j] {
            j += 1;
        }
    }
    if j == stored.len() {
        return crate::num::to_u64(kept.len());
    }
    let last = stored.last().map_or(0, |r| r.0);
    crate::num::to_u64(kept.iter().take_while(|r| r.0 <= last).count())
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PartyRecord {
    pub at: u64,
    pub boss: String,
    pub progress: f32,
    pub secs: u64,
    pub down: bool,
    pub here: usize,
    pub rows: Vec<SavedRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SavedRow {
    pub name: String,
    pub you: bool,
    pub dealt: u64,
    pub taken: u64,
}

pub(crate) struct History {
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
    pub(crate) fn open(path: &Path) -> Option<History> {
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

    pub(crate) fn checkpoint(&self) -> bool {
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
    pub(crate) fn in_memory() -> Option<History> {
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

    pub(crate) fn stored(&self, name: &str) -> u64 {
        self.db
            .query_row(
                "SELECT COUNT(*) FROM lines JOIN logs ON logs.id = lines.log WHERE logs.name = ?1",
                [name],
                |r| r.get::<_, i64>(0),
            )
            .map_or(0, i64::cast_unsigned)
    }

    pub(crate) fn resume(&mut self, name: &str, kept: &[(u64, String)]) -> (u64, u64) {
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
                let have = crate::num::to_usize(stored + skew).min(kept.len());
                let unsure =
                    crate::num::to_u64(kept[have..].iter().take_while(|r| r.0 <= seen).count());
                let upto = stored + crate::num::to_u64(kept.len() - have);
                if self.try_realign(name, skew, (stored, upto)).is_ok() && unsure > 0 {
                    self.unsure = Some((name.to_string(), stored, stored + unsure));
                }
                skew
            }
            Some(Some(n)) => n.max(0).cast_unsigned(),
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
            params![name, skew.cast_signed()],
        )?;
        for table in ["usage_cursor", "usage_floor"] {
            tx.execute(
                &format!("UPDATE {table} SET n = ?2 + 1 WHERE name = ?1 AND n > ?3"),
                params![name, upto.cast_signed(), old.cast_signed()],
            )?;
        }
        tx.commit()
    }

    #[cfg(test)]
    pub(crate) fn forget_skew(&self) {
        let _ = self.db.execute("UPDATE logs SET skew = NULL", []);
    }

    pub(crate) fn archive(&mut self, name: &str, first: u64, lines: &[(u64, String)], end_ts: u64) {
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
                let m = if let Some(&m) = ids.get(msg) {
                    m
                } else {
                    add.execute([msg])?;
                    let m: i64 = find.query_row([msg], |r| r.get(0))?;
                    ids.insert(msg.clone(), m);
                    m
                };
                put.execute(params![
                    id,
                    (first + crate::num::to_u64(i)).cast_signed(),
                    (*ts).cast_signed(),
                    m
                ])?;
            }
            tx.execute(
                "UPDATE logs SET end_ts = MAX(end_ts, ?2) WHERE id = ?1 AND skew IS NOT NULL",
                params![id, end_ts.cast_signed()],
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
                    params![name, upto.cast_signed(), old.cast_signed()],
                )?;
            }
        }
        let done = tx.commit();
        if done.is_err() {
            self.ids.clear();
        }
        done
    }

    pub(crate) fn logs(&self) -> Vec<String> {
        let Ok(mut q) = self.db.prepare("SELECT name FROM logs ORDER BY name") else {
            return Vec::new();
        };
        q.query_map([], |r| r.get(0))
            .map_or_default(|rows| rows.flatten().collect())
    }

    pub(crate) fn replay(&self, name: &str, mut f: impl FnMut(u64, &str)) -> u64 {
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
                    f(ts.cast_unsigned(), msg);
                }
            }
        }
        self.db
            .query_row("SELECT end_ts FROM logs WHERE name = ?1", [name], |r| {
                r.get::<_, i64>(0)
            })
            .map_or(0, i64::cast_unsigned)
    }
}

fn party_head(r: &rusqlite::Row) -> rusqlite::Result<PartyRecord> {
    Ok(PartyRecord {
        at: r.get::<_, i64>(0)?.cast_unsigned(),
        boss: r.get(1)?,
        progress: crate::num::to_f32(r.get::<_, f64>(2)?),
        secs: r.get::<_, i64>(3)?.cast_unsigned(),
        down: r.get(4)?,
        here: crate::num::to_usize(r.get::<_, i64>(5)?),
        rows: Vec::new(),
    })
}

#[cfg(test)]
mod tests;
