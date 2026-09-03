use std::ptr;

use serde::Serialize;
use windows::{
    Win32::{
        Media::Audio::{eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator},
        Media::Audio::Endpoints::IAudioEndpointVolume,
        System::Com::{
            CLSCTX_ALL, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance,
            CoInitializeEx, CoUninitialize,
        },
    },
    core::Error as WindowsError,
};

use crate::{ToolError, ToolResult};

#[derive(Debug, Clone, Serialize)]
pub struct AudioState {
    pub volume_percent: f32,
    pub muted: bool,
}

struct ComGuard;

impl ComGuard {
    fn initialize() -> ToolResult<Self> {
        let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if result.is_err() {
            return Err(ToolError::Windows(WindowsError::from_hresult(result)));
        }
        Ok(Self)
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

fn with_endpoint_volume<T>(
    operation: impl FnOnce(&IAudioEndpointVolume) -> windows::core::Result<T>,
) -> ToolResult<T> {
    let _com = ComGuard::initialize()?;

    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER)?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let endpoint: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;
        Ok(operation(&endpoint)?)
    }
}

pub fn get_state() -> ToolResult<AudioState> {
    with_endpoint_volume(|endpoint| unsafe {
        Ok(AudioState {
            volume_percent: endpoint.GetMasterVolumeLevelScalar()? * 100.0,
            muted: endpoint.GetMute()?.as_bool(),
        })
    })
}

pub fn set_volume(percent: f32) -> ToolResult<AudioState> {
    if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
        return Err(ToolError::InvalidArgument(
            "volume must be a finite percentage from 0 to 100".into(),
        ));
    }

    with_endpoint_volume(|endpoint| unsafe {
        endpoint.SetMasterVolumeLevelScalar(percent / 100.0, ptr::null())?;
        Ok(AudioState {
            volume_percent: endpoint.GetMasterVolumeLevelScalar()? * 100.0,
            muted: endpoint.GetMute()?.as_bool(),
        })
    })
}

pub fn set_mute(muted: bool) -> ToolResult<AudioState> {
    with_endpoint_volume(|endpoint| unsafe {
        endpoint.SetMute(muted, ptr::null())?;
        Ok(AudioState {
            volume_percent: endpoint.GetMasterVolumeLevelScalar()? * 100.0,
            muted: endpoint.GetMute()?.as_bool(),
        })
    })
}
