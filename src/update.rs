use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

pub const VERSION: &str = match option_env!("EHUD_VERSION") {
    Some(v) => v,
    None => "dev",
};
const REPO: &str = "RealWhyKnot/EclipticaHUD";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct UpdateInfo {
    pub tag: String,
    url: String,
    digest: String,
}

enum Status {
    Idle,
    Available(UpdateInfo),
    Installing,
    Restart,
    Failed,
}

static STATUS: Mutex<Status> = Mutex::new(Status::Idle);

#[derive(Clone, PartialEq, Default)]
pub enum Badge {
    #[default]
    None,
    Ready(String),
    Installing,
    Failed,
}

pub fn badge() -> Badge {
    match &*STATUS.lock().unwrap() {
        Status::Idle => Badge::None,
        Status::Available(i) => Badge::Ready(i.tag.clone()),
        Status::Installing | Status::Restart => Badge::Installing,
        Status::Failed => Badge::Failed,
    }
}

pub fn restart_pending() -> bool {
    matches!(*STATUS.lock().unwrap(), Status::Restart)
}

pub fn cleanup_old() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(exe.with_extension("exe.old"));
    }
}

pub fn spawn_check() {
    if parse_version(VERSION).is_none() {
        return;
    }
    std::thread::spawn(|| {
        if let Some(info) = check() {
            *STATUS.lock().unwrap() = Status::Available(info);
        }
    });
}

pub fn spawn_install() {
    let info = {
        let mut s = STATUS.lock().unwrap();
        match std::mem::replace(&mut *s, Status::Installing) {
            Status::Available(i) => i,
            other => {
                *s = other;
                return;
            }
        }
    };
    std::thread::spawn(move || {
        let done = install(&info);
        *STATUS.lock().unwrap() = if done {
            Status::Restart
        } else {
            Status::Failed
        };
    });
}

pub fn relaunch() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = Command::new(exe).spawn();
    }
}

fn check() -> Option<UpdateInfo> {
    let own = parse_version(VERSION)?;
    let body = run_capture(
        "curl.exe",
        &[
            "-s",
            "--max-time",
            "10",
            "-H",
            "Accept: application/vnd.github+json",
            "-A",
            concat!("EclipticaHUD/", env!("CARGO_PKG_VERSION")),
            &format!("https://api.github.com/repos/{REPO}/releases/latest"),
        ],
    )?;
    let tag = json_str(&body, "tag_name")?;
    if parse_version(&tag)? <= own {
        return None;
    }
    let (url, digest) = pick_exe_asset(&body)?;
    Some(UpdateInfo { tag, url, digest })
}

fn install(info: &UpdateInfo) -> bool {
    let tmp = std::env::temp_dir().join(format!("ecliptica-hud-{}.exe", info.tag));
    let fetched = sys_cmd("curl.exe")
        .args(["-fsL", "--max-time", "600", "-o"])
        .arg(&tmp)
        .arg(&info.url)
        .status()
        .is_ok_and(|s| s.success());
    if !fetched || sha256(&tmp).as_deref() != Some(&info.digest.to_ascii_lowercase()) {
        let _ = std::fs::remove_file(&tmp);
        return false;
    }
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let old = exe.with_extension("exe.old");
    let _ = std::fs::remove_file(&old);
    if std::fs::rename(&exe, &old).is_err() {
        return false;
    }
    if std::fs::copy(&tmp, &exe).is_err() {
        let _ = std::fs::rename(&old, &exe);
        return false;
    }
    let _ = std::fs::remove_file(&tmp);
    true
}

fn sys_cmd(exe: &str) -> Command {
    let path = match std::env::var_os("SystemRoot") {
        Some(root) => PathBuf::from(root).join("System32").join(exe),
        None => PathBuf::from(exe),
    };
    let mut cmd = Command::new(path);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

fn run_capture(exe: &str, args: &[&str]) -> Option<String> {
    let out = sys_cmd(exe).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn sha256(path: &Path) -> Option<String> {
    let out = sys_cmd("certutil.exe")
        .arg("-hashfile")
        .arg(path)
        .arg("SHA256")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .find(|t| t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit()))
        .map(str::to_ascii_lowercase)
}

fn parse_version(s: &str) -> Option<[u64; 4]> {
    let s = s.trim_start_matches(['v', 'V']);
    let s = s.split('-').next()?;
    let mut it = s.split('.').map(|p| p.parse().ok());
    let v = [it.next()??, it.next()??, it.next()??, it.next()??];
    if it.next().is_some() {
        return None;
    }
    Some(v)
}

fn json_str(body: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\":");
    let i = body.find(&pat)? + pat.len();
    let rest = body[i..].trim_start().strip_prefix('"')?;
    Some(rest[..rest.find('"')?].to_string())
}

fn pick_exe_asset(body: &str) -> Option<(String, String)> {
    let mut digest: Option<String> = None;
    let mut pos = 0;
    loop {
        let d = body[pos..].find("\"digest\":");
        let u = body[pos..].find("\"browser_download_url\":");
        match (d, u) {
            (Some(di), ui) if ui.is_none_or(|ui| di < ui) => {
                digest = json_str(&body[pos + di..], "digest");
                pos += di + 9;
            }
            (_, Some(ui)) => {
                let url = json_str(&body[pos + ui..], "browser_download_url")?;
                if url.ends_with(".exe") {
                    let sha = digest.take()?.strip_prefix("sha256:")?.to_string();
                    return Some((url, sha));
                }
                digest = None;
                pos += ui + 23;
            }
            _ => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_parsing() {
        assert_eq!(parse_version("v2026.9.8.1"), Some([2026, 9, 8, 1]));
        assert_eq!(
            parse_version("2026.12.31.10-beta"),
            Some([2026, 12, 31, 10])
        );
        assert_eq!(parse_version("dev"), None);
        assert_eq!(parse_version("v1.2.3"), None);
        assert_eq!(parse_version("v1.2.3.4.5"), None);
        assert!(parse_version("v2026.9.9.0").unwrap() > parse_version("v2026.9.8.11").unwrap());
    }

    #[test]
    fn asset_picking() {
        let body = concat!(
            "{\"tag_name\":\"v2026.9.9.1\",\"assets\":[",
            "{\"name\":\"a.sha256\",\"digest\":\"sha256:", "aa", "\",",
            "\"browser_download_url\":\"https://x/a.sha256\"},",
            "{\"name\":\"ecliptica-hud-2026.9.9.1.exe\",",
            "\"digest\":\"sha256:AB12ab12AB12ab12AB12ab12AB12ab12AB12ab12AB12ab12AB12ab12AB12ab12\",",
            "\"browser_download_url\":\"https://x/ecliptica-hud-2026.9.9.1.exe\"}]}"
        );
        assert_eq!(json_str(body, "tag_name").as_deref(), Some("v2026.9.9.1"));
        let (url, sha) = pick_exe_asset(body).unwrap();
        assert_eq!(url, "https://x/ecliptica-hud-2026.9.9.1.exe");
        assert_eq!(
            sha,
            "AB12ab12AB12ab12AB12ab12AB12ab12AB12ab12AB12ab12AB12ab12AB12ab12"
        );
    }

    #[test]
    fn asset_picking_refuses_without_digest() {
        let body = concat!(
            "{\"assets\":[{\"name\":\"e.exe\",\"digest\":null,",
            "\"browser_download_url\":\"https://x/e.exe\"}]}"
        );
        assert_eq!(pick_exe_asset(body), None);
    }
}
