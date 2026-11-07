//! [`Presentation`] → `.pptx` (ECMA-376 Part 1, §13.3 / §19).
//!
//! Modelled parts are regenerated from the model (opaque XML re-inserted at its
//! recorded position); relationships of those parts are kept, with the
//! structural ones (layout, master, theme, notes) re-targeted. Every other part
//! is copied byte-for-byte.

use crate::pml::{write_clr_map_ovr, write_csld};
use crate::read::{LAYOUT, MASTER, NOTES, PRES, SLD, TX_STYLES};
use crate::{rel_is, Result, REL_NS};
use bo_drawingml::dom::{self, El, Kids};
use bo_drawingml::text::write_list_style;
use bo_drawingml::theme::write_theme;
use bo_model::slide::{Attrs, Presentation, Rel, SlideSize};
use bo_opc::{ContentTypes, Package, Relationship, TargetMode};
use std::collections::HashSet;

const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const NS_P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const CT_PML: &str = "application/vnd.openxmlformats-officedocument.presentationml.";

/// Relative reference from part `from` to part `to` (both absolute).
pub(crate) fn relative(from: &str, to: &str) -> String {
    let from_dir: Vec<&str> = from
        .trim_start_matches('/')
        .split('/')
        .collect::<Vec<_>>()
        .split_last()
        .map(|(_, d)| d.to_vec())
        .unwrap_or_default();
    let to_parts: Vec<&str> = to.trim_start_matches('/').split('/').collect();
    let common = from_dir
        .iter()
        .zip(&to_parts)
        .take_while(|(a, b)| a.eq_ignore_ascii_case(b))
        .count();
    let mut out: Vec<&str> = vec![".."; from_dir.len() - common];
    out.extend_from_slice(&to_parts[common..]);
    out.join("/")
}

fn root_attrs(attrs: &Attrs) -> Attrs {
    if attrs.is_empty() {
        vec![
            ("xmlns:a".into(), NS_A.into()),
            ("xmlns:r".into(), NS_R.into()),
            ("xmlns:p".into(), NS_P.into()),
        ]
    } else {
        attrs.clone()
    }
}

/// Allocates unique part names and relationship ids.
struct Names {
    used: HashSet<String>,
}

impl Names {
    fn claim(&mut self, current: &str, pattern: &str) -> String {
        if !current.is_empty() && self.used.insert(current.to_ascii_lowercase()) {
            return current.to_owned();
        }
        for n in 1.. {
            let name = pattern.replace('#', &n.to_string());
            if self.used.insert(name.to_ascii_lowercase()) {
                return name;
            }
        }
        unreachable!("unbounded counter")
    }
}

fn next_rel_id(rels: &[Rel], used: &HashSet<String>) -> String {
    for n in 1.. {
        let id = format!("rId{n}");
        if !used.contains(&id) && rels.iter().all(|r| r.id != id) {
            return id;
        }
    }
    unreachable!("unbounded counter")
}

/// Point the (first) relationship of type `kind` at `target`; create it if missing.
fn set_rel(rels: &mut Vec<Rel>, kind: &str, target: String) -> String {
    if let Some(r) = rels.iter_mut().find(|r| rel_is(&r.rel_type, kind)) {
        r.target = target;
        r.external = false;
        return r.id.clone();
    }
    let id = next_rel_id(rels, &HashSet::new());
    rels.push(Rel {
        id: id.clone(),
        rel_type: format!("{REL_NS}{kind}"),
        target,
        external: false,
    });
    id
}

/// Structural relationship placement that keeps the original relationship
/// order: a relationship of type `kind` with id `want` is re-targeted in
/// place; otherwise a new one is appended. Returns the id used.
fn place_rel(
    rels: &mut Vec<Rel>,
    placed: &mut HashSet<String>,
    want: &str,
    kind: &str,
    target: String,
) -> String {
    if !want.is_empty() && !placed.contains(want) {
        if let Some(r) = rels
            .iter_mut()
            .find(|r| r.id == want && rel_is(&r.rel_type, kind))
        {
            r.target = target;
            r.external = false;
            placed.insert(want.to_owned());
            return want.to_owned();
        }
    }
    let id = if !want.is_empty() && !placed.contains(want) && rels.iter().all(|r| r.id != want) {
        want.to_owned()
    } else {
        next_rel_id(rels, placed)
    };
    placed.insert(id.clone());
    rels.push(Rel {
        id: id.clone(),
        rel_type: format!("{REL_NS}{kind}"),
        target,
        external: false,
    });
    id
}

/// Drop relationships of the given kinds that were not placed.
fn drop_unplaced(rels: &mut Vec<Rel>, placed: &HashSet<String>, kinds: &[&str]) {
    rels.retain(|r| placed.contains(&r.id) || !kinds.iter().any(|k| rel_is(&r.rel_type, k)));
}

fn put(pkg: &mut Package, name: &str, root: &bo_model::slide::XmlElement, rels: &[Rel]) {
    pkg.put_part(name, dom::to_part_xml(root));
    let r: Vec<Relationship> = rels
        .iter()
        .map(|r| Relationship {
            id: r.id.clone(),
            rel_type: r.rel_type.clone(),
            target: r.target.clone(),
            mode: if r.external {
                TargetMode::External
            } else {
                TargetMode::Internal
            },
        })
        .collect();
    if !r.is_empty() {
        pkg.set_relationships(name, &r);
    }
}

fn size_el(name: &str, s: &SlideSize) -> bo_model::slide::XmlElement {
    El::new(name)
        .a("cx", s.cx)
        .a("cy", s.cy)
        .opt("type", s.kind.as_ref())
        .done()
}

/// Write a presentation as `.pptx` bytes.
pub fn write_pptx(pres: &Presentation) -> Result<Vec<u8>> {
    let mut pkg = Package::new();
    let mut names = Names {
        used: pres
            .parts
            .iter()
            .map(|p| p.name.to_ascii_lowercase())
            .collect(),
    };
    for p in &pres.parts {
        pkg.put_part(&p.name, p.data.clone());
    }
    let mut ct = pkg.content_types().unwrap_or_else(|_| ContentTypes {
        defaults: vec![],
        overrides: vec![],
    });
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
    let mut overrides: Vec<(String, String)> = Vec::new();
    // Parts added after reading (e.g. an inserted picture) carry their own
    // content type; declare it when the package does not (or not with that
    // type) yet (ECMA-376 Part 2, §10.1.2.2 Override). Parts read from a file always
    // match the existing declarations, so this changes nothing for them.
    for p in &pres.parts {
        if let Some(t) = &p.content_type {
            if ct.get(&p.name) != Some(t.as_str()) {
                overrides.push((p.name.clone(), t.clone()));
            }
        }
    }

    // Part names.
    let pres_name = names.claim(&pres.info.part_name, "/ppt/presentation.xml");
    let theme_names: Vec<String> = pres
        .themes
        .iter()
        .map(|t| names.claim(&t.info.part_name, "/ppt/theme/theme#.xml"))
        .collect();
    let master_names: Vec<String> = pres
        .masters
        .iter()
        .map(|m| names.claim(&m.info.part_name, "/ppt/slideMasters/slideMaster#.xml"))
        .collect();
    let layout_names: Vec<String> = pres
        .layouts
        .iter()
        .map(|l| names.claim(&l.info.part_name, "/ppt/slideLayouts/slideLayout#.xml"))
        .collect();
    let slide_names: Vec<String> = pres
        .slides
        .iter()
        .map(|s| names.claim(&s.info.part_name, "/ppt/slides/slide#.xml"))
        .collect();
    let notes_names: Vec<Option<String>> = pres
        .slides
        .iter()
        .map(|s| {
            s.notes
                .as_ref()
                .map(|n| names.claim(&n.info.part_name, "/ppt/notesSlides/notesSlide#.xml"))
        })
        .collect();

    // Package relationships for a presentation built from scratch.
    if pkg.part(&bo_opc::rels_part_name("/")).is_none() {
        pkg.set_relationships(
            "/",
            &[Relationship {
                id: "rId1".into(),
                rel_type: bo_opc::REL_OFFICE_DOCUMENT.into(),
                target: pres_name.trim_start_matches('/').into(),
                mode: TargetMode::Internal,
            }],
        );
    }

    // Themes.
    for (t, name) in pres.themes.iter().zip(&theme_names) {
        pkg.put_part(name, write_theme(t));
        if !t.info.rels.is_empty() {
            put_rels(&mut pkg, name, &t.info.rels);
        }
        overrides.push((
            name.clone(),
            "application/vnd.openxmlformats-officedocument.theme+xml".into(),
        ));
    }

    // Masters.
    for (mi, (m, name)) in pres.masters.iter().zip(&master_names).enumerate() {
        let mut rels: Vec<Rel> = m.info.rels.clone();
        if let Some(tn) = theme_names.get(m.theme) {
            set_rel(&mut rels, "theme", relative(name, tn));
        }
        let mut lst = El::new("p:sldLayoutIdLst");
        let mut placed: HashSet<String> = HashSet::new();
        let mut next_id =
            2_147_483_649_u32.max(m.layouts.iter().map(|l| l.id).max().unwrap_or(0) + 1);
        // Layouts owned by this master: the recorded list, plus layouts pointing here.
        let mut layout_refs = m.layouts.clone();
        for (li, l) in pres.layouts.iter().enumerate() {
            if l.master == mi && !layout_refs.iter().any(|r| r.layout == li) {
                layout_refs.push(bo_model::slide::LayoutRef {
                    layout: li,
                    id: 0,
                    rel_id: String::new(),
                });
            }
        }
        for lr in &layout_refs {
            let Some(ln) = layout_names.get(lr.layout) else {
                continue;
            };
            let rid = place_rel(
                &mut rels,
                &mut placed,
                &lr.rel_id,
                "slideLayout",
                relative(name, ln),
            );
            let id = if lr.id == 0 {
                next_id += 1;
                next_id - 1
            } else {
                lr.id
            };
            lst = lst.child(El::new("p:sldLayoutId").a("id", id).a("r:id", rid).done());
        }
        drop_unplaced(&mut rels, &placed, &["slideLayout"]);
        let mut tk = Kids::new(TX_STYLES);
        let ts = &m.text_styles;
        tk.push_opt(
            ts.title
                .as_ref()
                .map(|s| write_list_style(s, "p:titleStyle")),
        );
        tk.push_opt(ts.body.as_ref().map(|s| write_list_style(s, "p:bodyStyle")));
        tk.push_opt(
            ts.other
                .as_ref()
                .map(|s| write_list_style(s, "p:otherStyle")),
        );
        tk.extras(&ts.extra);
        let mut k = Kids::new(MASTER);
        k.push(write_csld(&m.data));
        k.push(El::new("p:clrMap").rest(&m.clr_map).done());
        k.push(lst.done());
        k.push(tk.build(El::new("p:txStyles")));
        k.extras(&m.extra);
        let root = k.build(El::new("p:sldMaster").rest(&root_attrs(&m.info.root_attrs)));
        put(&mut pkg, name, &root, &rels);
        overrides.push((name.clone(), format!("{CT_PML}slideMaster+xml")));
    }

    // Layouts.
    for (l, name) in pres.layouts.iter().zip(&layout_names) {
        let mut rels = l.info.rels.clone();
        if let Some(mn) = master_names.get(l.master) {
            set_rel(&mut rels, "slideMaster", relative(name, mn));
        }
        let mut k = Kids::new(LAYOUT);
        k.push(write_csld(&l.data));
        k.push_opt(l.clr_map_ovr.as_ref().map(write_clr_map_ovr));
        k.extras(&l.extra);
        let root = k.build(
            El::new("p:sldLayout")
                .rest(&root_attrs(&l.info.root_attrs))
                .opt("type", l.kind.as_ref())
                .optb("showMasterSp", l.show_master_sp),
        );
        put(&mut pkg, name, &root, &rels);
        overrides.push((name.clone(), format!("{CT_PML}slideLayout+xml")));
    }

    // Slides and notes.
    for ((s, name), notes_name) in pres.slides.iter().zip(&slide_names).zip(&notes_names) {
        let mut rels = s.info.rels.clone();
        if let Some(ln) = layout_names.get(s.layout) {
            set_rel(&mut rels, "slideLayout", relative(name, ln));
        }
        match (&s.notes, notes_name) {
            (Some(n), Some(nn)) => {
                set_rel(&mut rels, "notesSlide", relative(name, nn));
                let mut nrels = n.info.rels.clone();
                set_rel(&mut nrels, "slide", relative(nn, name));
                let mut k = Kids::new(NOTES);
                k.push(write_csld(&n.data));
                k.push_opt(n.clr_map_ovr.as_ref().map(write_clr_map_ovr));
                k.extras(&n.extra);
                let root = k.build(El::new("p:notes").rest(&root_attrs(&n.info.root_attrs)));
                put(&mut pkg, nn, &root, &nrels);
                overrides.push((nn.clone(), format!("{CT_PML}notesSlide+xml")));
            }
            _ => rels.retain(|r| !rel_is(&r.rel_type, "notesSlide")),
        }
        let mut k = Kids::new(SLD);
        k.push(write_csld(&s.data));
        k.push_opt(s.clr_map_ovr.as_ref().map(write_clr_map_ovr));
        if let Some(t) = &s.transition {
            k.extras(&[crate::anim::write_transition(t)]);
        }
        if let Some(x) = s.timing.as_ref().and_then(crate::anim::write_timing) {
            k.extras(&[x]);
        }
        k.extras(&s.extra);
        let root = k.build(
            El::new("p:sld")
                .rest(&root_attrs(&s.info.root_attrs))
                .optb("show", s.show),
        );
        put(&mut pkg, name, &root, &rels);
        overrides.push((name.clone(), format!("{CT_PML}slide+xml")));
    }

    // presentation.xml
    let mut rels: Vec<Rel> = pres.info.rels.clone();
    let mut placed: HashSet<String> = HashSet::new();
    let mut claim = |rels: &mut Vec<Rel>, want: &str, kind: &str, target: String| -> String {
        place_rel(rels, &mut placed, want, kind, target)
    };
    let mut mlst = El::new("p:sldMasterIdLst");
    let mut next_master_id = 2_147_483_648_u32;
    for (m, mn) in pres.masters.iter().zip(&master_names) {
        let rid = claim(
            &mut rels,
            &m.pres_rel_id,
            "slideMaster",
            relative(&pres_name, mn),
        );
        let id = if m.master_id == 0 {
            next_master_id
        } else {
            m.master_id
        };
        next_master_id = next_master_id.max(id) + 1;
        mlst = mlst.child(El::new("p:sldMasterId").a("id", id).a("r:id", rid).done());
    }
    let mut slst = El::new("p:sldIdLst");
    let mut next_slide_id =
        256_u32.max(pres.slides.iter().map(|s| s.slide_id).max().unwrap_or(0) + 1);
    for (s, sn) in pres.slides.iter().zip(&slide_names) {
        let rid = claim(&mut rels, &s.pres_rel_id, "slide", relative(&pres_name, sn));
        let id = if s.slide_id == 0 {
            next_slide_id += 1;
            next_slide_id - 1
        } else {
            s.slide_id
        };
        slst = slst.child(El::new("p:sldId").a("id", id).a("r:id", rid).done());
    }
    drop_unplaced(&mut rels, &placed, &["slide", "slideMaster"]);
    let mut k = Kids::new(PRES);
    k.push(mlst.done());
    if !pres.slides.is_empty() {
        k.push(slst.done());
    }
    let mut slide_size = pres.slide_size.clone();
    if slide_size.cx == 0 || slide_size.cy == 0 {
        // §19.2.1.39: default 16:9 (12 192 000 × 6 858 000 EMU) for new decks.
        slide_size = SlideSize {
            cx: 12_192_000,
            cy: 6_858_000,
            kind: None,
        };
    }
    k.push(size_el("p:sldSz", &slide_size));
    let notes_size = if pres.notes_size.cx == 0 {
        SlideSize {
            cx: 6_858_000,
            cy: 9_144_000,
            kind: None,
        }
    } else {
        pres.notes_size.clone()
    };
    k.push(size_el("p:notesSz", &notes_size));
    k.push_opt(
        pres.default_text_style
            .as_ref()
            .map(|s| write_list_style(s, "p:defaultTextStyle")),
    );
    k.extras(&pres.extra);
    let root = k.build(El::new("p:presentation").rest(&root_attrs(&pres.info.root_attrs)));
    put(&mut pkg, &pres_name, &root, &rels);
    if ct.get(&pres_name).is_none() {
        overrides.push((pres_name.clone(), format!("{CT_PML}presentation.main+xml")));
    }

    // Content types: keep existing overrides (e.g. macro-enabled main type),
    // add missing ones, drop overrides of parts that no longer exist.
    for (n, t) in overrides {
        if ct.get(&n) != Some(t.as_str()) && !(n == pres_name && ct.get(&n).is_some()) {
            ct.set_override(&n, &t);
        }
    }
    let present: HashSet<String> = pkg
        .parts()
        .iter()
        .map(|p| p.name.to_ascii_lowercase())
        .collect();
    ct.overrides
        .retain(|(n, _)| present.contains(&n.to_ascii_lowercase()));
    pkg.set_content_types(&ct);
    Ok(pkg.write()?)
}

fn put_rels(pkg: &mut Package, name: &str, rels: &[Rel]) {
    let r: Vec<Relationship> = rels
        .iter()
        .map(|r| Relationship {
            id: r.id.clone(),
            rel_type: r.rel_type.clone(),
            target: r.target.clone(),
            mode: if r.external {
                TargetMode::External
            } else {
                TargetMode::Internal
            },
        })
        .collect();
    pkg.set_relationships(name, &r);
}

#[cfg(test)]
mod tests {
    use super::relative;

    #[test]
    fn relative_paths() {
        assert_eq!(
            relative(
                "/ppt/slides/slide1.xml",
                "/ppt/slideLayouts/slideLayout2.xml"
            ),
            "../slideLayouts/slideLayout2.xml"
        );
        assert_eq!(
            relative("/ppt/presentation.xml", "/ppt/slides/slide1.xml"),
            "slides/slide1.xml"
        );
        assert_eq!(
            relative("/ppt/slides/slide1.xml", "/ppt/slides/slide2.xml"),
            "slide2.xml"
        );
    }
}
