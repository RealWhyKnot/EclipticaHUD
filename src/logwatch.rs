use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

const SHARE_ALL: u32 = 0x1 | 0x2 | 0x4;
const CHUNK: usize = 65536;

pub fn log_dir() -> Option<PathBuf> {
    let profile = std::env::var_os("USERPROFILE")?;
    Some(Path::new(&profile).join("AppData/LocalLow/VRChat/VRChat"))
}

pub fn newest_log(dir: &Path) -> Option<PathBuf> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(dir).ok()? {
        let entry = entry.ok()?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("output_log_") || !name.ends_with(".txt") {
            continue;
        }
        let modified = entry.metadata().ok()?.modified().ok()?;
        if best.as_ref().is_none_or(|(t, _)| modified > *t) {
            best = Some((modified, entry.path()));
        }
    }
    best.map(|(_, p)| p)
}

pub fn open_shared(path: &Path) -> std::io::Result<File> {
    std::fs::OpenOptions::new()
        .read(true)
        .share_mode(SHARE_ALL)
        .open(path)
}

#[derive(Default)]
pub struct LogWatch {
    dir: Option<PathBuf>,
    pub path: Option<PathBuf>,
    file: Option<File>,
    offset: u64,
    carry: Vec<u8>,
}

impl LogWatch {
    pub fn new() -> Self {
        LogWatch {
            dir: log_dir(),
            ..Default::default()
        }
    }

    pub fn poll(&mut self, mut on_line: impl FnMut(&str)) {
        let Some(dir) = self.dir.as_deref() else {
            return;
        };
        let newest = newest_log(dir);
        if newest != self.path {
            self.file = newest.as_deref().and_then(|p| open_shared(p).ok());
            self.path = newest;
            self.offset = 0;
            self.carry.clear();
        }
        let Some(file) = self.file.as_mut() else {
            return;
        };
        let len = match file.metadata() {
            Ok(m) => m.len(),
            Err(_) => return,
        };
        if len < self.offset {
            self.offset = 0;
            self.carry.clear();
        }
        if len == self.offset || file.seek(SeekFrom::Start(self.offset)).is_err() {
            return;
        }
        let mut buf = [0u8; CHUNK];
        while self.offset < len {
            let n = match file.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            self.offset += n as u64;
            let mut data = &buf[..n];
            while let Some(nl) = data.iter().position(|&b| b == b'\n') {
                let (head, tail) = data.split_at(nl);
                data = &tail[1..];
                if self.carry.is_empty() {
                    emit(head, &mut on_line);
                } else {
                    self.carry.extend_from_slice(head);
                    let whole = std::mem::take(&mut self.carry);
                    emit(&whole, &mut on_line);
                }
            }
            self.carry.extend_from_slice(data);
        }
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
        LogWatch {
            dir: Some(dir.to_path_buf()),
            ..Default::default()
        }
    }

    fn collect(w: &mut LogWatch) -> Vec<String> {
        let mut out = Vec::new();
        w.poll(|l| out.push(l.to_string()));
        out
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
        assert_eq!(collect(&mut w), ["line one", "line two"]);

        fa.write_all("al done\n".as_bytes()).unwrap();
        fa.flush().unwrap();
        assert_eq!(collect(&mut w), ["partial done"]);
        assert_eq!(collect(&mut w), Vec::<String>::new());

        let log_b = dir.join("output_log_2026-01-02_00-00-00.txt");
        std::fs::write(&log_b, "fresh \u{1d04}\u{29c} log\n").unwrap();
        let future = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
        File::options()
            .write(true)
            .open(&log_b)
            .unwrap()
            .set_modified(future)
            .unwrap();
        assert_eq!(collect(&mut w), ["fresh \u{1d04}\u{29c} log"]);
        assert_eq!(w.path.as_deref(), Some(log_b.as_path()));

        drop(fa);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
