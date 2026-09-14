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
}
