//! What-if scenarios of a worksheet (ECMA-376 Part 1, §18.3.1 `scenarios` — CT_Scenarios,
//! `scenario` — CT_Scenario, `inputCells` — CT_InputCells).
//!
//! The schema allows no other attributes or children, so the element is always modelled;
//! anything unexpected (extension attributes) is dropped.

use crate::xml::{
    a, a_opt, a_true, attrs, for_children, lname, take, take_bool, take_u32, xstring_decode,
    xstring_encode,
};
use crate::Result;
use bo_model::sheet::{sqref_to_string, CellRange, CellRef, Scenario, ScenarioCell, Scenarios};
use bo_opc::xml::XmlReader;
use quick_xml::events::BytesStart;
use std::fmt::Write as _;

/// Read a `scenarios` element whose start tag `e` was just consumed.
pub(crate) fn read_scenarios(
    r: &mut XmlReader<'_>,
    e: &BytesStart<'_>,
    empty: bool,
) -> Result<Scenarios> {
    let mut at = attrs(e);
    let mut out = Scenarios {
        current: take_u32(&mut at, "current"),
        show: take_u32(&mut at, "show"),
        sqref: CellRange::parse_sqref(&take(&mut at, "sqref").unwrap_or_default()),
        items: Vec::new(),
    };
    if empty {
        return Ok(out);
    }
    for_children(r, |r, e, empty| {
        if lname(e) != "scenario" {
            return Ok(());
        }
        let mut at = attrs(e);
        let mut sc = Scenario {
            name: take(&mut at, "name")
                .map(|s| xstring_decode(&s))
                .unwrap_or_default(),
            locked: take_bool(&mut at, "locked").unwrap_or(false),
            hidden: take_bool(&mut at, "hidden").unwrap_or(false),
            user: take(&mut at, "user").map(|s| xstring_decode(&s)),
            comment: take(&mut at, "comment").map(|s| xstring_decode(&s)),
            cells: Vec::new(),
        };
        if !empty {
            for_children(r, |_, c, _| {
                if lname(c) != "inputCells" {
                    return Ok(());
                }
                let mut ca = attrs(c);
                let Some(cell) =
                    take(&mut ca, "r").and_then(|s| CellRef::parse(&s.replace('$', "")))
                else {
                    // `r` is required; a cell without one cannot be applied.
                    return Ok(());
                };
                sc.cells.push(ScenarioCell {
                    cell,
                    value: take(&mut ca, "val")
                        .map(|s| xstring_decode(&s))
                        .unwrap_or_default(),
                    deleted: take_bool(&mut ca, "deleted").unwrap_or(false),
                    undone: take_bool(&mut ca, "undone").unwrap_or(false),
                    num_fmt_id: take_u32(&mut ca, "numFmtId"),
                });
                Ok(())
            })?;
        }
        out.items.push(sc);
        Ok(())
    })?;
    Ok(out)
}

/// ` name="…"` for an `ST_Xstring` attribute: `_xHHHH_` escapes, XML escaping, and line
/// breaks / tabs as character references so attribute normalization keeps them.
fn a_x(out: &mut String, name: &str, value: &str) {
    let v = xstring_encode(value)
        .replace('\n', "&#10;")
        .replace('\t', "&#9;");
    let _ = write!(out, " {name}=\"{v}\"");
}

/// Write `scenarios`; nothing when there is no scenario (the schema needs at least one,
/// and each scenario at least one `inputCells`).
pub(crate) fn write_scenarios(out: &mut String, s: &Scenarios) {
    let items: Vec<&Scenario> = s.items.iter().filter(|x| !x.cells.is_empty()).collect();
    if items.is_empty() {
        return;
    }
    let n = u32::try_from(items.len()).unwrap_or(u32::MAX);
    out.push_str("<scenarios");
    a_opt(out, "current", s.current.filter(|c| *c < n).as_ref());
    a_opt(out, "show", s.show.filter(|c| *c < n).as_ref());
    if !s.sqref.is_empty() {
        a(out, "sqref", &sqref_to_string(&s.sqref));
    }
    out.push('>');
    for sc in items {
        out.push_str("<scenario");
        a_x(out, "name", &sc.name);
        a_true(out, "locked", sc.locked);
        a_true(out, "hidden", sc.hidden);
        let _ = write!(out, " count=\"{}\"", sc.cells.len());
        if let Some(u) = &sc.user {
            a_x(out, "user", u);
        }
        if let Some(c) = &sc.comment {
            a_x(out, "comment", c);
        }
        out.push('>');
        for c in &sc.cells {
            out.push_str("<inputCells");
            a(out, "r", &c.cell.to_string());
            a_true(out, "deleted", c.deleted);
            a_true(out, "undone", c.undone);
            a_x(out, "val", &c.value);
            a_opt(out, "numFmtId", c.num_fmt_id.as_ref());
            out.push_str("/>");
        }
        out.push_str("</scenario>");
    }
    out.push_str("</scenarios>");
}
