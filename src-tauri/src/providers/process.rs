//! Cheap "is this program running?" check.
//!
//! Uses a Toolhelp32 process snapshot on Windows (one syscall, no child
//! process), so it is fine to call on every poll.

#[cfg(target_os = "windows")]
pub fn is_running(exe_name: &str) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    // SAFETY: plain Win32 calls on a zero-initialised, correctly sized
    // PROCESSENTRY32W; the snapshot handle is closed on every path.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return false;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut found = false;
        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                if name.eq_ignore_ascii_case(exe_name) {
                    found = true;
                    break;
                }
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snapshot);
        found
    }
}

#[cfg(not(target_os = "windows"))]
pub fn is_running(_exe_name: &str) -> bool {
    false
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    #[test]
    fn finds_the_test_process_itself() {
        let me = std::env::current_exe().unwrap();
        let name = me.file_name().unwrap().to_string_lossy().to_string();
        assert!(super::is_running(&name));
        assert!(!super::is_running("definitely-not-running-qp.exe"));
    }
}
