use super::{History, PartyRecord, SavedRow, party_head};
use rusqlite::{OptionalExtension, params};

impl History {
    pub(crate) fn save_party(&mut self, rec: &PartyRecord) {
        let _ = self.try_save_party(rec);
    }

    fn try_save_party(&mut self, rec: &PartyRecord) -> rusqlite::Result<()> {
        let tx = self.db.transaction()?;
        let at = rec.at.cast_signed();
        tx.execute(
            "INSERT OR REPLACE INTO party_fights (at, boss, progress, secs, down, here) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                at,
                rec.boss,
                f64::from(rec.progress),
                rec.secs.cast_signed(),
                rec.down,
                crate::num::to_i64(rec.here)
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
                    crate::num::to_i64(slot),
                    r.name,
                    r.you,
                    r.dealt.cast_signed(),
                    r.taken.cast_signed()
                ])?;
            }
        }
        tx.commit()
    }

    pub(super) fn party_rows(&self, at: u64, order: &str) -> Vec<SavedRow> {
        let Ok(mut q) = self.db.prepare(&format!(
            "SELECT name, you, dealt, taken FROM party_rows WHERE at = ?1 ORDER BY {order}"
        )) else {
            return Vec::new();
        };
        q.query_map([at.cast_signed()], |r| {
            Ok(SavedRow {
                name: r.get(0)?,
                you: r.get(1)?,
                dealt: r.get::<_, i64>(2)?.cast_unsigned(),
                taken: r.get::<_, i64>(3)?.cast_unsigned(),
            })
        })
        .map_or_default(|rows| rows.flatten().collect())
    }

    pub(crate) fn party_count(&self, skip: u64) -> usize {
        self.db
            .query_row(
                "SELECT COUNT(*) FROM party_fights WHERE at != ?1",
                [skip.cast_signed()],
                |r| r.get::<_, i64>(0),
            )
            .map_or(0, crate::num::to_usize)
    }

    pub(crate) fn party_page(&self, skip: u64, page: usize) -> Option<PartyRecord> {
        let head = self
            .db
            .query_row(
                "SELECT at, boss, progress, secs, down, here FROM party_fights \
                 WHERE at != ?1 ORDER BY at DESC LIMIT 1 OFFSET ?2",
                params![skip.cast_signed(), crate::num::to_i64(page)],
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
