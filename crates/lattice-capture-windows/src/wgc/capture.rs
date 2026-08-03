//! Still-frame capture via Windows Graphics Capture.

use std::time::{Duration, Instant};

use lattice_capture_core::{
    CaptureError, CapturedImage, DisplayHandle, RegionHandle,
};
use windows::core::Interface;
use windows::Graphics::Capture::{
    Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Texture2D, D3D11_CPU_ACCESS_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Gdi::HMONITOR;
use windows::Win32::System::WinRT::Direct3D11::IDirect3DDxgiInterfaceAccess;
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;

use super::device::CaptureDevice;
use super::display::{find_monitor, find_monitor_for_region, list_monitors};
use super::encode::{crop_rgba, rgba_to_png_image};
use super::exclusion::with_process_windows_excluded;
use super::picker;

struct PoolGuard(Direct3D11CaptureFramePool);
impl Drop for PoolGuard {
    fn drop(&mut self) {
        let _ = self.0.Close();
    }
}
impl std::ops::Deref for PoolGuard {
    type Target = Direct3D11CaptureFramePool;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

struct SessionGuard(GraphicsCaptureSession);
impl Drop for SessionGuard {
    fn drop(&mut self) {
        let _ = self.0.Close();
    }
}
impl std::ops::Deref for SessionGuard {
    type Target = GraphicsCaptureSession;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

struct FrameGuard(Direct3D11CaptureFrame);
impl Drop for FrameGuard {
    fn drop(&mut self) {
        let _ = self.0.Close();
    }
}
impl std::ops::Deref for FrameGuard {
    type Target = Direct3D11CaptureFrame;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

struct RgbaFrame {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

pub fn capture_display(display: DisplayHandle) -> Result<CapturedImage, CaptureError> {
    let monitor = find_monitor(display.0)?;
    let frame = with_process_windows_excluded(|| capture_monitor(monitor.handle))?;
    rgba_to_png_image(frame.width, frame.height, &frame.rgba)
}

pub fn capture_region(region: RegionHandle) -> Result<CapturedImage, CaptureError> {
    if region.width == 0 || region.height == 0 {
        return Err(CaptureError::invalid_argument(
            "region width/height must be non-zero",
        ));
    }
    let monitor = find_monitor_for_region(&region)?;
    let frame = with_process_windows_excluded(|| capture_monitor(monitor.handle))?;

    let local_x = region.x.saturating_sub(monitor.left);
    let local_y = region.y.saturating_sub(monitor.top);
    if local_x < 0 || local_y < 0 {
        return Err(CaptureError::invalid_argument(
            "region origin is outside the target display",
        ));
    }
    let cropped = crop_rgba(
        frame.width,
        frame.height,
        &frame.rgba,
        local_x as u32,
        local_y as u32,
        region.width,
        region.height,
    )?;
    rgba_to_png_image(region.width, region.height, &cropped)
}

pub fn capture_interactive_region() -> Result<CapturedImage, CaptureError> {
    // Ensure we can enumerate displays before showing UI.
    let _ = list_monitors()?;
    let selection = picker::select_region()?;
    capture_region(selection)
}

fn capture_monitor(monitor: HMONITOR) -> Result<RgbaFrame, CaptureError> {
    if !GraphicsCaptureSession::IsSupported().unwrap_or(false) {
        return Err(CaptureError::Unsupported(
            "Windows Graphics Capture is not supported on this OS".into(),
        ));
    }

    let device = CaptureDevice::new()?;
    let item = create_item_for_monitor(monitor)?;
    let item_size = item
        .Size()
        .map_err(|err| CaptureError::provider(format!("GraphicsCaptureItem::Size: {err}")))?;
    if item_size.Width <= 0 || item_size.Height <= 0 {
        return Err(CaptureError::provider(format!(
            "invalid capture item size {}x{}",
            item_size.Width, item_size.Height
        )));
    }

    let pool = PoolGuard(
        Direct3D11CaptureFramePool::CreateFreeThreaded(
            &device.winrt,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            item_size,
        )
        .map_err(|err| CaptureError::provider(format!("CreateFreeThreaded: {err}")))?,
    );
    let session = SessionGuard(
        pool.CreateCaptureSession(&item)
            .map_err(|err| CaptureError::provider(format!("CreateCaptureSession: {err}")))?,
    );

    // Best-effort knobs; older builds may reject these.
    let _ = session.SetIsCursorCaptureEnabled(false);
    let _ = session.SetIsBorderRequired(false);

    session
        .StartCapture()
        .map_err(|err| CaptureError::provider(format!("StartCapture: {err}")))?;

    let frame = wait_for_frame(&pool, Duration::from_millis(1500))?;
    read_frame_rgba(&device, &frame)
}

fn create_item_for_monitor(monitor: HMONITOR) -> Result<GraphicsCaptureItem, CaptureError> {
    let interop = windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
        .map_err(|err| {
            CaptureError::provider(format!("IGraphicsCaptureItemInterop factory: {err}"))
        })?;
    unsafe {
        interop.CreateForMonitor::<GraphicsCaptureItem>(monitor).map_err(|err| {
            CaptureError::provider(format!("CreateForMonitor failed: {err}"))
        })
    }
}

fn wait_for_frame(
    pool: &Direct3D11CaptureFramePool,
    timeout: Duration,
) -> Result<FrameGuard, CaptureError> {
    let deadline = Instant::now() + timeout;
    let mut latest: Option<Direct3D11CaptureFrame> = None;
    loop {
        while let Ok(frame) = pool.TryGetNextFrame() {
            latest = Some(frame);
        }
        if latest.is_some() {
            // Drain once more after a short settle so we prefer a fresh frame.
            std::thread::sleep(Duration::from_millis(20));
            while let Ok(frame) = pool.TryGetNextFrame() {
                latest = Some(frame);
            }
            break;
        }
        if Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    latest
        .map(FrameGuard)
        .ok_or_else(|| CaptureError::provider("WGC produced no frame within timeout"))
}

fn read_frame_rgba(
    device: &CaptureDevice,
    frame: &Direct3D11CaptureFrame,
) -> Result<RgbaFrame, CaptureError> {
    let content = frame
        .ContentSize()
        .map_err(|err| CaptureError::provider(format!("ContentSize: {err}")))?;
    let surface = frame
        .Surface()
        .map_err(|err| CaptureError::provider(format!("Surface: {err}")))?;
    let access: IDirect3DDxgiInterfaceAccess = surface
        .cast()
        .map_err(|err| CaptureError::provider(format!("DxgiInterfaceAccess: {err}")))?;
    let texture: ID3D11Texture2D = unsafe {
        access
            .GetInterface()
            .map_err(|err| CaptureError::provider(format!("GetInterface ID3D11Texture2D: {err}")))?
    };

    let mut desc = D3D11_TEXTURE2D_DESC::default();
    unsafe { texture.GetDesc(&mut desc) };
    let surface_w = desc.Width;
    let surface_h = desc.Height;

    desc.Usage = D3D11_USAGE_STAGING;
    desc.BindFlags = 0;
    desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
    desc.MiscFlags = 0;

    let mut staging: Option<ID3D11Texture2D> = None;
    unsafe {
        device
            .d3d
            .CreateTexture2D(&desc, None, Some(&mut staging))
            .map_err(|err| CaptureError::provider(format!("CreateTexture2D staging: {err}")))?;
    }
    let staging = staging.ok_or_else(|| CaptureError::internal("staging texture was null"))?;
    unsafe {
        device.context.CopyResource(&staging, &texture);
    }

    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    unsafe {
        device
            .context
            .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
            .map_err(|err| CaptureError::provider(format!("Map staging texture: {err}")))?;
    }

    let content_w = (content.Width.max(0) as u32).min(surface_w);
    let content_h = (content.Height.max(0) as u32).min(surface_h);
    if content_w == 0 || content_h == 0 {
        unsafe { device.context.Unmap(&staging, 0) };
        return Err(CaptureError::provider("WGC content size was zero"));
    }

    let row_pitch = mapped.RowPitch as usize;
    let src = mapped.pData as *const u8;
    let mut rgba = vec![0u8; content_w as usize * content_h as usize * 4];
    unsafe {
        for y in 0..content_h as usize {
            let src_row = src.add(y * row_pitch);
            let dst_off = y * content_w as usize * 4;
            for x in 0..content_w as usize {
                let px = src_row.add(x * 4);
                // Staging is BGRA; convert to RGBA with opaque alpha.
                let b = *px;
                let g = *px.add(1);
                let r = *px.add(2);
                let o = dst_off + x * 4;
                rgba[o] = r;
                rgba[o + 1] = g;
                rgba[o + 2] = b;
                rgba[o + 3] = 255;
            }
        }
        device.context.Unmap(&staging, 0);
    }

    Ok(RgbaFrame {
        width: content_w,
        height: content_h,
        rgba,
    })
}
