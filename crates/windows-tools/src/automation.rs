use serde::{Deserialize, Serialize};
use windows::{
    core::{Error as WindowsError, Interface},
    Win32::{
        Foundation::RECT,
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
        },
        UI::Accessibility::{
            CUIAutomation, ExpandCollapseState, ExpandCollapseState_Collapsed,
            ExpandCollapseState_Expanded, ExpandCollapseState_LeafNode,
            ExpandCollapseState_PartiallyExpanded, IUIAutomation,
            IUIAutomationCondition, IUIAutomationElement, IUIAutomationExpandCollapsePattern,
            IUIAutomationGridItemPattern, IUIAutomationGridPattern, IUIAutomationInvokePattern,
            IUIAutomationRangeValuePattern, IUIAutomationScrollItemPattern,
            IUIAutomationScrollPattern, IUIAutomationSelectionItemPattern,
            IUIAutomationTogglePattern, IUIAutomationValuePattern,
            IUIAutomationVirtualizedItemPattern, ScrollAmount, ScrollAmount_LargeDecrement,
            ScrollAmount_LargeIncrement, ScrollAmount_NoAmount, ScrollAmount_SmallDecrement,
            ScrollAmount_SmallIncrement, ToggleState, ToggleState_Indeterminate, ToggleState_Off,
            ToggleState_On, TreeScope_Children, UIA_InvokePatternId, UIA_ValuePatternId,
        },
    },
};

use crate::{window::WindowHandle, ToolError, ToolResult};

const DEFAULT_MAX_DEPTH: u32 = 4;
const DEFAULT_MAX_NODES: usize = 160;
const HARD_MAX_DEPTH: u32 = 8;
const HARD_MAX_NODES: usize = 500;

// Microsoft UI Automation control-pattern identifiers. Keeping these local avoids
// coupling pattern support to generated constant availability while matching the
// stable UIAutomationClient.h values.
const UIA_RANGE_VALUE_PATTERN_ID: i32 = 10003;
const UIA_SCROLL_PATTERN_ID: i32 = 10004;
const UIA_EXPAND_COLLAPSE_PATTERN_ID: i32 = 10005;
const UIA_GRID_PATTERN_ID: i32 = 10006;
const UIA_GRID_ITEM_PATTERN_ID: i32 = 10007;
const UIA_SELECTION_ITEM_PATTERN_ID: i32 = 10010;
const UIA_TOGGLE_PATTERN_ID: i32 = 10015;
const UIA_SCROLL_ITEM_PATTERN_ID: i32 = 10017;
const UIA_VIRTUALIZED_ITEM_PATTERN_ID: i32 = 10020;

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

#[derive(Debug, Clone, Copy, Serialize)]
pub struct UiScrollSnapshot {
    pub horizontally_scrollable: bool,
    pub vertically_scrollable: bool,
    pub horizontal_percent: f64,
    pub vertical_percent: f64,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct UiRangeValueSnapshot {
    pub value: f64,
    pub minimum: f64,
    pub maximum: f64,
    pub small_change: f64,
    pub large_change: f64,
    pub read_only: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct UiGridSnapshot {
    pub row_count: i32,
    pub column_count: i32,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct UiGridItemSnapshot {
    pub row: i32,
    pub column: i32,
    pub row_span: i32,
    pub column_span: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiToggleState {
    Off,
    On,
    Indeterminate,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiExpandCollapseState {
    Collapsed,
    Expanded,
    PartiallyExpanded,
    Leaf,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiScrollAmount {
    LargeDecrement,
    SmallDecrement,
    None,
    LargeIncrement,
    SmallIncrement,
}

impl UiScrollAmount {
    fn native(self) -> ScrollAmount {
        match self {
            Self::LargeDecrement => ScrollAmount_LargeDecrement,
            Self::SmallDecrement => ScrollAmount_SmallDecrement,
            Self::None => ScrollAmount_NoAmount,
            Self::LargeIncrement => ScrollAmount_LargeIncrement,
            Self::SmallIncrement => ScrollAmount_SmallIncrement,
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
    pub supports_scroll_item: bool,
    pub supports_virtualized_item: bool,
    pub range_value: Option<UiRangeValueSnapshot>,
    pub grid: Option<UiGridSnapshot>,
    pub grid_item: Option<UiGridItemSnapshot>,
    pub toggle_state: Option<UiToggleState>,
    pub is_selected: Option<bool>,
    pub expand_collapse_state: Option<UiExpandCollapseState>,
    pub scroll: Option<UiScrollSnapshot>,
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
/// action in a later layer. Numeric RangeValue state and grid coordinates are
/// exposed because they are bounded structural control state, not arbitrary text.
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
    let pattern = get_pattern::<IUIAutomationInvokePattern>(
        &element,
        UIA_InvokePatternId,
        "InvokePattern",
    )?;
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
    let pattern = get_pattern::<IUIAutomationValuePattern>(
        &element,
        UIA_ValuePatternId,
        "ValuePattern",
    )?;

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

/// Set a bounded numeric control through UIA RangeValuePattern.
pub fn set_range_value(handle: WindowHandle, path: &[u32], value: f64) -> ToolResult<()> {
    if !value.is_finite() {
        return Err(ToolError::InvalidArgument(
            "UI Automation range value must be finite".into(),
        ));
    }

    let client = AutomationClient::new(handle)?;
    let element = client.resolve(path)?;
    let pattern = get_pattern::<IUIAutomationRangeValuePattern>(
        &element,
        UIA_RANGE_VALUE_PATTERN_ID,
        "RangeValuePattern",
    )?;

    if unsafe { pattern.CurrentIsReadOnly()? }.as_bool() {
        return Err(ToolError::Unsupported(
            "element RangeValuePattern is read-only".into(),
        ));
    }

    let minimum = unsafe { pattern.CurrentMinimum()? };
    let maximum = unsafe { pattern.CurrentMaximum()? };
    if value < minimum || value > maximum {
        return Err(ToolError::InvalidArgument(format!(
            "UI Automation range value {value} is outside [{minimum}, {maximum}]"
        )));
    }

    unsafe { pattern.SetValue(value)? };
    Ok(())
}

/// Toggle a checkbox/switch-like element using UIA TogglePattern.
pub fn toggle(handle: WindowHandle, path: &[u32]) -> ToolResult<()> {
    let client = AutomationClient::new(handle)?;
    let element = client.resolve(path)?;
    let pattern = get_pattern::<IUIAutomationTogglePattern>(
        &element,
        UIA_TOGGLE_PATTERN_ID,
        "TogglePattern",
    )?;
    unsafe { pattern.Toggle()? };
    Ok(())
}

/// Select one item using UIA SelectionItemPattern. This intentionally uses
/// `Select` rather than additive selection in the first public contract.
pub fn select(handle: WindowHandle, path: &[u32]) -> ToolResult<()> {
    let client = AutomationClient::new(handle)?;
    let element = client.resolve(path)?;
    let pattern = get_pattern::<IUIAutomationSelectionItemPattern>(
        &element,
        UIA_SELECTION_ITEM_PATTERN_ID,
        "SelectionItemPattern",
    )?;
    unsafe { pattern.Select()? };
    Ok(())
}

/// Expand or collapse an element through UIA ExpandCollapsePattern.
pub fn set_expanded(handle: WindowHandle, path: &[u32], expanded: bool) -> ToolResult<()> {
    let client = AutomationClient::new(handle)?;
    let element = client.resolve(path)?;
    let pattern = get_pattern::<IUIAutomationExpandCollapsePattern>(
        &element,
        UIA_EXPAND_COLLAPSE_PATTERN_ID,
        "ExpandCollapsePattern",
    )?;
    unsafe {
        if expanded {
            pattern.Expand()?;
        } else {
            pattern.Collapse()?;
        }
    }
    Ok(())
}

/// Scroll a UIA scroll container by discrete horizontal/vertical amounts.
pub fn scroll(
    handle: WindowHandle,
    path: &[u32],
    horizontal: UiScrollAmount,
    vertical: UiScrollAmount,
) -> ToolResult<()> {
    if matches!(horizontal, UiScrollAmount::None) && matches!(vertical, UiScrollAmount::None) {
        return Err(ToolError::InvalidArgument(
            "at least one scroll axis must request a non-none amount".into(),
        ));
    }

    let client = AutomationClient::new(handle)?;
    let element = client.resolve(path)?;
    let pattern = get_pattern::<IUIAutomationScrollPattern>(
        &element,
        UIA_SCROLL_PATTERN_ID,
        "ScrollPattern",
    )?;
    unsafe { pattern.Scroll(horizontal.native(), vertical.native())? };
    Ok(())
}

/// Ask the element's owning scroll container to bring the item into its viewport.
/// UI Automation chooses the final position inside the viewport; no coordinates
/// or raw wheel input are synthesized.
pub fn scroll_into_view(handle: WindowHandle, path: &[u32]) -> ToolResult<()> {
    let client = AutomationClient::new(handle)?;
    let element = client.resolve(path)?;
    let pattern = get_pattern::<IUIAutomationScrollItemPattern>(
        &element,
        UIA_SCROLL_ITEM_PATTERN_ID,
        "ScrollItemPattern",
    )?;
    unsafe { pattern.ScrollIntoView()? };
    Ok(())
}

/// Materialize a virtualized UIA item through VirtualizedItemPattern. This does
/// not select, focus or invoke the item; it asks the provider to fully realize it.
pub fn realize(handle: WindowHandle, path: &[u32]) -> ToolResult<()> {
    let client = AutomationClient::new(handle)?;
    let element = client.resolve(path)?;
    let pattern = get_pattern::<IUIAutomationVirtualizedItemPattern>(
        &element,
        UIA_VIRTUALIZED_ITEM_PATTERN_ID,
        "VirtualizedItemPattern",
    )?;
    unsafe { pattern.Realize()? };
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
        supports_scroll_item: supports_pattern::<IUIAutomationScrollItemPattern>(
            element,
            UIA_SCROLL_ITEM_PATTERN_ID,
        ),
        supports_virtualized_item: supports_pattern::<IUIAutomationVirtualizedItemPattern>(
            element,
            UIA_VIRTUALIZED_ITEM_PATTERN_ID,
        ),
        range_value: pattern::<IUIAutomationRangeValuePattern>(element, UIA_RANGE_VALUE_PATTERN_ID)
            .and_then(|pattern| {
                Some(UiRangeValueSnapshot {
                    value: unsafe { pattern.CurrentValue().ok()? },
                    minimum: unsafe { pattern.CurrentMinimum().ok()? },
                    maximum: unsafe { pattern.CurrentMaximum().ok()? },
                    small_change: unsafe { pattern.CurrentSmallChange().ok()? },
                    large_change: unsafe { pattern.CurrentLargeChange().ok()? },
                    read_only: unsafe { pattern.CurrentIsReadOnly().ok()? }.as_bool(),
                })
            }),
        grid: pattern::<IUIAutomationGridPattern>(element, UIA_GRID_PATTERN_ID).and_then(|pattern| {
            Some(UiGridSnapshot {
                row_count: unsafe { pattern.CurrentRowCount().ok()? },
                column_count: unsafe { pattern.CurrentColumnCount().ok()? },
            })
        }),
        grid_item: pattern::<IUIAutomationGridItemPattern>(element, UIA_GRID_ITEM_PATTERN_ID)
            .and_then(|pattern| {
                Some(UiGridItemSnapshot {
                    row: unsafe { pattern.CurrentRow().ok()? },
                    column: unsafe { pattern.CurrentColumn().ok()? },
                    row_span: unsafe { pattern.CurrentRowSpan().ok()? },
                    column_span: unsafe { pattern.CurrentColumnSpan().ok()? },
                })
            }),
        toggle_state: pattern::<IUIAutomationTogglePattern>(element, UIA_TOGGLE_PATTERN_ID)
            .and_then(|pattern| unsafe { pattern.CurrentToggleState().ok() })
            .map(normalize_toggle_state),
        is_selected: pattern::<IUIAutomationSelectionItemPattern>(element, UIA_SELECTION_ITEM_PATTERN_ID)
            .and_then(|pattern| unsafe { pattern.CurrentIsSelected().ok() })
            .map(|value| value.as_bool()),
        expand_collapse_state: pattern::<IUIAutomationExpandCollapsePattern>(
            element,
            UIA_EXPAND_COLLAPSE_PATTERN_ID,
        )
        .and_then(|pattern| unsafe { pattern.CurrentExpandCollapseState().ok() })
        .map(normalize_expand_collapse_state),
        scroll: pattern::<IUIAutomationScrollPattern>(element, UIA_SCROLL_PATTERN_ID)
            .map(|pattern| UiScrollSnapshot {
                horizontally_scrollable: unsafe { pattern.CurrentHorizontallyScrollable() }
                    .map(|value| value.as_bool())
                    .unwrap_or(false),
                vertically_scrollable: unsafe { pattern.CurrentVerticallyScrollable() }
                    .map(|value| value.as_bool())
                    .unwrap_or(false),
                horizontal_percent: unsafe { pattern.CurrentHorizontalScrollPercent() }
                    .unwrap_or(-1.0),
                vertical_percent: unsafe { pattern.CurrentVerticalScrollPercent() }
                    .unwrap_or(-1.0),
            }),
    }
}

fn normalize_toggle_state(state: ToggleState) -> UiToggleState {
    if state == ToggleState_Off {
        UiToggleState::Off
    } else if state == ToggleState_On {
        UiToggleState::On
    } else if state == ToggleState_Indeterminate {
        UiToggleState::Indeterminate
    } else {
        UiToggleState::Unknown
    }
}

fn normalize_expand_collapse_state(state: ExpandCollapseState) -> UiExpandCollapseState {
    if state == ExpandCollapseState_Collapsed {
        UiExpandCollapseState::Collapsed
    } else if state == ExpandCollapseState_Expanded {
        UiExpandCollapseState::Expanded
    } else if state == ExpandCollapseState_PartiallyExpanded {
        UiExpandCollapseState::PartiallyExpanded
    } else if state == ExpandCollapseState_LeafNode {
        UiExpandCollapseState::Leaf
    } else {
        UiExpandCollapseState::Unknown
    }
}

fn pattern<T: Interface>(element: &IUIAutomationElement, pattern_id: i32) -> Option<T> {
    unsafe { element.GetCurrentPattern(pattern_id) }
        .ok()
        .and_then(|pattern| pattern.cast::<T>().ok())
}

fn get_pattern<T: Interface>(
    element: &IUIAutomationElement,
    pattern_id: i32,
    name: &str,
) -> ToolResult<T> {
    pattern::<T>(element, pattern_id)
        .ok_or_else(|| ToolError::Unsupported(format!("element does not expose {name}")))
}

fn supports_pattern<T: Interface>(element: &IUIAutomationElement, pattern_id: i32) -> bool {
    pattern::<T>(element, pattern_id).is_some()
}

fn bstr_or_default(
    getter: impl FnOnce() -> windows::core::Result<windows::core::BSTR>,
) -> String {
    getter.map(|_| windows::core::BSTR::new());
    getter().map(|value| value.to_string()).unwrap_or_default()
}

struct ComGuard;

impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}
