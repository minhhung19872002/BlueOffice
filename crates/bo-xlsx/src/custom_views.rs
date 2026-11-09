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
use bo_model::sheet::{CustomSheetView, CustomWorkbookView, PageSetup};
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
