use serde::Serialize;
use windows::{
    core::{Error as WindowsError, Interface},
    Win32::{
        Foundation::RECT,
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
        },
        UI::Accessibility::{
            CUIAutomation, IUIAutomation, IUIAutomationCondition, IUIAutomationElement,
            IUIAutomationInvokePattern, IUIAutomationValuePattern, TreeScope_Children,
            UIA_InvokePatternId, UIA_ValuePatternId,
        },
    },
};

use crate::{window::WindowHandle, ToolError, ToolResult};

const DEFAULT_MAX_DEPTH: u32 = 4;
const DEFAULT_MAX_NODES: usize = 160;
const HARD_MAX_DEPTH: u32 = 8;
const HARD_MAX_NODES: usize = 500;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct UiRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl From<RECT> for UiRect {
    fn from(value: RECT) -> Self {
        Self {
            left: value.left,
            top: value.top,
            right: value.right,
            bottom: value.bottom,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UiElementSnapshot {
    /// Stable only for the current UI tree shape. Each number is the child index
    /// in the UI Automation Control View from the source window root.
    pub path: Vec<u32>,
    pub depth: u32,
    pub name: String,
    pub automation_id: String,
    pub class_name: String,
    pub localized_control_type: String,
    pub control_type: i32,
    pub process_id: i32,
    pub enabled: bool,
    pub keyboard_focusable: bool,
    pub has_keyboard_focus: bool,
    pub offscreen: bool,
    pub bounds: Option<UiRect>,
    pub supports_invoke: bool,
    pub supports_value: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct UiTreeSnapshot {
    pub root_window_handle: isize,
    pub max_depth: u32,
    pub max_nodes: usize,
    pub node_count: usize,
    pub truncated: bool,
    pub nodes: Vec<UiElementSnapshot>,
}

#[derive(Debug, Clone, Copy)]
pub struct UiInspectOptions {
    pub max_depth: u32,
    pub max_nodes: usize,
}

impl Default for UiInspectOptions {
    fn default() -> Self {
        Self {
            max_depth: DEFAULT_MAX_DEPTH,
            max_nodes: DEFAULT_MAX_NODES,
        }
    }
}

impl UiInspectOptions {
    fn normalized(self) -> Self {
        Self {
            max_depth: self.max_depth.min(HARD_MAX_DEPTH),
            max_nodes: self.max_nodes.clamp(1, HARD_MAX_NODES),
        }
    }
}

/// Inspect the UI Automation Control View for one Windows HWND.
///
/// This intentionally does not read ValuePattern text. The tree is structural
/// context only; field content is retrieved or modified only through an explicit
/// action in a later layer.
pub fn inspect(handle: WindowHandle, options: UiInspectOptions) -> ToolResult<UiTreeSnapshot> {
    let options = options.normalized();
    let client = AutomationClient::new(handle)?;
    let condition = client.true_condition()?;

    let mut nodes = Vec::with_capacity(options.max_nodes.min(64));
    let mut stack = vec![(client.root.clone(), Vec::<u32>::new(), 0u32)];
    let mut truncated = false;

    while let Some((element, path, depth)) = stack.pop() {
        if nodes.len() >= options.max_nodes {
            truncated = true;
            break;
        }

        nodes.push(snapshot_element(&element, path.clone(), depth));

        if depth >= options.max_depth {
            continue;
        }

        let children = children_of(&element, &condition)?;
        if nodes.len().saturating_add(children.len()) > options.max_nodes {
            truncated = true;
        }

        // Reverse push preserves native UIA child order when using a LIFO stack.
        for (index, child) in children.into_iter().enumerate().rev() {
            let mut child_path = path.clone();
            child_path.push(index as u32);
            stack.push((child, child_path, depth + 1));
        }
    }

    Ok(UiTreeSnapshot {
        root_window_handle: handle.0,
        max_depth: options.max_depth,
        max_nodes: options.max_nodes,
        node_count: nodes.len(),
        truncated,
        nodes,
    })
}

/// Move keyboard focus to a UIA element resolved from a previously returned path.
pub fn focus(handle: WindowHandle, path: &[u32]) -> ToolResult<()> {
    let client = AutomationClient::new(handle)?;
    let element = client.resolve(path)?;
    unsafe { element.SetFocus()? };
    Ok(())
}

/// Invoke a control that exposes the UIA Invoke pattern, e.g. many buttons and
/// command items. This does not synthesize a mouse click.
pub fn invoke(handle: WindowHandle, path: &[u32]) -> ToolResult<()> {
    let client = AutomationClient::new(handle)?;
    let element = client.resolve(path)?;
    let pattern = unsafe { element.GetCurrentPattern(UIA_InvokePatternId)? };
    let pattern: IUIAutomationInvokePattern = pattern
        .cast()
        .map_err(|_| ToolError::Unsupported("element does not expose InvokePattern".into()))?;
    unsafe { pattern.Invoke()? };
    Ok(())
}

/// Set text/value on a control that exposes a writable UIA Value pattern.
pub fn set_value(handle: WindowHandle, path: &[u32], value: &str) -> ToolResult<()> {
    if value.len() > 32_768 {
        return Err(ToolError::InvalidArgument(
            "UI Automation value exceeds 32768 bytes".into(),
        ));
    }

    let client = AutomationClient::new(handle)?;
    let element = client.resolve(path)?;
    let pattern = unsafe { element.GetCurrentPattern(UIA_ValuePatternId)? };
    let pattern: IUIAutomationValuePattern = pattern
        .cast()
        .map_err(|_| ToolError::Unsupported("element does not expose ValuePattern".into()))?;

    let read_only = unsafe { pattern.CurrentIsReadOnly()? }.as_bool();
    if read_only {
        return Err(ToolError::Unsupported(
            "element ValuePattern is read-only".into(),
        ));
    }

    let value = windows::core::BSTR::from(value);
    unsafe { pattern.SetValue(&value)? };
    Ok(())
}

struct AutomationClient {
    _com: ComGuard,
    automation: IUIAutomation,
    root: IUIAutomationElement,
}

impl AutomationClient {
    fn new(handle: WindowHandle) -> ToolResult<Self> {
        let initialize = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if initialize.is_err() {
            return Err(ToolError::Windows(WindowsError::from_hresult(initialize)));
        }
        let com = ComGuard;

        let automation: IUIAutomation = unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL)? };
        let root = unsafe { automation.ElementFromHandle(handle.hwnd().0)? };

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
        if path.len() > HARD_MAX_DEPTH as usize + 8 {
            return Err(ToolError::InvalidArgument(
                "UI Automation path is unexpectedly deep".into(),
            ));
        }

        let condition = self.true_condition()?;
        let mut current = self.root.clone();

        for (depth, index) in path.iter().copied().enumerate() {
            let children = unsafe { current.FindAll(TreeScope_Children, &condition)? };
            let length = unsafe { children.Length()? };
            if index >= length.max(0) as u32 {
                return Err(ToolError::NotFound(format!(
                    "UI Automation path no longer exists at depth {depth}, child {index}"
                )));
            }
            current = unsafe { children.GetElement(index as i32)? };
        }

        Ok(current)
    }
}

fn children_of(
    element: &IUIAutomationElement,
    condition: &IUIAutomationCondition,
) -> ToolResult<Vec<IUIAutomationElement>> {
    let found = unsafe { element.FindAll(TreeScope_Children, condition)? };
    let length = unsafe { found.Length()? }.max(0) as usize;
    let mut children = Vec::with_capacity(length);
    for index in 0..length {
        children.push(unsafe { found.GetElement(index as i32)? });
    }
    Ok(children)
}

fn snapshot_element(
    element: &IUIAutomationElement,
    path: Vec<u32>,
    depth: u32,
) -> UiElementSnapshot {
    UiElementSnapshot {
        path,
        depth,
        name: bstr_or_default(|| unsafe { element.CurrentName() }),
        automation_id: bstr_or_default(|| unsafe { element.CurrentAutomationId() }),
        class_name: bstr_or_default(|| unsafe { element.CurrentClassName() }),
        localized_control_type: bstr_or_default(|| unsafe {
            element.CurrentLocalizedControlType()
        }),
        control_type: unsafe { element.CurrentControlType() }.unwrap_or_default(),
        process_id: unsafe { element.CurrentProcessId() }.unwrap_or_default(),
        enabled: unsafe { element.CurrentIsEnabled() }
            .map(|value| value.as_bool())
            .unwrap_or(false),
        keyboard_focusable: unsafe { element.CurrentIsKeyboardFocusable() }
            .map(|value| value.as_bool())
            .unwrap_or(false),
        has_keyboard_focus: unsafe { element.CurrentHasKeyboardFocus() }
            .map(|value| value.as_bool())
            .unwrap_or(false),
        offscreen: unsafe { element.CurrentIsOffscreen() }
            .map(|value| value.as_bool())
            .unwrap_or(false),
        bounds: unsafe { element.CurrentBoundingRectangle() }
            .ok()
            .map(UiRect::from),
        supports_invoke: supports_pattern::<IUIAutomationInvokePattern>(element, UIA_InvokePatternId),
        supports_value: supports_pattern::<IUIAutomationValuePattern>(element, UIA_ValuePatternId),
    }
}

fn supports_pattern<T: Interface>(element: &IUIAutomationElement, pattern_id: i32) -> bool {
    unsafe { element.GetCurrentPattern(pattern_id) }
        .and_then(|pattern| pattern.cast::<T>())
        .is_ok()
}

fn bstr_or_default(
    getter: impl FnOnce() -> windows::core::Result<windows::core::BSTR>,
) -> String {
    getter().map(|value| value.to_string()).unwrap_or_default()
}

struct ComGuard;

impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}
