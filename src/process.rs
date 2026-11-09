use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};

pub(crate) fn running(exe: &str) -> bool {
    // SAFETY: the snapshot handle is checked and closed here, and `entry` has its size set before the first call.
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return false;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = crate::num::to_u32(std::mem::size_of::<PROCESSENTRY32W>());
        let mut found = false;
        let mut more = Process32FirstW(snap, &raw mut entry) != 0;
        while more && !found {
            let name: Vec<u16> = entry
                .szExeFile
                .iter()
                .copied()
                .take_while(|&c| c != 0)
                .collect();
            let name = String::from_utf16_lossy(&name);
            found = name.eq_ignore_ascii_case(exe);
            more = Process32NextW(snap, &raw mut entry) != 0;
        }
        CloseHandle(snap);
        found
    }
}
