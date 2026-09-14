use std::path::PathBuf;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
};

const KEY: &str = "config:discordactive";

pub fn db_path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")?;
    let path = PathBuf::from(base).join("VRCX").join("VRCX.sqlite3");
    path.exists().then_some(path)
}

pub fn presence_on_at(path: &std::path::Path) -> Option<bool> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    let value: String = conn
        .query_row("select value from configs where key = ?1", [KEY], |r| {
            r.get(0)
        })
        .ok()?;
    Some(value == "true")
}

pub fn presence_on() -> Option<bool> {
    presence_on_at(&db_path()?)
}

pub fn set_presence_at(path: &std::path::Path, on: bool) -> bool {
    let Ok(conn) = rusqlite::Connection::open(path) else {
        return false;
    };
    let _ = conn.busy_timeout(std::time::Duration::from_secs(2));
    conn.execute(
        "insert into configs (key, value) values (?1, ?2) on conflict(key) do update set value = excluded.value",
        [KEY, if on { "true" } else { "false" }],
    )
    .is_ok()
}

pub fn set_presence(on: bool) -> bool {
    db_path().is_some_and(|p| set_presence_at(&p, on))
}

fn image_path(pid: u32) -> Option<PathBuf> {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len);
        CloseHandle(h);
        (ok != 0).then(|| PathBuf::from(String::from_utf16_lossy(&buf[..len as usize])))
    }
}

pub fn restart() -> bool {
    let pids = crate::vr::process_ids("VRCX.exe");
    let Some(exe) = pids.iter().find_map(|p| image_path(*p)) else {
        return false;
    };
    let exe_dir = exe.parent().map(|d| d.to_path_buf());
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/IM", "VRCX.exe"])
        .output();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while crate::vr::process_running("VRCX.exe") && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    let mut cmd = std::process::Command::new(&exe);
    if let Some(dir) = exe_dir {
        cmd.current_dir(dir);
    }
    cmd.spawn().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_writes_the_presence_flag() {
        let dir = std::env::temp_dir().join(format!("ehud_vrcx_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("VRCX.sqlite3");
        assert_eq!(presence_on_at(&path), None);
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute(
            "create table configs (key text primary key, value text)",
            [],
        )
        .unwrap();
        conn.execute(
            "insert into configs values ('config:discordactive', 'true')",
            [],
        )
        .unwrap();
        drop(conn);
        assert_eq!(presence_on_at(&path), Some(true));
        assert!(set_presence_at(&path, false));
        assert_eq!(presence_on_at(&path), Some(false));
        assert!(set_presence_at(&path, true));
        assert_eq!(presence_on_at(&path), Some(true));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
