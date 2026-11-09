//! Page setup commands beyond the basic `setPageSetup` fields: print titles
//! (rows and columns), several print areas, printOptions (§18.3.1.70),
//! pageSetup attributes (§18.3.1.63 pageOrder, firstPageNumber,
//! useFirstPageNumber, blackAndWhite, draft, errors, cellComments), header /
//! footer margins (§18.3.1.62) and texts (§18.3.1.46), manual page breaks
//! (§18.3.1.74 rowBreaks / §18.3.1.14 colBreaks) and the print preview.

// `Option<Option<T>>` = absent (keep) / null (clear) / value, as in `SheetCommand`.
#![allow(clippy::option_option)]

use super::cmd::Rng;
use super::print::{attr, parse_titles, set_attr, titles_text};
use crate::{EngineError, Result};
use bo_model::sheet::{PageMargins, Worksheet};
use serde::{Deserialize, Deserializer};
use serde_json::{json, Value};

/// Header / footer texts with their `&` codes (empty text clears).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HeaderFooterSpec {
    pub odd_header: String,
    pub odd_footer: String,
    pub even_header: String,
    pub even_footer: String,
    pub first_header: String,
    pub first_footer: String,
    /// `differentOddEven`.
    pub different_odd_even: bool,
    /// `differentFirst`.
    pub different_first: bool,
    /// `scaleWithDoc` (default on).
    pub scale_with_doc: Option<bool>,
    /// `alignWithMargins` (default on).
    pub align_with_margins: Option<bool>,
}

/// Optional `setPageSetup` fields (absent = kept).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PrintSetupExtra {
    /// Columns repeated at the left `[first, last]`; `null` clears.
    #[serde(deserialize_with = "nullable")]
    pub title_cols: Option<Option<[u32; 2]>>,
    /// Several print areas, each printed from a new page; empty clears.
    pub print_areas: Option<Vec<Rng>>,
    /// Print row and column headings.
    pub headings: Option<bool>,
    /// Centre horizontally on the page.
    pub center_h: Option<bool>,
    /// Centre vertically on the page.
    pub center_v: Option<bool>,
    /// `downThenOver` or `overThenDown`.
    pub page_order: Option<String>,
    /// Number of the first page; `null` = automatic (1).
    #[serde(deserialize_with = "nullable")]
    pub first_page_number: Option<Option<u32>>,
    /// Black and white.
    pub black_and_white: Option<bool>,
    /// Draft quality (no graphics).
    pub draft: Option<bool>,
    /// `displayed`, `blank`, `dash` or `NA`.
    pub errors: Option<String>,
    /// `none`, `atEnd` or `asDisplayed`.
    pub comments: Option<String>,
    /// Header and footer margins in inches.
    pub hf_margins: Option<[f64; 2]>,
    /// Header / footer texts.
    pub header_footer: Option<HeaderFooterSpec>,
}

/// `Option<Option<T>>` field where `null` means "clear" (`Some(None)`) and an
/// absent field means "keep" (`None`, via `#[serde(default)]`).
pub(super) fn nullable<'de, T: Deserialize<'de>, D: Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(d).map(Some)
}

fn bad(msg: &str) -> EngineError {
    EngineError::Sheet(msg.to_owned())
}

fn opt_text(s: String) -> Option<String> {
    (!s.is_empty()).then_some(s)
}

/// Applies the extra fields to `ws` (inside an undoable edit).
pub(super) fn apply_extra(ws: &mut Worksheet, x: PrintSetupExtra) -> Result<()> {
    let name = bo_sheet::cells::quote_sheet(&ws.name);
    if let Some(tc) = x.title_cols {
        let (rows, _) = ws
            .print_titles
            .as_deref()
            .map_or((None, None), parse_titles);
        let cols = tc.map(|[a, b]| (a.min(b), a.max(b)));
        ws.print_titles = titles_text(&name, rows, cols);
    }
    if let Some(areas) = x.print_areas {
        ws.print_area = (!areas.is_empty()).then(|| {
            areas
                .iter()
                .map(|r| bo_sheet::cells::abs_ref(&ws.name, r.range()))
                .collect::<Vec<_>>()
                .join(",")
        });
    }
    let p = &mut ws.page_setup;
    let opts = |v: Option<bool>, field: &mut bool| {
        if let Some(v) = v {
            *field = v;
        }
    };
    opts(x.headings, &mut p.print_headings);
    opts(x.center_h, &mut p.horizontal_centered);
    opts(x.center_v, &mut p.vertical_centered);
    if x.headings.is_some() || x.center_h.is_some() || x.center_v.is_some() {
        p.has_print_options = true;
    }
    let mut touched = false;
    if let Some(o) = x.page_order {
        // §18.18 ST_PageOrder.
        let v = match o.as_str() {
            "downThenOver" => None,
            "overThenDown" => Some(o),
            _ => return Err(bad("pageOrder must be downThenOver or overThenDown")),
        };
        set_attr(&mut p.attrs, "pageOrder", v);
        touched = true;
    }
    if let Some(n) = x.first_page_number {
        set_attr(&mut p.attrs, "firstPageNumber", n.map(|n| n.to_string()));
        set_attr(
            &mut p.attrs,
            "useFirstPageNumber",
            n.map(|_| "1".to_owned()),
        );
        touched = true;
    }
    for (v, name) in [(x.black_and_white, "blackAndWhite"), (x.draft, "draft")] {
        if let Some(v) = v {
            set_attr(&mut p.attrs, name, v.then(|| "1".to_owned()));
            touched = true;
        }
    }
    if let Some(e) = x.errors {
        // §18.18 ST_PrintError.
        let v = match e.as_str() {
            "displayed" => None,
            "blank" | "dash" | "NA" => Some(e),
            _ => return Err(bad("errors must be displayed, blank, dash or NA")),
        };
        set_attr(&mut p.attrs, "errors", v);
        touched = true;
    }
    if let Some(c) = x.comments {
        // §18.18 ST_CellComments.
        let v = match c.as_str() {
            "none" => None,
            "atEnd" | "asDisplayed" => Some(c),
            _ => return Err(bad("comments must be none, atEnd or asDisplayed")),
        };
        set_attr(&mut p.attrs, "cellComments", v);
        touched = true;
    }
    if touched {
        p.has_page_setup = true;
    }
    if let Some([h, f]) = x.hf_margins {
        let base = p.margins.unwrap_or_default();
        p.margins = Some(PageMargins {
            header: h.clamp(0.0, 10.0),
            footer: f.clamp(0.0, 10.0),
            ..base
        });
    }
    if let Some(hf) = x.header_footer {
        p.header_footer_raw = None;
        p.odd_header = opt_text(hf.odd_header);
        p.odd_footer = opt_text(hf.odd_footer);
        p.even_header = opt_text(hf.even_header);
        p.even_footer = opt_text(hf.even_footer);
        p.first_header = opt_text(hf.first_header);
        p.first_footer = opt_text(hf.first_footer);
        p.different_odd_even = hf.different_odd_even;
        p.different_first = hf.different_first;
        // Defaults are on (§18.3.1.46): only "off" is written.
        for (v, name) in [
            (hf.scale_with_doc, "scaleWithDoc"),
            (hf.align_with_margins, "alignWithMargins"),
        ] {
            if let Some(v) = v {
                set_attr(
                    &mut p.header_footer_attrs,
                    name,
                    (!v).then(|| "0".to_owned()),
                );
            }
        }
    }
    Ok(())
}

/// Extra fields of the `pageSetup` query.
pub(super) fn extra_json(ws: &Worksheet) -> Value {
    let p = &ws.page_setup;
    let (_, cols) = ws
        .print_titles
        .as_deref()
        .map_or((None, None), parse_titles);
    let m = p.margins.unwrap_or_default();
    let a = &p.attrs;
    let on = |v: Option<&str>| matches!(v, Some("1" | "true"));
    let hfa = &p.header_footer_attrs;
    json!({
        "titleCols": cols.map(|(a, b)| [a, b]),
        "headings": p.print_headings,
        "centerH": p.horizontal_centered,
        "centerV": p.vertical_centered,
        "pageOrder": attr(a, "pageOrder").unwrap_or("downThenOver"),
        "firstPageNumber": on(attr(a, "useFirstPageNumber"))
            .then(|| attr(a, "firstPageNumber").and_then(|v| v.parse::<u32>().ok()).unwrap_or(1)),
        "blackAndWhite": on(attr(a, "blackAndWhite")),
        "draft": on(attr(a, "draft")),
        "errors": attr(a, "errors").unwrap_or("displayed"),
        "comments": attr(a, "cellComments").unwrap_or("none"),
        "hfMargins": [m.header, m.footer],
        "headerFooter": {
            "oddHeader": p.odd_header.clone().unwrap_or_default(),
            "oddFooter": p.odd_footer.clone().unwrap_or_default(),
            "evenHeader": p.even_header.clone().unwrap_or_default(),
            "evenFooter": p.even_footer.clone().unwrap_or_default(),
            "firstHeader": p.first_header.clone().unwrap_or_default(),
            "firstFooter": p.first_footer.clone().unwrap_or_default(),
            "differentOddEven": p.different_odd_even,
            "differentFirst": p.different_first,
            "scaleWithDoc": !matches!(attr(hfa, "scaleWithDoc"), Some("0" | "false")),
            "alignWithMargins": !matches!(attr(hfa, "alignWithMargins"), Some("0" | "false")),
        },
        "rowBreaks": p.row_breaks.iter().map(|b| b + 1).collect::<Vec<_>>(),
        "colBreaks": p.col_breaks.iter().map(|b| b + 1).collect::<Vec<_>>(),
    })
}

/// Inserts (or removes) a manual break before `row` / `col` (the first row /
/// column of the next page); `row` / `col` 0 is ignored.
pub(super) fn page_break(ws: &mut Worksheet, row: Option<u32>, col: Option<u32>, remove: bool) {
    let p = &mut ws.page_setup;
    for (at, list) in [(row, &mut p.row_breaks), (col, &mut p.col_breaks)] {
        let Some(at) = at.filter(|v| *v > 0) else {
            continue;
        };
        let after = at - 1;
        list.retain(|b| *b != after);
        if !remove {
            list.push(after);
            list.sort_unstable();
        }
    }
}
