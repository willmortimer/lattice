//! Best-effort Lattice self-exclusion via `WDA_EXCLUDEFROMCAPTURE`.

use std::cell::RefCell;

use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, TRUE};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowDisplayAffinity, GetWindowThreadProcessId, IsWindowVisible,
    SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WINDOW_DISPLAY_AFFINITY,
};

struct AffinityRestore {
    hwnd: HWND,
    previous: u32,
}

/// Temporarily exclude this process's visible top-level windows from screen
/// capture, run `f`, then restore prior display affinities.
///
/// Desktop also applies always-on `WDA_EXCLUDEFROMCAPTURE` to shelf + main
/// HWNDs at install; this is the capture-time second line when affinity is
/// available.
pub fn with_process_windows_excluded<T>(f: impl FnOnce() -> T) -> T {
    let restored = apply_exclusion();
    let result = f();
    for entry in restored.into_iter().rev() {
        unsafe {
            let _ = SetWindowDisplayAffinity(entry.hwnd, WINDOW_DISPLAY_AFFINITY(entry.previous));
        }
    }
    result
}

fn apply_exclusion() -> Vec<AffinityRestore> {
    let pid = unsafe { GetCurrentProcessId() };
    let collected: RefCell<Vec<AffinityRestore>> = RefCell::new(Vec::new());
    let state = EnumState {
        pid,
        collected: &collected,
    };
    unsafe {
        let _ = EnumWindows(
            Some(enum_windows_callback),
            LPARAM(&state as *const EnumState as isize),
        );
    }
    collected.into_inner()
}

struct EnumState<'a> {
    pid: u32,
    collected: &'a RefCell<Vec<AffinityRestore>>,
}

unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let state = &*(lparam.0 as *const EnumState);
    let mut window_pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
        if window_pid != state.pid || !IsWindowVisible(hwnd).as_bool() {
            return TRUE;
        }
        let mut previous = 0u32;
        let _ = GetWindowDisplayAffinity(hwnd, &mut previous);
        if SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE).is_ok() {
            state.collected.borrow_mut().push(AffinityRestore { hwnd, previous });
        }
    }
    TRUE
}

/// Mark a single HWND as excluded from capture (picker overlay).
pub fn exclude_window_from_capture(hwnd: HWND) {
    unsafe {
        let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
    }
}
