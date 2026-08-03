//! D3D11 + WinRT device helpers for WGC.

use lattice_capture_core::CaptureError;

use windows::core::{Interface, HRESULT};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_SDK_VERSION,
};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows::Win32::System::WinRT::Direct3D11::CreateDirect3D11DeviceFromDXGIDevice;

/// COM + D3D11 + WinRT capture device bundle for one still capture.
pub struct CaptureDevice {
    pub d3d: ID3D11Device,
    pub context: ID3D11DeviceContext,
    pub winrt: IDirect3DDevice,
}

impl CaptureDevice {
    pub fn new() -> Result<Self, CaptureError> {
        ensure_com_initialized()?;
        unsafe {
            let mut device: Option<ID3D11Device> = None;
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                None,
            )
            .map_err(|err| CaptureError::provider(format!("D3D11CreateDevice failed: {err}")))?;

            let d3d = device.ok_or_else(|| CaptureError::internal("D3D11 device was null"))?;
            let context = d3d
                .GetImmediateContext()
                .map_err(|err| CaptureError::provider(format!("GetImmediateContext: {err}")))?;
            let dxgi: IDXGIDevice = d3d
                .cast()
                .map_err(|err| CaptureError::provider(format!("IDXGIDevice cast failed: {err}")))?;
            let inspectable = CreateDirect3D11DeviceFromDXGIDevice(&dxgi).map_err(|err| {
                CaptureError::provider(format!("CreateDirect3D11DeviceFromDXGIDevice: {err}"))
            })?;
            let winrt: IDirect3DDevice = inspectable.cast().map_err(|err| {
                CaptureError::provider(format!("IDirect3DDevice cast failed: {err}"))
            })?;
            Ok(Self {
                d3d,
                context,
                winrt,
            })
        }
    }
}

fn ensure_com_initialized() -> Result<(), CaptureError> {
    // S_OK / S_FALSE succeed; RPC_E_CHANGED_MODE means another apartment owns
    // this thread — continue and let WinRT fail clearly if needed.
    const RPC_E_CHANGED_MODE: HRESULT = HRESULT(0x8001_0106u32 as i32);
    let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    if hr.is_ok() || hr == RPC_E_CHANGED_MODE {
        Ok(())
    } else {
        Err(CaptureError::provider(format!("CoInitializeEx failed: {hr}")))
    }
}
