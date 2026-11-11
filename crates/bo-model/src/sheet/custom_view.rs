//! Sheet view types and custom views (SHEET-VIEW-001):
//! * `sheetView/@view` (ECMA-376 Part 1, §18.3.1.87, §18.18.69 `ST_SheetViewType`),
//!   `@showFormulas` and the per-view zoom attributes, read from the attributes the
//!   model keeps on [`SheetView::attrs`];
//! * `customSheetViews` / `customSheetView` (§18.3.1.25–26) and the workbook's
//!   `customWorkbookViews` / `customWorkbookView` (§18.2.3 / §18.2.2).

use serde::{Deserialize, Serialize};

use super::{AutoFilter, PageSetup, Pane, RawAttr, RawXml, SheetView};

/// How a sheet is shown (§18.18.69 `ST_SheetViewType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SheetViewType {
    /// `normal`: the cell grid.
    #[default]
    Normal,
    /// `pageBreakPreview`: the grid with page regions and page breaks.
    PageBreakPreview,
    /// `pageLayout`: printed pages with margins, headers and footers.
    PageLayout,
}

impl SheetViewType {
    /// The `ST_SheetViewType` value (§18.18.69).
    pub fn as_str(self) -> &'static str {
        match self {
            SheetViewType::Normal => "normal",
            SheetViewType::PageBreakPreview => "pageBreakPreview",
            SheetViewType::PageLayout => "pageLayout",
        }
    }

    /// Parses an `ST_SheetViewType` value (unknown values: normal).
    pub fn parse(s: &str) -> SheetViewType {
        match s {
            "pageBreakPreview" => SheetViewType::PageBreakPreview,
            "pageLayout" => SheetViewType::PageLayout,
            _ => SheetViewType::Normal,
        }
    }

    /// The `sheetView` attribute holding the zoom of this view type (§18.3.1.87:
    /// `zoomScaleNormal`, `zoomScaleSheetLayoutView` for page break preview,
    /// `zoomScalePageLayoutView`).
    pub fn zoom_attr(self) -> &'static str {
        match self {
            SheetViewType::Normal => "zoomScaleNormal",
            SheetViewType::PageBreakPreview => "zoomScaleSheetLayoutView",
            SheetViewType::PageLayout => "zoomScalePageLayoutView",
        }
    }
}

impl SheetView {
    /// Value of a preserved attribute.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// Sets (`Some`) or removes (`None`) a preserved attribute, keeping its position.
    pub fn set_attr(&mut self, name: &str, value: Option<String>) {
        match (self.attrs.iter().position(|(k, _)| k == name), value) {
            (Some(i), Some(v)) => self.attrs[i].1 = v,
            (Some(i), None) => {
                self.attrs.remove(i);
            }
            (None, Some(v)) => self.attrs.push((name.to_owned(), v)),
            (None, None) => {}
        }
    }

    /// `@view` (§18.3.1.87; default normal).
    pub fn view_type(&self) -> SheetViewType {
        self.attr("view")
            .map_or(SheetViewType::Normal, SheetViewType::parse)
    }

    /// Sets `@view`; normal (the default) is not written.
    pub fn set_view_type(&mut self, v: SheetViewType) {
        let value = (v != SheetViewType::Normal).then(|| v.as_str().to_owned());
        self.set_attr("view", value);
    }

    /// `@showFormulas` (§18.3.1.87; default false).
    pub fn show_formulas(&self) -> bool {
        matches!(self.attr("showFormulas"), Some("1" | "true"))
    }

    /// Sets `@showFormulas`; false (the default) is not written.
    pub fn set_show_formulas(&mut self, on: bool) {
        self.set_attr("showFormulas", on.then(|| "1".to_owned()));
    }

    /// `@showOutlineSymbols` (§18.3.1.87; default true): the outline bar is shown.
    pub fn show_outline_symbols(&self) -> bool {
        !matches!(self.attr("showOutlineSymbols"), Some("0" | "false"))
    }

    /// Sets `@showOutlineSymbols`; true (the default) is not written.
    pub fn set_show_outline_symbols(&mut self, on: bool) {
        self.set_attr("showOutlineSymbols", (!on).then(|| "0".to_owned()));
    }

    /// Zoom percent stored for a view type (`0` or absent: none).
    pub fn zoom_of(&self, v: SheetViewType) -> Option<u32> {
        self.attr(v.zoom_attr())
            .and_then(|z| z.parse::<u32>().ok())
            .filter(|z| *z > 0)
    }
}

/// A custom view of one sheet (§18.3.1.25 `customSheetView`). The workbook-level
/// half with the name is a [`CustomWorkbookView`] with the same `guid`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CustomSheetView {
    /// `guid` (ST_Guid) shared with the [`CustomWorkbookView`].
    pub guid: String,
    /// `scale`: zoom percent (default 100).
    pub scale: Option<u32>,
    /// `showFormulas`.
    pub show_formulas: bool,
    /// `showGridLines` (default true).
    pub show_grid_lines: Option<bool>,
    /// `showRowCol`: row and column headers (default true).
    pub show_row_col: Option<bool>,
    /// `fitToPage`: the print settings fit to pages.
    pub fit_to_page: bool,
    /// `printArea`: the view includes a print area.
    pub print_area: bool,
    /// `filter`: the view includes an AutoFilter.
    pub filter: bool,
    /// `showAutoFilter`: AutoFilter buttons are shown.
    pub show_auto_filter: bool,
    /// `hiddenRows`: the view has hidden rows.
    pub hidden_rows: bool,
    /// `hiddenColumns`: the view has hidden columns.
    pub hidden_columns: bool,
    /// `state` (ST_SheetState): `visible` (default), `hidden`, `veryHidden`.
    pub state: Option<String>,
    /// `view` (ST_SheetViewType).
    pub view: Option<String>,
    /// `topLeftCell`.
    pub top_left_cell: Option<String>,
    /// Other attributes (colorId, showPageBreaks, outlineSymbols, zeroValues, …).
    pub attrs: Vec<RawAttr>,
    /// `pane` (§18.3.1.66).
    pub pane: Option<Pane>,
    /// `selection` (§18.3.1.78) attributes.
    pub selection: Option<Vec<RawAttr>>,
    /// `rowBreaks`, `colBreaks`, `pageMargins`, `printOptions`, `pageSetup`,
    /// `headerFooter` (same types as on the worksheet), when present.
    pub page_setup: Option<PageSetup>,
    /// `autoFilter` (§18.3.1.2).
    pub auto_filter: Option<AutoFilter>,
    /// Unknown children (`extLst`).
    pub unknown: Vec<RawXml>,
    /// Hidden row spans of the view. `customSheetView` only flags them
    /// (`hiddenRows`); the list is written as the hidden defined name
    /// `Z_<guid>_.wvu.Rows` (§18.2.5) and read back from it.
    #[serde(default)]
    pub session_hidden_rows: Option<Vec<(u32, u32)>>,
    /// Hidden column spans of the view (`Z_<guid>_.wvu.Cols`).
    #[serde(default)]
    pub session_hidden_cols: Option<Vec<(u32, u32)>>,
    /// Print area text of the view (`Z_<guid>_.wvu.PrintArea`).
    #[serde(default)]
    pub session_print_area: Option<Option<String>>,
}

/// The workbook half of a custom view (§18.2.2 `customWorkbookView`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CustomWorkbookView {
    /// `name`: what the user called the view.
    pub name: String,
    /// `guid` (ST_Guid) of the matching [`CustomSheetView`]s.
    pub guid: String,
    /// `activeSheetId`: `sheetId` of the active sheet.
    pub active_sheet_id: u32,
    /// `windowWidth` (required).
    pub window_width: u32,
    /// `windowHeight` (required).
    pub window_height: u32,
    /// `includePrintSettings` (default true).
    pub include_print_settings: bool,
    /// `includeHiddenRowCol` (default true).
    pub include_hidden_row_col: bool,
    /// Other attributes (personalView, maximized, xWindow, tabRatio, …).
    pub attrs: Vec<RawAttr>,
    /// Unknown children (`extLst`).
    pub unknown: Vec<RawXml>,
}
