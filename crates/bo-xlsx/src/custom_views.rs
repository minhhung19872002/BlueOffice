//! Custom views (SHEET-VIEW-001): the worksheet's `customSheetViews` /
//! `customSheetView` (ECMA-376 Part 1, §18.3.1.26 / §18.3.1.25) and the
//! workbook's `customWorkbookViews` / `customWorkbookView` (§18.2.3 / §18.2.2).
//! Print settings inside a `customSheetView` use the worksheet's own element
//! types and go through the same reader / writer.

use crate::read::{read_page_part, read_pane};
use crate::write::{write_page_part, write_pane};
use crate::xml::{
    a, a_opt, a_optb, a_raw, a_true, attrs, capture, emit_unknown, for_children, lname,
    push_unknown, take, take_bool, take_u32,
};
use crate::Result;
use bo_model::sheet::{CustomSheetView, CustomWorkbookView, PageSetup, Workbook, Worksheet};
use bo_opc::xml::XmlReader;
use quick_xml::events::BytesStart;

/// Child sequence of `customSheetView` (§18.3.1.25 CT_CustomSheetView).
const SHEET_VIEW_SCHEMA: &[&str] = &[
    "pane",
    "selection",
    "rowBreaks",
    "colBreaks",
    "pageMargins",
    "printOptions",
    "pageSetup",
    "headerFooter",
    "autoFilter",
    "extLst",
];

/// Print setting children of `customSheetView`, in schema order.
const PAGE_PARTS: &[&str] = &[
    "rowBreaks",
    "colBreaks",
    "pageMargins",
    "printOptions",
    "pageSetup",
    "headerFooter",
];

/// Reads `customSheetViews` (§18.3.1.26) whose start tag `e` was just consumed.
pub(crate) fn read_custom_sheet_views(
    r: &mut XmlReader<'_>,
    empty: bool,
) -> Result<Vec<CustomSheetView>> {
    let mut out = Vec::new();
    if empty {
        return Ok(out);
    }
    for_children(r, |r, e, empty| {
        if lname(e) == "customSheetView" {
            out.push(read_custom_sheet_view(r, e, empty)?);
        }
        Ok(())
    })?;
    Ok(out)
}

/// §18.3.1.25 customSheetView.
fn read_custom_sheet_view(
    r: &mut XmlReader<'_>,
    e: &BytesStart<'_>,
    empty: bool,
) -> Result<CustomSheetView> {
    let mut at = attrs(e);
    let mut v = CustomSheetView {
        guid: take(&mut at, "guid").unwrap_or_default(),
        scale: take_u32(&mut at, "scale"),
        show_formulas: take_bool(&mut at, "showFormulas").unwrap_or(false),
        show_grid_lines: take_bool(&mut at, "showGridLines"),
        show_row_col: take_bool(&mut at, "showRowCol"),
        fit_to_page: take_bool(&mut at, "fitToPage").unwrap_or(false),
        print_area: take_bool(&mut at, "printArea").unwrap_or(false),
        filter: take_bool(&mut at, "filter").unwrap_or(false),
        show_auto_filter: take_bool(&mut at, "showAutoFilter").unwrap_or(false),
        hidden_rows: take_bool(&mut at, "hiddenRows").unwrap_or(false),
        hidden_columns: take_bool(&mut at, "hiddenColumns").unwrap_or(false),
        state: take(&mut at, "state"),
        view: take(&mut at, "view"),
        top_left_cell: take(&mut at, "topLeftCell"),
        attrs: at,
        ..CustomSheetView::default()
    };
    if empty {
        return Ok(v);
    }
    let mut prev = String::new();
    for_children(r, |r, e, empty| {
        let name = lname(e);
        match name.as_str() {
            // §18.3.1.66 pane
            "pane" => v.pane = Some(read_pane(e)),
            // §18.3.1.78 selection
            "selection" => v.selection = Some(attrs(e)),
            n if PAGE_PARTS.contains(&n) => {
                let p = v.page_setup.get_or_insert_with(PageSetup::default);
                read_page_part(r, e, empty, n, p)?;
            }
            // §18.3.1.2 autoFilter
            "autoFilter" => {
                v.auto_filter = Some(crate::autofilter::read_auto_filter(r, e, empty)?);
            }
            _ => {
                let xml = capture(r, e, empty)?;
                push_unknown(&mut v.unknown, &mut prev, SHEET_VIEW_SCHEMA, &name, xml);
                return Ok(());
            }
        }
        prev = name;
        Ok(())
    })?;
    Ok(v)
}

/// Writes `customSheetViews` (§18.3.1.26); nothing when there are none.
pub(crate) fn write_custom_sheet_views(out: &mut String, views: &[CustomSheetView]) {
    if views.is_empty() {
        return;
    }
    out.push_str("<customSheetViews>");
    for v in views {
        out.push_str("<customSheetView");
        a(out, "guid", &v.guid);
        a_opt(out, "scale", v.scale.as_ref());
        a_true(out, "showFormulas", v.show_formulas);
        a_optb(out, "showGridLines", v.show_grid_lines);
        a_optb(out, "showRowCol", v.show_row_col);
        a_true(out, "fitToPage", v.fit_to_page);
        a_true(out, "printArea", v.print_area);
        a_true(out, "filter", v.filter);
        a_true(out, "showAutoFilter", v.show_auto_filter);
        a_true(out, "hiddenRows", v.hidden_rows);
        a_true(out, "hiddenColumns", v.hidden_columns);
        a_opt(out, "state", v.state.as_ref());
        a_opt(out, "view", v.view.as_ref());
        a_opt(out, "topLeftCell", v.top_left_cell.as_ref());
        a_raw(out, &v.attrs);
        out.push('>');
        emit_unknown(out, &v.unknown, "");
        for slot in SHEET_VIEW_SCHEMA {
            match *slot {
                "pane" => {
                    if let Some(p) = &v.pane {
                        write_pane(out, p);
                    }
                }
                "selection" => {
                    if let Some(s) = &v.selection {
                        out.push_str("<selection");
                        a_raw(out, s);
                        out.push_str("/>");
                    }
                }
                "autoFilter" => {
                    if let Some(af) = &v.auto_filter {
                        crate::autofilter::write_auto_filter(out, af);
                    }
                }
                s if PAGE_PARTS.contains(&s) => {
                    if let Some(p) = &v.page_setup {
                        write_page_part(out, s, p);
                    }
                }
                _ => {}
            }
            emit_unknown(out, &v.unknown, slot);
        }
        out.push_str("</customSheetView>");
    }
    out.push_str("</customSheetViews>");
}

/// Reads `customWorkbookViews` (§18.2.3) whose start tag was just consumed.
pub(crate) fn read_custom_workbook_views(
    r: &mut XmlReader<'_>,
    empty: bool,
) -> Result<Vec<CustomWorkbookView>> {
    let mut out = Vec::new();
    if empty {
        return Ok(out);
    }
    for_children(r, |r, e, empty| {
        if lname(e) != "customWorkbookView" {
            return Ok(());
        }
        // §18.2.2 customWorkbookView
        let mut at = attrs(e);
        let mut v = CustomWorkbookView {
            name: take(&mut at, "name").unwrap_or_default(),
            guid: take(&mut at, "guid").unwrap_or_default(),
            active_sheet_id: take_u32(&mut at, "activeSheetId").unwrap_or(0),
            window_width: take_u32(&mut at, "windowWidth").unwrap_or(0),
            window_height: take_u32(&mut at, "windowHeight").unwrap_or(0),
            include_print_settings: take_bool(&mut at, "includePrintSettings").unwrap_or(true),
            include_hidden_row_col: take_bool(&mut at, "includeHiddenRowCol").unwrap_or(true),
            attrs: at,
            unknown: Vec::new(),
        };
        if !empty {
            let mut prev = String::new();
            for_children(r, |r, e, empty| {
                let name = lname(e);
                let xml = capture(r, e, empty)?;
                push_unknown(&mut v.unknown, &mut prev, &["extLst"], &name, xml);
                Ok(())
            })?;
        }
        out.push(v);
        Ok(())
    })?;
    Ok(out)
}

/// Writes `customWorkbookViews` (§18.2.3); nothing when there are none.
pub(crate) fn write_custom_workbook_views(out: &mut String, views: &[CustomWorkbookView]) {
    if views.is_empty() {
        return;
    }
    out.push_str("<customWorkbookViews>");
    for v in views {
        out.push_str("<customWorkbookView");
        a(out, "name", &v.name);
        a(out, "guid", &v.guid);
        // Defaults are true (§18.2.2): only "off" is written.
        if !v.include_print_settings {
            a(out, "includePrintSettings", "0");
        }
        if !v.include_hidden_row_col {
            a(out, "includeHiddenRowCol", "0");
        }
        a_raw(out, &v.attrs);
        a(out, "windowWidth", &v.window_width.to_string());
        a(out, "windowHeight", &v.window_height.to_string());
        a(out, "activeSheetId", &v.active_sheet_id.max(1).to_string());
        if v.unknown.is_empty() {
            out.push_str("/>");
        } else {
            out.push('>');
            emit_unknown(out, &v.unknown, "");
            emit_unknown(out, &v.unknown, "extLst");
            out.push_str("</customWorkbookView>");
        }
    }
    out.push_str("</customWorkbookViews>");
}

// ---------------------------------------------------------------------------
// Hidden rows / columns and print area of a custom view
// ---------------------------------------------------------------------------
//
// `customSheetView` only flags them (§18.3.1.25 `hiddenRows`, `hiddenColumns`,
// `printArea`); the lists themselves travel as hidden sheet-scoped defined names
// (§18.2.5 `definedName`) named after the view's GUID — `Z_<guid>_.wvu.Rows`,
// `.wvu.Cols`, `.wvu.PrintArea`, the GUID without braces and with `_` for `-`
// (the convention seen in files written by common spreadsheet applications).

/// What a view name holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewName {
    Rows,
    Cols,
    PrintArea,
}

/// The GUID part of a view name: no braces, `_` for `-`, upper case.
fn guid_key(guid: &str) -> String {
    guid.trim_matches(|c| c == '{' || c == '}')
        .replace('-', "_")
        .to_ascii_uppercase()
}

fn view_name(guid: &str, kind: ViewName) -> String {
    let suffix = match kind {
        ViewName::Rows => "Rows",
        ViewName::Cols => "Cols",
        ViewName::PrintArea => "PrintArea",
    };
    format!("Z_{}_.wvu.{suffix}", guid_key(guid))
}

/// Splits `Z_<guid>_.wvu.<kind>` into the GUID key and the kind.
fn parse_view_name(name: &str) -> Option<(String, ViewName)> {
    let (key, kind) = name.strip_prefix("Z_")?.rsplit_once("_.wvu.")?;
    let kind = match kind {
        "Rows" => ViewName::Rows,
        "Cols" => ViewName::Cols,
        "PrintArea" => ViewName::PrintArea,
        _ => return None,
    };
    Some((key.to_ascii_uppercase(), kind))
}

/// `'Sheet'!$3:$5,'Sheet'!$8:$8` (rows) or `'Sheet'!$C:$D` (columns) of 0-based spans.
fn spans_formula(sheet: &str, rows: bool, spans: &[(u32, u32)]) -> String {
    let q = format!("'{}'", sheet.replace('\'', "''"));
    let one = |i: u32| {
        if rows {
            format!("${}", u64::from(i) + 1)
        } else {
            format!("${}", bo_model::sheet::col_to_letters(i))
        }
    };
    spans
        .iter()
        .map(|&(a, b)| format!("{q}!{}:{}", one(a), one(b)))
        .collect::<Vec<_>>()
        .join(",")
}

/// Parses whole-row / whole-column references like [`spans_formula`] writes (sheet
/// prefixes and `$` ignored); `None` on anything else.
fn parse_spans(f: &str, rows: bool) -> Option<Vec<(u32, u32)>> {
    let one = |s: &str| -> Option<u32> {
        let s = s.trim().trim_start_matches('$');
        if rows {
            s.parse::<u32>().ok().filter(|n| *n >= 1).map(|n| n - 1)
        } else {
            bo_model::sheet::letters_to_col(s)
        }
    };
    // Quoted sheet names may hold commas: split outside quotes only.
    let mut parts = Vec::new();
    let (mut start, mut quote) = (0, false);
    for (i, ch) in f.char_indices() {
        match ch {
            '\'' => quote = !quote,
            ',' if !quote => {
                parts.push(&f[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&f[start..]);
    let mut out = Vec::new();
    for p in parts {
        let r = p.rsplit_once('!').map_or(p, |(_, r)| r);
        let (a, b) = r.split_once(':').unwrap_or((r, r));
        let (a, b) = (one(a)?, one(b)?);
        out.push((a.min(b), a.max(b)));
    }
    Some(out)
}

/// Moves the `Z_<guid>_.wvu.*` names of `wb` into the custom sheet views they
/// belong to; names that match no view (or do not parse) stay defined names.
pub(crate) fn take_view_names(wb: &mut Workbook) {
    let names = std::mem::take(&mut wb.defined_names);
    for dn in names {
        let found = parse_view_name(&dn.name).and_then(|(key, kind)| {
            let ws = wb.sheets.get_mut(usize::try_from(dn.local_sheet?).ok()?)?;
            let cv = ws
                .custom_views
                .iter_mut()
                .find(|v| guid_key(&v.guid) == key)?;
            match kind {
                ViewName::Rows => cv.session_hidden_rows = Some(parse_spans(&dn.formula, true)?),
                ViewName::Cols => cv.session_hidden_cols = Some(parse_spans(&dn.formula, false)?),
                ViewName::PrintArea => cv.session_print_area = Some(Some(dn.formula.clone())),
            }
            Some(())
        });
        if found.is_none() {
            wb.defined_names.push(dn);
        }
    }
}

/// The `Z_<guid>_.wvu.*` names to write for the custom views of `ws`: `(name, formula)`.
pub(crate) fn view_names(ws: &Worksheet) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for v in &ws.custom_views {
        if let Some(spans) = v.session_hidden_rows.as_ref().filter(|s| !s.is_empty()) {
            out.push((
                view_name(&v.guid, ViewName::Rows),
                spans_formula(&ws.name, true, spans),
            ));
        }
        if let Some(spans) = v.session_hidden_cols.as_ref().filter(|s| !s.is_empty()) {
            out.push((
                view_name(&v.guid, ViewName::Cols),
                spans_formula(&ws.name, false, spans),
            ));
        }
        if let Some(Some(pa)) = &v.session_print_area {
            out.push((view_name(&v.guid, ViewName::PrintArea), pa.clone()));
        }
    }
    out
}
