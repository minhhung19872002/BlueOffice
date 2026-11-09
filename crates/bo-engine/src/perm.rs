//! Editable regions of a protected BlueDoc document.
//!
//! * `w:permStart` / `w:permEnd` — ECMA-376 Part 1, §17.13.7.2 / §17.13.7.1:
//!   a range that `w:edGrp` (§17.18.18 `ST_EdGrp`: `everyone`, `current`,
//!   `editors`, …) or the single user `w:ed` may edit while the document is
//!   protected. `w:colFirst` / `w:colLast` (table columns) are kept but the
//!   range is treated as running text.
//! * `w:documentProtection` with `w:edit="readOnly"` (§17.15.1.29,
//!   §17.18.37 `ST_DocProtect`): only those ranges may change.
//!
//! The markers live in paragraphs as zero-width markup (`RunContent::Markup`).
//! Enforcement compares a fingerprint of everything *outside* the regions the
//! current user may edit before and after each edit ([`skeleton`]).

use crate::sheet::protect::{element_name, get, parse_attrs, Attrs};
use bo_edit::para::{atom_len, flatten};
use bo_edit::Pos;
use bo_model::index::{body_paragraphs, para_at};
use bo_model::{Block, Document, RunContent};
use serde::Serialize;
use std::hash::{Hash, Hasher};

/// One editable region.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    /// `w:id` shared by the start and end markers.
    pub id: String,
    /// `w:edGrp` group allowed to edit, when given.
    pub group: Option<String>,
    /// `w:ed` single user allowed to edit, when given.
    pub editor: Option<String>,
    /// The current user may edit it.
    pub mine: bool,
    /// Start (the position of `w:permStart`).
    pub start: Pos,
    /// End (the position of `w:permEnd`; the document end when it is missing).
    pub end: Pos,
}

/// `Some((is_start, attributes))` for a `w:permStart` / `w:permEnd` marker.
pub(crate) fn perm_mark(xml: &str) -> Option<(bool, Attrs)> {
    match element_name(xml) {
        "permStart" => Some((true, parse_attrs(xml))),
        "permEnd" => Some((false, parse_attrs(xml))),
        _ => None,
    }
}

/// Whether the current user (`identity`: display name, e-mail, …) may edit a
/// range with these attributes. Groups other than `everyone` / `current` need
/// a directory BlueOffice does not have, so they do not match.
pub fn is_mine(attrs: &Attrs, identity: &[String]) -> bool {
    if let Some(g) = get(attrs, "edGrp") {
        if g.eq_ignore_ascii_case("everyone") || g.eq_ignore_ascii_case("current") {
            return true;
        }
    }
    let Some(ed) = get(attrs, "ed").map(str::trim).filter(|e| !e.is_empty()) else {
        return false;
    };
    // `DOMAIN\user` also matches `user`.
    let short = ed.rsplit('\\').next().unwrap_or(ed);
    identity.iter().any(|i| {
        let i = i.trim();
        !i.is_empty() && (i.eq_ignore_ascii_case(ed) || i.eq_ignore_ascii_case(short))
    })
}

/// Editable regions of the body in document order.
pub fn regions(doc: &Document, identity: &[String]) -> Vec<Region> {
    let paths = body_paragraphs(doc);
    let mut open: Vec<(String, Attrs, Pos)> = Vec::new();
    let mut out = Vec::new();
    let mut last = Pos::default();
    for (pi, path) in paths.iter().enumerate() {
        let Some(p) = para_at(doc, path) else {
            continue;
        };
        let mut off = 0;
        for (_, c) in flatten(p) {
            if let RunContent::Markup(raw) = &c {
                if let Some((start, a)) = perm_mark(&raw.xml) {
                    let id = get(&a, "id").unwrap_or_default().to_owned();
                    if start {
                        open.push((id, a, Pos::new(pi, off)));
                    } else if let Some(k) = open.iter().position(|o| o.0 == id) {
                        let (id, a, s) = open.remove(k);
                        out.push(region(id, &a, s, Pos::new(pi, off), identity));
                    }
                }
            }
            off += atom_len(&c);
        }
        last = Pos::new(pi, off);
    }
    for (id, a, s) in open {
        out.push(region(id, &a, s, last, identity));
    }
    out.sort_by_key(|r| (r.start, r.end));
    out
}

fn region(id: String, a: &Attrs, start: Pos, end: Pos, identity: &[String]) -> Region {
    Region {
        id,
        group: get(a, "edGrp").map(str::to_owned),
        editor: get(a, "ed").map(str::to_owned),
        mine: is_mine(a, identity),
        start,
        end,
    }
}

/// The body has permission markers (any user).
pub fn has_regions(doc: &Document) -> bool {
    body_paragraphs(doc).iter().any(|path| {
        para_at(doc, path).is_some_and(|p| {
            p.runs.iter().any(|r| {
                r.content
                    .iter()
                    .any(|c| matches!(c, RunContent::Markup(raw) if perm_mark(&raw.xml).is_some()))
            })
        })
    })
}

struct Fnv(u64);

impl Hasher for Fnv {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 ^= u64::from(*b);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
}

/// Fingerprint of the protected content: every paragraph, run and table cell
/// outside the regions the current user may edit, plus the section settings.
/// An edit that leaves it unchanged only touched editable regions.
pub fn skeleton(doc: &Document, identity: &[String]) -> u64 {
    let mut h = Fnv(0xcbf2_9ce4_8422_2325);
    for sec in &doc.sections {
        // Page setup, columns, header/footer references, section break.
        format!(
            "{:?}{:?}{:?}{:?}{}{:?}",
            sec.page, sec.columns, sec.headers, sec.footers, sec.title_page, sec.break_type
        )
        .hash(&mut h);
    }
    let mut open: Vec<String> = Vec::new();
    for path in body_paragraphs(doc) {
        let Some(p) = para_at(doc, &path) else {
            continue;
        };
        let atoms = flatten(p);
        // Markers at the very start of the paragraph open their region first, so
        // a paragraph that starts an editable region may be reformatted.
        let mut lead = 0;
        for (_, c) in &atoms {
            if atom_len(c) > 0 {
                break;
            }
            if let RunContent::Markup(raw) = c {
                if let Some((true, a)) = perm_mark(&raw.xml) {
                    if is_mine(&a, identity) {
                        open.push(get(&a, "id").unwrap_or_default().to_owned());
                    }
                }
            }
            lead += 1;
        }
        if open.is_empty() {
            (b'P', path.len()).hash(&mut h);
            p.props.hash(&mut h);
        }
        for (k, (props, c)) in atoms.iter().enumerate() {
            if let RunContent::Markup(raw) = c {
                if let Some((start, a)) = perm_mark(&raw.xml) {
                    let id = get(&a, "id").unwrap_or_default().to_owned();
                    // An end marker carries only the id: it closes an open region.
                    let mine = if start {
                        is_mine(&a, identity)
                    } else {
                        open.contains(&id)
                    };
                    if mine {
                        // The markers themselves are protected content.
                        (b'M', start, &id).hash(&mut h);
                        if start {
                            if k >= lead {
                                open.push(id);
                            }
                        } else if let Some(i) = open.iter().position(|o| *o == id) {
                            open.remove(i);
                        }
                        continue;
                    }
                }
            }
            if open.is_empty() {
                props.hash(&mut h);
                c.hash(&mut h);
            }
        }
        if open.is_empty() {
            b'\n'.hash(&mut h);
        }
    }
    // Tables outside regions keep their shape (rows and cells per row).
    for sec in &doc.sections {
        for b in &sec.blocks {
            if let Block::Table(t) = b {
                t.rows.len().hash(&mut h);
                for r in &t.rows {
                    r.cells.len().hash(&mut h);
                }
            }
        }
    }
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_and_users() {
        let a = parse_attrs(r#"<w:permStart w:id="1" w:edGrp="everyone"/>"#);
        assert!(is_mine(&a, &[]));
        let a = parse_attrs(r#"<w:permStart w:id="2" w:ed="CONTOSO\an.nguyen"/>"#);
        assert!(!is_mine(&a, &[]));
        assert!(is_mine(&a, &["An.Nguyen".into()]));
        let a = parse_attrs(r#"<w:permStart w:id="3" w:ed="an@bluestar.vn"/>"#);
        assert!(is_mine(&a, &["Nguyễn An".into(), "an@bluestar.vn".into()]));
        let a = parse_attrs(r#"<w:permStart w:id="4" w:edGrp="editors"/>"#);
        assert!(!is_mine(&a, &["x".into()]));
    }
}
