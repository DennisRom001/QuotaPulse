//! Cheap "is this program running?" check, per OS.
//!
//! `name` is a logical program name; it is compared after
//! [`crate::platform::normalize_process_name`], so `Cursor.exe` matches
//! `Cursor.exe` on Windows, `…/Cursor.app/Contents/MacOS/Cursor` on macOS and
//! `cursor` on Linux.
//!
//! * Windows: one Toolhelp32 snapshot (one syscall, no child process).
//! * Linux: `/proc/<pid>/comm` and argv[0] from `/proc/<pid>/cmdline`.
//! * macOS: `ps -A -o comm=` (no /proc on macOS).

use crate::platform::process_list_contains;

#[cfg(target_os = "windows")]
pub fn is_running(name: &str) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let mut names: Vec<String> = Vec::new();
    // SAFETY: plain Win32 calls on a zero-initialised, correctly sized
    // PROCESSENTRY32W; the snapshot handle is closed on every path.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return false;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                names.push(String::from_utf16_lossy(&entry.szExeFile[..len]));
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snapshot);
    }
    process_list_contains(names.iter().map(String::as_str), name)
}

#[cfg(target_os = "linux")]
pub fn is_running(name: &str) -> bool {
    let Ok(entries) = std::fs::read_dir("/proc") else { return false };
    let mut names: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        let pid_dir = entry.path();
        let is_pid = entry.file_name().to_str().is_some_and(|s| s.bytes().all(|b| b.is_ascii_digit()));
        if !is_pid {
            continue;
        }
        if let Ok(comm) = std::fs::read_to_string(pid_dir.join("comm")) {
            names.push(comm);
        }
        if let Some(argv0) = std::fs::read(pid_dir.join("cmdline"))
            .ok()
            .and_then(|raw| crate::platform::proc_cmdline_name(&raw))
        {
            names.push(argv0);
        }
    }
    process_list_contains(names.iter().map(String::as_str), name)
}

#[cfg(target_os = "macos")]
pub fn is_running(name: &str) -> bool {
    let Ok(out) = std::process::Command::new("/bin/ps").args(["-A", "-o", "comm="]).output() else {
        return false;
    };
    let names = crate::platform::parse_ps_output(&String::from_utf8_lossy(&out.stdout));
    process_list_contains(names.iter().map(String::as_str), name)
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub fn is_running(_name: &str) -> bool {
    false
}

#[cfg(test)]
mod tests {
    /// The test binary itself is a running process on every OS.
    #[test]
    fn finds_the_test_process_itself() {
        let me = std::env::current_exe().unwrap();
        let name = me.file_name().unwrap().to_string_lossy().to_string();
        assert!(super::is_running(&name), "{name} not found");
        assert!(!super::is_running("definitely-not-running-qp.exe"));
    }
}
