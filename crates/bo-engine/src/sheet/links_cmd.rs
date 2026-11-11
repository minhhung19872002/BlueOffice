//! Links to other workbooks (ECMA-376 Part 1, §18.14 External Workbook References): the
//! list the Edit Links dialog shows, reading the values again from the linked file, pointing
//! a link at another file, and breaking a link.
//!
//! Formulas refer to link `n` (1-based) as `[n]Sheet!A1`; the values last read from the
//! linked workbook are kept in the file and used for calculation
//! ([`bo_model::sheet::ExternalLink::cells`]). The host supplies the linked file's bytes (a
//! browser cannot read a path), so updating and changing the source both take a file.
//!
//! * `externalLinks` → `{ links: [{ index, source, target, sheets, formulas }] }`;
//! * `externalLinkUpdate { index, base64, name, changeSource? }` → `{ missing: [sheet] }` —
//!   reads the cells the formulas use from the file (`changeSource`: the link now names it);
//! * `externalLinkBreak { index }` → `{ converted }` — formulas using the link become their
//!   values, the link is removed and the links after it are renumbered.
//!
//! Each change is one undo step and recalculates the workbook.

use super::cmd::Recalc;
use super::SheetEngine;
use crate::{EngineError, Result};
use base64::Engine as _;
use bo_formula::{format, parse, Expr, SheetRef};
use bo_model::sheet::{Cell, CellValue, FormulaKind, Workbook};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};

fn bad(msg: impl Into<String>) -> EngineError {
    EngineError::Sheet(msg.into())
}

/// Most cells read from a linked sheet in one update.
const MAX_LINK_CELLS: usize = 1_000_000;

/// External link commands.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum LinkCommand {
    /// The links of the workbook.
    ExternalLinks,
    /// Read the values of link `index` (0-based) again from a file.
    ExternalLinkUpdate {
        index: usize,
        /// The linked workbook, base64-encoded.
        base64: String,
        /// Its file name (format detection; the new source with `change_source`).
        name: String,
        /// The link now names this file.
        #[serde(default)]
        change_source: bool,
    },
    /// Replace the formulas using link `index` by their values and remove it.
    ExternalLinkBreak { index: usize },
}

impl LinkCommand {
    /// The command when `v` is one of these.
    pub fn parse(v: &Value) -> Option<Result<LinkCommand>> {
        if !matches!(
            v.get("cmd")?.as_str()?,
            "externalLinks" | "externalLinkUpdate" | "externalLinkBreak"
        ) {
            return None;
        }
        Some(serde_json::from_value(v.clone()).map_err(|e| bad(format!("bad command: {e}"))))
    }
}

/// `(link number, sheet)` of a sheet qualifier `[n]Sheet`.
fn link_of(name: &str) -> Option<(usize, &str)> {
    let rest = name.strip_prefix('[')?;
    let (n, sheet) = rest.split_once(']')?;
    Some((n.parse().ok()?, sheet))
}

fn sheet_links(s: &SheetRef) -> impl Iterator<Item = usize> + '_ {
    std::iter::once(s.first.as_str())
        .chain(s.last.as_deref())
        .filter_map(|n| link_of(n).map(|(k, _)| k))
}

/// Whether `expr` uses link `n` (1-based).
fn uses_link(expr: &Expr, n: usize) -> bool {
    let mut found = false;
    expr.walk(&mut |e| {
        let sheet = match e {
            Expr::Ref(r) => r.sheet.as_ref(),
            Expr::Name { sheet, .. } => sheet.as_ref(),
            _ => None,
        };
        found |= sheet.is_some_and(|s| sheet_links(s).any(|k| k == n));
    });
    found
}

/// `[m]` → `[m - 1]` for every link after the removed link `n`.
fn renumber(expr: &Expr, n: usize) -> Expr {
    let fix = |s: &str| -> String {
        match link_of(s) {
            Some((m, sheet)) if m > n => format!("[{}]{sheet}", m - 1),
            _ => s.to_owned(),
        }
    };
    let fix_ref = |s: &SheetRef| SheetRef {
        first: fix(&s.first),
        last: s.last.as_deref().map(fix),
    };
    expr.map(&mut |e| match e {
        Expr::Ref(mut r) => {
            r.sheet = r.sheet.as_ref().map(fix_ref);
            Expr::Ref(r)
        }
        Expr::Name { sheet, name } => Expr::Name {
            sheet: sheet.as_ref().map(fix_ref),
            name,
        },
        e => e,
    })
}

/// Formula text with the links after `n` renumbered, `None` when unchanged.
fn renumbered_text(text: &str, n: usize) -> Option<String> {
    let expr = parse(text).ok()?;
    let out = renumber(&expr, n);
    (out != expr).then(|| format(&out))
}

/// Display name of a link target: its last path segment, `%xx` escapes decoded.
fn source_name(target: &str) -> String {
    let last = target.rsplit(['/', '\\']).next().unwrap_or(target);
    let bytes = last.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&last[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| last.to_owned())
}

/// Every formula text of the workbook with the extent of its shared group (rows, cols)
/// beyond the master: cells, then defined names.
fn formulas(wb: &Workbook) -> Vec<(String, u32, u32)> {
    let mut out = Vec::new();
    for ws in &wb.sheets {
        for c in ws.cells.values() {
            let Some(f) = c.formula.as_ref().filter(|f| !f.text.is_empty()) else {
                continue;
            };
            let (h, w) = match &f.kind {
                FormulaKind::Shared { range: Some(r), .. } => bo_model::sheet::CellRange::parse(r)
                    .map_or((0, 0), |r| (r.rows() - 1, r.cols() - 1)),
                _ => (0, 0),
            };
            out.push((f.text.clone(), h, w));
        }
    }
    for dn in &wb.defined_names {
        out.push((dn.formula.clone(), 0, 0));
    }
    out
}

/// The areas `(r1, c1, r2, c2)` the formulas read from each sheet of link `n`, keyed by
/// the lower-case sheet name.
fn used_areas(wb: &Workbook, n: usize) -> BTreeMap<String, Vec<(u32, u32, u32, u32)>> {
    let mut out: BTreeMap<String, Vec<(u32, u32, u32, u32)>> = BTreeMap::new();
    for (text, h, w) in formulas(wb) {
        let Ok(expr) = parse(&text) else { continue };
        expr.walk(&mut |e| {
            let Expr::Ref(r) = e else { return };
            let Some(s) = &r.sheet else { return };
            let Some((k, sheet)) = link_of(&s.first) else {
                return;
            };
            if k != n {
                return;
            }
            let (r1, c1, r2, c2) = r.bounds();
            out.entry(sheet.to_lowercase()).or_default().push((
                r1,
                c1,
                r2.saturating_add(h),
                c2.saturating_add(w),
            ));
        });
    }
    out
}

/// Formula text of a constant (ECMA-376 Part 1, §18.17.2 constants and array constants).
fn constant_text(v: &bo_formula::Value) -> String {
    use bo_formula::Value as V;
    let one = |v: &V| -> String {
        match v {
            V::Number(n) => format!("{n}"),
            V::Text(t) => format!("\"{}\"", t.replace('"', "\"\"")),
            V::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_owned(),
            V::Error(e) => e.as_str().to_owned(),
            V::Empty => "0".to_owned(),
            _ => "#VALUE!".to_owned(),
        }
    };
    match v {
        V::Array(a) if a.rows() * a.cols() > 1 => {
            let rows: Vec<String> = (0..a.rows())
                .map(|r| {
                    (0..a.cols())
                        .map(|c| one(a.get(r, c)))
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .collect();
            format!("{{{}}}", rows.join(";"))
        }
        V::Array(a) => one(a.get(0, 0)),
        v => one(v),
    }
}

impl SheetEngine {
    /// Runs an external link command.
    pub(super) fn link_command(&mut self, cmd: LinkCommand) -> Result<Value> {
        match cmd {
            LinkCommand::ExternalLinks => {
                let wb = self.wb()?;
                let all = formulas(wb);
                let parsed: Vec<Expr> = all.iter().filter_map(|(t, _, _)| parse(t).ok()).collect();
                let links: Vec<Value> = wb
                    .external_links
                    .iter()
                    .enumerate()
                    .map(|(i, l)| {
                        let uses = parsed.iter().filter(|e| uses_link(e, i + 1)).count();
                        json!({
                            "index": i,
                            "source": source_name(&l.target),
                            "target": l.target,
                            "sheets": l.sheets,
                            "formulas": uses,
                            "workbook": !l.part.is_empty(),
                        })
                    })
                    .collect();
                Ok(json!({ "links": links }))
            }
            LinkCommand::ExternalLinkUpdate {
                index,
                base64,
                name,
                change_source,
            } => {
                self.writable()?;
                if self
                    .wb()?
                    .external_links
                    .get(index)
                    .is_none_or(|l| l.part.is_empty())
                {
                    return Err(bad("no such link"));
                }
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(base64.as_bytes())
                    .map_err(|_| bad("bad file data"))?;
                // The linked workbook, read the way a workbook is opened (any format).
                let mut src = SheetEngine::new();
                src.open(&bytes, &name)?;
                let src = src.workbook().ok_or(EngineError::NoDocument)?.clone();
                let areas = used_areas(self.wb()?, index + 1);
                self.edit(0, Recalc::All, |wb, _| {
                    let link = &mut wb.external_links[index];
                    let mut missing = Vec::new();
                    let mut total = 0usize;
                    for (si, sheet) in link.sheets.clone().iter().enumerate() {
                        let Some(ws) = src.sheet(sheet) else {
                            missing.push(sheet.clone());
                            continue;
                        };
                        let (last_row, last_col) = ws
                            .cells
                            .keys()
                            .fold((0, 0), |(r, c), &(a, b)| (r.max(a), c.max(b)));
                        let mut cells: BTreeMap<(u32, u32), CellValue> = BTreeMap::new();
                        for &(r1, c1, r2, c2) in areas
                            .get(&sheet.to_lowercase())
                            .map_or(&[][..], Vec::as_slice)
                        {
                            let (r2, c2) = (r2.min(last_row), c2.min(last_col));
                            if r1 > r2 || c1 > c2 {
                                continue;
                            }
                            for (&(r, c), cell) in ws.cells.range((r1, c1)..=(r2, c2)) {
                                if c < c1 || c > c2 {
                                    continue;
                                }
                                let v = match &cell.value {
                                    CellValue::RichText(_) => {
                                        CellValue::Text(cell.value.as_text().unwrap_or_default())
                                    }
                                    v => v.clone(),
                                };
                                if !matches!(v, CellValue::Empty) {
                                    cells.insert((r, c), v);
                                }
                            }
                        }
                        total += cells.len();
                        if total > MAX_LINK_CELLS {
                            return Err(bad("too many linked cells"));
                        }
                        if let Some(slot) = link.cells.get_mut(si) {
                            *slot = cells.into_iter().map(|((r, c), v)| (r, c, v)).collect();
                        }
                    }
                    if change_source {
                        link.target.clone_from(&name);
                        link.names = src
                            .defined_names
                            .iter()
                            .filter(|d| d.local_sheet.is_none())
                            .map(|d| (d.name.clone(), format!("={}", d.formula)))
                            .collect();
                    }
                    bo_xlsx::extlink::store(wb, index);
                    Ok(json!({ "missing": missing }))
                })
            }
            LinkCommand::ExternalLinkBreak { index } => {
                self.writable()?;
                if index >= self.wb()?.external_links.len() {
                    return Err(bad("no such link"));
                }
                let n = index + 1;
                // Names using the link keep the value last read, as a constant.
                let constants: Vec<(usize, String)> = {
                    let wb = self.wb()?;
                    let engine = self.live.engine();
                    wb.defined_names
                        .iter()
                        .enumerate()
                        .filter(|(_, d)| parse(&d.formula).is_ok_and(|e| uses_link(&e, n)))
                        .map(|(i, d)| {
                            let v = engine
                                .and_then(|fw| fw.evaluate(0, 0, 0, &d.formula).ok())
                                .unwrap_or(bo_formula::Value::Error(bo_formula::ErrorKind::Ref));
                            (i, constant_text(&v))
                        })
                        .collect()
                };
                self.edit(0, Recalc::All, |wb, _| {
                    let mut converted = 0usize;
                    for ws in &mut wb.sheets {
                        // Shared groups whose master uses the link: every member becomes a value.
                        let mut groups: HashSet<u32> = HashSet::new();
                        for c in ws.cells.values() {
                            if let Some(f) = &c.formula {
                                if let (FormulaKind::Shared { index: g, .. }, false) =
                                    (&f.kind, f.text.is_empty())
                                {
                                    if parse(&f.text).is_ok_and(|e| uses_link(&e, n)) {
                                        groups.insert(*g);
                                    }
                                }
                            }
                        }
                        for cell in ws.cells.values_mut() {
                            let Some(f) = cell.formula.as_mut() else {
                                continue;
                            };
                            let to_value = match (&f.kind, f.text.is_empty()) {
                                (FormulaKind::Shared { index: g, .. }, true) => groups.contains(g),
                                _ => parse(&f.text).is_ok_and(|e| uses_link(&e, n)),
                            };
                            if to_value {
                                *cell = Cell {
                                    formula: None,
                                    cm: None,
                                    ..cell.clone()
                                };
                                converted += 1;
                            } else if let Some(t) = renumbered_text(&f.text, n) {
                                f.text = t;
                            }
                        }
                    }
                    for (i, dn) in wb.defined_names.iter_mut().enumerate() {
                        if let Some((_, c)) = constants.iter().find(|(k, _)| *k == i) {
                            dn.formula.clone_from(c);
                        } else if let Some(t) = renumbered_text(&dn.formula, n) {
                            dn.formula = t;
                        }
                    }
                    bo_xlsx::extlink::remove(wb, index);
                    Ok(json!({ "converted": converted }))
                })
            }
        }
    }
}
