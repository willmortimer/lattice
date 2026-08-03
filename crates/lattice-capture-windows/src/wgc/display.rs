//! Monitor enumeration for WGC display sources.

use lattice_capture_core::{
    CaptureError, CaptureSource, CaptureSourceInfo, DisplayHandle, RegionHandle,
};

use windows::core::BOOL;
use windows::Win32::Foundation::{LPARAM, POINT, RECT, TRUE};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromPoint, HDC, HMONITOR, MONITORINFOEXW,
    MONITOR_DEFAULTTONULL,
};
use windows::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;

/// One attached display, keyed by a stable-enough `DisplayHandle` for this process.
#[derive(Debug, Clone)]
pub struct MonitorEntry {
    pub id: u32,
    pub handle: HMONITOR,
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
    pub name: String,
}

impl MonitorEntry {
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        x >= self.left
            && y >= self.top
            && x < self.left + self.width as i32
            && y < self.top + self.height as i32
    }

    pub fn contains_region(&self, region: &RegionHandle) -> bool {
        let right = region.x.saturating_add(region.width as i32);
        let bottom = region.y.saturating_add(region.height as i32);
        region.x >= self.left
            && region.y >= self.top
            && right <= self.left + self.width as i32
            && bottom <= self.top + self.height as i32
    }
}

/// Enumerate attached monitors (primary first).
pub fn list_monitors() -> Result<Vec<MonitorEntry>, CaptureError> {
    let mut raw: Vec<(HMONITOR, RECT, bool, String)> = Vec::new();
    unsafe {
        if !EnumDisplayMonitors(
            None,
            None,
            Some(enum_monitor_callback),
            LPARAM(&mut raw as *mut _ as isize),
        )
        .as_bool()
        {
            return Err(CaptureError::provider("EnumDisplayMonitors failed"));
        }
    }

    if raw.is_empty() {
        return Err(CaptureError::provider("no displays found"));
    }

    raw.sort_by_key(|(_, _, primary, _)| if *primary { 0u8 } else { 1u8 });

    let mut out = Vec::with_capacity(raw.len());
    for (index, (handle, rect, is_primary, name)) in raw.into_iter().enumerate() {
        let width = (rect.right - rect.left).max(0) as u32;
        let height = (rect.bottom - rect.top).max(0) as u32;
        let id = display_id_for_handle(handle).unwrap_or((index as u32).saturating_add(1));
        out.push(MonitorEntry {
            id,
            handle,
            left: rect.left,
            top: rect.top,
            width,
            height,
            is_primary,
            name,
        });
    }
    Ok(out)
}

pub fn enumerate_display_sources() -> Result<Vec<CaptureSourceInfo>, CaptureError> {
    let monitors = list_monitors()?;
    Ok(monitors
        .into_iter()
        .map(|monitor| {
            let title = if monitor.is_primary {
                format!("Primary display ({})", monitor.name)
            } else {
                format!("Display {} ({})", monitor.id, monitor.name)
            };
            CaptureSourceInfo {
                source: CaptureSource::Display(DisplayHandle(monitor.id)),
                title: Some(title),
                width: Some(monitor.width),
                height: Some(monitor.height),
            }
        })
        .collect())
}

pub fn find_monitor(display_id: u32) -> Result<MonitorEntry, CaptureError> {
    list_monitors()?
        .into_iter()
        .find(|m| m.id == display_id)
        .ok_or_else(|| CaptureError::not_found(format!("display {display_id}")))
}

pub fn find_monitor_for_region(region: &RegionHandle) -> Result<MonitorEntry, CaptureError> {
    let monitors = list_monitors()?;
    if let Some(by_id) = monitors.iter().find(|m| m.id == region.display_id) {
        if by_id.contains_region(region) || region_intersects(by_id, region) {
            return Ok(by_id.clone());
        }
    }
    // Fall back to the monitor containing the region's top-left.
    if let Some(hit) = monitors
        .iter()
        .find(|m| m.contains_point(region.x, region.y))
    {
        return Ok(hit.clone());
    }
    Err(CaptureError::not_found(format!(
        "display for region {}@{},{} {}x{}",
        region.display_id, region.x, region.y, region.width, region.height
    )))
}

fn region_intersects(monitor: &MonitorEntry, region: &RegionHandle) -> bool {
    let r_right = region.x.saturating_add(region.width as i32);
    let r_bottom = region.y.saturating_add(region.height as i32);
    let m_right = monitor.left + monitor.width as i32;
    let m_bottom = monitor.top + monitor.height as i32;
    region.x < m_right && r_right > monitor.left && region.y < m_bottom && r_bottom > monitor.top
}

pub fn display_id_for_handle(handle: HMONITOR) -> Option<u32> {
    let raw = handle.0 as usize;
    if raw == 0 {
        None
    } else {
        // Fold pointer bits into a non-zero u32 key stable for the process.
        let id = ((raw as u32) ^ ((raw >> 32) as u32)).max(1);
        Some(id)
    }
}

unsafe extern "system" fn enum_monitor_callback(
    monitor: HMONITOR,
    _hdc: HDC,
    lprect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let list = &mut *(lparam.0 as *mut Vec<(HMONITOR, RECT, bool, String)>);
    let rect = if lprect.is_null() {
        RECT::default()
    } else {
        *lprect
    };

    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    let (is_primary, name) =
        if GetMonitorInfoW(monitor, &mut info as *mut _ as *mut _).as_bool() {
            let primary = (info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY) != 0;
            let name = utf16_z_to_string(&info.szDevice);
            (primary, name)
        } else {
            (false, "Display".into())
        };

    list.push((monitor, rect, is_primary, name));
    TRUE
}

fn utf16_z_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

/// Resolve an HMONITOR for a virtual-desktop point (used by the region picker).
pub fn monitor_from_point(x: i32, y: i32) -> Option<HMONITOR> {
    let pt = POINT { x, y };
    let monitor = unsafe { MonitorFromPoint(pt, MONITOR_DEFAULTTONULL) };
    if monitor.0.is_null() {
        None
    } else {
        Some(monitor)
    }
}
