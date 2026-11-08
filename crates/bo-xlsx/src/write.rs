//! [`Workbook`] → `.xlsx`.

use crate::styles::{write_color, write_font, write_styles, SML_NS};
use crate::xml::{
    a, a_opt, a_optb, a_optf, a_raw, a_true, emit_ordered, emit_unknown, fmt_f64, needs_preserve,
    xstring_encode,
};
use crate::{
    Result, CT_COMMENTS, CT_SHARED_STRINGS, CT_STYLES, CT_TABLE, CT_WORKBOOK, CT_WORKSHEET,
    REL_COMMENTS, REL_HYPERLINK, REL_SHARED_STRINGS, REL_TABLE, REL_WORKSHEET, SHEET_SCHEMA,
    TABLE_SCHEMA, WORKBOOK_SCHEMA,
};
use bo_model::sheet::{
    sqref_to_string, Cell, CellRange, CellRef, CellValue, CfRule, Cfvo, Comment, FormulaKind,
    OpaqueRel, PaneState, RawAttr, SheetKind, SheetState, Table, TextRun, Workbook, Worksheet,
};
use bo_opc::xml::escape;
use bo_opc::{ContentTypes, Package, Relationship, TargetMode, REL_OFFICE_DOCUMENT, REL_STYLES};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;

const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

/// Write the root element's attributes: the SpreadsheetML default namespace (and
/// the relationships namespace when `need_r`), then every preserved attribute.
pub(crate) fn root_attrs(out: &mut String, attrs: &[RawAttr], need_r: bool) {
    a(out, "xmlns", SML_NS);
    if need_r {
        a(out, "xmlns:r", R_NS);
    }
    for (k, v) in attrs {
        if k == "xmlns" || (need_r && k == "xmlns:r") {
            continue;
        }
        a(out, k, v);
    }
}

/// Allocates relationship ids that do not collide with preserved ones.
struct RelIds {
    used: HashSet<String>,
    next: u32,
}

impl RelIds {
    fn new(preserved: &[OpaqueRel]) -> Self {
        RelIds {
            used: preserved.iter().map(|r| r.id.clone()).collect(),
            next: 1,
        }
    }

    fn alloc(&mut self) -> String {
        loop {
            let id = format!("rId{}", self.next);
            self.next += 1;
            if self.used.insert(id.clone()) {
                return id;
            }
        }
    }
}

/// Relative reference from `source` part to absolute part `target` (§9.3 Part 2).
pub(crate) fn relative_target(source: &str, target: &str) -> String {
    let src_dir: Vec<&str> = source
        .rsplit_once('/')
        .map_or("", |(d, _)| d)
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    let tgt: Vec<&str> = target.split('/').filter(|s| !s.is_empty()).collect();
    let (tgt_dir, file) = match tgt.split_last() {
        Some((f, d)) => (d, *f),
        None => return target.to_owned(),
    };
    let common = src_dir
        .iter()
        .zip(tgt_dir.iter())
        .take_while(|(a, b)| a == b)
        .count();
    let mut parts: Vec<&str> = vec![".."; src_dir.len() - common];
    parts.extend_from_slice(&tgt_dir[common..]);
    parts.push(file);
    parts.join("/")
}

fn to_rels(source: &str, opaque: &[OpaqueRel]) -> Vec<Relationship> {
    opaque
        .iter()
        .map(|r| Relationship {
            id: r.id.clone(),
            rel_type: r.rel_type.clone(),
            target: if r.external {
                r.target.clone()
            } else {
                relative_target(source, &r.target)
            },
            mode: if r.external {
                TargetMode::External
            } else {
                TargetMode::Internal
            },
        })
        .collect()
}

/// Generates part names that do not collide with preserved parts.
struct Names {
    used: HashSet<String>,
}

impl Names {
    fn fresh(&mut self, f: impl Fn(u32) -> String) -> String {
        let mut n = 1;
        loop {
            let name = f(n);
            if self.used.insert(name.to_ascii_lowercase()) {
                return name;
            }
            n += 1;
        }
    }
}

/// Serialize a workbook to `.xlsx` bytes.
///
/// Opaque parts and relationships captured by the reader are written back
/// unchanged; workbook, sheets, styles, shared strings, tables and comments are
/// regenerated from the model. The calculation chain is not written.
pub fn write_xlsx(wb: &Workbook) -> Result<Vec<u8>> {
    let mut names = Names {
        used: wb
            .opaque
            .parts
            .iter()
            .map(|p| p.name.to_ascii_lowercase())
            .collect(),
    };
    let mut out_parts: Vec<(String, Option<String>, Vec<u8>)> = Vec::new();
    // Opaque parts rewritten because a chart was edited or added.
    let mut overrides: HashMap<String, Vec<u8>> = HashMap::new();
    let wb_path = names.fresh(|n| {
        if n == 1 {
            "/xl/workbook.xml".into()
        } else {
            format!("/xl/workbook{n}.xml")
        }
    });

    // Package relationships.
    let mut pkg_ids = RelIds::new(&wb.opaque.package_rels);
    let mut pkg_rels = to_rels("/", &wb.opaque.package_rels);
    pkg_rels.insert(
        0,
        Relationship {
            id: pkg_ids.alloc(),
            rel_type: REL_OFFICE_DOCUMENT.into(),
            target: wb_path.trim_start_matches('/').into(),
            mode: TargetMode::Internal,
        },
    );

    // Workbook relationships.
    let mut wb_ids = RelIds::new(&wb.opaque.workbook_rels);
    let mut wb_rels = to_rels(&wb_path, &wb.opaque.workbook_rels);
    let mut sst = SharedStrings::default();

    // Unique sheet ids (§18.2.19: sheetId must be unique).
    let mut seen_ids = BTreeSet::new();
    let max_id = wb.sheets.iter().map(|s| s.sheet_id).max().unwrap_or(0);
    let mut next_id = max_id + 1;
    let mut sheet_ids = Vec::with_capacity(wb.sheets.len());
    for s in &wb.sheets {
        let id = if s.sheet_id == 0 || !seen_ids.insert(s.sheet_id) {
            next_id += 1;
            next_id - 1
        } else {
            s.sheet_id
        };
        sheet_ids.push(id);
    }

    // Pivot caches (§18.10): new and refreshed caches get regenerated parts.
    let (cache_rids, cache_parts) = write_pivot_caches(
        wb,
        &wb_path,
        &mut names,
        &mut wb_ids,
        &mut wb_rels,
        &mut out_parts,
        &mut overrides,
    );

    let mut sheet_rel_ids = Vec::with_capacity(wb.sheets.len());
    for ws in &wb.sheets {
        let rid = wb_ids.alloc();
        match &ws.kind {
            SheetKind::Worksheet => {
                let path = names.fresh(|n| format!("/xl/worksheets/sheet{n}.xml"));
                let mut ids = RelIds::new(&ws.rels);
                let mut rels = to_rels(&path, &ws.rels);
                // Hyperlinks with external targets (§18.3.1.47 r:id).
                let mut link_ids = Vec::new();
                for h in &ws.hyperlinks {
                    link_ids.push(h.target.as_ref().map(|t| {
                        let id = ids.alloc();
                        rels.push(Relationship {
                            id: id.clone(),
                            rel_type: REL_HYPERLINK.into(),
                            target: t.clone(),
                            mode: TargetMode::External,
                        });
                        id
                    }));
                }
                // Tables (§18.5).
                let mut table_rids = Vec::new();
                for t in &ws.tables {
                    let tpath = names.fresh(|n| format!("/xl/tables/table{n}.xml"));
                    let id = ids.alloc();
                    rels.push(Relationship {
                        id: id.clone(),
                        rel_type: REL_TABLE.into(),
                        target: relative_target(&path, &tpath),
                        mode: TargetMode::Internal,
                    });
                    table_rids.push(id);
                    out_parts.push((tpath, Some(CT_TABLE.into()), table_xml(t).into_bytes()));
                }
                // Comments (§18.7).
                if !ws.comments.is_empty() {
                    let cpath = names.fresh(|n| format!("/xl/comments{n}.xml"));
                    rels.push(Relationship {
                        id: ids.alloc(),
                        rel_type: REL_COMMENTS.into(),
                        target: relative_target(&path, &cpath),
                        mode: TargetMode::Internal,
                    });
                    out_parts.push((
                        cpath,
                        Some(CT_COMMENTS.into()),
                        comments_xml(&ws.comments, &ws.comment_authors).into_bytes(),
                    ));
                }
                // Threaded comments ([MS-XLSX] threaded comments part).
                if !ws.threaded_comments.is_empty() {
                    let tpath = names.fresh(|n| format!("/xl/threadedComments/threadedComment{n}.xml"));
                    rels.push(Relationship {
                        id: ids.alloc(),
                        rel_type: crate::threaded::REL_THREADED.into(),
                        target: relative_target(&path, &tpath),
                        mode: TargetMode::Internal,
                    });
                    out_parts.push((
                        tpath,
                        Some(crate::threaded::CT_THREADED.into()),
                        crate::threaded::threaded_xml(&ws.threaded_comments).into_bytes(),
                    ));
                }
                // Charts (§20.5 drawing → §21.2 chart parts).
                let drawing_rid = write_charts(
                    wb,
                    ws,
                    &path,
                    &mut names,
                    &mut ids,
                    &mut rels,
                    &mut out_parts,
                    &mut overrides,
                );
                // Pivot tables (§18.10.1.73).
                write_pivot_tables(
                    wb,
                    ws,
                    &path,
                    &cache_parts,
                    &mut names,
                    &mut ids,
                    &mut rels,
                    &mut out_parts,
                    &mut overrides,
                );
                let xml = sheet_xml(ws, &mut sst, &link_ids, &table_rids, drawing_rid.as_deref());
                if !rels.is_empty() {
                    out_parts.push((
                        bo_opc::rels_part_name(&path),
                        None,
                        bo_opc_rels_xml(&rels).into_bytes(),
                    ));
                }
                wb_rels.push(Relationship {
                    id: rid.clone(),
                    rel_type: REL_WORKSHEET.into(),
                    target: relative_target(&wb_path, &path),
                    mode: TargetMode::Internal,
                });
                out_parts.push((path, Some(CT_WORKSHEET.into()), xml.into_bytes()));
            }
            SheetKind::Opaque { rel_type, part } => {
                wb_rels.push(Relationship {
                    id: rid.clone(),
                    rel_type: rel_type.clone(),
                    target: relative_target(&wb_path, part),
                    mode: TargetMode::Internal,
                });
            }
        }
        sheet_rel_ids.push(rid);
    }

    let styles_path = names.fresh(|n| {
        if n == 1 {
            "/xl/styles.xml".into()
        } else {
            format!("/xl/styles{n}.xml")
        }
    });
    // [MS-XLSX] person part (authors of threaded comments).
    if !wb.persons.is_empty() {
        let ppath = "/xl/persons/person.xml".to_owned();
        wb_rels.push(Relationship {
            id: wb_ids.alloc(),
            rel_type: crate::threaded::REL_PERSON.into(),
            target: relative_target(&wb_path, &ppath),
            mode: TargetMode::Internal,
        });
        out_parts.push((
            ppath,
            Some(crate::threaded::CT_PERSON.into()),
            crate::threaded::persons_xml(&wb.persons).into_bytes(),
        ));
    }
    wb_rels.push(Relationship {
        id: wb_ids.alloc(),
        rel_type: REL_STYLES.into(),
        target: relative_target(&wb_path, &styles_path),
        mode: TargetMode::Internal,
    });
    out_parts.push((
        styles_path,
        Some(CT_STYLES.into()),
        write_styles(&wb.styles).into_bytes(),
    ));

    if !sst.items.is_empty() {
        let sst_path = names.fresh(|n| {
            if n == 1 {
                "/xl/sharedStrings.xml".into()
            } else {
                format!("/xl/sharedStrings{n}.xml")
            }
        });
        wb_rels.push(Relationship {
            id: wb_ids.alloc(),
            rel_type: REL_SHARED_STRINGS.into(),
            target: relative_target(&wb_path, &sst_path),
            mode: TargetMode::Internal,
        });
        out_parts.push((
            sst_path,
            Some(CT_SHARED_STRINGS.into()),
            sst.to_xml().into_bytes(),
        ));
    }

    out_parts.push((
        bo_opc::rels_part_name(&wb_path),
        None,
        bo_opc_rels_xml(&wb_rels).into_bytes(),
    ));
    let wb_ct = wb
        .opaque
        .workbook_content_type
        .clone()
        .unwrap_or_else(|| CT_WORKBOOK.into());
    out_parts.push((
        wb_path.clone(),
        Some(wb_ct),
        workbook_xml(wb, &sheet_ids, &sheet_rel_ids, &cache_rids).into_bytes(),
    ));
    out_parts.push((
        "/_rels/.rels".into(),
        None,
        bo_opc_rels_xml(&pkg_rels).into_bytes(),
    ));

    // Content types (ECMA-376 Part 2, §10.1.2).
    let mut ct = ContentTypes {
        defaults: wb.opaque.content_type_defaults.clone(),
        overrides: Vec::new(),
    };
    for (ext, t) in [
        (
            "rels",
            "application/vnd.openxmlformats-package.relationships+xml",
        ),
        ("xml", "application/xml"),
    ] {
        if !ct.defaults.iter().any(|(e, _)| e.eq_ignore_ascii_case(ext)) {
            ct.defaults.push((ext.into(), t.into()));
        }
    }
    for p in &wb.opaque.parts {
        if let Some(t) = &p.content_type {
            ct.overrides.push((p.name.clone(), t.clone()));
        }
    }
    for (name, t, _) in &out_parts {
        if let Some(t) = t {
            ct.overrides.push((name.clone(), t.clone()));
        }
    }

    let mut pkg = Package::new();
    pkg.set_content_types(&ct);
    for (name, _, data) in out_parts {
        pkg.put_part(&name, data);
    }
    for p in &wb.opaque.parts {
        let data = overrides.remove(&p.name).unwrap_or_else(|| p.data.clone());
        pkg.put_part(&p.name, data);
    }
    Ok(pkg.write()?)
}

/// Writes new and edited charts of `ws`; returns the relationship id of a
/// drawing created for the sheet (to reference from `<drawing r:id>`).
#[allow(clippy::too_many_arguments)]
fn write_charts(
    wb: &Workbook,
    ws: &Worksheet,
    sheet_path: &str,
    names: &mut Names,
    ids: &mut RelIds,
    rels: &mut Vec<Relationship>,
    out_parts: &mut Vec<(String, Option<String>, Vec<u8>)>,
    overrides: &mut HashMap<String, Vec<u8>>,
) -> Option<String> {
    use crate::chart_write::{
        add_relationships, anchor_xml, append_to_drawing, chart_space_xml, drawing_xml, CT_CHART,
        CT_DRAWING, REL_DRAWING,
    };
    let opaque = |name: &str| wb.opaque.parts.iter().find(|p| p.name == name);
    // Edited charts: regenerate the part in place.
    for c in ws.charts.iter().filter(|c| c.dirty && !c.part.is_empty()) {
        if opaque(&c.part).is_some() {
            overrides.insert(c.part.clone(), chart_space_xml(c).into_bytes());
        }
    }
    let new: Vec<&bo_model::sheet::Chart> =
        ws.charts.iter().filter(|c| c.part.is_empty()).collect();
    if new.is_empty() {
        return None;
    }
    let paths: Vec<String> = new
        .iter()
        .map(|_| names.fresh(|n| format!("/xl/charts/chart{n}.xml")))
        .collect();
    for (c, p) in new.iter().zip(&paths) {
        out_parts.push((
            p.clone(),
            Some(CT_CHART.into()),
            chart_space_xml(c).into_bytes(),
        ));
    }
    let existing = ws
        .rels
        .iter()
        .find(|r| !r.external && r.rel_type.ends_with("/drawing"))
        .map(|r| r.target.clone())
        .filter(|d| opaque(d).is_some());
    let (dpath, created) = match existing {
        Some(d) => (d, false),
        None => (
            names.fresh(|n| format!("/xl/drawings/drawing{n}.xml")),
            true,
        ),
    };
    let rels_name = bo_opc::rels_part_name(&dpath);
    let targets: Vec<String> = paths.iter().map(|p| relative_target(&dpath, p)).collect();
    let old_rels = overrides
        .get(&rels_name)
        .cloned()
        .or_else(|| opaque(&rels_name).map(|p| p.data.clone()));
    let (rels_bytes, chart_ids) = add_relationships(old_rels.as_deref(), &targets);
    // Shape ids unique within the drawing (§20.5.2.8 cNvPr/@id).
    let base_id: u32 = if created {
        2
    } else {
        1000 + 10 * ws.charts.len() as u32
    };
    let anchors: String = new
        .iter()
        .zip(&chart_ids)
        .enumerate()
        .map(|(i, (c, id))| anchor_xml(c, base_id + i as u32, id))
        .collect();
    if opaque(&rels_name).is_some() {
        overrides.insert(rels_name, rels_bytes);
    } else {
        out_parts.push((rels_name, None, rels_bytes));
    }
    if created {
        out_parts.push((
            dpath.clone(),
            Some(CT_DRAWING.into()),
            drawing_xml(&anchors).into_bytes(),
        ));
        let id = ids.alloc();
        rels.push(Relationship {
            id: id.clone(),
            rel_type: REL_DRAWING.into(),
            target: relative_target(sheet_path, &dpath),
            mode: TargetMode::Internal,
        });
        Some(id)
    } else {
        let base = overrides
            .get(&dpath)
            .cloned()
            .or_else(|| opaque(&dpath).map(|p| p.data.clone()))
            .unwrap_or_default();
        if let Some(d) = append_to_drawing(&base, &anchors) {
            overrides.insert(dpath, d);
        }
        None
    }
}

/// Writes the parts of new and refreshed pivot caches; returns the workbook
/// relationship id of every cache (in [`Workbook::pivot_caches`] order) and the
/// definition part of each cache id.
#[allow(clippy::too_many_arguments)]
fn write_pivot_caches(
    wb: &Workbook,
    wb_path: &str,
    names: &mut Names,
    wb_ids: &mut RelIds,
    wb_rels: &mut Vec<Relationship>,
    out_parts: &mut Vec<(String, Option<String>, Vec<u8>)>,
    overrides: &mut HashMap<String, Vec<u8>>,
) -> (Vec<String>, HashMap<u32, String>) {
    use crate::pivot_write::{
        cache_definition_xml, cache_records_xml, single_rels_xml, CT_PIVOT_CACHE_DEFINITION,
        CT_PIVOT_CACHE_RECORDS, REL_PIVOT_CACHE_DEFINITION, REL_PIVOT_CACHE_RECORDS,
    };
    let opaque = |name: &str| wb.opaque.parts.iter().any(|p| p.name == name);
    let mut put = |name: String, ct: &str, data: Vec<u8>, out_parts: &mut Vec<_>| {
        if opaque(&name) {
            overrides.insert(name, data);
        } else {
            out_parts.push((name, (!ct.is_empty()).then(|| ct.to_owned()), data));
        }
    };
    let mut rids = Vec::with_capacity(wb.pivot_caches.len());
    let mut parts = HashMap::new();
    for c in &wb.pivot_caches {
        let new = c.part.is_empty() && c.rel_id.is_empty();
        if new || (c.dirty && !c.part.is_empty()) {
            let def = if new {
                names.fresh(|n| format!("/xl/pivotCache/pivotCacheDefinition{n}.xml"))
            } else {
                c.part.clone()
            };
            let rec = c.records_part.clone().filter(|_| !new).unwrap_or_else(|| {
                names.fresh(|n| format!("/xl/pivotCache/pivotCacheRecords{n}.xml"))
            });
            put(
                def.clone(),
                CT_PIVOT_CACHE_DEFINITION,
                cache_definition_xml(c, "rId1", wb.date1904).into_bytes(),
                out_parts,
            );
            put(
                rec.clone(),
                CT_PIVOT_CACHE_RECORDS,
                cache_records_xml(c).into_bytes(),
                out_parts,
            );
            put(
                bo_opc::rels_part_name(&def),
                "",
                single_rels_xml(REL_PIVOT_CACHE_RECORDS, &relative_target(&def, &rec)).into_bytes(),
                out_parts,
            );
            if new {
                let id = wb_ids.alloc();
                wb_rels.push(Relationship {
                    id: id.clone(),
                    rel_type: REL_PIVOT_CACHE_DEFINITION.into(),
                    target: relative_target(wb_path, &def),
                    mode: TargetMode::Internal,
                });
                rids.push(id);
            } else {
                rids.push(c.rel_id.clone());
            }
            parts.insert(c.cache_id, def);
        } else {
            rids.push(c.rel_id.clone());
            if !c.part.is_empty() {
                parts.insert(c.cache_id, c.part.clone());
            }
        }
    }
    (rids, parts)
}

/// Writes new and edited pivot tables of `ws`.
#[allow(clippy::too_many_arguments)]
fn write_pivot_tables(
    wb: &Workbook,
    ws: &Worksheet,
    sheet_path: &str,
    cache_parts: &HashMap<u32, String>,
    names: &mut Names,
    ids: &mut RelIds,
    rels: &mut Vec<Relationship>,
    out_parts: &mut Vec<(String, Option<String>, Vec<u8>)>,
    overrides: &mut HashMap<String, Vec<u8>>,
) {
    use crate::pivot_write::{
        pivot_table_xml, single_rels_xml, CT_PIVOT_TABLE, REL_PIVOT_CACHE_DEFINITION,
        REL_PIVOT_TABLE,
    };
    for pt in &ws.pivots {
        let Some(cache) = wb.pivot_caches.iter().find(|c| c.cache_id == pt.cache_id) else {
            continue;
        };
        if pt.part.is_empty() {
            let Some(def) = cache_parts.get(&pt.cache_id) else {
                continue;
            };
            let path = names.fresh(|n| format!("/xl/pivotTables/pivotTable{n}.xml"));
            rels.push(Relationship {
                id: ids.alloc(),
                rel_type: REL_PIVOT_TABLE.into(),
                target: relative_target(sheet_path, &path),
                mode: TargetMode::Internal,
            });
            out_parts.push((
                bo_opc::rels_part_name(&path),
                None,
                single_rels_xml(REL_PIVOT_CACHE_DEFINITION, &relative_target(&path, def))
                    .into_bytes(),
            ));
            out_parts.push((
                path,
                Some(CT_PIVOT_TABLE.into()),
                pivot_table_xml(pt, cache).into_bytes(),
            ));
        } else if pt.dirty && wb.opaque.parts.iter().any(|p| p.name == pt.part) {
            overrides.insert(pt.part.clone(), pivot_table_xml(pt, cache).into_bytes());
        }
    }
}

fn bo_opc_rels_xml(rels: &[Relationship]) -> String {
    // Same serialization as bo-opc's relationships writer.
    let mut p = Package::new();
    p.set_relationships("/", rels);
    p.part("/_rels/.rels")
        .map(|x| String::from_utf8_lossy(&x.data).into_owned())
        .unwrap_or_default()
}

/// §18.2.27 workbook.
fn workbook_xml(
    wb: &Workbook,
    sheet_ids: &[u32],
    rel_ids: &[String],
    cache_rids: &[String],
) -> String {
    let mut out = String::from(XML_DECL);
    out.push_str("<workbook");
    root_attrs(&mut out, &wb.root_attrs, true);
    out.push('>');
    emit_ordered(
        &mut out,
        WORKBOOK_SCHEMA,
        &wb.unknown,
        |slot, out| match slot {
            // §18.2.28 workbookPr
            "workbookPr" if wb.date1904 || !wb.workbook_pr_attrs.is_empty() => {
                out.push_str("<workbookPr");
                a_true(out, "date1904", wb.date1904);
                a_raw(out, &wb.workbook_pr_attrs);
                out.push_str("/>");
            }
            // §18.2.20 sheets
            "sheets" => {
                out.push_str("<sheets>");
                for (i, ws) in wb.sheets.iter().enumerate() {
                    out.push_str("<sheet");
                    a(out, "name", &ws.name);
                    a(out, "sheetId", &sheet_ids[i].to_string());
                    match ws.state {
                        SheetState::Visible => {}
                        SheetState::Hidden => a(out, "state", "hidden"),
                        SheetState::VeryHidden => a(out, "state", "veryHidden"),
                    }
                    a(out, "r:id", &rel_ids[i]);
                    out.push_str("/>");
                }
                out.push_str("</sheets>");
            }
            // §18.2.6 definedNames
            "definedNames" => {
                let mut items = String::new();
                let mut dn =
                    |name: &str, local: Option<u32>, hidden: bool, attrs: &[RawAttr], f: &str| {
                        items.push_str("<definedName");
                        a(&mut items, "name", name);
                        a_opt(&mut items, "localSheetId", local.as_ref());
                        a_true(&mut items, "hidden", hidden);
                        a_raw(&mut items, attrs);
                        let _ = write!(items, ">{}</definedName>", escape(f));
                    };
                for d in &wb.defined_names {
                    dn(&d.name, d.local_sheet, d.hidden, &d.attrs, &d.formula);
                }
                for (i, ws) in wb.sheets.iter().enumerate() {
                    if let Some(f) = &ws.print_area {
                        dn("_xlnm.Print_Area", Some(i as u32), false, &[], f);
                    }
                    if let Some(f) = &ws.print_titles {
                        dn("_xlnm.Print_Titles", Some(i as u32), false, &[], f);
                    }
                }
                if !items.is_empty() {
                    let _ = write!(out, "<definedNames>{items}</definedNames>");
                }
            }
            // §18.2.22 pivotCaches
            "pivotCaches" if !wb.pivot_caches.is_empty() => {
                out.push_str("<pivotCaches>");
                for (c, rid) in wb.pivot_caches.iter().zip(cache_rids) {
                    out.push_str("<pivotCache");
                    a(out, "cacheId", &c.cache_id.to_string());
                    a(out, "r:id", rid);
                    out.push_str("/>");
                }
                out.push_str("</pivotCaches>");
            }
            _ => {}
        },
    );
    out.push_str("</workbook>");
    out
}

/// Shared string table builder (§18.4.9).
#[derive(Default)]
struct SharedStrings {
    items: Vec<String>,
    index: HashMap<String, usize>,
    count: usize,
}

impl SharedStrings {
    fn add(&mut self, v: &CellValue) -> usize {
        let mut si = String::from("<si>");
        match v {
            CellValue::RichText(runs) => write_runs(&mut si, runs),
            other => write_t(&mut si, &other.as_text().unwrap_or_default()),
        }
        si.push_str("</si>");
        self.count += 1;
        if let Some(i) = self.index.get(&si) {
            return *i;
        }
        self.items.push(si.clone());
        self.index.insert(si, self.items.len() - 1);
        self.items.len() - 1
    }

    fn to_xml(&self) -> String {
        let mut out = String::from(XML_DECL);
        let _ = write!(
            out,
            "<sst xmlns=\"{SML_NS}\" count=\"{}\" uniqueCount=\"{}\">",
            self.count,
            self.items.len()
        );
        for i in &self.items {
            out.push_str(i);
        }
        out.push_str("</sst>");
        out
    }
}

/// `t` element (§18.4.12) with `ST_Xstring` escaping.
fn write_t(out: &mut String, text: &str) {
    if needs_preserve(text) {
        out.push_str("<t xml:space=\"preserve\">");
    } else {
        out.push_str("<t>");
    }
    out.push_str(&xstring_encode(text));
    out.push_str("</t>");
}

/// Rich text runs (§18.4.4 r).
fn write_runs(out: &mut String, runs: &[TextRun]) {
    for r in runs {
        out.push_str("<r>");
        if let Some(f) = &r.font {
            write_font(out, f, true);
        }
        write_t(out, &r.text);
        out.push_str("</r>");
    }
}

/// §18.3.1.99 worksheet.
fn sheet_xml(
    ws: &Worksheet,
    sst: &mut SharedStrings,
    link_ids: &[Option<String>],
    table_rids: &[String],
    drawing_rid: Option<&str>,
) -> String {
    let mut out = String::from(XML_DECL);
    out.push_str("<worksheet");
    root_attrs(&mut out, &ws.root_attrs, true);
    out.push('>');
    emit_ordered(
        &mut out,
        SHEET_SCHEMA,
        &ws.unknown,
        |slot, out| match slot {
            "sheetPr" => write_sheet_pr(out, ws),
            // §18.3.1.35 dimension
            "dimension" => {
                let dim = ws
                    .used_range()
                    .unwrap_or_else(|| CellRange::single(CellRef::new(0, 0)));
                let _ = write!(out, "<dimension ref=\"{dim}\"/>");
            }
            "sheetViews" if !ws.views.is_empty() => write_views(out, ws),
            "sheetFormatPr" => write_format(out, ws),
            "cols" if !ws.cols.is_empty() => {
                out.push_str("<cols>");
                for c in &ws.cols {
                    out.push_str("<col");
                    a(out, "min", &(c.min + 1).to_string());
                    a(out, "max", &(c.max + 1).to_string());
                    a_optf(out, "width", c.width);
                    a_opt(out, "style", c.style.as_ref());
                    a_true(out, "hidden", c.hidden);
                    a_true(out, "bestFit", c.best_fit);
                    a_true(out, "customWidth", c.custom_width);
                    if c.outline_level > 0 {
                        a(out, "outlineLevel", &c.outline_level.to_string());
                    }
                    a_true(out, "collapsed", c.collapsed);
                    out.push_str("/>");
                }
                out.push_str("</cols>");
            }
            "sheetData" => write_sheet_data(out, ws, sst),
            "autoFilter" => {
                if let Some(af) = &ws.auto_filter {
                    crate::autofilter::write_auto_filter(out, af);
                }
            }
            "mergeCells" if !ws.merges.is_empty() => {
                let _ = write!(out, "<mergeCells count=\"{}\">", ws.merges.len());
                for m in &ws.merges {
                    let _ = write!(out, "<mergeCell ref=\"{m}\"/>");
                }
                out.push_str("</mergeCells>");
            }
            "conditionalFormatting" => {
                for cf in &ws.conditional_formats {
                    out.push_str("<conditionalFormatting");
                    a_true(out, "pivot", cf.pivot);
                    a(out, "sqref", &sqref_to_string(&cf.sqref));
                    out.push('>');
                    for r in &cf.rules {
                        write_cf_rule(out, r);
                    }
                    out.push_str("</conditionalFormatting>");
                }
            }
            "dataValidations" if !ws.data_validations.is_empty() => {
                out.push_str("<dataValidations");
                a(out, "count", &ws.data_validations.len().to_string());
                a_raw(out, &ws.data_validations_attrs);
                out.push('>');
                for dv in &ws.data_validations {
                    out.push_str("<dataValidation");
                    a_opt(out, "type", dv.kind.as_ref());
                    a_opt(out, "errorStyle", dv.error_style.as_ref());
                    a_opt(out, "operator", dv.operator.as_ref());
                    a_true(out, "allowBlank", dv.allow_blank);
                    a_true(out, "showDropDown", dv.show_drop_down);
                    a_true(out, "showInputMessage", dv.show_input_message);
                    a_true(out, "showErrorMessage", dv.show_error_message);
                    a_opt(out, "errorTitle", dv.error_title.as_ref());
                    a_opt(out, "error", dv.error.as_ref());
                    a_opt(out, "promptTitle", dv.prompt_title.as_ref());
                    a_opt(out, "prompt", dv.prompt.as_ref());
                    a(out, "sqref", &sqref_to_string(&dv.sqref));
                    a_raw(out, &dv.attrs);
                    out.push('>');
                    if let Some(f) = &dv.formula1 {
                        let _ = write!(out, "<formula1>{}</formula1>", escape(f));
                    }
                    if let Some(f) = &dv.formula2 {
                        let _ = write!(out, "<formula2>{}</formula2>", escape(f));
                    }
                    out.push_str("</dataValidation>");
                }
                out.push_str("</dataValidations>");
            }
            "hyperlinks" if !ws.hyperlinks.is_empty() => {
                out.push_str("<hyperlinks>");
                for (h, id) in ws.hyperlinks.iter().zip(link_ids) {
                    out.push_str("<hyperlink");
                    a(out, "ref", &h.range.to_string());
                    a_opt(out, "r:id", id.as_ref());
                    a_opt(out, "location", h.location.as_ref());
                    a_opt(out, "tooltip", h.tooltip.as_ref());
                    a_opt(out, "display", h.display.as_ref());
                    out.push_str("/>");
                }
                out.push_str("</hyperlinks>");
            }
            "printOptions" => {
                let p = &ws.page_setup;
                if p.has_print_options
                    || p.print_grid_lines
                    || p.print_headings
                    || p.horizontal_centered
                    || p.vertical_centered
                {
                    out.push_str("<printOptions");
                    a_true(out, "horizontalCentered", p.horizontal_centered);
                    a_true(out, "verticalCentered", p.vertical_centered);
                    a_true(out, "headings", p.print_headings);
                    a_true(out, "gridLines", p.print_grid_lines);
                    out.push_str("/>");
                }
            }
            "pageMargins" => {
                if let Some(m) = &ws.page_setup.margins {
                    out.push_str("<pageMargins");
                    a(out, "left", &fmt_f64(m.left));
                    a(out, "right", &fmt_f64(m.right));
                    a(out, "top", &fmt_f64(m.top));
                    a(out, "bottom", &fmt_f64(m.bottom));
                    a(out, "header", &fmt_f64(m.header));
                    a(out, "footer", &fmt_f64(m.footer));
                    out.push_str("/>");
                }
            }
            "pageSetup" => {
                let p = &ws.page_setup;
                if p.has_page_setup
                    || p.paper_size.is_some()
                    || p.orientation.is_some()
                    || p.scale.is_some()
                    || p.fit_to_width.is_some()
                    || p.fit_to_height.is_some()
                    || !p.attrs.is_empty()
                {
                    out.push_str("<pageSetup");
                    a_opt(out, "paperSize", p.paper_size.as_ref());
                    a_opt(out, "scale", p.scale.as_ref());
                    a_opt(out, "fitToWidth", p.fit_to_width.as_ref());
                    a_opt(out, "fitToHeight", p.fit_to_height.as_ref());
                    a_opt(out, "orientation", p.orientation.as_ref());
                    a_raw(out, &p.attrs);
                    out.push_str("/>");
                }
            }
            "headerFooter" => {
                let p = &ws.page_setup;
                if let Some(raw) = &p.header_footer_raw {
                    out.push_str(raw);
                } else if p.odd_header.is_some() || p.odd_footer.is_some() {
                    out.push_str("<headerFooter>");
                    if let Some(h) = &p.odd_header {
                        let _ = write!(out, "<oddHeader>{}</oddHeader>", escape(h));
                    }
                    if let Some(f) = &p.odd_footer {
                        let _ = write!(out, "<oddFooter>{}</oddFooter>", escape(f));
                    }
                    out.push_str("</headerFooter>");
                }
            }
            "rowBreaks" => write_breaks(out, "rowBreaks", &ws.page_setup.row_breaks, 16_383),
            "colBreaks" => write_breaks(out, "colBreaks", &ws.page_setup.col_breaks, 1_048_575),
            // §18.3.1.36 drawing (only when the writer created the drawing part).
            "drawing" if drawing_rid.is_some() => {
                let _ = write!(
                    out,
                    "<drawing r:id=\"{}\"/>",
                    escape(drawing_rid.unwrap_or_default())
                );
            }
            "tableParts" if !table_rids.is_empty() => {
                let _ = write!(out, "<tableParts count=\"{}\">", table_rids.len());
                for id in table_rids {
                    let _ = write!(out, "<tablePart r:id=\"{}\"/>", escape(id));
                }
                out.push_str("</tableParts>");
            }
            _ => {}
        },
    );
    out.push_str("</worksheet>");
    out
}

/// §18.3.1.74 rowBreaks / §18.3.1.14 colBreaks.
fn write_breaks(out: &mut String, tag: &str, breaks: &[u32], max: u32) {
    if breaks.is_empty() {
        return;
    }
    let _ = write!(
        out,
        "<{tag} count=\"{n}\" manualBreakCount=\"{n}\">",
        n = breaks.len()
    );
    for b in breaks {
        let _ = write!(out, "<brk id=\"{}\" max=\"{max}\" man=\"1\"/>", b + 1);
    }
    let _ = write!(out, "</{tag}>");
}

/// §18.3.1.82 sheetPr.
fn write_sheet_pr(out: &mut String, ws: &Worksheet) {
    let p = &ws.props;
    let outline =
        p.summary_below.is_some() || p.summary_right.is_some() || !p.outline_attrs.is_empty();
    let setup = p.fit_to_page.is_some() || !p.page_setup_attrs.is_empty();
    if !p.present && p.tab_color.is_none() && !outline && !setup && p.attrs.is_empty() {
        return;
    }
    out.push_str("<sheetPr");
    a_raw(out, &p.attrs);
    if p.tab_color.is_none() && !outline && !setup {
        out.push_str("/>");
        return;
    }
    out.push('>');
    if let Some(c) = &p.tab_color {
        write_color(out, "tabColor", c);
    }
    if outline {
        out.push_str("<outlinePr");
        a_optb(out, "summaryBelow", p.summary_below);
        a_optb(out, "summaryRight", p.summary_right);
        a_raw(out, &p.outline_attrs);
        out.push_str("/>");
    }
    if setup {
        out.push_str("<pageSetUpPr");
        a_optb(out, "fitToPage", p.fit_to_page);
        a_raw(out, &p.page_setup_attrs);
        out.push_str("/>");
    }
    out.push_str("</sheetPr>");
}

/// §18.3.1.88 sheetViews.
fn write_views(out: &mut String, ws: &Worksheet) {
    out.push_str("<sheetViews>");
    for v in &ws.views {
        out.push_str("<sheetView");
        a_true(out, "tabSelected", v.tab_selected);
        a_optb(out, "showGridLines", v.show_grid_lines);
        a_optb(out, "showRowColHeaders", v.show_headers);
        a_true(out, "rightToLeft", v.right_to_left);
        a_opt(out, "topLeftCell", v.top_left_cell.as_ref());
        a_opt(out, "zoomScale", v.zoom_scale.as_ref());
        a_raw(out, &v.attrs);
        a(out, "workbookViewId", &v.workbook_view_id.to_string());
        out.push('>');
        emit_unknown(out, &v.unknown, "");
        if let Some(p) = &v.pane {
            out.push_str("<pane");
            if p.x_split != 0.0 {
                a(out, "xSplit", &fmt_f64(p.x_split));
            }
            if p.y_split != 0.0 {
                a(out, "ySplit", &fmt_f64(p.y_split));
            }
            a_opt(out, "topLeftCell", p.top_left_cell.as_ref());
            a_opt(out, "activePane", p.active_pane.as_ref());
            match p.state {
                PaneState::Split => {}
                PaneState::Frozen => a(out, "state", "frozen"),
                PaneState::FrozenSplit => a(out, "state", "frozenSplit"),
            }
            out.push_str("/>");
        }
        emit_unknown(out, &v.unknown, "pane");
        for s in &v.selections {
            out.push_str("<selection");
            a_raw(out, s);
            out.push_str("/>");
        }
        emit_unknown(out, &v.unknown, "selection");
        emit_unknown(out, &v.unknown, "pivotSelection");
        emit_unknown(out, &v.unknown, "extLst");
        out.push_str("</sheetView>");
    }
    out.push_str("</sheetViews>");
}

/// §18.3.1.81 sheetFormatPr.
fn write_format(out: &mut String, ws: &Worksheet) {
    let f = &ws.format;
    if *f == bo_model::sheet::SheetFormat::default() {
        return;
    }
    out.push_str("<sheetFormatPr");
    a_opt(out, "baseColWidth", f.base_col_width.as_ref());
    a_optf(out, "defaultColWidth", f.default_col_width);
    // `defaultRowHeight` is required by the schema.
    a(
        out,
        "defaultRowHeight",
        &fmt_f64(f.default_row_height.unwrap_or(15.0)),
    );
    a_true(out, "customHeight", f.custom_height);
    a_true(out, "zeroHeight", f.zero_height);
    if f.outline_level_row > 0 {
        a(out, "outlineLevelRow", &f.outline_level_row.to_string());
    }
    if f.outline_level_col > 0 {
        a(out, "outlineLevelCol", &f.outline_level_col.to_string());
    }
    a_raw(out, &f.attrs);
    out.push_str("/>");
}

/// §18.3.1.80 sheetData.
fn write_sheet_data(out: &mut String, ws: &Worksheet, sst: &mut SharedStrings) {
    let rows: BTreeSet<u32> = ws
        .cells
        .keys()
        .map(|(r, _)| *r)
        .chain(ws.rows.keys().copied())
        .collect();
    if rows.is_empty() {
        out.push_str("<sheetData/>");
        return;
    }
    out.push_str("<sheetData>");
    for row in rows {
        out.push_str("<row");
        a(out, "r", &(row + 1).to_string());
        if let Some(info) = ws.rows.get(&row) {
            if let Some(s) = info.style {
                a(out, "s", &s.to_string());
                a(out, "customFormat", "1");
            }
            a_optf(out, "ht", info.height);
            a_true(out, "hidden", info.hidden);
            a_true(out, "customHeight", info.custom_height);
            if info.outline_level > 0 {
                a(out, "outlineLevel", &info.outline_level.to_string());
            }
            a_true(out, "collapsed", info.collapsed);
            a_true(out, "thickTop", info.thick_top);
            a_true(out, "thickBot", info.thick_bot);
            a_raw(out, &info.attrs);
        }
        let mut cells = ws.cells.range((row, 0)..=(row, u32::MAX)).peekable();
        if cells.peek().is_none() {
            out.push_str("/>");
            continue;
        }
        out.push('>');
        for (&(r, c), cell) in cells {
            write_cell(out, CellRef::new(r, c), cell, sst);
        }
        out.push_str("</row>");
    }
    out.push_str("</sheetData>");
}

/// §18.3.1.4 c.
fn write_cell(out: &mut String, at: CellRef, cell: &Cell, sst: &mut SharedStrings) {
    out.push_str("<c");
    a(out, "r", &at.to_string());
    if cell.style != 0 {
        a(out, "s", &cell.style.to_string());
    }
    let has_formula = cell.formula.is_some();
    // §18.18.11 ST_CellType
    let (t, v): (Option<&str>, Option<String>) = match &cell.value {
        CellValue::Empty => (None, None),
        CellValue::Number(n) if n.is_finite() => (None, Some(fmt_f64(*n))),
        CellValue::Number(_) => (Some("e"), Some("#NUM!".into())),
        CellValue::Text(_) | CellValue::RichText(_) if has_formula => {
            (Some("str"), Some(cell.value.as_text().unwrap_or_default()))
        }
        CellValue::Text(_) | CellValue::RichText(_) => {
            (Some("s"), Some(sst.add(&cell.value).to_string()))
        }
        CellValue::Bool(b) => (Some("b"), Some(if *b { "1" } else { "0" }.into())),
        CellValue::Error(e) => (Some("e"), Some(e.as_str().into())),
        CellValue::Date(d) => (Some("d"), Some(d.clone())),
    };
    if let Some(t) = t {
        a(out, "t", t);
    }
    a_opt(out, "cm", cell.cm.as_ref());
    a_opt(out, "vm", cell.vm.as_ref());
    a_true(out, "ph", cell.show_phonetic);
    if cell.formula.is_none() && v.is_none() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    if let Some(f) = &cell.formula {
        out.push_str("<f");
        match &f.kind {
            FormulaKind::Normal => {}
            FormulaKind::Shared { index, range } => {
                a(out, "t", "shared");
                a_opt(out, "ref", range.as_ref());
                a(out, "si", &index.to_string());
            }
            FormulaKind::Array { range } => {
                a(out, "t", "array");
                a(out, "ref", range);
            }
            FormulaKind::DataTable { range } => {
                a(out, "t", "dataTable");
                a(out, "ref", range);
            }
        }
        a_true(out, "ca", f.calc_always);
        a_raw(out, &f.attrs);
        if f.text.is_empty() {
            out.push_str("/>");
        } else {
            let _ = write!(out, ">{}</f>", escape(&f.text));
        }
    }
    if let Some(v) = v {
        let enc = if t == Some("str") {
            xstring_encode(&v)
        } else {
            escape(&v)
        };
        if t == Some("str") && needs_preserve(&v) {
            let _ = write!(out, "<v xml:space=\"preserve\">{enc}</v>");
        } else {
            let _ = write!(out, "<v>{enc}</v>");
        }
    }
    out.push_str("</c>");
}

/// §18.3.1.10 cfRule.
fn write_cf_rule(out: &mut String, r: &CfRule) {
    out.push_str("<cfRule");
    a(out, "type", &r.kind);
    a_opt(out, "dxfId", r.dxf_id.as_ref());
    a(out, "priority", &r.priority.to_string());
    a_true(out, "stopIfTrue", r.stop_if_true);
    a_opt(out, "operator", r.operator.as_ref());
    a_opt(out, "text", r.text.as_ref());
    a_raw(out, &r.attrs);
    out.push('>');
    emit_ordered(
        out,
        &["formula", "colorScale", "dataBar", "iconSet", "extLst"],
        &r.unknown,
        |slot, out| match slot {
            "formula" => {
                for f in &r.formulas {
                    let _ = write!(out, "<formula>{}</formula>", escape(f));
                }
            }
            "colorScale" => {
                if let Some(cs) = &r.color_scale {
                    out.push_str("<colorScale>");
                    for c in &cs.cfvos {
                        write_cfvo(out, c);
                    }
                    for c in &cs.colors {
                        write_color(out, "color", c);
                    }
                    out.push_str("</colorScale>");
                }
            }
            "dataBar" => {
                if let Some(db) = &r.data_bar {
                    out.push_str("<dataBar");
                    a_opt(out, "minLength", db.min_length.as_ref());
                    a_opt(out, "maxLength", db.max_length.as_ref());
                    a_optb(out, "showValue", db.show_value);
                    out.push('>');
                    for c in &db.cfvos {
                        write_cfvo(out, c);
                    }
                    if let Some(c) = &db.color {
                        write_color(out, "color", c);
                    }
                    out.push_str("</dataBar>");
                }
            }
            "iconSet" => {
                if let Some(is) = &r.icon_set {
                    out.push_str("<iconSet");
                    a_opt(out, "iconSet", is.icon_set.as_ref());
                    a_optb(out, "showValue", is.show_value);
                    a_optb(out, "percent", is.percent);
                    a_optb(out, "reverse", is.reverse);
                    out.push('>');
                    for c in &is.cfvos {
                        write_cfvo(out, c);
                    }
                    out.push_str("</iconSet>");
                }
            }
            _ => {}
        },
    );
    out.push_str("</cfRule>");
}

/// §18.3.1.11 cfvo.
fn write_cfvo(out: &mut String, c: &Cfvo) {
    out.push_str("<cfvo");
    a(out, "type", &c.kind);
    a_opt(out, "val", c.val.as_ref());
    a_optb(out, "gte", c.gte);
    out.push_str("/>");
}

/// §18.7.6 comments.
fn comments_xml(comments: &[Comment], known_authors: &[String]) -> String {
    let mut authors: Vec<&str> = known_authors.iter().map(String::as_str).collect();
    for c in comments {
        if !authors.contains(&c.author.as_str()) {
            authors.push(&c.author);
        }
    }
    let mut out = String::from(XML_DECL);
    let _ = write!(out, "<comments xmlns=\"{SML_NS}\"><authors>");
    for au in &authors {
        let _ = write!(out, "<author>{}</author>", escape(au));
    }
    out.push_str("</authors><commentList>");
    for c in comments {
        let id = authors.iter().position(|x| *x == c.author).unwrap_or(0);
        let _ = write!(out, "<comment ref=\"{}\" authorId=\"{id}\"><text>", c.cell);
        if c.text.len() == 1 && c.text[0].font.is_none() {
            write_t(&mut out, &c.text[0].text);
        } else {
            write_runs(&mut out, &c.text);
        }
        out.push_str("</text></comment>");
    }
    out.push_str("</commentList></comments>");
    out
}

/// §18.5.1.2 table.
fn table_xml(t: &Table) -> String {
    let mut out = String::from(XML_DECL);
    out.push_str("<table");
    root_attrs(&mut out, &t.attrs, false);
    a(&mut out, "id", &t.id.to_string());
    a(&mut out, "name", &t.name);
    a(&mut out, "displayName", &t.display_name);
    a(&mut out, "ref", &t.range.to_string());
    a_opt(&mut out, "headerRowCount", t.header_row_count.as_ref());
    a_opt(&mut out, "totalsRowCount", t.totals_row_count.as_ref());
    a_optb(&mut out, "totalsRowShown", t.totals_row_shown);
    out.push('>');
    emit_ordered(&mut out, TABLE_SCHEMA, &t.unknown, |slot, out| match slot {
        "autoFilter" => {
            if let Some(af) = &t.auto_filter {
                out.push_str(af);
            }
        }
        "tableColumns" => {
            let _ = write!(out, "<tableColumns count=\"{}\">", t.columns.len());
            for c in &t.columns {
                out.push_str("<tableColumn");
                a(out, "id", &c.id.to_string());
                a(out, "name", &c.name);
                a_opt(out, "totalsRowFunction", c.totals_row_function.as_ref());
                a_opt(out, "totalsRowLabel", c.totals_row_label.as_ref());
                a_raw(out, &c.attrs);
                if c.calculated_formula.is_none()
                    && c.totals_row_formula.is_none()
                    && c.unknown.is_empty()
                {
                    out.push_str("/>");
                    continue;
                }
                out.push('>');
                emit_ordered(
                    out,
                    &[
                        "calculatedColumnFormula",
                        "totalsRowFormula",
                        "xmlColumnPr",
                        "extLst",
                    ],
                    &c.unknown,
                    |slot, out| match slot {
                        "calculatedColumnFormula" => {
                            if let Some(f) = &c.calculated_formula {
                                let _ = write!(
                                    out,
                                    "<calculatedColumnFormula>{}</calculatedColumnFormula>",
                                    escape(f)
                                );
                            }
                        }
                        "totalsRowFormula" => {
                            if let Some(f) = &c.totals_row_formula {
                                let _ = write!(
                                    out,
                                    "<totalsRowFormula>{}</totalsRowFormula>",
                                    escape(f)
                                );
                            }
                        }
                        _ => {}
                    },
                );
                out.push_str("</tableColumn>");
            }
            out.push_str("</tableColumns>");
        }
        "tableStyleInfo" => {
            if let Some(s) = &t.style {
                out.push_str("<tableStyleInfo");
                a_opt(out, "name", s.name.as_ref());
                a(
                    out,
                    "showFirstColumn",
                    if s.show_first_column { "1" } else { "0" },
                );
                a(
                    out,
                    "showLastColumn",
                    if s.show_last_column { "1" } else { "0" },
                );
                a(
                    out,
                    "showRowStripes",
                    if s.show_row_stripes { "1" } else { "0" },
                );
                a(
                    out,
                    "showColumnStripes",
                    if s.show_column_stripes { "1" } else { "0" },
                );
                out.push_str("/>");
            }
        }
        _ => {}
    });
    out.push_str("</table>");
    out
}

#[cfg(test)]
mod tests {
    use super::relative_target;

    #[test]
    fn relative_targets() {
        assert_eq!(
            relative_target("/xl/worksheets/sheet1.xml", "/xl/drawings/drawing1.xml"),
            "../drawings/drawing1.xml"
        );
        assert_eq!(
            relative_target("/xl/workbook.xml", "/xl/worksheets/sheet1.xml"),
            "worksheets/sheet1.xml"
        );
        assert_eq!(
            relative_target("/", "/docProps/app.xml"),
            "docProps/app.xml"
        );
        assert_eq!(
            relative_target("/xl/workbook.xml", "/customXml/item1.xml"),
            "../customXml/item1.xml"
        );
    }
}
