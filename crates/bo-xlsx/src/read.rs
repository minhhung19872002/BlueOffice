//! `.xlsx` → [`Workbook`].

use crate::styles::{read_color, read_font, read_styles};
use crate::xml::{
    attrs, capture, for_children, lname, pbool, push_unknown, read_text, root, take, take_bool,
    take_f64, take_q, take_u32, xstring_decode,
};
use crate::{rel_kind, RelKind, Result, XlsxError, SHEET_SCHEMA, WORKBOOK_SCHEMA};
use bo_model::sheet::{
    Cell, CellError, CellRange, CellRef, CellValue, CfRule, Cfvo, ColInfo, ColorScale, Comment,
    ConditionalFormat, DataBar, DataValidation, DefinedName, Formula, FormulaKind, Hyperlink,
    IconSet, OpaquePart, OpaqueRel, PageMargins, Pane, PaneState, RawXml, RowInfo, SheetKind,
    SheetState, SheetView, Table, TableColumn, TableStyleInfo, TextRun, ThemeColors, Workbook,
    Worksheet,
};
use bo_opc::xml::XmlReader;
use bo_opc::{rels_part_name, resolve_target, Limits, Package, TargetMode, REL_OFFICE_DOCUMENT};
use std::collections::{HashMap, HashSet};

/// Read an `.xlsx` (or `.xlsm`) file with default [`Limits`].
pub fn read_xlsx(bytes: &[u8]) -> Result<Workbook> {
    read_xlsx_with_limits(bytes, &Limits::default())
}

/// Read an `.xlsx` file with explicit zip-bomb limits.
pub fn read_xlsx_with_limits(bytes: &[u8], limits: &Limits) -> Result<Workbook> {
    let pkg = Package::read(bytes, limits)?;
    let ct = pkg.content_types()?;
    let mut consumed: HashSet<String> = HashSet::new();
    let mut mark = |name: &str| {
        consumed.insert(name.to_ascii_lowercase());
    };
    mark("/[Content_Types].xml");
    mark("/_rels/.rels");

    let mut wb = Workbook::default();

    // Package relationships (ECMA-376 Part 2, §9.3): keep everything but the main part.
    let mut wb_path = None;
    for rel in pkg.relationships("/")? {
        let is_main =
            rel.rel_type == REL_OFFICE_DOCUMENT || rel.rel_type.ends_with("/officeDocument");
        if is_main && rel.mode == TargetMode::Internal && wb_path.is_none() {
            wb_path = Some(resolve_target("/", &rel.target));
            continue;
        }
        wb.opaque.package_rels.push(opaque_rel("/", &rel));
    }
    let wb_path = wb_path.ok_or(XlsxError::NoWorkbook)?;
    let wb_part = pkg.part(&wb_path).ok_or(XlsxError::NoWorkbook)?;
    mark(&wb_path);
    mark(&rels_part_name(&wb_path));
    if let Some(t) = ct.get(&wb_path) {
        if t != crate::CT_WORKBOOK {
            wb.opaque.workbook_content_type = Some(t.to_owned());
        }
    }

    // Workbook relationships (§12.3.23 workbook part).
    let wb_rels = pkg.relationships(&wb_path)?;
    let mut sst: Vec<CellValue> = Vec::new();
    let mut sheet_rels: HashMap<String, (RelKind, String, String)> = HashMap::new();
    for rel in &wb_rels {
        let kind = rel_kind(&rel.rel_type);
        let target = resolve_target(&wb_path, &rel.target);
        let internal = rel.mode == TargetMode::Internal;
        match kind {
            RelKind::Styles if internal => {
                if let Some(p) = pkg.part(&target) {
                    wb.styles = read_styles(&p.data, &target)?;
                }
                mark(&target);
            }
            RelKind::SharedStrings if internal => {
                if let Some(p) = pkg.part(&target) {
                    sst = read_shared_strings(&p.data, &target)?;
                }
                mark(&target);
            }
            // The calculation chain (§18.6) is an optional cache; it is dropped on
            // write because edited cells would make it stale.
            RelKind::CalcChain if internal => mark(&target),
            RelKind::Worksheet | RelKind::OtherSheet if internal => {
                sheet_rels.insert(rel.id.clone(), (kind, rel.rel_type.clone(), target));
            }
            _ => {
                if kind == RelKind::Theme && internal {
                    if let Some(p) = pkg.part(&target) {
                        wb.theme = read_theme(&p.data, &target).ok().flatten();
                    }
                }
                wb.opaque.workbook_rels.push(opaque_rel(&wb_path, rel));
            }
        }
    }

    let sheet_entries = read_workbook(&wb_part.data, &wb_path, &mut wb)?;
    for entry in sheet_entries {
        let Some((kind, rel_type, target)) = sheet_rels.get(&entry.rel_id) else {
            continue;
        };
        let mut ws = Worksheet {
            name: entry.name,
            sheet_id: entry.sheet_id,
            state: entry.state,
            ..Worksheet::default()
        };
        if *kind == RelKind::Worksheet {
            let part = pkg
                .part(target)
                .ok_or_else(|| XlsxError::MissingPart(target.clone()))?;
            mark(target);
            mark(&rels_part_name(target));
            let rels = pkg.relationships(target)?;
            let mut links: HashMap<String, String> = HashMap::new();
            for rel in &rels {
                let rk = rel_kind(&rel.rel_type);
                let abs = resolve_target(target, &rel.target);
                match rk {
                    RelKind::Hyperlink if rel.mode == TargetMode::External => {
                        links.insert(rel.id.clone(), rel.target.clone());
                    }
                    RelKind::Comments if rel.mode == TargetMode::Internal => {
                        if let Some(p) = pkg.part(&abs) {
                            let (authors, comments) = read_comments(&p.data, &abs)?;
                            // Keep the author list only when it is not the one the writer
                            // derives anyway (distinct authors in comment order).
                            let mut derived: Vec<&str> = Vec::new();
                            for c in &comments {
                                if !derived.contains(&c.author.as_str()) {
                                    derived.push(&c.author);
                                }
                            }
                            if authors != derived {
                                ws.comment_authors = authors;
                            }
                            ws.comments = comments;
                        }
                        mark(&abs);
                    }
                    RelKind::Table if rel.mode == TargetMode::Internal => {
                        // Tables are re-attached in tableParts order below.
                        links.insert(format!("table:{}", rel.id), abs);
                    }
                    _ => ws.rels.push(opaque_rel(target, rel)),
                }
            }
            let table_ids = read_sheet(&part.data, target, &sst, &links, &mut ws)?;
            ws.charts = crate::chart::read_charts(&pkg, target, &rels);
            for id in table_ids {
                if let Some(abs) = links.get(&format!("table:{id}")) {
                    if let Some(p) = pkg.part(abs) {
                        ws.tables.push(read_table(&p.data, abs)?);
                        mark(abs);
                    }
                }
            }
        } else {
            ws.kind = SheetKind::Opaque {
                rel_type: rel_type.clone(),
                part: target.clone(),
            };
        }
        wb.sheets.push(ws);
    }

    // Built-in names that belong to a sheet (§18.2.6 + Part 1 §18.2.5 reserved names).
    let names = std::mem::take(&mut wb.defined_names);
    for dn in names {
        let sheet = dn
            .local_sheet
            .and_then(|i| wb.sheets.get_mut(i as usize))
            .filter(|_| dn.attrs.is_empty() && !dn.hidden);
        match (dn.name.as_str(), sheet) {
            ("_xlnm.Print_Area", Some(s)) if s.print_area.is_none() => {
                s.print_area = Some(dn.formula);
            }
            ("_xlnm.Print_Titles", Some(s)) if s.print_titles.is_none() => {
                s.print_titles = Some(dn.formula);
            }
            _ => wb.defined_names.push(dn),
        }
    }

    // Everything not regenerated by the writer stays byte-for-byte.
    for p in pkg.parts() {
        if consumed.contains(&p.name.to_ascii_lowercase()) {
            continue;
        }
        let content_type = ct
            .overrides
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(&p.name))
            .map(|(_, t)| t.clone());
        wb.opaque.parts.push(OpaquePart {
            name: p.name.clone(),
            content_type,
            data: p.data.clone(),
        });
    }
    // The two standard defaults are always written; keep only the others.
    wb.opaque.content_type_defaults = ct
        .defaults
        .into_iter()
        .filter(|(e, t)| {
            !((e == "rels" && t == "application/vnd.openxmlformats-package.relationships+xml")
                || (e == "xml" && t == "application/xml"))
        })
        .collect();
    Ok(wb)
}

/// Root attributes minus the namespace declarations the writer always emits.
pub(crate) fn root_attrs_of(
    e: &quick_xml::events::BytesStart<'_>,
    drop_r: bool,
) -> Vec<bo_model::sheet::RawAttr> {
    attrs(e)
        .into_iter()
        .filter(|(k, _)| k != "xmlns" && !(drop_r && k == "xmlns:r"))
        .collect()
}

fn opaque_rel(source: &str, rel: &bo_opc::Relationship) -> OpaqueRel {
    let external = rel.mode == TargetMode::External;
    OpaqueRel {
        id: rel.id.clone(),
        rel_type: rel.rel_type.clone(),
        target: if external {
            rel.target.clone()
        } else {
            resolve_target(source, &rel.target)
        },
        external,
    }
}

struct SheetEntry {
    name: String,
    sheet_id: u32,
    state: SheetState,
    rel_id: String,
}

/// §18.2.27 workbook.
// Empty-element arms stay next to the element they belong to.
#[allow(clippy::match_same_arms)]
fn read_workbook(data: &[u8], part: &str, wb: &mut Workbook) -> Result<Vec<SheetEntry>> {
    let mut r = XmlReader::new(data, part);
    let (rootel, empty) = root(&mut r)?;
    wb.root_attrs = root_attrs_of(&rootel, true);
    let mut entries = Vec::new();
    if empty {
        return Ok(entries);
    }
    let mut prev = String::new();
    let mut unknown: Vec<RawXml> = Vec::new();
    for_children(&mut r, |r, e, empty| {
        let name = lname(e);
        match name.as_str() {
            // §18.2.28 workbookPr
            "workbookPr" => {
                let mut at = attrs(e);
                wb.date1904 = take_bool(&mut at, "date1904").unwrap_or(false);
                wb.workbook_pr_attrs = at;
            }
            // §18.2.20 sheets / §18.2.19 sheet
            "sheets" if !empty => for_children(r, |_, e, _| {
                if lname(e) == "sheet" {
                    let mut at = attrs(e);
                    entries.push(SheetEntry {
                        name: take(&mut at, "name").unwrap_or_default(),
                        sheet_id: take_u32(&mut at, "sheetId").unwrap_or(0),
                        state: match take(&mut at, "state").as_deref() {
                            Some("hidden") => SheetState::Hidden,
                            Some("veryHidden") => SheetState::VeryHidden,
                            _ => SheetState::Visible,
                        },
                        rel_id: take(&mut at, "id").unwrap_or_default(),
                    });
                }
                Ok(())
            })?,
            "sheets" => {}
            // §18.2.6 definedNames / §18.2.5 definedName
            "definedNames" if !empty => for_children(r, |r, e, empty| {
                if lname(e) == "definedName" {
                    let mut at = attrs(e);
                    let name = take(&mut at, "name").unwrap_or_default();
                    let local_sheet = take_u32(&mut at, "localSheetId");
                    let hidden = take_bool(&mut at, "hidden").unwrap_or(false);
                    let formula = if empty { String::new() } else { read_text(r)? };
                    wb.defined_names.push(DefinedName {
                        name,
                        formula,
                        local_sheet,
                        hidden,
                        attrs: at,
                    });
                }
                Ok(())
            })?,
            "definedNames" => {}
            _ => {
                let xml = capture(r, e, empty)?;
                push_unknown(&mut unknown, &mut prev, WORKBOOK_SCHEMA, &name, xml);
                return Ok(());
            }
        }
        prev = name;
        Ok(())
    })?;
    wb.unknown = unknown;
    Ok(entries)
}

/// §20.1.6.2 clrScheme inside the theme part (§14.2.7).
fn read_theme(data: &[u8], part: &str) -> Result<Option<ThemeColors>> {
    let mut r = XmlReader::new(data, part);
    let order = [
        "dk1", "lt1", "dk2", "lt2", "accent1", "accent2", "accent3", "accent4", "accent5",
        "accent6", "hlink", "folHlink",
    ];
    let mut found: HashMap<String, String> = HashMap::new();
    let mut current: Option<String> = None;
    loop {
        match r.read()? {
            quick_xml::events::Event::Start(e) => {
                let n = lname(&e);
                if order.contains(&n.as_str()) {
                    current = Some(n);
                }
            }
            quick_xml::events::Event::Empty(e) => {
                let n = lname(&e);
                if let Some(slot) = &current {
                    let mut at = attrs(&e);
                    let v = match n.as_str() {
                        "srgbClr" => take(&mut at, "val"),
                        "sysClr" => take(&mut at, "lastClr"),
                        _ => None,
                    };
                    if let Some(v) = v {
                        found.entry(slot.clone()).or_insert(v);
                    }
                }
            }
            quick_xml::events::Event::End(e) => {
                let n: &str = &e;
                if n.ends_with("clrScheme") {
                    break;
                }
            }
            quick_xml::events::Event::Eof => break,
            _ => {}
        }
    }
    if found.is_empty() {
        return Ok(None);
    }
    Ok(Some(ThemeColors {
        colors: order
            .iter()
            .map(|k| found.get(*k).cloned().unwrap_or_else(|| "000000".into()))
            .collect(),
    }))
}

/// §18.4.9 sst — shared string table.
fn read_shared_strings(data: &[u8], part: &str) -> Result<Vec<CellValue>> {
    let mut r = XmlReader::new(data, part);
    let (_, empty) = root(&mut r)?;
    let mut out = Vec::new();
    if empty {
        return Ok(out);
    }
    for_children(&mut r, |r, e, empty| {
        if lname(e) == "si" {
            out.push(if empty {
                CellValue::Text(String::new())
            } else {
                read_rich(r)?
            });
        }
        Ok(())
    })?;
    Ok(out)
}

/// Rich string (`CT_Rst`, §18.4.8 si / §18.3.1.53 is): `t` or runs `r` (§18.4.4).
/// Phonetic runs (`rPh`) and `phoneticPr` are not kept.
fn read_rich(r: &mut XmlReader<'_>) -> Result<CellValue> {
    let mut plain: Option<String> = None;
    let mut runs: Vec<TextRun> = Vec::new();
    for_children(r, |r, e, empty| {
        match lname(e).as_str() {
            "t" => {
                let t = if empty { String::new() } else { read_text(r)? };
                plain
                    .get_or_insert_with(String::new)
                    .push_str(&xstring_decode(&t));
            }
            "r" if !empty => {
                let mut run = TextRun::default();
                for_children(r, |r, e, empty| {
                    match lname(e).as_str() {
                        "rPr" => run.font = Some(read_font(r, empty)?),
                        "t" if !empty => run.text.push_str(&xstring_decode(&read_text(r)?)),
                        _ => {}
                    }
                    Ok(())
                })?;
                runs.push(run);
            }
            _ => {}
        }
        Ok(())
    })?;
    if runs.is_empty() {
        Ok(CellValue::Text(plain.unwrap_or_default()))
    } else {
        Ok(CellValue::RichText(runs))
    }
}

fn rich_to_runs(v: CellValue) -> Vec<TextRun> {
    match v {
        CellValue::RichText(r) => r,
        CellValue::Text(t) if t.is_empty() => Vec::new(),
        CellValue::Text(t) => vec![TextRun {
            text: t,
            font: None,
        }],
        _ => Vec::new(),
    }
}

/// §18.7.6 comments.
fn read_comments(data: &[u8], part: &str) -> Result<(Vec<String>, Vec<Comment>)> {
    let mut r = XmlReader::new(data, part);
    let (_, empty) = root(&mut r)?;
    let mut authors: Vec<String> = Vec::new();
    let mut out = Vec::new();
    if empty {
        return Ok((authors, out));
    }
    for_children(&mut r, |r, e, empty| {
        match lname(e).as_str() {
            // §18.7.2 authors
            "authors" if !empty => for_children(r, |r, e, empty| {
                if lname(e) == "author" {
                    authors.push(if empty { String::new() } else { read_text(r)? });
                }
                Ok(())
            })?,
            // §18.7.4 commentList / §18.7.3 comment
            "commentList" if !empty => for_children(r, |r, e, empty| {
                if lname(e) == "comment" {
                    let mut at = attrs(e);
                    let cell = take(&mut at, "ref")
                        .and_then(|s| CellRef::parse(&s))
                        .unwrap_or_default();
                    let author_id = take_u32(&mut at, "authorId").unwrap_or(0) as usize;
                    let mut text = Vec::new();
                    if !empty {
                        for_children(r, |r, e, empty| {
                            if lname(e) == "text" && !empty {
                                text = rich_to_runs(read_rich(r)?);
                            }
                            Ok(())
                        })?;
                    }
                    out.push(Comment {
                        cell,
                        author: authors.get(author_id).cloned().unwrap_or_default(),
                        text,
                    });
                }
                Ok(())
            })?,
            _ => {}
        }
        Ok(())
    })?;
    Ok((authors, out))
}

/// §18.5.1.2 table.
fn read_table(data: &[u8], part: &str) -> Result<Table> {
    const TABLE_SCHEMA: &[&str] = &[
        "autoFilter",
        "sortState",
        "tableColumns",
        "tableStyleInfo",
        "extLst",
    ];
    let mut r = XmlReader::new(data, part);
    let (rootel, empty) = root(&mut r)?;
    let mut at = root_attrs_of(&rootel, false);
    let mut t = Table {
        id: take_u32(&mut at, "id").unwrap_or(1),
        name: take(&mut at, "name").unwrap_or_default(),
        display_name: take(&mut at, "displayName").unwrap_or_default(),
        range: take(&mut at, "ref")
            .and_then(|s| CellRange::parse(&s))
            .unwrap_or_default(),
        header_row_count: take_u32(&mut at, "headerRowCount"),
        totals_row_count: take_u32(&mut at, "totalsRowCount"),
        totals_row_shown: take_bool(&mut at, "totalsRowShown"),
        attrs: at,
        ..Table::default()
    };
    if empty {
        return Ok(t);
    }
    let mut prev = String::new();
    for_children(&mut r, |r, e, empty| {
        let name = lname(e);
        match name.as_str() {
            "autoFilter" => t.auto_filter = Some(capture(r, e, empty)?),
            // §18.5.1.4 tableColumns
            "tableColumns" if !empty => for_children(r, |r, e, empty| {
                if lname(e) == "tableColumn" {
                    t.columns.push(read_table_column(r, e, empty)?);
                }
                Ok(())
            })?,
            "tableColumns" => {}
            "tableStyleInfo" => {
                let mut at = attrs(e);
                t.style = Some(TableStyleInfo {
                    name: take(&mut at, "name"),
                    show_first_column: take_bool(&mut at, "showFirstColumn").unwrap_or(false),
                    show_last_column: take_bool(&mut at, "showLastColumn").unwrap_or(false),
                    show_row_stripes: take_bool(&mut at, "showRowStripes").unwrap_or(false),
                    show_column_stripes: take_bool(&mut at, "showColumnStripes").unwrap_or(false),
                });
            }
            _ => {
                let xml = capture(r, e, empty)?;
                push_unknown(&mut t.unknown, &mut prev, TABLE_SCHEMA, &name, xml);
                return Ok(());
            }
        }
        prev = name;
        Ok(())
    })?;
    Ok(t)
}

/// §18.5.1.3 tableColumn.
fn read_table_column(
    r: &mut XmlReader<'_>,
    e: &quick_xml::events::BytesStart<'_>,
    empty: bool,
) -> Result<TableColumn> {
    const COL_SCHEMA: &[&str] = &[
        "calculatedColumnFormula",
        "totalsRowFormula",
        "xmlColumnPr",
        "extLst",
    ];
    let mut at = attrs(e);
    let mut c = TableColumn {
        id: take_u32(&mut at, "id").unwrap_or(0),
        name: take(&mut at, "name").unwrap_or_default(),
        totals_row_function: take(&mut at, "totalsRowFunction"),
        totals_row_label: take(&mut at, "totalsRowLabel"),
        attrs: at,
        ..TableColumn::default()
    };
    if empty {
        return Ok(c);
    }
    let mut prev = String::new();
    for_children(r, |r, e, empty| {
        let name = lname(e);
        match name.as_str() {
            "calculatedColumnFormula" if !empty => c.calculated_formula = Some(read_text(r)?),
            "totalsRowFormula" if !empty => c.totals_row_formula = Some(read_text(r)?),
            _ => {
                let xml = capture(r, e, empty)?;
                push_unknown(&mut c.unknown, &mut prev, COL_SCHEMA, &name, xml);
                return Ok(());
            }
        }
        prev = name;
        Ok(())
    })?;
    Ok(c)
}

/// §18.3.1.99 worksheet. Returns the relationship ids listed in `tableParts`.
#[allow(clippy::match_same_arms)]
fn read_sheet(
    data: &[u8],
    part: &str,
    sst: &[CellValue],
    links: &HashMap<String, String>,
    ws: &mut Worksheet,
) -> Result<Vec<String>> {
    let mut r = XmlReader::new(data, part);
    let (rootel, empty) = root(&mut r)?;
    ws.root_attrs = root_attrs_of(&rootel, true);
    let mut tables = Vec::new();
    if empty {
        return Ok(tables);
    }
    let mut prev = String::new();
    let mut unknown: Vec<RawXml> = Vec::new();
    for_children(&mut r, |r, e, empty| {
        let name = lname(e);
        match name.as_str() {
            // §18.3.1.82 sheetPr
            "sheetPr" => {
                let mut at = attrs(e);
                ws.props.present = true;
                if !empty {
                    for_children(r, |_, e, _| {
                        let mut a2 = attrs(e);
                        match lname(e).as_str() {
                            // §18.3.1.93 tabColor
                            "tabColor" => ws.props.tab_color = Some(read_color(e)),
                            // §18.3.1.61 outlinePr
                            "outlinePr" => {
                                ws.props.summary_below = take_bool(&mut a2, "summaryBelow");
                                ws.props.summary_right = take_bool(&mut a2, "summaryRight");
                                ws.props.outline_attrs = a2;
                            }
                            // §18.3.1.65 pageSetUpPr
                            "pageSetUpPr" => {
                                ws.props.fit_to_page = take_bool(&mut a2, "fitToPage");
                                ws.props.page_setup_attrs = a2;
                            }
                            _ => {}
                        }
                        Ok(())
                    })?;
                }
                ws.props.attrs = std::mem::take(&mut at);
            }
            // §18.3.1.35 dimension — recomputed on write.
            "dimension" => {}
            // §18.3.1.88 sheetViews
            "sheetViews" if !empty => for_children(r, |r, e, empty| {
                if lname(e) == "sheetView" {
                    ws.views.push(read_sheet_view(r, e, empty)?);
                }
                Ok(())
            })?,
            "sheetViews" => {}
            // §18.3.1.81 sheetFormatPr
            "sheetFormatPr" => {
                let mut at = attrs(e);
                let f = &mut ws.format;
                f.default_row_height = take_f64(&mut at, "defaultRowHeight");
                f.default_col_width = take_f64(&mut at, "defaultColWidth");
                f.base_col_width = take_u32(&mut at, "baseColWidth");
                f.custom_height = take_bool(&mut at, "customHeight").unwrap_or(false);
                f.zero_height = take_bool(&mut at, "zeroHeight").unwrap_or(false);
                f.outline_level_row = take_u32(&mut at, "outlineLevelRow").unwrap_or(0) as u8;
                f.outline_level_col = take_u32(&mut at, "outlineLevelCol").unwrap_or(0) as u8;
                f.attrs = at;
            }
            // §18.3.1.17 cols / §18.3.1.13 col
            "cols" if !empty => for_children(r, |_, e, _| {
                if lname(e) == "col" {
                    let mut at = attrs(e);
                    let min = take_u32(&mut at, "min").unwrap_or(1).max(1) - 1;
                    let max = take_u32(&mut at, "max").unwrap_or(min + 1).max(min + 1) - 1;
                    ws.cols.push(ColInfo {
                        min,
                        max: max.min(bo_model::sheet::MAX_COLS - 1),
                        width: take_f64(&mut at, "width"),
                        custom_width: take_bool(&mut at, "customWidth").unwrap_or(false),
                        hidden: take_bool(&mut at, "hidden").unwrap_or(false),
                        best_fit: take_bool(&mut at, "bestFit").unwrap_or(false),
                        outline_level: take_u32(&mut at, "outlineLevel").unwrap_or(0) as u8,
                        collapsed: take_bool(&mut at, "collapsed").unwrap_or(false),
                        style: take_u32(&mut at, "style"),
                    });
                }
                Ok(())
            })?,
            "cols" => {}
            // §18.3.1.80 sheetData
            "sheetData" if !empty => read_sheet_data(r, sst, ws)?,
            "sheetData" => {}
            // §18.3.1.55 mergeCells / §18.3.1.54 mergeCell
            "mergeCells" if !empty => for_children(r, |_, e, _| {
                if lname(e) == "mergeCell" {
                    let mut at = attrs(e);
                    if let Some(rg) = take(&mut at, "ref").and_then(|s| CellRange::parse(&s)) {
                        ws.merges.push(rg);
                    }
                }
                Ok(())
            })?,
            "mergeCells" => {}
            // §18.3.1.18 conditionalFormatting
            "conditionalFormatting" => {
                let mut at = attrs(e);
                let mut cf = ConditionalFormat {
                    sqref: CellRange::parse_sqref(&take(&mut at, "sqref").unwrap_or_default()),
                    pivot: take_bool(&mut at, "pivot").unwrap_or(false),
                    rules: Vec::new(),
                };
                if !empty {
                    for_children(r, |r, e, empty| {
                        if lname(e) == "cfRule" {
                            cf.rules.push(read_cf_rule(r, e, empty)?);
                        }
                        Ok(())
                    })?;
                }
                ws.conditional_formats.push(cf);
            }
            // §18.3.1.33 dataValidations
            "dataValidations" => {
                let mut at = attrs(e);
                take(&mut at, "count");
                ws.data_validations_attrs = at;
                if !empty {
                    for_children(r, |r, e, empty| {
                        if lname(e) == "dataValidation" {
                            ws.data_validations.push(read_data_validation(r, e, empty)?);
                        }
                        Ok(())
                    })?;
                }
            }
            // §18.3.1.48 hyperlinks / §18.3.1.47 hyperlink
            "hyperlinks" if !empty => for_children(r, |_, e, _| {
                if lname(e) == "hyperlink" {
                    let mut at = attrs(e);
                    let range = take(&mut at, "ref")
                        .and_then(|s| CellRange::parse(&s))
                        .unwrap_or_default();
                    let id = take_q(&mut at, "r:id").or_else(|| take(&mut at, "id"));
                    ws.hyperlinks.push(Hyperlink {
                        range,
                        target: id.and_then(|id| links.get(&id).cloned()),
                        location: take(&mut at, "location"),
                        display: take(&mut at, "display"),
                        tooltip: take(&mut at, "tooltip"),
                    });
                }
                Ok(())
            })?,
            "hyperlinks" => {}
            // §18.3.1.70 printOptions
            "printOptions" => {
                let mut at = attrs(e);
                let p = &mut ws.page_setup;
                p.has_print_options = true;
                p.print_grid_lines = take_bool(&mut at, "gridLines").unwrap_or(false);
                p.print_headings = take_bool(&mut at, "headings").unwrap_or(false);
                p.horizontal_centered = take_bool(&mut at, "horizontalCentered").unwrap_or(false);
                p.vertical_centered = take_bool(&mut at, "verticalCentered").unwrap_or(false);
            }
            // §18.3.1.62 pageMargins
            "pageMargins" => {
                let mut at = attrs(e);
                let d = PageMargins::default();
                ws.page_setup.margins = Some(PageMargins {
                    left: take_f64(&mut at, "left").unwrap_or(d.left),
                    right: take_f64(&mut at, "right").unwrap_or(d.right),
                    top: take_f64(&mut at, "top").unwrap_or(d.top),
                    bottom: take_f64(&mut at, "bottom").unwrap_or(d.bottom),
                    header: take_f64(&mut at, "header").unwrap_or(d.header),
                    footer: take_f64(&mut at, "footer").unwrap_or(d.footer),
                });
            }
            // §18.3.1.63 pageSetup
            "pageSetup" => {
                let mut at = attrs(e);
                let p = &mut ws.page_setup;
                p.has_page_setup = true;
                p.paper_size = take_u32(&mut at, "paperSize");
                p.orientation = take(&mut at, "orientation");
                p.scale = take_u32(&mut at, "scale");
                p.fit_to_width = take_u32(&mut at, "fitToWidth");
                p.fit_to_height = take_u32(&mut at, "fitToHeight");
                p.attrs = at;
            }
            // §18.3.1.46 headerFooter
            "headerFooter" => {
                let at = attrs(e);
                let mut odd_header = None;
                let mut odd_footer = None;
                let mut other = !at.is_empty();
                let xml = capture(r, e, empty)?;
                // Parse the captured element again for the two common children.
                let mut r2 = XmlReader::new(xml.as_bytes(), "headerFooter");
                let (_, e2) = root(&mut r2)?;
                if !e2 {
                    for_children(&mut r2, |r2, c, cempty| {
                        match lname(c).as_str() {
                            "oddHeader" if !cempty => odd_header = Some(read_text(r2)?),
                            "oddFooter" if !cempty => odd_footer = Some(read_text(r2)?),
                            _ => other = true,
                        }
                        Ok(())
                    })?;
                }
                let p = &mut ws.page_setup;
                if other {
                    p.header_footer_raw = Some(xml);
                } else {
                    p.odd_header = odd_header;
                    p.odd_footer = odd_footer;
                }
            }
            // §18.3.1.74 rowBreaks / §18.3.1.14 colBreaks (brk §18.3.1.3)
            "rowBreaks" | "colBreaks" if !empty => {
                let mut v = Vec::new();
                for_children(r, |_, e, _| {
                    if lname(e) == "brk" {
                        let mut at = attrs(e);
                        if let Some(id) = take_u32(&mut at, "id") {
                            // `id` is the zero-based index of the first row/col of the next page.
                            if id > 0 {
                                v.push(id - 1);
                            }
                        }
                    }
                    Ok(())
                })?;
                if name == "rowBreaks" {
                    ws.page_setup.row_breaks = v;
                } else {
                    ws.page_setup.col_breaks = v;
                }
            }
            "rowBreaks" | "colBreaks" => {}
            // §18.3.1.2 autoFilter
            "autoFilter" => {
                ws.auto_filter = Some(crate::autofilter::read_auto_filter(r, e, empty)?);
            }
            // §18.3.1.95 tableParts / §18.3.1.94 tablePart
            "tableParts" if !empty => for_children(r, |_, e, _| {
                if lname(e) == "tablePart" {
                    let mut at = attrs(e);
                    if let Some(id) = take_q(&mut at, "r:id").or_else(|| take(&mut at, "id")) {
                        tables.push(id);
                    }
                }
                Ok(())
            })?,
            "tableParts" => {}
            _ => {
                let xml = capture(r, e, empty)?;
                push_unknown(&mut unknown, &mut prev, SHEET_SCHEMA, &name, xml);
                return Ok(());
            }
        }
        prev = name;
        Ok(())
    })?;
    ws.unknown = unknown;
    Ok(tables)
}

/// §18.3.1.87 sheetView.
fn read_sheet_view(
    r: &mut XmlReader<'_>,
    e: &quick_xml::events::BytesStart<'_>,
    empty: bool,
) -> Result<SheetView> {
    let mut at = attrs(e);
    let mut v = SheetView {
        workbook_view_id: take_u32(&mut at, "workbookViewId").unwrap_or(0),
        tab_selected: take_bool(&mut at, "tabSelected").unwrap_or(false),
        show_grid_lines: take_bool(&mut at, "showGridLines"),
        show_headers: take_bool(&mut at, "showRowColHeaders"),
        zoom_scale: take_u32(&mut at, "zoomScale"),
        right_to_left: take_bool(&mut at, "rightToLeft").unwrap_or(false),
        top_left_cell: take(&mut at, "topLeftCell"),
        attrs: at,
        ..SheetView::default()
    };
    if empty {
        return Ok(v);
    }
    let mut prev = String::new();
    for_children(r, |r, e, empty| {
        let name = lname(e);
        match name.as_str() {
            // §18.3.1.66 pane
            "pane" => {
                let mut at = attrs(e);
                v.pane = Some(Pane {
                    x_split: take_f64(&mut at, "xSplit").unwrap_or(0.0),
                    y_split: take_f64(&mut at, "ySplit").unwrap_or(0.0),
                    top_left_cell: take(&mut at, "topLeftCell"),
                    active_pane: take(&mut at, "activePane"),
                    state: match take(&mut at, "state").as_deref() {
                        Some("frozen") => PaneState::Frozen,
                        Some("frozenSplit") => PaneState::FrozenSplit,
                        _ => PaneState::Split,
                    },
                });
            }
            // §18.3.1.78 selection
            "selection" => v.selections.push(attrs(e)),
            _ => {
                let xml = capture(r, e, empty)?;
                push_unknown(
                    &mut v.unknown,
                    &mut prev,
                    &["pane", "selection", "pivotSelection", "extLst"],
                    &name,
                    xml,
                );
                return Ok(());
            }
        }
        prev = name;
        Ok(())
    })?;
    Ok(v)
}

/// §18.3.1.10 cfRule.
fn read_cf_rule(
    r: &mut XmlReader<'_>,
    e: &quick_xml::events::BytesStart<'_>,
    empty: bool,
) -> Result<CfRule> {
    let mut at = attrs(e);
    let mut rule = CfRule {
        kind: take(&mut at, "type").unwrap_or_default(),
        priority: take(&mut at, "priority")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1),
        dxf_id: take_u32(&mut at, "dxfId"),
        operator: take(&mut at, "operator"),
        stop_if_true: take_bool(&mut at, "stopIfTrue").unwrap_or(false),
        text: take(&mut at, "text"),
        attrs: at,
        ..CfRule::default()
    };
    if empty {
        return Ok(rule);
    }
    let mut prev = String::new();
    for_children(r, |r, e, empty| {
        let name = lname(e);
        match name.as_str() {
            "formula" => rule
                .formulas
                .push(if empty { String::new() } else { read_text(r)? }),
            // §18.3.1.16 colorScale
            "colorScale" if !empty => {
                let mut cs = ColorScale::default();
                for_children(r, |_, e, _| {
                    match lname(e).as_str() {
                        "cfvo" => cs.cfvos.push(read_cfvo(e)),
                        "color" => cs.colors.push(read_color(e)),
                        _ => {}
                    }
                    Ok(())
                })?;
                rule.color_scale = Some(cs);
            }
            // §18.3.1.28 dataBar
            "dataBar" => {
                let mut at = attrs(e);
                let mut db = DataBar {
                    min_length: take_u32(&mut at, "minLength"),
                    max_length: take_u32(&mut at, "maxLength"),
                    show_value: take_bool(&mut at, "showValue"),
                    ..DataBar::default()
                };
                if !empty {
                    for_children(r, |_, e, _| {
                        match lname(e).as_str() {
                            "cfvo" => db.cfvos.push(read_cfvo(e)),
                            "color" => db.color = Some(read_color(e)),
                            _ => {}
                        }
                        Ok(())
                    })?;
                }
                rule.data_bar = Some(db);
            }
            // §18.3.1.49 iconSet
            "iconSet" => {
                let mut at = attrs(e);
                let mut is = IconSet {
                    icon_set: take(&mut at, "iconSet"),
                    show_value: take_bool(&mut at, "showValue"),
                    percent: take_bool(&mut at, "percent"),
                    reverse: take_bool(&mut at, "reverse"),
                    ..IconSet::default()
                };
                if !empty {
                    for_children(r, |_, e, _| {
                        if lname(e) == "cfvo" {
                            is.cfvos.push(read_cfvo(e));
                        }
                        Ok(())
                    })?;
                }
                rule.icon_set = Some(is);
            }
            _ => {
                let xml = capture(r, e, empty)?;
                push_unknown(
                    &mut rule.unknown,
                    &mut prev,
                    &["formula", "colorScale", "dataBar", "iconSet", "extLst"],
                    &name,
                    xml,
                );
                return Ok(());
            }
        }
        prev = name;
        Ok(())
    })?;
    Ok(rule)
}

/// §18.3.1.11 cfvo.
fn read_cfvo(e: &quick_xml::events::BytesStart<'_>) -> Cfvo {
    let mut at = attrs(e);
    Cfvo {
        kind: take(&mut at, "type").unwrap_or_default(),
        val: take(&mut at, "val"),
        gte: take_bool(&mut at, "gte"),
    }
}

/// §18.3.1.32 dataValidation.
fn read_data_validation(
    r: &mut XmlReader<'_>,
    e: &quick_xml::events::BytesStart<'_>,
    empty: bool,
) -> Result<DataValidation> {
    let mut at = attrs(e);
    let mut dv = DataValidation {
        sqref: CellRange::parse_sqref(&take(&mut at, "sqref").unwrap_or_default()),
        kind: take(&mut at, "type"),
        operator: take(&mut at, "operator"),
        allow_blank: take_bool(&mut at, "allowBlank").unwrap_or(false),
        show_input_message: take_bool(&mut at, "showInputMessage").unwrap_or(false),
        show_error_message: take_bool(&mut at, "showErrorMessage").unwrap_or(false),
        show_drop_down: take_bool(&mut at, "showDropDown").unwrap_or(false),
        error_style: take(&mut at, "errorStyle"),
        error_title: take(&mut at, "errorTitle"),
        error: take(&mut at, "error"),
        prompt_title: take(&mut at, "promptTitle"),
        prompt: take(&mut at, "prompt"),
        ..DataValidation::default()
    };
    dv.attrs = at;
    if !empty {
        for_children(r, |r, e, empty| {
            match lname(e).as_str() {
                // §18.3.1.40-ish formula1/formula2 (CT_Formula text)
                "formula1" if !empty => dv.formula1 = Some(read_text(r)?),
                "formula2" if !empty => dv.formula2 = Some(read_text(r)?),
                _ => {}
            }
            Ok(())
        })?;
    }
    Ok(dv)
}

/// §18.3.1.80 sheetData: rows (§18.3.1.73) and cells (§18.3.1.4).
fn read_sheet_data(r: &mut XmlReader<'_>, sst: &[CellValue], ws: &mut Worksheet) -> Result<()> {
    let mut next_row: u32 = 0;
    for_children(r, |r, e, empty| {
        if lname(e) != "row" {
            return Ok(());
        }
        let mut at = attrs(e);
        let row = take_u32(&mut at, "r")
            .filter(|v| (1..=bo_model::sheet::MAX_ROWS).contains(v))
            .map_or(next_row, |v| v - 1);
        next_row = row + 1;
        take(&mut at, "spans");
        let custom_format = take_bool(&mut at, "customFormat").unwrap_or(false);
        let s = take_u32(&mut at, "s");
        let info = RowInfo {
            height: take_f64(&mut at, "ht"),
            custom_height: take_bool(&mut at, "customHeight").unwrap_or(false),
            hidden: take_bool(&mut at, "hidden").unwrap_or(false),
            outline_level: take_u32(&mut at, "outlineLevel").unwrap_or(0) as u8,
            collapsed: take_bool(&mut at, "collapsed").unwrap_or(false),
            style: if custom_format { s } else { None },
            thick_top: take_bool(&mut at, "thickTop").unwrap_or(false),
            thick_bot: take_bool(&mut at, "thickBot").unwrap_or(false),
            attrs: at,
        };
        if info != RowInfo::default() {
            ws.rows.insert(row, info);
        }
        if empty {
            return Ok(());
        }
        let mut next_col: u32 = 0;
        for_children(r, |r, e, empty| {
            if lname(e) != "c" {
                return Ok(());
            }
            let mut at = attrs(e);
            let col = take(&mut at, "r")
                .and_then(|s| CellRef::parse(&s))
                .map_or(next_col, |c| c.col);
            next_col = col + 1;
            let t = take(&mut at, "t").unwrap_or_else(|| "n".into());
            let mut cell = Cell {
                style: take_u32(&mut at, "s").unwrap_or(0),
                cm: take_u32(&mut at, "cm"),
                vm: take_u32(&mut at, "vm"),
                show_phonetic: take_bool(&mut at, "ph").unwrap_or(false),
                ..Cell::default()
            };
            let mut v: Option<String> = None;
            let mut inline: Option<CellValue> = None;
            if !empty {
                for_children(r, |r, e, empty| {
                    match lname(e).as_str() {
                        // §18.3.1.96 v
                        "v" => v = Some(if empty { String::new() } else { read_text(r)? }),
                        // §18.3.1.40 f
                        "f" => {
                            let mut fa = attrs(e);
                            let ft = take(&mut fa, "t");
                            let range = take(&mut fa, "ref");
                            let si = take_u32(&mut fa, "si");
                            let ca = take_bool(&mut fa, "ca").unwrap_or(false);
                            let kind = match ft.as_deref() {
                                Some("shared") => FormulaKind::Shared {
                                    index: si.unwrap_or(0),
                                    range,
                                },
                                Some("array") => FormulaKind::Array {
                                    range: range.unwrap_or_default(),
                                },
                                Some("dataTable") => FormulaKind::DataTable {
                                    range: range.unwrap_or_default(),
                                },
                                _ => {
                                    if let Some(rg) = range {
                                        fa.push(("ref".into(), rg));
                                    }
                                    if let Some(si) = si {
                                        fa.push(("si".into(), si.to_string()));
                                    }
                                    FormulaKind::Normal
                                }
                            };
                            let text = if empty { String::new() } else { read_text(r)? };
                            cell.formula = Some(Formula {
                                text,
                                kind,
                                calc_always: ca,
                                attrs: fa,
                            });
                        }
                        // §18.3.1.53 is (inline rich string)
                        "is" if !empty => inline = Some(read_rich(r)?),
                        _ => {}
                    }
                    Ok(())
                })?;
            }
            // §18.18.11 ST_CellType
            cell.value = match (t.as_str(), v) {
                ("inlineStr", _) => inline.unwrap_or(CellValue::Empty),
                (_, None) => CellValue::Empty,
                ("s", Some(v)) => v
                    .trim()
                    .parse::<usize>()
                    .ok()
                    .and_then(|i| sst.get(i).cloned())
                    .unwrap_or_else(|| CellValue::Text(String::new())),
                ("b", Some(v)) => CellValue::Bool(pbool(v.trim())),
                ("e", Some(v)) => CellValue::Error(CellError::parse(v.trim())),
                ("str", Some(v)) => CellValue::Text(xstring_decode(&v)),
                ("d", Some(v)) => CellValue::Date(v),
                (_, Some(v)) => match v.trim().parse::<f64>() {
                    Ok(n) if n.is_finite() => CellValue::Number(n),
                    _ => CellValue::Error(CellError::Value),
                },
            };
            if col < bo_model::sheet::MAX_COLS {
                ws.cells.insert((row, col), cell);
            }
            Ok(())
        })
    })
}
