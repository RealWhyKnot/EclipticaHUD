use super::{History, PartyRecord, party_head};
use crate::game::event::Event;
use rusqlite::params;
use std::collections::HashMap;

impl History {
    pub(crate) fn usage_start(&mut self) {
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

    pub(crate) fn log_ends(&self) -> Vec<(String, u64)> {
        let Ok(mut q) = self
            .db
            .prepare("SELECT name, upto FROM log_ends ORDER BY name")
        else {
            return Vec::new();
        };
        q.query_map([], |r| Ok((r.get(0)?, r.get::<_, i64>(1)?.cast_unsigned())))
            .map_or_default(|rows| rows.flatten().collect())
    }

    pub(crate) fn usage_cursors(&self) -> HashMap<String, u64> {
        let Ok(mut q) = self.db.prepare("SELECT name, n FROM usage_cursor") else {
            return HashMap::new();
        };
        q.query_map([], |r| Ok((r.get(0)?, r.get::<_, i64>(1)?.cast_unsigned())))
            .map_or_default(|rows| rows.flatten().collect())
    }

    pub(crate) fn usage_floors(&self) -> HashMap<String, u64> {
        let Ok(mut q) = self.db.prepare("SELECT name, n FROM usage_floor") else {
            return HashMap::new();
        };
        q.query_map([], |r| Ok((r.get(0)?, r.get::<_, i64>(1)?.cast_unsigned())))
            .map_or_default(|rows| rows.flatten().collect())
    }

    pub(crate) fn snapshot<T>(&self, f: impl FnOnce() -> T) -> T {
        let began = self.db.execute_batch("BEGIN").is_ok();
        let out = f();
        if began && self.db.execute_batch("COMMIT").is_err() {
            let _ = self.db.execute_batch("ROLLBACK");
        }
        out
    }

    pub(crate) fn usage_commit(&mut self, cursors: &[(String, u64)]) -> bool {
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
                put.execute(params![name, (*n).cast_signed()])?;
            }
        }
        tx.commit()
    }

    pub(crate) fn party_saved_after(&self, seq: u64, limit: usize) -> Vec<(u64, PartyRecord)> {
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
            .query_map(params![seq.cast_signed(), crate::num::to_i64(limit)], |r| {
                Ok((r.get::<_, i64>(6)?.cast_unsigned(), party_head(r)?))
            })
            .map_or_default(|rows| rows.flatten().collect());
        heads
            .into_iter()
            .map(|(seq, head)| {
                let rows = self.party_rows(head.at, "slot");
                (seq, PartyRecord { rows, ..head })
            })
            .collect()
    }

    pub(crate) fn known_names(&self) -> (Vec<String>, Vec<String>) {
        let mut me = Vec::new();
        let mut names = Vec::new();
        if let Ok(mut q) = self.db.prepare(
            "SELECT text FROM msgs WHERE text LIKE 'User Authenticated: %' \
             OR text LIKE '[Behaviour] OnPlayerJoined %' OR text LIKE '[Behaviour] OnPlayerLeft %' \
             OR text LIKE 'ownership of % transferred to %'",
        ) && let Ok(rows) = q.query_map([], |r| r.get::<_, String>(0))
        {
            for msg in rows.flatten() {
                match crate::game::event::parse_msg(&msg) {
                    Some(Event::SelfName { name, .. }) => me.push(name),
                    Some(
                        Event::PlayerJoined { name, .. }
                        | Event::PlayerLeft { name, .. }
                        | Event::Ownership { player: name, .. },
                    ) => names.push(name),
                    _ => {}
                }
            }
        }
        if let Ok(mut q) = self.db.prepare("SELECT DISTINCT name, you FROM party_rows")
            && let Ok(rows) =
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
        me.sort();
        me.dedup();
        names.sort();
        names.dedup();
        names.retain(|n| me.binary_search(n).is_err());
        (me, names)
    }

    pub(crate) fn salt(&self) -> String {
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
}
