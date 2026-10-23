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

pub(super) fn default_log_pos(main: HWND, w: i32) -> (i32, i32) {
    let work = work_area();
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
    (x, my)
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
    party_spot(&work_area(), &mr, left, w, h)
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
