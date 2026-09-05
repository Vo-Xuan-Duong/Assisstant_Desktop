use serde::Serialize;
use windows::{
    Win32::{
        System::Com::{
            CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
        },
        UI::Accessibility::{
            CUIAutomation, IUIAutomation, IUIAutomationCondition, IUIAutomationElement,
            IUIAutomationVirtualizedItemPattern, TreeScope_Children, UIA_PATTERN_ID,
        },
    },
    core::{Error as WindowsError, Interface},
};

use crate::{ToolError, ToolResult, window::WindowHandle};

const UIA_VIRTUALIZED_ITEM_PATTERN_ID: UIA_PATTERN_ID = UIA_PATTERN_ID(10020);
const MAX_PATH_DEPTH: usize = 16;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct VirtualizedItemStatus {
    pub supported: bool,
}

/// Check whether an explicitly resolved UI Automation element exposes
/// VirtualizedItemPattern. This is read-only and does not realize the item.
pub fn status(handle: WindowHandle, path: &[u32]) -> ToolResult<VirtualizedItemStatus> {
    let client = VirtualizedClient::new(handle)?;
    let element = client.resolve(path)?;
    Ok(VirtualizedItemStatus {
        supported: pattern::<IUIAutomationVirtualizedItemPattern>(
            &element,
            UIA_VIRTUALIZED_ITEM_PATTERN_ID,
        )
        .is_some(),
    })
}

/// Ask the UI Automation provider to materialize a virtualized item.
///
/// This does not select, focus, invoke, click or type. The provider decides how
/// to realize the semantic item. Callers should re-inspect after success because
/// realization can change the accessibility tree and invalidate the old path.
pub fn realize(handle: WindowHandle, path: &[u32]) -> ToolResult<()> {
    let client = VirtualizedClient::new(handle)?;
    let element = client.resolve(path)?;
    let pattern =
        pattern::<IUIAutomationVirtualizedItemPattern>(&element, UIA_VIRTUALIZED_ITEM_PATTERN_ID)
            .ok_or_else(|| {
            ToolError::Unsupported("element does not expose VirtualizedItemPattern".into())
        })?;

    unsafe { pattern.Realize()? };
    Ok(())
}

struct VirtualizedClient {
    _com: ComGuard,
    automation: IUIAutomation,
    root: IUIAutomationElement,
}

impl VirtualizedClient {
    fn new(handle: WindowHandle) -> ToolResult<Self> {
        let initialize = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if initialize.is_err() {
            return Err(ToolError::Windows(WindowsError::from_hresult(initialize)));
        }
        let com = ComGuard;
        let automation: IUIAutomation =
            unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL)? };
        let root = unsafe { automation.ElementFromHandle(handle.hwnd())? };

        Ok(Self {
            _com: com,
            automation,
            root,
        })
    }

    fn true_condition(&self) -> ToolResult<IUIAutomationCondition> {
        Ok(unsafe { self.automation.CreateTrueCondition()? })
    }

    fn resolve(&self, path: &[u32]) -> ToolResult<IUIAutomationElement> {
        if path.len() > MAX_PATH_DEPTH {
            return Err(ToolError::InvalidArgument(
                "UI Automation path is unexpectedly deep".into(),
            ));
        }

        let condition = self.true_condition()?;
        let mut current = self.root.clone();
        for (depth, index) in path.iter().copied().enumerate() {
            let children = unsafe { current.FindAll(TreeScope_Children, &condition)? };
            let length = unsafe { children.Length()? }.max(0) as u32;
            if index >= length {
                return Err(ToolError::NotFound(format!(
                    "UI Automation path no longer exists at depth {depth}, child {index}"
                )));
            }
            current = unsafe { children.GetElement(index as i32)? };
        }
        Ok(current)
    }
}

fn pattern<T: Interface>(element: &IUIAutomationElement, pattern_id: UIA_PATTERN_ID) -> Option<T> {
    unsafe { element.GetCurrentPattern(pattern_id) }
        .ok()
        .and_then(|pattern| pattern.cast::<T>().ok())
}

struct ComGuard;

impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}
