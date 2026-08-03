//! Minimal Win32 rubber-band region picker over the virtual desktop.

use std::cell::RefCell;
use std::sync::OnceLock;

use lattice_capture_core::{CaptureError, RegionHandle};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, GetStockObject, InvalidateRect,
    SetBkMode, SetTextColor, TextOutW, HBRUSH, HGDIOBJ, PAINTSTRUCT, TRANSPARENT, WHITE_BRUSH,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect, GetMessageW,
    GetSystemMetrics, LoadCursorW, PostQuitMessage, RegisterClassExW, SetCursor,
    SetLayeredWindowAttributes, ShowWindow, TranslateMessage, CS_HREDRAW, CS_VREDRAW, IDC_CROSS,
    LWA_ALPHA, MSG, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
    SW_SHOW, WM_DESTROY, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT,
    WM_RBUTTONUP, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
};

use super::display::{display_id_for_handle, list_monitors, monitor_from_point};
use super::exclusion::exclude_window_from_capture;

#[derive(Default)]
struct PickerState {
    dragging: bool,
    start: POINT,
    current: POINT,
    result: Option<Result<RegionHandle, CaptureError>>,
}

thread_local! {
    static STATE: RefCell<PickerState> = const { RefCell::new(PickerState {
        dragging: false,
        start: POINT { x: 0, y: 0 },
        current: POINT { x: 0, y: 0 },
        result: None,
    }) };
}

/// Interactive rubber-band selection in virtual-screen coordinates.
///
/// Esc / right-click cancels ([`CaptureError::Cancelled`]).
pub fn select_region() -> Result<RegionHandle, CaptureError> {
    ensure_dpi_aware();
    register_class()?;

    let (vx, vy, vw, vh) = virtual_screen();
    if vw <= 0 || vh <= 0 {
        return Err(CaptureError::provider("virtual screen metrics invalid"));
    }

    STATE.with(|state| {
        *state.borrow_mut() = PickerState::default();
    });

    let hinstance = unsafe {
        GetModuleHandleW(None)
            .map_err(|err| CaptureError::provider(format!("GetModuleHandleW: {err}")))?
            .into()
    };

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            windows::core::w!("LatticeCaptureRegionPicker"),
            windows::core::w!("Lattice Capture"),
            WS_POPUP | WS_VISIBLE,
            vx,
            vy,
            vw,
            vh,
            None,
            None,
            Some(hinstance),
            None,
        )
        .map_err(|err| CaptureError::provider(format!("CreateWindowExW failed: {err}")))?
    };

    exclude_window_from_capture(hwnd);
    unsafe {
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 120, LWA_ALPHA);
        let _ = ShowWindow(hwnd, SW_SHOW);
        if let Ok(cursor) = LoadCursorW(None, IDC_CROSS) {
            let _ = SetCursor(Some(cursor));
        }
    }

    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    STATE.with(|state| {
        state
            .borrow_mut()
            .result
            .take()
            .unwrap_or(Err(CaptureError::Cancelled))
    })
}

fn ensure_dpi_aware() {
    static ONCE: OnceLock<()> = OnceLock::new();
    let _ = ONCE.get_or_init(|| unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    });
}

fn register_class() -> Result<(), CaptureError> {
    static REGISTERED: OnceLock<Result<(), String>> = OnceLock::new();
    let result = REGISTERED.get_or_init(|| {
        let class_name = windows::core::w!("LatticeCaptureRegionPicker");
        let hinstance = unsafe { GetModuleHandleW(None).unwrap_or_default().into() };
        let mut wc = WNDCLASSEXW::default();
        wc.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
        wc.style = CS_HREDRAW | CS_VREDRAW;
        wc.lpfnWndProc = Some(picker_wnd_proc);
        wc.hInstance = hinstance;
        wc.hCursor = unsafe { LoadCursorW(None, IDC_CROSS).unwrap_or_default() };
        wc.hbrBackground = HBRUSH(unsafe { GetStockObject(WHITE_BRUSH).0 });
        wc.lpszClassName = class_name;
        let atom = unsafe { RegisterClassExW(&wc) };
        if atom == 0 {
            Err("RegisterClassExW failed for region picker".into())
        } else {
            Ok(())
        }
    });
    result.clone().map_err(CaptureError::provider)
}

fn virtual_screen() -> (i32, i32, i32, i32) {
    unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    }
}

unsafe extern "system" fn picker_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            paint_overlay(hwnd);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            STATE.with(|state| {
                let mut s = state.borrow_mut();
                s.dragging = true;
                s.start = client_point(lparam);
                s.current = s.start;
            });
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            STATE.with(|state| {
                let mut s = state.borrow_mut();
                if s.dragging {
                    s.current = client_point(lparam);
                    let _ = InvalidateRect(Some(hwnd), None, true);
                }
            });
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let finished = STATE.with(|state| {
                let mut s = state.borrow_mut();
                if !s.dragging {
                    return None;
                }
                s.dragging = false;
                s.current = client_point(lparam);
                Some((s.start, s.current))
            });
            if let Some((start, end)) = finished {
                let outcome = finalize_selection(start, end);
                STATE.with(|state| {
                    state.borrow_mut().result = Some(outcome);
                });
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            if wparam.0 as u16 == VK_ESCAPE.0 {
                cancel_picker(hwnd);
            }
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            cancel_picker(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => {
            if unsafe { GetAsyncKeyState(VK_ESCAPE.0 as i32) } as u16 & 0x8000 != 0 {
                cancel_picker(hwnd);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }
}

fn cancel_picker(hwnd: HWND) {
    STATE.with(|state| {
        state.borrow_mut().result = Some(Err(CaptureError::Cancelled));
    });
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
}

fn client_point(lparam: LPARAM) -> POINT {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    POINT { x, y }
}

fn finalize_selection(start: POINT, end: POINT) -> Result<RegionHandle, CaptureError> {
    let (vx, vy, _, _) = virtual_screen();
    let left = start.x.min(end.x) + vx;
    let top = start.y.min(end.y) + vy;
    let right = start.x.max(end.x) + vx;
    let bottom = start.y.max(end.y) + vy;
    let width = (right - left).max(0) as u32;
    let height = (bottom - top).max(0) as u32;
    if width < 2 || height < 2 {
        return Err(CaptureError::Cancelled);
    }

    let monitor = monitor_from_point(left, top)
        .or_else(|| {
            list_monitors()
                .ok()
                .and_then(|m| m.into_iter().find(|e| e.is_primary).map(|e| e.handle))
        })
        .ok_or_else(|| CaptureError::not_found("display for selection"))?;

    let display_id = display_id_for_handle(monitor).unwrap_or(1);
    Ok(RegionHandle {
        display_id,
        x: left,
        y: top,
        width,
        height,
    })
}

fn paint_overlay(hwnd: HWND) {
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        let mut rect = RECT::default();
        let _ = GetClientRect(hwnd, &mut rect);
        let brush = CreateSolidBrush(COLORREF(0x0010_1010));
        let _ = FillRect(hdc, &rect, brush);
        let _ = DeleteObject(HGDIOBJ(brush.0));

        let selection = STATE.with(|state| {
            let s = state.borrow();
            if s.dragging {
                Some((s.start, s.current))
            } else {
                None
            }
        });

        if let Some((start, current)) = selection {
            let sel = RECT {
                left: start.x.min(current.x),
                top: start.y.min(current.y),
                right: start.x.max(current.x),
                bottom: start.y.max(current.y),
            };
            let clear = CreateSolidBrush(COLORREF(0x00C0_C0C0));
            let _ = FillRect(hdc, &sel, clear);
            let _ = DeleteObject(HGDIOBJ(clear.0));
        }

        let _ = SetBkMode(hdc, TRANSPARENT);
        let _ = SetTextColor(hdc, COLORREF(0x00FF_FFFF));
        let hint: Vec<u16> = "Drag to select · Esc to cancel"
            .encode_utf16()
            .collect();
        let _ = TextOutW(hdc, 24, 24, &hint);
        let _ = EndPaint(hwnd, &ps);
    }
}
