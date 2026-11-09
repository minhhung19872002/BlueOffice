//! BlueSheet checks over the SpreadsheetML model (ECMA-376 Part 1, §18):
//! inspector categories, accessibility rules and compatibility features, plus
//! the model-level removals.

use super::a11y;
use super::compat::{self, uses_new_function, BOTH, LEGACY, ODF};
use super::package::{self, PackageScan, Scrub};
use super::{
    looks_like_url, sample, Action, Category, Feature, Finding, Issue, Report, Rule, Severity,
    Target,
};
use bo_model::sheet::{CellRange, CellValue, Fill, SheetColor, SheetState, Workbook, Worksheet};
use bo_sheet::calc::FormulaAdjuster;
use std::collections::{BTreeMap, BTreeSet};

/// Last row / column of the 97–2003 grid (65 536 rows, 256 columns), 0-based.
const LEGACY_ROWS: u32 = 65_536;
const LEGACY_COLS: u32 = 256;
/// Cell formats the 97–2003 format holds.
const LEGACY_XFS: usize = 4000;
/// Contrast issues listed per worksheet.
const CONTRAST_PER_SHEET: usize = 20;

/// Package scan of the parts the workbook keeps verbatim.
pub fn scan(wb: &Workbook) -> PackageScan {
    package::scan(
        wb.opaque
            .parts
            .iter()
            .map(|p| (p.name.as_str(), p.data.as_slice())),
    )
}

/// Inspector, accessibility and compatibility report of a workbook.
pub fn report(wb: &Workbook, pending: &Scrub) -> Report {
    let pkg = scan(wb);
    Report::new(
        inspector(wb, &pkg),
        accessibility(wb, &pkg),
        compatibility(wb, &pkg),
        pending,
    )
}

fn comment_cells(ws: &Worksheet) -> BTreeSet<(u32, u32)> {
    ws.comments
        .iter()
        .map(|c| (c.cell.row, c.cell.col))
        .chain(
            ws.threaded_comments
                .iter()
                .map(|c| (c.cell.row, c.cell.col)),
        )
        .collect()
}

fn hidden_cols(ws: &Worksheet) -> usize {
    ws.cols
        .iter()
        .filter(|c| c.hidden)
        .map(|c| (c.max.saturating_sub(c.min) + 1) as usize)
        .sum()
}

fn has_header_footer(ws: &Worksheet) -> bool {
    let p = &ws.page_setup;
    [&p.odd_header, &p.odd_footer, &p.header_footer_raw]
        .iter()
        .any(|v| v.as_deref().is_some_and(|s| !s.trim().is_empty()))
}

fn inspector(wb: &Workbook, pkg: &PackageScan) -> Vec<Finding> {
    let comments: usize = wb.sheets.iter().map(|s| comment_cells(s).len()).sum();
    let mut authors: Vec<String> = wb
        .sheets
        .iter()
        .flat_map(|s| s.comments.iter().map(|c| c.author.clone()))
        .collect();
    authors.extend(wb.persons.iter().map(|p| p.display_name.clone()));
    let rows: usize = wb
        .sheets
        .iter()
        .map(|s| s.rows.values().filter(|r| r.hidden).count())
        .sum();
    let cols: usize = wb.sheets.iter().map(hidden_cols).sum();
    let hidden_sheets: Vec<String> = wb
        .sheets
        .iter()
        .filter(|s| s.state != SheetState::Visible)
        .map(|s| s.name.clone())
        .collect();
    let hf = wb.sheets.iter().filter(|s| has_header_footer(s)).count();
    vec![
        Finding::new(Category::Comments, comments, Action::Remove).with_details(authors),
        Finding::new(Category::Properties, pkg.properties.len(), Action::OnSave).with_details(
            pkg.properties
                .iter()
                .map(|(k, v)| format!("{k}: {}", sample(v))),
        ),
        Finding::new(
            Category::CustomProperties,
            pkg.custom_properties,
            Action::OnSave,
        )
        .with_details(pkg.custom_names.iter().cloned()),
        Finding::new(Category::CustomXml, pkg.custom_xml, Action::OnSave),
        Finding::new(
            Category::ServerProperties,
            pkg.server_properties,
            Action::OnSave,
        ),
        Finding::new(Category::Macros, pkg.macros, Action::OnSave),
        Finding::new(Category::HiddenRows, rows, Action::Remove),
        Finding::new(Category::HiddenColumns, cols, Action::Remove),
        Finding::new(Category::HiddenSheets, hidden_sheets.len(), Action::Remove)
            .with_details(hidden_sheets),
        Finding::new(Category::HeadersFooters, hf, Action::Remove),
        Finding::new(Category::Embedded, pkg.embedded, Action::None)
            .with_details(pkg.embedded_names.iter().cloned()),
    ]
}

/// A name the app gives new sheets (`Sheet3`, `Trang tính 2`, …).
fn default_name(name: &str) -> bool {
    let n = name.trim();
    [
        "Sheet",
        "Trang tính",
        "Feuil",
        "Tabelle",
        "Hoja",
        "Foglio",
        "Planilha",
        "Blad",
    ]
    .iter()
    .any(|p| {
        n.strip_prefix(p)
            .map(str::trim_start)
            .is_some_and(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()))
    })
}

/// Resolve a SpreadsheetML colour (§18.8.19 `CT_Color`): `rgb`, or a theme colour
/// without tint (theme indices 0–3 map to lt1, dk1, lt2, dk2).
fn color(wb: &Workbook, c: &SheetColor) -> Option<(u8, u8, u8)> {
    if c.auto || c.tint.is_some_and(|t| t.abs() > 1e-6) {
        return None;
    }
    if let Some(rgb) = &c.rgb {
        return a11y::parse_hex(rgb);
    }
    let i = c.theme? as usize;
    let i = match i {
        0 => 1,
        1 => 0,
        2 => 3,
        3 => 2,
        n => n,
    };
    a11y::parse_hex(wb.theme.as_ref()?.colors.get(i)?)
}

/// Contrast ratio below the minimum for cells of style `xf`.
fn style_contrast(wb: &Workbook, xf: u32) -> Option<f64> {
    let x = wb.styles.cell_xfs.get(xf as usize)?;
    let font = wb.styles.fonts.get(x.font_id as usize)?;
    let fg = match &font.color {
        Some(c) => color(wb, c)?,
        None => (0, 0, 0),
    };
    let bg = match wb.styles.fills.get(x.fill_id as usize) {
        Some(Fill::Pattern(p)) if p.pattern.as_deref() == Some("solid") => {
            color(wb, p.fg.as_ref()?)?
        }
        Some(Fill::Gradient(_)) => return None,
        _ => (255, 255, 255),
    };
    a11y::low_contrast(fg, bg, font.size.unwrap_or(11.0), font.bold)
}

fn cell_text(v: &CellValue) -> String {
    match v {
        CellValue::Empty => String::new(),
        CellValue::Number(n) => n.to_string(),
        CellValue::Bool(b) => b.to_string(),
        CellValue::Error(_) => "#".to_owned(),
        CellValue::Date(d) => d.clone(),
        CellValue::Text(_) | CellValue::RichText(_) => v.as_text().unwrap_or_default(),
    }
}

fn range(sheet: usize, r: &CellRange) -> Target {
    Target::Range {
        sheet,
        r0: r.first.row,
        c0: r.first.col,
        r1: r.last.row,
        c1: r.last.col,
    }
}

/// Drawing objects of a sheet without alternative text (`xdr:cNvPr/@descr`,
/// §20.5.2.8 nvPicPr / nvSpPr / nvGraphicFramePr).
fn drawing_alt_text(wb: &Workbook, ws: &Worksheet) -> Vec<String> {
    let mut out = Vec::new();
    for r in ws
        .rels
        .iter()
        .filter(|r| !r.external && r.rel_type.ends_with("/drawing"))
    {
        let Some(part) = wb
            .opaque
            .parts
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(&r.target))
        else {
            continue;
        };
        let xml = String::from_utf8_lossy(&part.data);
        for (i, _) in xml.match_indices("cNvPr") {
            let Some(tag) = xml[..i].rfind('<').map(|s| &xml[s..]) else {
                continue;
            };
            let Some(end) = tag.find('>') else { continue };
            let tag = &tag[..end];
            if !tag
                .trim_start_matches('<')
                .split_whitespace()
                .next()
                .is_some_and(|n| n.ends_with(":cNvPr") || n == "cNvPr")
            {
                continue;
            }
            let descr = package::attr_in(tag, "descr").unwrap_or_default();
            if descr.trim().is_empty() {
                out.push(package::attr_in(tag, "name").unwrap_or_default());
            }
        }
    }
    out
}

fn accessibility(wb: &Workbook, pkg: &PackageScan) -> Vec<Issue> {
    let mut out = Vec::new();
    for (si, ws) in wb.sheets.iter().enumerate() {
        if ws.state != SheetState::Visible {
            continue;
        }
        for name in drawing_alt_text(wb, ws) {
            out.push(Issue::new(
                Rule::AltText,
                Severity::Error,
                Target::Sheet { sheet: si },
                name,
            ));
        }
        for m in &ws.merges {
            out.push(Issue::new(
                Rule::MergedCells,
                Severity::Warning,
                range(si, m),
                m.to_string(),
            ));
        }
        for t in &ws.tables {
            if t.header_row_count == Some(0) {
                out.push(Issue::new(
                    Rule::TableHeader,
                    Severity::Warning,
                    range(si, &t.range),
                    t.display_name.clone(),
                ));
            }
        }
        if default_name(&ws.name) {
            out.push(Issue::new(
                Rule::SheetName,
                Severity::Tip,
                Target::Sheet { sheet: si },
                ws.name.clone(),
            ));
        }
        for h in &ws.hyperlinks {
            let at = h.range.first;
            let text = ws
                .cell(at.row, at.col)
                .map(|c| cell_text(&c.value))
                .unwrap_or_default();
            let text = if text.is_empty() {
                h.display.clone().unwrap_or_default()
            } else {
                text
            };
            if text.trim().is_empty() || looks_like_url(&text) {
                out.push(Issue::new(
                    Rule::LinkText,
                    Severity::Warning,
                    range(si, &h.range),
                    sample(&text),
                ));
            }
        }
        // One contrast issue per cell format, at its first cell.
        let mut cache: BTreeMap<u32, Option<f64>> = BTreeMap::new();
        let mut seen = BTreeSet::new();
        for (&(r, c), cell) in &ws.cells {
            if seen.len() >= CONTRAST_PER_SHEET {
                break;
            }
            if matches!(cell.value, CellValue::Empty) || seen.contains(&cell.style) {
                continue;
            }
            let ratio = *cache
                .entry(cell.style)
                .or_insert_with(|| style_contrast(wb, cell.style));
            if let Some(ratio) = ratio {
                seen.insert(cell.style);
                let at = bo_model::sheet::CellRef::new(r, c);
                let mut i = Issue::new(
                    Rule::Contrast,
                    Severity::Warning,
                    range(si, &CellRange::single(at)),
                    sample(&cell_text(&cell.value)),
                );
                i.ratio = Some(ratio);
                out.push(i);
            }
        }
    }
    if pkg.title.is_empty() {
        out.push(Issue::new(
            Rule::DocumentTitle,
            Severity::Tip,
            Target::Document,
            "",
        ));
    }
    out
}

fn compatibility(wb: &Workbook, pkg: &PackageScan) -> Vec<super::CompatItem> {
    let (mut beyond, mut new_fns, mut dynamic) = (0, 0, 0);
    let (mut cf, mut adv_cf, mut dv, mut tables, mut slicers, mut timelines, mut sparklines) =
        (0, 0, 0, 0, 0, 0, 0);
    let (mut pivots, mut threads, mut notes, mut long_names, mut charts) = (0, 0, 0, 0, 0);
    for ws in &wb.sheets {
        for (&(r, c), cell) in &ws.cells {
            if r >= LEGACY_ROWS || c >= LEGACY_COLS {
                beyond += 1;
            }
            if let Some(f) = &cell.formula {
                if uses_new_function(&f.text) {
                    new_fns += 1;
                }
            }
            if cell.cm.is_some() {
                dynamic += 1;
            }
        }
        cf += ws
            .conditional_formats
            .iter()
            .map(|b| b.rules.len())
            .sum::<usize>();
        adv_cf += ws
            .conditional_formats
            .iter()
            .map(|b| {
                let fancy = b
                    .rules
                    .iter()
                    .filter(|r| {
                        r.data_bar.is_some() || r.color_scale.is_some() || r.icon_set.is_some()
                    })
                    .count();
                if b.rules.len() > 3 {
                    fancy.max(b.rules.len() - 3)
                } else {
                    fancy
                }
            })
            .sum::<usize>();
        dv += ws.data_validations.len();
        tables += ws.tables.len();
        slicers += ws.slicers.len();
        timelines += ws.timelines.len();
        sparklines += ws.sparkline_groups.len();
        pivots += ws.pivots.len();
        threads += ws
            .threaded_comments
            .iter()
            .filter(|c| c.parent_id.is_none())
            .count();
        notes += ws.comments.len();
        charts += ws.charts.len();
        if ws.name.chars().count() > 31 {
            long_names += 1;
        }
    }
    let xfs = wb.styles.cell_xfs.len();
    vec![
        compat::item(Feature::BeyondGrid, beyond, LEGACY),
        compat::item(Feature::NewFunctions, new_fns, LEGACY),
        compat::item(Feature::DynamicArrays, dynamic, BOTH),
        compat::item(Feature::Tables, tables, BOTH),
        compat::item(Feature::Slicers, slicers, BOTH),
        compat::item(Feature::Timelines, timelines, BOTH),
        compat::item(Feature::Sparklines, sparklines, BOTH),
        compat::item(Feature::AdvancedConditionalFormats, adv_cf, LEGACY),
        compat::item(Feature::ConditionalFormats, cf, ODF),
        compat::item(Feature::DataValidation, dv, ODF),
        compat::item(Feature::PivotTables, pivots, ODF),
        compat::item(Feature::Charts, charts.max(pkg.charts), ODF),
        compat::item(Feature::ModernCharts, pkg.chart_ex, BOTH),
        compat::item(Feature::CommentThreads, threads, BOTH),
        compat::item(Feature::Comments, notes, ODF),
        compat::item(Feature::LongSheetNames, long_names, LEGACY),
        compat::item(
            Feature::ManyCellFormats,
            if xfs > LEGACY_XFS { xfs } else { 0 },
            LEGACY,
        ),
    ]
}

/// Delete runs of consecutive indices, last run first (so earlier indices stay valid).
fn runs_desc(mut idx: Vec<u32>) -> Vec<(u32, u32)> {
    idx.sort_unstable();
    idx.dedup();
    let mut runs: Vec<(u32, u32)> = Vec::new();
    for i in idx {
        match runs.last_mut() {
            Some((start, n)) if *start + *n == i => *n += 1,
            _ => runs.push((i, 1)),
        }
    }
    runs.reverse();
    runs
}

/// Remove the model-level categories in `cats` from the workbook.
pub fn remove(
    wb: &mut Workbook,
    cats: &[Category],
    adj: &dyn FormulaAdjuster,
) -> crate::Result<()> {
    let has = |c: Category| cats.contains(&c);
    let err = |e: bo_sheet::ops::EditError| crate::EngineError::Sheet(e.to_string());
    if has(Category::Comments) {
        for ws in &mut wb.sheets {
            ws.comments.clear();
            ws.comment_authors.clear();
            ws.threaded_comments.clear();
        }
        wb.persons.clear();
    }
    if has(Category::HeadersFooters) {
        for ws in &mut wb.sheets {
            let p = &mut ws.page_setup;
            p.odd_header = None;
            p.odd_footer = None;
            p.header_footer_raw = None;
        }
    }
    for si in 0..wb.sheets.len() {
        if has(Category::HiddenRows) {
            let rows: Vec<u32> = wb.sheets[si]
                .rows
                .iter()
                .filter(|(_, r)| r.hidden)
                .map(|(i, _)| *i)
                .collect();
            for (at, n) in runs_desc(rows) {
                bo_sheet::ops::delete_rows(wb, si, at, n, adj).map_err(err)?;
            }
        }
        if has(Category::HiddenColumns) {
            let cols: Vec<u32> = wb.sheets[si]
                .cols
                .iter()
                .filter(|c| c.hidden)
                .flat_map(|c| c.min..=c.max)
                .collect();
            for (at, n) in runs_desc(cols) {
                bo_sheet::ops::delete_cols(wb, si, at, n, adj).map_err(err)?;
            }
        }
    }
    if has(Category::HiddenSheets) && wb.sheets.iter().any(|s| s.state == SheetState::Visible) {
        // Sheet-scoped names follow their sheet (§18.2.5 `localSheetId`).
        let keep: Vec<bool> = wb
            .sheets
            .iter()
            .map(|s| s.state == SheetState::Visible)
            .collect();
        let new_index: Vec<Option<u32>> = keep
            .iter()
            .scan(0u32, |n, k| {
                let v = k.then_some(*n);
                if *k {
                    *n += 1;
                }
                Some(v)
            })
            .collect();
        wb.defined_names.retain_mut(|d| match d.local_sheet {
            Some(i) => match new_index.get(i as usize).copied().flatten() {
                Some(n) => {
                    d.local_sheet = Some(n);
                    true
                }
                None => false,
            },
            None => true,
        });
        let mut i = 0;
        wb.sheets.retain(|_| {
            let k = keep[i];
            i += 1;
            k
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_names() {
        assert!(default_name("Sheet1"));
        assert!(default_name("Trang tính 12"));
        assert!(!default_name("Sheets"));
        assert!(!default_name("Doanh thu"));
    }

    #[test]
    fn runs() {
        assert_eq!(runs_desc(vec![5, 1, 2, 3, 7, 6]), vec![(5, 3), (1, 3)]);
        assert_eq!(runs_desc(vec![]), Vec::<(u32, u32)>::new());
    }
}
