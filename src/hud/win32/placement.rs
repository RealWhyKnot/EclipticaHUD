use super::*;

pub(super) fn parse_pos(text: &str) -> Option<(i32, i32, bool)> {
    let mut it = text.split_whitespace();
    let x = it.next()?.parse().ok()?;
    let y = it.next()?.parse().ok()?;
    let open = it.next().is_some_and(|v| v == "1");
    Some((x, y, open))
}

pub(super) fn load_pos(name: &str) -> Option<(i32, i32, bool)> {
    parse_pos(&std::fs::read_to_string(data_file(name)?).ok()?)
}

pub(super) fn monitor_work(r: &RECT) -> Option<RECT> {
    monitor_info(r, MONITOR_DEFAULTTONULL)
}

fn monitor_info(r: &RECT, flag: MONITOR_FROM_FLAGS) -> Option<RECT> {
    unsafe {
        let mon = MonitorFromRect(r, flag);
        if mon.is_null() {
            return None;
        }
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        (GetMonitorInfoW(mon, &mut info) != 0).then_some(info.rcWork)
    }
}

pub(super) fn fit_spot(work: Option<RECT>, x: i32, y: i32, w: i32, h: i32) -> Option<(i32, i32)> {
    let work = work?;
    let x = x.clamp(work.left, (work.right - w).max(work.left));
    let y = y.clamp(work.top, (work.bottom - h).max(work.top));
    Some((x, y))
}

pub(super) fn saved_pos(name: &str, w: i32, h: i32) -> Option<(i32, i32)> {
    let (x, y, _) = load_pos(name)?;
    let r = RECT {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    };
    fit_spot(monitor_work(&r), x, y, w, h)
}

pub(super) fn window_pos(hwnd: HWND) -> Option<(i32, i32)> {
    let mut r = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    (unsafe { GetWindowRect(hwnd, &mut r) } != 0).then_some((r.left, r.top))
}

pub(super) fn save_pos(hwnd: HWND) {
    if let Some((x, y)) = window_pos(hwnd) {
        write_data("pos.txt", format!("{x} {y}"));
    }
}

pub(super) fn save_log_pos(hwnd: HWND, open: bool) {
    if let Some((x, y)) = window_pos(hwnd) {
        write_data("logpos.txt", format!("{x} {y} {}", u8::from(open)));
    }
}

pub(super) fn save_party_pos(hwnd: HWND, open: bool) {
    if let Some((x, y)) = window_pos(hwnd) {
        write_data("partypos.txt", format!("{x} {y} {}", u8::from(open)));
    }
}

pub(super) fn work_area() -> RECT {
    let mut work = RECT {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    };
    unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            &mut work as *mut RECT as *mut core::ffi::c_void,
            0,
        );
    }
    work
}

pub(super) fn default_pos(w: i32, h: i32) -> (i32, i32) {
    let work = work_area();
    (work.right - w - 24, work.bottom - h - 24)
}

pub(super) fn work_of(hwnd: HWND) -> RECT {
    rect_of(hwnd)
        .and_then(|r| monitor_info(&r, MONITOR_DEFAULTTONEAREST))
        .unwrap_or_else(work_area)
}

pub(super) fn keep_on_screen(hwnd: HWND) {
    let Some(r) = rect_of(hwnd) else {
        return;
    };
    let spot = fit_spot(
        Some(work_of(hwnd)),
        r.left,
        r.top,
        r.right - r.left,
        r.bottom - r.top,
    );
    if let Some((x, y)) = spot.filter(|&p| p != (r.left, r.top)) {
        unsafe {
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}

pub(super) fn default_log_pos(main: HWND, w: i32, h: i32) -> (i32, i32) {
    let work = work_of(main);
    let (mx, my) = window_pos(main).unwrap_or((work.right, work.bottom));
    let mut mr = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    unsafe { GetWindowRect(main, &mut mr) };
    let x = if mx - w - 12 >= work.left {
        mx - w - 12
    } else {
        mr.right + 12
    };
    fit_spot(Some(work), x, my, w, h).unwrap_or((x, my))
}

fn rect_of(hwnd: HWND) -> Option<RECT> {
    let mut r = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    (unsafe { GetWindowRect(hwnd, &mut r) } != 0).then_some(r)
}

pub(super) fn default_party_pos(main: HWND, log: Option<HWND>, w: i32, h: i32) -> (i32, i32) {
    let Some(mr) = rect_of(main) else {
        return default_pos(w, h);
    };
    let left = log
        .and_then(rect_of)
        .map_or(mr.left, |lr| lr.left.min(mr.left));
    party_spot(&work_of(main), &mr, left, w, h)
}

pub(super) fn party_spot(work: &RECT, main: &RECT, left: i32, w: i32, h: i32) -> (i32, i32) {
    let x = if left - w - 12 >= work.left {
        left - w - 12
    } else {
        main.right + 12
    };
    let x = x.clamp(work.left, (work.right - w).max(work.left));
    let y = (main.bottom - h).clamp(work.top, (work.bottom - h).max(work.top));
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pos_parse() {
        assert_eq!(parse_pos("10 20"), Some((10, 20, false)));
        assert_eq!(parse_pos("10 20 1"), Some((10, 20, true)));
        assert_eq!(parse_pos("10 20 0\n"), Some((10, 20, false)));
        assert_eq!(parse_pos("-5 7 1"), Some((-5, 7, true)));
        assert_eq!(parse_pos("x"), None);
        assert_eq!(parse_pos(""), None);
    }

    #[test]
    fn saved_spot_on_a_missing_monitor_is_dropped() {
        let far = RECT {
            left: 1_000_000,
            top: 1_000_000,
            right: 1_000_360,
            bottom: 1_000_392,
        };
        assert!(monitor_work(&far).is_none());
        assert_eq!(fit_spot(None, 2686, 56, 360, 392), None);
    }

    #[test]
    fn saved_spot_is_pulled_onto_its_monitor() {
        let work = RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        };
        assert_eq!(fit_spot(Some(work), 1043, 203, 360, 640), Some((1043, 203)));
        assert_eq!(fit_spot(Some(work), 1800, 56, 360, 392), Some((1560, 56)));
        assert_eq!(fit_spot(Some(work), -200, 900, 360, 392), Some((0, 648)));
        assert_eq!(fit_spot(Some(work), 50, 50, 2400, 1200), Some((0, 0)));
        let left = RECT {
            left: -3440,
            top: 0,
            right: 0,
            bottom: 1400,
        };
        assert_eq!(
            fit_spot(Some(left), -100, 1300, 360, 392),
            Some((-360, 1008))
        );
    }

    #[test]
    fn party_window_goes_left_of_the_leftmost_window() {
        let work = RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        };
        let main = RECT {
            left: 1536,
            top: 376,
            right: 1896,
            bottom: 1016,
        };
        assert_eq!(party_spot(&work, &main, main.left, 360, 392), (1164, 624));
        assert_eq!(party_spot(&work, &main, 1104, 360, 392), (732, 624));
        assert_eq!(party_spot(&work, &main, 300, 360, 392), (1560, 624));
        let short = RECT {
            left: 1536,
            top: 0,
            right: 1896,
            bottom: 200,
        };
        assert_eq!(party_spot(&work, &short, short.left, 360, 392), (1164, 0));
    }
}
