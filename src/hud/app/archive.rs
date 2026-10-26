use crate::game::state::GameState;
use crate::history::History;
use crate::vrchat::log::{all_logs, read_lines};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub(super) struct Archive {
    pub(super) db: Option<History>,
    key: Option<(String, u64)>,
    next: u64,
    have: Option<(u64, u64)>,
    end: u64,
    live: bool,
}

pub(super) fn log_name(p: &Path) -> Option<String> {
    p.file_name().map(|n| n.to_string_lossy().into_owned())
}

impl Archive {
    pub(super) fn settle(&mut self, live: bool) {
        if std::mem::replace(&mut self.live, live) && !live {
            if let Some(db) = self.db.as_ref() {
                db.checkpoint();
            }
        }
    }

    pub(super) fn flush(
        &mut self,
        path: Option<&Path>,
        pass: u64,
        kept: &[(u64, String)],
        end: u64,
    ) {
        let Some(db) = self.db.as_mut() else {
            return;
        };
        let Some(name) = path.and_then(log_name) else {
            return;
        };
        if self
            .key
            .as_ref()
            .is_none_or(|(k, p)| *k != name || *p != pass)
        {
            self.have = None;
            self.next = 0;
            self.end = 0;
            self.key = Some((name.clone(), pass));
        }
        if kept.is_empty() && self.have.is_none() {
            if end > self.end {
                db.archive(&name, 0, &[], end);
                self.end = end;
            }
            return;
        }
        let (have, skew) = *self.have.get_or_insert_with(|| db.resume(&name, kept));
        let n = kept.len() as u64;
        let skip = have.saturating_sub(self.next).min(n);
        let first = (self.next + skip).saturating_sub(skew);
        self.next += n;
        if skip < n || end > self.end {
            db.archive(&name, first, &kept[skip as usize..], end);
            self.end = self.end.max(end);
        }
    }
}

pub(super) fn backfill(gs: &mut GameState, dir: Option<&Path>, mut db: Option<&mut History>) {
    let files = dir.map(all_logs).unwrap_or_default();
    let newest = files.last().and_then(|p| log_name(p));
    let mut logs: BTreeMap<String, Option<PathBuf>> = files
        .iter()
        .filter_map(|p| Some((log_name(p)?, Some(p.clone()))))
        .collect();
    if let Some(db) = db.as_deref() {
        for name in db.logs() {
            logs.entry(name).or_insert(None);
        }
    }
    for (name, path) in logs {
        if newest.as_ref() == Some(&name) {
            continue;
        }
        match path {
            Some(path) => {
                let mut kept = Vec::new();
                let mut end = 0;
                let read = read_lines(&path, |line| {
                    if let Some((ts, keep)) = gs.feed_kept(line) {
                        end = end.max(ts);
                        if let Some(at) = keep {
                            kept.push((ts, line[at..].to_string()));
                        }
                    }
                });
                if read.is_ok() {
                    if let Some(db) = db.as_deref_mut() {
                        let (have, skew) = db.resume(&name, &kept);
                        let have = (have as usize).min(kept.len());
                        let first = (have as u64).saturating_sub(skew);
                        db.archive(&name, first, &kept[have..], end);
                    }
                    gs.log_rotated();
                }
            }
            None => {
                if let Some(db) = db.as_deref() {
                    let end = db.replay(&name, |ts, msg| {
                        gs.feed_msg(ts, msg);
                    });
                    gs.seen(end);
                    gs.log_rotated();
                }
            }
        }
    }
}
