use std::fs::File;
use std::io::{BufRead, Read, Seek, SeekFrom};
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const SHARE_ALL: u32 = 0x1 | 0x2 | 0x4;
const CHUNK: usize = 65536;
const RESCAN: Duration = Duration::from_secs(1);
const MAX_LINE: usize = 1 << 20;
const DRAINS: u8 = 5;

pub fn log_dir() -> Option<PathBuf> {
    let profile = std::env::var_os("USERPROFILE")?;
    Some(Path::new(&profile).join("AppData/LocalLow/VRChat/VRChat"))
}

pub fn all_logs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut logs: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.starts_with("output_log_") && name.ends_with(".txt")
        })
        .map(|e| e.path())
        .collect();
    logs.sort();
    logs
}

pub fn newest_log(dir: &Path) -> Option<PathBuf> {
    all_logs(dir).pop()
}

pub fn open_shared(path: &Path) -> std::io::Result<File> {
    std::fs::OpenOptions::new()
        .read(true)
        .share_mode(SHARE_ALL)
        .open(path)
}

pub fn read_lines(path: &Path, f: impl FnMut(&str)) -> std::io::Result<()> {
    read_lines_from(std::io::BufReader::new(open_shared(path)?), f)
}

fn read_lines_from(mut reader: impl BufRead, mut f: impl FnMut(&str)) -> std::io::Result<()> {
    let mut raw = Vec::new();
    while (&mut reader)
        .take(MAX_LINE as u64 + 1)
        .read_until(b'\n', &mut raw)?
        > 0
    {
        let line = raw.strip_suffix(b"\n").unwrap_or(&raw);
        if line.len() <= MAX_LINE {
            f(String::from_utf8_lossy(line).trim_end_matches(['\r', '\n']));
        } else {
            reader.skip_until(b'\n')?;
        }
        raw.clear();
    }
    Ok(())
}

fn file_id(file: &File) -> Option<(u32, u32, u32)> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    let ok = unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, &mut info) };
    (ok != 0).then_some((
        info.dwVolumeSerialNumber,
        info.nFileIndexHigh,
        info.nFileIndexLow,
    ))
}

#[derive(Default)]
pub struct LogWatch {
    dir: Option<PathBuf>,
    pub path: Option<PathBuf>,
    pub pass: u64,
    file: Option<File>,
    offset: u64,
    carry: Vec<u8>,
    skip: bool,
    drains: u8,
    scanned: Option<Instant>,
    rescan: Duration,
}

impl LogWatch {
    pub fn new() -> Self {
        LogWatch {
            dir: log_dir(),
            rescan: RESCAN,
            ..Default::default()
        }
    }

    #[cfg(test)]
    pub fn for_dir(dir: PathBuf) -> Self {
        LogWatch {
            dir: Some(dir),
            ..Default::default()
        }
    }

    pub fn poll(&mut self, mut on_line: impl FnMut(&str)) -> bool {
        let Some(dir) = self.dir.as_deref() else {
            return false;
        };
        let due = self.scanned.is_none_or(|t| t.elapsed() >= self.rescan);
        if due {
            self.scanned = Some(Instant::now());
            let newest = newest_log(dir);
            if newest.is_some() && (newest != self.path || !self.same_file()) {
                if self.drains < DRAINS && self.read_new(&mut on_line) {
                    self.drains += 1;
                    self.scanned = None;
                    return false;
                }
                let rotated = self.path.is_some();
                self.file = newest.as_deref().and_then(|p| open_shared(p).ok());
                self.path = newest;
                self.offset = 0;
                self.pass += 1;
                self.carry.clear();
                self.skip = false;
                self.drains = 0;
                if rotated {
                    return true;
                }
            }
        }
        self.read_new(&mut on_line);
        false
    }

    fn same_file(&self) -> bool {
        let (Some(path), Some(file)) = (self.path.as_deref(), self.file.as_ref()) else {
            return true;
        };
        match (
            open_shared(path).ok().and_then(|f| file_id(&f)),
            file_id(file),
        ) {
            (Some(now), Some(open)) => now == open,
            _ => true,
        }
    }

    fn read_new(&mut self, on_line: &mut impl FnMut(&str)) -> bool {
        let mut got = false;
        let Some(file) = self.file.as_mut() else {
            return false;
        };
        let len = match file.metadata() {
            Ok(m) => m.len(),
            Err(_) => return false,
        };
        if len < self.offset {
            self.offset = 0;
            self.pass += 1;
            self.carry.clear();
            self.skip = false;
        }
        if len == self.offset || file.seek(SeekFrom::Start(self.offset)).is_err() {
            return false;
        }
        let mut buf = [0u8; CHUNK];
        while self.offset < len {
            let n = match file.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            got = true;
            self.offset += n as u64;
            let mut data = &buf[..n];
            while let Some(nl) = data.iter().position(|&b| b == b'\n') {
                let (head, tail) = data.split_at(nl);
                data = &tail[1..];
                if std::mem::take(&mut self.skip) {
                    continue;
                }
                if self.carry.len() + head.len() > MAX_LINE {
                    self.carry = Vec::new();
                } else if self.carry.is_empty() {
                    emit(head, on_line);
                } else {
                    self.carry.extend_from_slice(head);
                    let whole = std::mem::take(&mut self.carry);
                    emit(&whole, on_line);
                }
            }
            if self.skip {
                continue;
            }
            if self.carry.len() + data.len() > MAX_LINE {
                self.carry = Vec::new();
                self.skip = true;
            } else {
                self.carry.extend_from_slice(data);
            }
        }
        got
    }
}

fn emit(bytes: &[u8], on_line: &mut impl FnMut(&str)) {
    let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => on_line(s),
        Err(_) => on_line(&String::from_utf8_lossy(bytes)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn watch_for(dir: &Path) -> LogWatch {
        LogWatch::for_dir(dir.to_path_buf())
    }

    fn collect(w: &mut LogWatch) -> (Vec<String>, bool) {
        let mut out = Vec::new();
        let rotated = w.poll(|l| out.push(l.to_string()));
        (out, rotated)
    }

    #[test]
    fn tail_partial_and_rotation() {
        let dir = std::env::temp_dir().join(format!("ehud_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log_a = dir.join("output_log_2026-01-01_00-00-00.txt");
        let mut fa = File::create(&log_a).unwrap();
        fa.write_all("line one\r\nline two\nparti".as_bytes())
            .unwrap();
        fa.flush().unwrap();

        let mut w = watch_for(&dir);
        let (lines, rotated) = collect(&mut w);
        assert_eq!(lines, ["line one", "line two"]);
        assert!(!rotated);

        fa.write_all("al done\n".as_bytes()).unwrap();
        fa.flush().unwrap();
        assert_eq!(collect(&mut w), (vec!["partial done".to_string()], false));
        assert_eq!(collect(&mut w), (Vec::new(), false));

        let log_b = dir.join("output_log_2026-01-02_00-00-00.txt");
        std::fs::write(&log_b, "fresh \u{1d04}\u{29c} log\n").unwrap();
        let future = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
        File::options()
            .write(true)
            .open(&log_b)
            .unwrap()
            .set_modified(future)
            .unwrap();
        assert_eq!(collect(&mut w), (Vec::new(), true));
        assert_eq!(
            collect(&mut w),
            (vec!["fresh \u{1d04}\u{29c} log".to_string()], false)
        );
        assert_eq!(w.path.as_deref(), Some(log_b.as_path()));
        assert_eq!(all_logs(&dir), [log_a.clone(), log_b.clone()]);

        drop(fa);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(collect(&mut w), (Vec::new(), false));
        assert_eq!(w.path.as_deref(), Some(log_b.as_path()));
    }

    #[test]
    fn newest_by_name_not_mtime() {
        let dir = std::env::temp_dir().join(format!("ehud_names_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let old = dir.join("output_log_2026-01-01_00-00-00.txt");
        let new = dir.join("output_log_2026-01-02_00-00-00.txt");
        std::fs::write(&new, "b\n").unwrap();
        std::fs::write(&old, "a\n").unwrap();
        let future = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
        File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_modified(future)
            .unwrap();
        assert_eq!(newest_log(&dir).as_deref(), Some(new.as_path()));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rescan_is_throttled() {
        let dir = std::env::temp_dir().join(format!("ehud_throttle_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("output_log_2026-01-01_00-00-00.txt"), "a\n").unwrap();
        let mut w = LogWatch {
            dir: Some(dir.clone()),
            rescan: Duration::from_secs(60),
            ..Default::default()
        };
        assert_eq!(collect(&mut w), (vec!["a".to_string()], false));
        std::fs::write(dir.join("output_log_2026-01-02_00-00-00.txt"), "b\n").unwrap();
        assert_eq!(collect(&mut w), (Vec::new(), false));
        w.scanned = None;
        assert_eq!(collect(&mut w), (Vec::new(), true));
        assert_eq!(collect(&mut w), (vec!["b".to_string()], false));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn append(path: &Path, bytes: &[u8]) {
        File::options()
            .append(true)
            .open(path)
            .unwrap()
            .write_all(bytes)
            .unwrap();
    }

    #[test]
    fn oversized_line_is_dropped_and_resyncs() {
        let dir = std::env::temp_dir().join(format!("ehud_long_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("output_log_2026-01-01_00-00-00.txt");
        let mut start = b"before\n".to_vec();
        start.extend(std::iter::repeat_n(b'x', MAX_LINE + 10));
        std::fs::write(&log, &start).unwrap();
        let mut w = watch_for(&dir);
        assert_eq!(collect(&mut w), (vec!["before".to_string()], false));
        assert!(w.skip && w.carry.capacity() == 0);
        for _ in 0..3 {
            append(&log, &vec![b'y'; CHUNK * 3]);
            assert_eq!(collect(&mut w), (Vec::new(), false));
            assert!(w.carry.capacity() == 0);
        }
        append(&log, b"end of junk\nafter\r\npart");
        assert_eq!(collect(&mut w), (vec!["after".to_string()], false));
        assert_eq!(w.carry, b"part");

        let mut edge = b"ial\n".to_vec();
        edge.extend(std::iter::repeat_n(b'a', MAX_LINE - 1));
        edge.extend(b"\r\n");
        edge.extend(std::iter::repeat_n(b'b', MAX_LINE + 1));
        edge.extend(b"\nlast\n");
        append(&log, &edge);
        let (lines, _) = collect(&mut w);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "partial");
        assert_eq!(lines[1].len(), MAX_LINE - 1);
        assert_eq!(lines[2], "last");
        assert!(!w.skip && w.carry.is_empty());

        append(&log, &vec![b'c'; MAX_LINE - 4]);
        assert_eq!(collect(&mut w), (Vec::new(), false));
        append(&log, b"dddd");
        assert_eq!(collect(&mut w), (Vec::new(), false));
        append(&log, b"e\nok\n");
        assert_eq!(collect(&mut w), (vec!["ok".to_string()], false));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn read_lines_drops_oversized_lines() {
        let mut data = b"a\r\n".to_vec();
        data.extend(std::iter::repeat_n(b'x', MAX_LINE + 1));
        data.extend(b"\nb\n");
        data.extend(std::iter::repeat_n(b'y', MAX_LINE));
        data.extend(b"\n");
        data.extend(std::iter::repeat_n(b'z', MAX_LINE * 2));
        data.extend(b"\nc\n");
        data.extend(std::iter::repeat_n(b'w', MAX_LINE + 5));
        let mut got = Vec::new();
        read_lines_from(std::io::Cursor::new(data), |l| got.push(l.to_string())).unwrap();
        assert_eq!(got.len(), 4);
        assert_eq!(got[..2], ["a", "b"]);
        assert_eq!(got[2].len(), MAX_LINE);
        assert_eq!(got[3], "c");
        let mut got = Vec::new();
        read_lines_from(std::io::Cursor::new(b"one\ntwo".to_vec()), |l| {
            got.push(l.to_string())
        })
        .unwrap();
        assert_eq!(got, ["one", "two"]);
    }

    fn lines(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn rotation_dir(name: &str) -> (PathBuf, PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("ehud_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let old = dir.join("output_log_2026-01-01_00-00-00.txt");
        let new = dir.join("output_log_2026-01-02_00-00-00.txt");
        (dir, old, new)
    }

    #[test]
    fn old_file_grows_after_the_new_one_appears() {
        let (dir, old, new) = rotation_dir("drain");
        std::fs::write(&old, "one\n").unwrap();
        let mut w = watch_for(&dir);
        assert_eq!(collect(&mut w), (lines(&["one"]), false));
        std::fs::write(&new, "fresh\n").unwrap();
        append(&old, b"boss down\n");
        assert_eq!(collect(&mut w), (lines(&["boss down"]), false));
        assert_eq!(w.path.as_deref(), Some(old.as_path()));
        append(&old, b"session end\n");
        assert_eq!(collect(&mut w), (lines(&["session end"]), false));
        assert_eq!(collect(&mut w), (Vec::new(), true));
        assert_eq!(w.path.as_deref(), Some(new.as_path()));
        append(&old, b"too late\n");
        assert_eq!(collect(&mut w), (lines(&["fresh"]), false));
        assert_eq!(collect(&mut w), (Vec::new(), false));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_busy_old_file_cannot_hold_the_switch() {
        let (dir, old, new) = rotation_dir("busy");
        std::fs::write(&old, "one\n").unwrap();
        let mut w = watch_for(&dir);
        collect(&mut w);
        std::fs::write(&new, "fresh\n").unwrap();
        for i in 0..DRAINS {
            append(&old, format!("still going {i}\n").as_bytes());
            assert_eq!(collect(&mut w), (vec![format!("still going {i}")], false));
        }
        append(&old, b"more\n");
        assert_eq!(collect(&mut w), (Vec::new(), true));
        assert_eq!(collect(&mut w), (lines(&["fresh"]), false));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rotation_mid_line() {
        let (dir, old, new) = rotation_dir("midline");
        std::fs::write(&old, "one\nboss ki").unwrap();
        let mut w = watch_for(&dir);
        assert_eq!(collect(&mut w), (lines(&["one"]), false));
        std::fs::write(&new, "fresh\n").unwrap();
        append(&old, b"lled\ncut of");
        assert_eq!(collect(&mut w), (lines(&["boss killed"]), false));
        assert_eq!(w.carry, b"cut of");
        assert_eq!(collect(&mut w), (Vec::new(), true));
        assert!(w.carry.is_empty());
        assert_eq!(collect(&mut w), (lines(&["fresh"]), false));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn replaced_in_place_is_read_from_the_start() {
        let (dir, old, _) = rotation_dir("replace");
        std::fs::write(&old, "first\n").unwrap();
        let mut w = watch_for(&dir);
        assert_eq!(collect(&mut w), (lines(&["first"]), false));
        let pass = w.pass;
        append(&old, b"last words\n");
        std::fs::remove_file(&old).unwrap();
        std::fs::write(&old, "brand new\n").unwrap();
        assert_eq!(collect(&mut w), (lines(&["last words"]), false));
        assert_eq!(collect(&mut w), (Vec::new(), true));
        assert_eq!(w.pass, pass + 1);
        assert_eq!(collect(&mut w), (lines(&["brand new"]), false));
        assert_eq!(collect(&mut w), (Vec::new(), false));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn truncated_file_is_read_again() {
        let (dir, old, _) = rotation_dir("truncate");
        std::fs::write(&old, "aaaa\nbbbb\npart").unwrap();
        let mut w = watch_for(&dir);
        assert_eq!(collect(&mut w), (lines(&["aaaa", "bbbb"]), false));
        let pass = w.pass;
        File::options()
            .write(true)
            .truncate(true)
            .open(&old)
            .unwrap()
            .write_all(b"c\n")
            .unwrap();
        assert_eq!(collect(&mut w), (lines(&["c"]), false));
        assert_eq!(w.pass, pass + 1);
        assert_eq!(w.path.as_deref(), Some(old.as_path()));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
