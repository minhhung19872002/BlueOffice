//! Sections of a presentation (SLIDE-VIEW-001).
//!
//! Sections are a PowerPoint 2010 extension of `presentation.xml`
//! ([MS-PPTX] `p14:sectionLst`, CT_SectionList, carried in the presentation's
//! `p:extLst` / `p:ext` — ECMA-376 Part 1, §19.2.1.12 extLst — under the
//! extension URI below): an ordered list of named sections (CT_Section
//! `p14:section`, attributes `name` and `id`), each listing the `p:sldId@id`
//! of its slides (`p14:sldIdLst` / `p14:sldId`).
//!
//! The section list is kept as the raw extension element: it is only rewritten
//! when a section command or a slide-list change alters it, so untouched decks
//! round-trip byte for byte. After every command that changes the slide list,
//! [`SlideEditor::sections_sync`] gives new slides an id and puts every slide in
//! the section it was dropped into (see [`normalize`]), in the same undo step.

use super::ops::{PresFields, SlideOp, Tx};
use super::{refuse, EditError, SlideEditor, SlideSel, R};
use bo_model::slide::{Extra, Presentation, XmlElement, XmlNode};
use serde::Serialize;

/// [MS-PPTX]: `p:ext@uri` of the section list extension.
pub const EXT_URI: &str = "{521415D9-36F7-43E2-AB2F-B90AF26B5E84}";
/// [MS-PPTX]: PowerPoint 2010 namespace (`p14`).
pub const P14_NS: &str = "http://schemas.microsoft.com/office/powerpoint/2010/main";

/// One section ([MS-PPTX] CT_Section).
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    /// `name`.
    pub name: String,
    /// `id` (a GUID in braces).
    pub id: String,
    /// `p14:sldIdLst/p14:sldId@id` in order.
    pub slides: Vec<u32>,
    /// The element as read (unknown attributes and children such as `extLst` are kept).
    el: Option<XmlElement>,
}

/// A section for the slide pane and the slide sorter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionOut {
    /// Section name.
    pub name: String,
    /// Section id (stable across edits: the UI keys its collapsed state on it).
    pub id: String,
    /// Index of the section's first slide (where it would start when empty).
    pub first: usize,
    /// Number of slides in the section.
    pub count: usize,
}

fn element_children(el: &XmlElement) -> impl Iterator<Item = (usize, &XmlElement)> {
    el.children.iter().enumerate().filter_map(|(i, c)| match c {
        XmlNode::Element(e) => Some((i, e)),
        XmlNode::Text(_) => None,
    })
}

/// (index in `extra` of `p:extLst`, child index of the section `p:ext`).
fn locate(extra: &[Extra]) -> Option<(usize, usize)> {
    for (i, x) in extra.iter().enumerate() {
        if x.xml.local() != "extLst" {
            continue;
        }
        for (j, e) in element_children(&x.xml) {
            if e.local() == "ext" && e.attr("uri") == Some(EXT_URI) {
                return Some((i, j));
            }
        }
    }
    None
}

fn section_list(extra: &[Extra]) -> Option<&XmlElement> {
    let (i, j) = locate(extra)?;
    match extra.get(i)?.xml.children.get(j)? {
        XmlNode::Element(ext) => ext.elements().find(|e| e.local() == "sectionLst"),
        XmlNode::Text(_) => None,
    }
}

/// Sections of a deck (empty when it has none).
pub fn read(extra: &[Extra]) -> Vec<Section> {
    let Some(list) = section_list(extra) else {
        return Vec::new();
    };
    list.elements()
        .filter(|e| e.local() == "section")
        .map(|e| Section {
            name: e.attr("name").unwrap_or_default().to_owned(),
            id: e.attr("id").unwrap_or_default().to_owned(),
            slides: e
                .elements()
                .filter(|c| c.local() == "sldIdLst")
                .flat_map(XmlElement::elements)
                .filter(|c| c.local() == "sldId")
                .filter_map(|c| c.attr("id").and_then(|v| v.parse().ok()))
                .collect(),
            el: Some(e.clone()),
        })
        .collect()
}

/// Namespace prefix of a qualified name (`p14:section` → `p14`).
fn prefix(name: &str) -> Option<&str> {
    name.split_once(':').map(|(p, _)| p)
}

fn qualified(prefix: Option<&str>, local: &str) -> String {
    match prefix {
        Some(p) => format!("{p}:{local}"),
        None => local.to_owned(),
    }
}

fn set_attr(el: &mut XmlElement, name: &str, value: &str) {
    if let Some(a) = el.attrs.iter_mut().find(|(k, _)| k == name) {
        a.1 = value.to_owned();
    } else {
        el.attrs.push((name.to_owned(), value.to_owned()));
    }
}

/// `p14:section` for `s` (its original element updated, or a new one).
fn section_element(s: &Section, pfx: Option<&str>) -> XmlElement {
    let mut el = s.el.clone().unwrap_or_else(|| {
        let mut e = XmlElement::new(&qualified(pfx, "section"));
        e.attrs.push(("name".into(), String::new()));
        e.attrs.push(("id".into(), String::new()));
        e
    });
    set_attr(&mut el, "name", &s.name);
    set_attr(&mut el, "id", &s.id);
    let p = prefix(&el.name).map(str::to_owned);
    let mut ids = XmlElement::new(&qualified(p.as_deref(), "sldIdLst"));
    for id in &s.slides {
        let mut x = XmlElement::new(&qualified(p.as_deref(), "sldId"));
        x.attrs.push(("id".into(), id.to_string()));
        ids.children.push(XmlNode::Element(x));
    }
    // CT_Section: sldIdLst then extLst.
    let at = el
        .children
        .iter()
        .position(|c| matches!(c, XmlNode::Element(e) if e.local() == "sldIdLst"));
    match at {
        Some(i) => el.children[i] = XmlNode::Element(ids),
        None => el.children.insert(0, XmlNode::Element(ids)),
    }
    el
}

/// `extra` with the section list replaced by `sections` (removed when empty).
pub fn write(extra: &[Extra], sections: &[Section]) -> Vec<Extra> {
    let mut out = extra.to_vec();
    let found = locate(&out);
    if sections.is_empty() {
        if let Some((i, j)) = found {
            out[i].xml.children.remove(j);
            if out[i].xml.elements().next().is_none() {
                out.remove(i);
            }
        }
        return out;
    }
    let old = section_list(&out).cloned();
    let pfx = old
        .as_ref()
        .and_then(|l| prefix(&l.name).map(str::to_owned))
        .unwrap_or_else(|| "p14".to_owned());
    let mut list = old.unwrap_or_else(|| {
        let mut l = XmlElement::new(&format!("{pfx}:sectionLst"));
        l.attrs.push((format!("xmlns:{pfx}"), P14_NS.to_owned()));
        l
    });
    list.children = sections
        .iter()
        .map(|s| XmlNode::Element(section_element(s, Some(&pfx))))
        .collect();
    if let Some((i, j)) = found {
        if let XmlNode::Element(ext) = &mut out[i].xml.children[j] {
            let at = ext
                .children
                .iter()
                .position(|c| matches!(c, XmlNode::Element(e) if e.local() == "sectionLst"));
            match at {
                Some(k) => ext.children[k] = XmlNode::Element(list),
                None => ext.children.push(XmlNode::Element(list)),
            }
        }
    } else {
        // §19.2.1.12 extLst / §19.2.1.11 ext.
        let mut ext = XmlElement::new("p:ext");
        ext.attrs.push(("uri".into(), EXT_URI.into()));
        ext.children.push(XmlNode::Element(list));
        if let Some(x) = out.iter_mut().find(|x| x.xml.local() == "extLst") {
            // [MS-PPTX]: the section list comes first among PowerPoint's extensions.
            x.xml.children.insert(0, XmlNode::Element(ext));
        } else {
            let mut lst = XmlElement::new("p:extLst");
            lst.children.push(XmlNode::Element(ext));
            out.push(bo_pptx::presentation_extra(lst));
        }
    }
    out
}

/// Section of each slide given possibly stale section lists.
///
/// The longest run of slides (in deck order) whose listed sections never go
/// backwards keeps its sections; every other slide — new, or moved across a
/// section boundary — joins the section of the slide before it (or, at the
/// start, of the first kept slide after it). Among equally long runs the one
/// leaving out `moved` (the slide just moved or inserted) wins, so a moved
/// slide takes the section it was dropped into. Returns the slide ids per section.
pub fn normalize(sections: &[Section], ids: &[u32], moved: Option<usize>) -> Vec<Vec<u32>> {
    let n = ids.len();
    let listed: Vec<Option<usize>> = ids
        .iter()
        .map(|id| {
            if *id == 0 {
                None
            } else {
                sections.iter().position(|s| s.slides.contains(id))
            }
        })
        .collect();
    // Weighted longest non-decreasing subsequence (O(n²), decks are small).
    let weight = |i: usize| -> u64 {
        let w = 2 * n as u64 + 2;
        if Some(i) == moved {
            w - 1
        } else {
            w
        }
    };
    let mut best = vec![0_u64; n];
    let mut prev = vec![usize::MAX; n];
    for i in 0..n {
        let Some(si) = listed[i] else { continue };
        best[i] = weight(i);
        for j in 0..i {
            if let Some(sj) = listed[j] {
                if sj <= si && best[j] + weight(i) > best[i] {
                    best[i] = best[j] + weight(i);
                    prev[i] = j;
                }
            }
        }
    }
    let mut kept = vec![false; n];
    let end = (0..n).filter(|&i| listed[i].is_some()).max_by(|&a, &b| {
        best[a]
            .cmp(&best[b])
            // Ties: the earlier end.
            .then(b.cmp(&a))
    });
    let mut cur = end;
    while let Some(i) = cur {
        kept[i] = true;
        cur = (prev[i] != usize::MAX).then_some(prev[i]);
    }
    let mut of = vec![0_usize; n];
    let mut last: Option<usize> = None;
    for i in 0..n {
        let s = if kept[i] {
            listed[i]
        } else {
            last.or_else(|| (i..n).find(|&k| kept[k]).and_then(|k| listed[k]))
        };
        let s = s.unwrap_or(0);
        of[i] = s;
        last = Some(s);
    }
    let mut out = vec![Vec::new(); sections.len().max(1)];
    for (i, id) in ids.iter().enumerate() {
        out[of[i]].push(*id);
    }
    out.truncate(sections.len());
    out
}

/// Sections for the UI (normalized against the current slide order).
pub fn sections_out(pres: &Presentation) -> Vec<SectionOut> {
    let sections = read(&pres.extra);
    if sections.is_empty() {
        return Vec::new();
    }
    let ids: Vec<u32> = pres.slides.iter().map(|s| s.slide_id).collect();
    let groups = normalize(&sections, &ids, None);
    let mut first = 0;
    sections
        .iter()
        .zip(groups)
        .map(|(s, g)| {
            let o = SectionOut {
                name: s.name.clone(),
                id: s.id.clone(),
                first,
                count: g.len(),
            };
            first += g.len();
            o
        })
        .collect()
}

/// A new section id: a GUID in braces (the form of [MS-PPTX] CT_Section `id`),
/// derived from `seed` and unique among `taken`.
pub(super) fn new_id(taken: &[&str], seed: &str) -> String {
    // FNV-1a of the seed, then SplitMix64.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in seed.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h ^= taken.len() as u64;
    let mut next = || {
        h = h.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = h;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    loop {
        let (a, b) = (next(), next());
        let id = format!(
            "{{{:08X}-{:04X}-4{:03X}-{:04X}-{:012X}}}",
            a >> 32,
            (a >> 16) & 0xFFFF,
            a & 0x0FFF,
            0x8000 | (b >> 48) & 0x3FFF,
            b & 0xFFFF_FFFF_FFFF
        );
        if !taken.iter().any(|t| t.eq_ignore_ascii_case(&id)) {
            return id;
        }
    }
}

/// Give every slide a unique `p:sldId@id` (§19.2.1.34: 256 ≤ id < 2 147 483 648)
/// so the section list can name it; new slides have none until saved.
pub(super) fn assign_ids(tx: &mut Tx<'_>) -> R<()> {
    let mut next = tx
        .pres
        .slides
        .iter()
        .map(|s| s.slide_id)
        .max()
        .unwrap_or(0)
        .saturating_add(1)
        .max(256);
    let mut seen = std::collections::BTreeSet::new();
    for i in 0..tx.pres.slides.len() {
        let id = tx.pres.slides[i].slide_id;
        if id != 0 && seen.insert(id) {
            continue;
        }
        let old = tx.pres.slides[i].clone();
        let mut new = old.clone();
        new.slide_id = next;
        seen.insert(next);
        next += 1;
        tx.op(SlideOp::Slides {
            index: i,
            remove: vec![old],
            insert: vec![new],
        })?;
    }
    Ok(())
}

/// Replace the section list (`p:extLst`) through a presentation op.
fn store(tx: &mut Tx<'_>, sections: &[Section]) -> R<()> {
    let before = PresFields::of(tx.pres);
    let extra = write(&before.extra, sections);
    if extra == before.extra {
        return Ok(());
    }
    let mut after = before.clone();
    after.extra = extra;
    tx.op(SlideOp::Pres {
        before: Box::new(before),
        after: Box::new(after),
    })?;
    Ok(())
}

/// Current sections with memberships normalized against the slide order.
fn current(pres: &Presentation, moved: Option<usize>) -> Vec<Section> {
    let mut sections = read(&pres.extra);
    let ids: Vec<u32> = pres.slides.iter().map(|s| s.slide_id).collect();
    let groups = normalize(&sections, &ids, moved);
    for (s, g) in sections.iter_mut().zip(groups) {
        s.slides = g;
    }
    sections
}

/// Index of the section holding slide `slide` (normalized sections).
fn section_of(sections: &[Section], pres: &Presentation, slide: usize) -> Option<usize> {
    let id = pres.slides.get(slide)?.slide_id;
    sections.iter().position(|s| s.slides.contains(&id))
}

/// Start a new section at `slide` (default: the current slide). Without
/// sections, the slides before it go into a first section named `first_name`.
pub(crate) fn cmd_add(
    tx: &mut Tx<'_>,
    sel: &mut SlideSel,
    name: &str,
    slide: Option<usize>,
    first_name: &str,
) -> R<()> {
    assign_ids(tx)?;
    let n = tx.pres.slides.len();
    let at = slide.unwrap_or(sel.slide).min(n);
    let mut sections = current(tx.pres, None);
    let ids: Vec<u32> = tx.pres.slides.iter().map(|s| s.slide_id).collect();
    let taken: Vec<String> = sections.iter().map(|s| s.id.clone()).collect();
    let taken_ref: Vec<&str> = taken.iter().map(String::as_str).collect();
    let id = new_id(&taken_ref, &format!("{name}\u{0}{at}"));
    if sections.is_empty() {
        if at > 0 {
            let first = new_id(&[id.as_str()], &format!("{first_name}\u{1}0"));
            sections.push(Section {
                name: first_name.to_owned(),
                id: first,
                slides: ids[..at].to_vec(),
                el: None,
            });
        }
        sections.push(Section {
            name: name.to_owned(),
            id,
            slides: ids[at..].to_vec(),
            el: None,
        });
    } else if at >= n {
        sections.push(Section {
            name: name.to_owned(),
            id,
            slides: Vec::new(),
            el: None,
        });
    } else {
        let k = section_of(&sections, tx.pres, at).unwrap_or(0);
        let start = ids[at];
        let pos = sections[k]
            .slides
            .iter()
            .position(|x| *x == start)
            .unwrap_or(0);
        let tail = sections[k].slides.split_off(pos);
        sections.insert(
            k + 1,
            Section {
                name: name.to_owned(),
                id,
                slides: tail,
                el: None,
            },
        );
    }
    store(tx, &sections)?;
    if at < n {
        *sel = SlideSel {
            slide: at,
            ids: vec![],
        };
    }
    Ok(())
}

/// Rename section `index`.
pub(crate) fn cmd_rename(tx: &mut Tx<'_>, index: usize, name: &str) -> R<()> {
    let mut sections = current(tx.pres, None);
    let s = sections
        .get_mut(index)
        .ok_or(EditError::Refused("noSection"))?;
    s.name = name.to_owned();
    store(tx, &sections)
}

/// Remove section `index`; its slides join the previous section (the next one
/// for the first section) or, with `slides`, are deleted too. Removing the
/// last section removes the section list.
pub(crate) fn cmd_remove(tx: &mut Tx<'_>, sel: &mut SlideSel, index: usize, slides: bool) -> R<()> {
    let mut sections = current(tx.pres, None);
    if index >= sections.len() {
        return refuse("noSection");
    }
    let gone = sections.remove(index);
    if slides {
        for i in (0..tx.pres.slides.len()).rev() {
            if gone.slides.contains(&tx.pres.slides[i].slide_id) {
                let s = tx.pres.slides[i].clone();
                tx.op(SlideOp::Slides {
                    index: i,
                    remove: vec![s],
                    insert: vec![],
                })?;
            }
        }
        let n = tx.pres.slides.len();
        *sel = SlideSel {
            slide: sel.slide.min(n.saturating_sub(1)),
            ids: vec![],
        };
    } else if !sections.is_empty() {
        let k = index.saturating_sub(1);
        if index == 0 {
            let mut v = gone.slides;
            v.append(&mut sections[0].slides);
            sections[0].slides = v;
        } else {
            sections[k].slides.extend(gone.slides);
        }
    }
    store(tx, &sections)
}

/// Remove every section (the slides stay).
pub(crate) fn cmd_remove_all(tx: &mut Tx<'_>) -> R<()> {
    store(tx, &[])
}

/// Move section `index` with its slides to position `to`.
pub(crate) fn cmd_move(tx: &mut Tx<'_>, sel: &mut SlideSel, index: usize, to: usize) -> R<()> {
    let mut sections = current(tx.pres, None);
    if index >= sections.len() || to >= sections.len() {
        return refuse("noSection");
    }
    if index == to {
        return Ok(());
    }
    let s = sections.remove(index);
    sections.insert(to, s);
    let order: Vec<u32> = sections.iter().flat_map(|s| s.slides.clone()).collect();
    let old = tx.pres.slides.clone();
    let mut new = Vec::with_capacity(old.len());
    for id in &order {
        if let Some(s) = old.iter().find(|s| s.slide_id == *id) {
            new.push(s.clone());
        }
    }
    if new.len() == old.len() && new != old {
        tx.op(SlideOp::Slides {
            index: 0,
            remove: old,
            insert: new,
        })?;
    }
    store(tx, &sections)?;
    let first: usize = sections[..to].iter().map(|s| s.slides.len()).sum();
    if first < tx.pres.slides.len() {
        *sel = SlideSel {
            slide: first,
            ids: vec![],
        };
    }
    Ok(())
}

impl SlideEditor {
    /// After a command that changed something: give new slides an id and keep
    /// the section list in step with the slide order (same undo step).
    pub(super) fn sections_sync(
        &mut self,
        pres: &mut Presentation,
        mark: (usize, usize, usize),
        history: bool,
    ) {
        if history || self.history_mark() == mark || self.in_any_master_view() {
            return;
        }
        if section_list(&pres.extra).is_none() {
            return;
        }
        let moved = Some(self.sel.slide);
        let mut tx = Tx::new(pres);
        let ok = assign_ids(&mut tx).is_ok() && {
            let sections = current(tx.pres, moved);
            store(&mut tx, &sections).is_ok()
        };
        if !ok {
            tx.rollback();
            return;
        }
        let ops = std::mem::take(&mut tx.ops);
        if let Some(last) = self.undo.last_mut() {
            last.ops.extend(ops);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sec(name: &str, ids: &[u32]) -> Section {
        Section {
            name: name.into(),
            id: format!("{{{name}}}"),
            slides: ids.to_vec(),
            el: None,
        }
    }

    #[test]
    fn moved_slide_joins_the_section_it_is_dropped_into() {
        let s = [sec("A", &[1, 2]), sec("B", &[3, 4])];
        // Slide 4 dragged to the front: it lands at the top of A.
        assert_eq!(
            normalize(&s, &[4, 1, 2, 3], Some(0)),
            vec![vec![4, 1, 2], vec![3]]
        );
        // Slide 1 dragged to the end: it lands at the end of B.
        assert_eq!(
            normalize(&s, &[2, 3, 4, 1], Some(3)),
            vec![vec![2], vec![3, 4, 1]]
        );
        // Order 1,3,2,4 — slide 2 moved down (selected at 2) goes to B …
        assert_eq!(
            normalize(&s, &[1, 3, 2, 4], Some(2)),
            vec![vec![1], vec![3, 2, 4]]
        );
        // … slide 3 moved up (selected at 1) goes to A.
        assert_eq!(
            normalize(&s, &[1, 3, 2, 4], Some(1)),
            vec![vec![1, 3, 2], vec![4]]
        );
    }

    #[test]
    fn new_and_deleted_slides() {
        let s = [sec("A", &[1, 2]), sec("B", &[3]), sec("C", &[])];
        // A new slide (no id) after 2 joins A; deleted slide 3 leaves B empty.
        assert_eq!(
            normalize(&s, &[1, 2, 0], Some(2)),
            vec![vec![1, 2, 0], vec![], vec![]]
        );
        // A new slide at the very top joins the first kept slide's section.
        assert_eq!(
            normalize(&s, &[0, 3], Some(0)),
            vec![vec![], vec![0, 3], vec![]]
        );
    }

    #[test]
    fn writes_and_reads_the_extension() {
        let s = vec![sec("Mở đầu", &[256, 257]), sec("Kết", &[258])];
        let extra = write(&[], &s);
        assert_eq!(extra.len(), 1);
        assert_eq!(extra[0].xml.name, "p:extLst");
        let back = read(&extra);
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].name, "Mở đầu");
        assert_eq!(back[0].slides, vec![256, 257]);
        assert_eq!(back[1].slides, vec![258]);
        // Removing every section drops the empty extLst.
        assert_eq!(write(&extra, &[]), Vec::<Extra>::new());
    }

    #[test]
    fn ids_are_braced_guids() {
        let id = new_id(&[], "x");
        assert_eq!(id.len(), 38);
        assert!(id.starts_with('{') && id.ends_with('}'));
        assert_eq!(&id[15..16], "4");
        assert_ne!(new_id(&[id.as_str()], "x"), id);
    }
}
