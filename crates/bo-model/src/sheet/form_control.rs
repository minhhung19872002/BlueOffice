//! Form controls of a worksheet: buttons, check boxes, combo / list boxes, option buttons,
//! scroll bars, spin buttons, group boxes and labels ([MS-XLSX] `CT_FormControlPr` in a
//! control properties part, ECMA-376 Part 1 §18.3.1.21 `controls` / `control` with the
//! [MS-XLSX] `controlPr` extension, and the legacy VML `x:ClientData` shape that carries the
//! same settings), plus ActiveX controls kept as they are with their fallback picture.

use serde::{Deserialize, Serialize};

use super::CellRef;

/// What a form control is ([MS-XLSX] `ST_ObjectType`; VML `x:ClientData/@ObjectType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FormControlKind {
    /// Push button (`Button`); runs its assigned macro.
    #[default]
    Button,
    /// Check box (`CheckBox`, VML `Checkbox`): writes TRUE / FALSE to its linked cell.
    CheckBox,
    /// Drop-down combo box (`Drop`): writes the 1-based index of the chosen item.
    ComboBox,
    /// List box (`List`): writes the 1-based index of the chosen item.
    ListBox,
    /// Option button (`Radio`): writes the 1-based index of the chosen button of its group.
    OptionButton,
    /// Scroll bar (`Scroll`): writes its value.
    ScrollBar,
    /// Spin button (`Spin`): writes its value.
    Spinner,
    /// Group box (`GBox`): a captioned frame that groups option buttons.
    GroupBox,
    /// Label (`Label`).
    Label,
    /// An ActiveX control (`activeX` part): kept as is, shown through its fallback picture.
    ActiveX,
}

impl FormControlKind {
    /// The [MS-XLSX] `ST_ObjectType` value (`None` for ActiveX).
    #[must_use]
    pub fn object_type(self) -> Option<&'static str> {
        Some(match self {
            Self::Button => "Button",
            Self::CheckBox => "CheckBox",
            Self::ComboBox => "Drop",
            Self::ListBox => "List",
            Self::OptionButton => "Radio",
            Self::ScrollBar => "Scroll",
            Self::Spinner => "Spin",
            Self::GroupBox => "GBox",
            Self::Label => "Label",
            Self::ActiveX => return None,
        })
    }

    /// The VML `x:ClientData/@ObjectType` value (`None` for ActiveX).
    #[must_use]
    pub fn vml_object_type(self) -> Option<&'static str> {
        match self {
            Self::CheckBox => Some("Checkbox"),
            k => k.object_type(),
        }
    }

    /// Kind of an `ST_ObjectType` or VML `ObjectType` value (case-insensitive); `None` for
    /// kinds that are not modelled (edit boxes, dialog frames, notes, pictures …).
    #[must_use]
    pub fn from_object_type(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "button" => Self::Button,
            "checkbox" => Self::CheckBox,
            "drop" => Self::ComboBox,
            "list" => Self::ListBox,
            "radio" => Self::OptionButton,
            "scroll" => Self::ScrollBar,
            "spin" => Self::Spinner,
            "gbox" => Self::GroupBox,
            "label" => Self::Label,
            _ => return None,
        })
    }
}

/// State of a check box or option button ([MS-XLSX] `ST_Checked`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckState {
    /// Not checked (`Unchecked`, VML `0`).
    #[default]
    Unchecked,
    /// Checked (`Checked`, VML `1`).
    Checked,
    /// Mixed / indeterminate (`Mixed`, VML `2`).
    Mixed,
}

/// How a control follows the cells under it when rows / columns are inserted, deleted or
/// resized ([MS-XLSX] `CT_ObjectAnchor` `moveWithCells` / `sizeWithCells`; the DrawingML
/// twin's §20.5.2.33 `twoCellAnchor/@editAs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ControlPlacement {
    /// Moves with its top-left cell, keeps its size (`moveWithCells="1"`, the default of
    /// a new control).
    #[default]
    Move,
    /// Moves and sizes with the cells (`moveWithCells="1" sizeWithCells="1"`).
    MoveAndSize,
    /// Neither moves nor sizes (no attribute).
    Fixed,
}

/// Selection type of a list box ([MS-XLSX] `ST_SelType` attribute `seltype`; VML `x:SelType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SelType {
    /// One item (`single`): the chosen index goes to the linked cell.
    #[default]
    Single,
    /// A click toggles an item (`multi`); the linked cell is not used.
    Multi,
    /// A click chooses one item, Ctrl+click toggles, Shift+click extends (`extend`); the
    /// linked cell is not used.
    Extend,
}

impl SelType {
    /// The [MS-XLSX] `ST_SelType` value of `formControlPr/@seltype`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Multi => "multi",
            Self::Extend => "extended",
        }
    }

    /// The VML `x:SelType` text.
    #[must_use]
    pub fn vml_str(self) -> &'static str {
        match self {
            Self::Single => "Single",
            Self::Multi => "Multi",
            Self::Extend => "Extend",
        }
    }

    /// From an `ST_SelType` / VML value (case-insensitive).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "single" => Self::Single,
            "multi" => Self::Multi,
            "extend" | "extended" => Self::Extend,
            _ => return None,
        })
    }
}

/// Where a control sits: two cells with offsets in EMU
/// (`controlPr/anchor` `from` / `to`, ECMA-376 Part 1 §20.5.2.15 `CT_Marker`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlAnchor {
    /// Top-left cell (zero-based).
    pub from: CellRef,
    /// Offset into the top-left cell: (column offset, row offset) in EMU.
    pub from_off: (i64, i64),
    /// Bottom-right cell.
    pub to: CellRef,
    /// Offset into the bottom-right cell in EMU.
    pub to_off: (i64, i64),
}

/// A form control (or ActiveX control) on a worksheet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormControl {
    /// What the control is.
    pub kind: FormControlKind,
    /// `control/@name` (e.g. the name shown in the name box).
    pub name: String,
    /// Caption (VML `v:textbox`): button, check box, option button, group box, label.
    pub text: String,
    /// `checked` (check box, option button).
    pub checked: CheckState,
    /// `fmlaLink`: the linked cell (e.g. `$B$2` or `Sheet1!$B$2`).
    pub link: Option<String>,
    /// `fmlaRange`: the input range listing a combo / list box's items.
    pub input_range: Option<String>,
    /// `itemLst/item/@val`: items stored in the control itself (no input range).
    pub items: Vec<String>,
    /// `sel`: 1-based index of the chosen item (0 = none).
    pub sel: u32,
    /// `val`: current value of a scroll bar / spin button.
    pub val: i32,
    /// `min` (default 0).
    pub min: i32,
    /// `max` (default 100).
    pub max: i32,
    /// `inc`: step of the arrows (default 1).
    pub inc: i32,
    /// `page`: step of a click in a scroll bar's track (default 10).
    pub page: i32,
    /// `dropLines`: rows shown by an open combo box (default 8).
    pub drop_lines: u32,
    /// `horiz`: a horizontal scroll bar / spin button.
    pub horiz: bool,
    /// `noThreeD`: flat look.
    pub no_three_d: bool,
    /// `firstButton`: this option button starts a new group.
    pub first_button: bool,
    /// Macro assigned to the control (`controlPr/@macro`, VML `x:FmlaMacro`); kept, not run.
    pub macro_name: Option<String>,
    /// Placement.
    pub anchor: ControlAnchor,
    /// How the control follows its cells.
    pub placement: ControlPlacement,
    /// List box selection type.
    pub sel_type: SelType,
    /// Items chosen in a multi-select list box (1-based, ascending; [MS-XLSX] `multiSel`,
    /// VML `x:MultiSel`, comma-separated).
    pub multi_sel: Vec<u32>,
    /// `control/@shapeId` (also the VML shape `_x0000_s<id>`; 0 for a new control).
    pub shape_id: u32,
    /// Absolute name of the control properties part (empty when there is none).
    pub ctrl_prop_part: String,
    /// Absolute name of the VML drawing part holding the control's shape (empty if none).
    pub vml_part: String,
    /// Absolute name of an ActiveX control's `activeX` part (kept byte-for-byte).
    pub activex_part: String,
    /// Absolute name of the fallback picture (`controlPr/@r:id`), if any.
    pub image_part: Option<String>,
    /// The control changed since the file was read: its parts are patched on save.
    pub dirty: bool,
}

impl Default for FormControl {
    fn default() -> Self {
        FormControl {
            kind: FormControlKind::Button,
            name: String::new(),
            text: String::new(),
            checked: CheckState::Unchecked,
            link: None,
            input_range: None,
            items: Vec::new(),
            sel: 0,
            val: 0,
            min: 0,
            max: 100,
            inc: 1,
            page: 10,
            drop_lines: 8,
            horiz: false,
            no_three_d: false,
            first_button: false,
            macro_name: None,
            anchor: ControlAnchor::default(),
            placement: ControlPlacement::Move,
            sel_type: SelType::Single,
            multi_sel: Vec::new(),
            shape_id: 0,
            ctrl_prop_part: String::new(),
            vml_part: String::new(),
            activex_part: String::new(),
            image_part: None,
            dirty: false,
        }
    }
}

impl FormControl {
    /// A control created in this session (no part in the package yet).
    #[must_use]
    pub fn is_new(&self) -> bool {
        self.ctrl_prop_part.is_empty() && self.vml_part.is_empty() && self.activex_part.is_empty()
    }
}
