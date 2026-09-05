use std::{ffi::c_void, mem::size_of, ptr};

use windows::Win32::{
    Foundation::HWND,
    Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleDC, CreateDIBSection,
        DIB_RGB_COLORS, DeleteDC, DeleteObject, GetWindowDC, HDC, HGDIOBJ, ReleaseDC, SRCCOPY,
        SelectObject,
    },
    UI::WindowsAndMessaging::GetWindowRect,
};

use crate::{
    ToolError, ToolResult,
    window::{self, WindowHandle},
};

#[derive(Debug, Clone)]
pub struct ScreenFrame {
    pub width: u32,
    pub height: u32,
    /// Top-down pixels in Windows BGRA byte order.
    pub bgra: Vec<u8>,
}

struct WindowDc {
    hwnd: HWND,
    hdc: HDC,
}

impl Drop for WindowDc {
    fn drop(&mut self) {
        unsafe {
            ReleaseDC(Some(self.hwnd), self.hdc);
        }
    }
}

struct MemoryDc(HDC);

impl Drop for MemoryDc {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteDC(self.0);
        }
    }
}

pub fn capture_active_window() -> ToolResult<ScreenFrame> {
    capture(window::get_active_handle()?)
}

pub fn capture(handle: WindowHandle) -> ToolResult<ScreenFrame> {
    unsafe {
        let hwnd = handle.hwnd();
        if hwnd.0.is_null() {
            return Err(ToolError::NotFound("window handle is empty".into()));
        }

        let mut rect = Default::default();
        GetWindowRect(hwnd, &mut rect)?;
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width <= 0 || height <= 0 {
            return Err(ToolError::Unsupported(
                "window has an empty capture rectangle".into(),
            ));
        }

        let source_dc = GetWindowDC(Some(hwnd));
        if source_dc.0.is_null() {
            return Err(ToolError::Unsupported(
                "Windows did not provide a window device context".into(),
            ));
        }
        let _source_guard = WindowDc {
            hwnd,
            hdc: source_dc,
        };

        let memory_dc = CreateCompatibleDC(Some(source_dc));
        if memory_dc.0.is_null() {
            return Err(ToolError::Unsupported(
                "Windows could not create the context capture device context".into(),
            ));
        }
        let _memory_guard = MemoryDc(memory_dc);

        // Negative height creates a top-down DIB, which avoids a vertical flip later.
        let bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                biSizeImage: (width as u32)
                    .saturating_mul(height as u32)
                    .saturating_mul(4),
                ..Default::default()
            },
            ..Default::default()
        };

        let mut bits: *mut c_void = ptr::null_mut();
        let bitmap = CreateDIBSection(
            Some(source_dc),
            &bitmap_info,
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        )?;
        if bits.is_null() {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            return Err(ToolError::Unsupported(
                "Windows created an empty context capture bitmap".into(),
            ));
        }

        let old_object = SelectObject(memory_dc, HGDIOBJ(bitmap.0));
        if old_object.0.is_null() {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            return Err(ToolError::Unsupported(
                "Windows could not select the context capture bitmap".into(),
            ));
        }

        let capture_result = BitBlt(
            memory_dc,
            0,
            0,
            width,
            height,
            Some(source_dc),
            0,
            0,
            SRCCOPY,
        );

        // Restore the DC before deleting the selected bitmap.
        SelectObject(memory_dc, old_object);

        if let Err(error) = capture_result {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            return Err(ToolError::Windows(error));
        }

        let length = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(|| {
                ToolError::Unsupported("capture dimensions overflow memory size".into())
            })?;
        let bgra = std::slice::from_raw_parts(bits.cast::<u8>(), length).to_vec();
        let _ = DeleteObject(HGDIOBJ(bitmap.0));

        Ok(ScreenFrame {
            width: width as u32,
            height: height as u32,
            bgra,
        })
    }
}
