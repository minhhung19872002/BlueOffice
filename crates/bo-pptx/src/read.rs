//! `.pptx` → [`Presentation`] (ECMA-376 Part 1, §13.3.6 Presentation part and
//! its related slide, layout, master, theme and notes parts).

use crate::anim;
use crate::pml::{parse_clr_map_ovr, parse_csld};
use crate::{rel_is, PptxError, Result};
use bo_drawingml::dom::{self, attr_int, walk, AttrBag};
use bo_drawingml::text::parse_list_style;
use bo_drawingml::theme::parse_theme;
use bo_model::slide::{
    LayoutRef, Notes, PartInfo, Presentation, RawPart, Rel, Slide, SlideLayout, SlideMaster,
    SlideSize, TextStyles, XmlElement,
};
use bo_opc::{resolve_target, Limits, Package, TargetMode, REL_OFFICE_DOCUMENT};
use std::collections::{HashMap, HashSet};

/// Child sequence of p:presentation (CT_Presentation, §19.2.1.26).
pub(crate) const PRES: &[&str] = &[
    "p:sldMasterIdLst",
    "p:notesMasterIdLst",
    "p:handoutMasterIdLst",
    "p:sldIdLst",
    "p:sldSz",
    "p:notesSz",
    "p:smartTags",
    "p:embeddedFontLst",
    "p:custShowLst",
    "p:photoAlbum",
    "p:custDataLst",
    "p:kinsoku",
    "p:defaultTextStyle",
    "p:modifyVerifier",
    "p:extLst",
];
/// CT_Slide (§19.3.1.38).
pub(crate) const SLD: &[&str] = &[
    "p:cSld",
    "p:clrMapOvr",
    "p:transition",
    "p:timing",
    "p:extLst",
];
/// CT_SlideLayout (§19.3.1.39).
pub(crate) const LAYOUT: &[&str] = &[
    "p:cSld",
    "p:clrMapOvr",
    "p:transition",
    "p:timing",
    "p:hf",
    "p:extLst",
];
/// CT_SlideMaster (§19.3.1.42).
pub(crate) const MASTER: &[&str] = &[
    "p:cSld",
    "p:clrMap",
    "p:sldLayoutIdLst",
    "p:transition",
    "p:timing",
    "p:hf",
    "p:txStyles",
    "p:extLst",
];
/// CT_NotesSlide (§19.3.1.26).
pub(crate) const NOTES: &[&str] = &["p:cSld", "p:clrMapOvr", "p:extLst"];
/// CT_SlideMasterTextStyles (§19.3.1.52).
pub(crate) const TX_STYLES: &[&str] = &["p:titleStyle", "p:bodyStyle", "p:otherStyle", "p:extLst"];

struct Ctx<'a> {
    pkg: &'a Package,
    consumed: HashSet<String>,
}

impl Ctx<'_> {
    fn load(&mut self, name: &str) -> Result<(XmlElement, PartInfo)> {
        let part = self
            .pkg
            .part(name)
            .ok_or_else(|| PptxError::MissingPart(name.to_owned()))?;
        let root = dom::parse(&part.data, name)?;
        let rels = self
            .pkg
            .relationships(name)?
            .into_iter()
            .map(|r| Rel {
                id: r.id,
                rel_type: r.rel_type,
                target: r.target,
                external: r.mode == TargetMode::External,
            })
            .collect();
        self.consume(name);
        let info = PartInfo {
            part_name: part.name.clone(),
            rels,
            root_attrs: Vec::new(),
        };
        Ok((root, info))
    }

    fn consume(&mut self, name: &str) {
        self.consumed.insert(name.to_ascii_lowercase());
        self.consumed
            .insert(bo_opc::rels_part_name(name).to_ascii_lowercase());
    }
}

fn target_of(info: &PartInfo, pred: impl Fn(&Rel) -> bool) -> Option<String> {
    info.rels
        .iter()
        .find(|r| !r.external && pred(r))
        .map(|r| resolve_target(&info.part_name, &r.target))
}

fn target_by_id(info: &PartInfo, id: &str) -> Option<String> {
    target_of(info, |r| r.id == id)
}

fn size(el: &XmlElement) -> SlideSize {
    SlideSize {
        cx: attr_int(el, "cx").unwrap_or(0),
        cy: attr_int(el, "cy").unwrap_or(0),
        kind: el.attr("type").map(str::to_owned),
    }
}

/// Read a `.pptx` package.
pub fn read_pptx(bytes: &[u8]) -> Result<Presentation> {
    let pkg = Package::read(bytes, &Limits::default())?;
    let main = pkg
        .related_part("/", REL_OFFICE_DOCUMENT)?
        .ok_or(PptxError::NoPresentation)?;
    let mut cx = Ctx {
        pkg: &pkg,
        consumed: HashSet::new(),
    };
    let (root, info) = cx.load(&main)?;
    let mut pres = Presentation {
        info,
        ..Presentation::default()
    };
    pres.info.root_attrs = root.attrs.clone();

    let mut master_ids: Vec<(u32, String)> = Vec::new();
    let mut slide_ids: Vec<(u32, String)> = Vec::new();
    pres.extra = walk(&root, PRES, |c| {
        match c.name.as_str() {
            // §19.2.1.37 sldMasterIdLst
            "p:sldMasterIdLst" => {
                for m in c.elements() {
                    master_ids.push((
                        attr_int(m, "id").unwrap_or(0),
                        m.attr("r:id").unwrap_or_default().to_owned(),
                    ));
                }
            }
            // §19.2.1.34 sldIdLst
            "p:sldIdLst" => {
                for s in c.elements() {
                    slide_ids.push((
                        attr_int(s, "id").unwrap_or(0),
                        s.attr("r:id").unwrap_or_default().to_owned(),
                    ));
                }
            }
            // §19.2.1.39 sldSz
            "p:sldSz"
                if c.attrs
                    .iter()
                    .all(|(k, _)| matches!(k.as_str(), "cx" | "cy" | "type")) =>
            {
                pres.slide_size = size(c);
            }
            // §19.2.1.23 notesSz
            "p:notesSz" if c.attrs.len() == 2 => pres.notes_size = size(c),
            // §19.2.1.8 defaultTextStyle
            "p:defaultTextStyle" => pres.default_text_style = Some(parse_list_style(c)),
            _ => return false,
        }
        true
    });

    let mut layout_index: HashMap<String, usize> = HashMap::new();
    let mut theme_index: HashMap<String, usize> = HashMap::new();

    for (id, rid) in &master_ids {
        let Some(name) = target_by_id(&pres.info, rid) else {
            continue;
        };
        let (root, info) = cx.load(&name)?;
        let mut m = SlideMaster {
            master_id: *id,
            pres_rel_id: rid.clone(),
            ..SlideMaster::default()
        };
        m.info = info;
        m.info.root_attrs = root.attrs.clone();
        // Theme (§13.3.8 / §14.2.7)
        if let Some(tn) = target_of(&m.info, |r| rel_is(&r.rel_type, "theme")) {
            let idx = if let Some(i) = theme_index.get(&tn.to_ascii_lowercase()) {
                *i
            } else {
                let data = &pkg
                    .part(&tn)
                    .ok_or_else(|| PptxError::MissingPart(tn.clone()))?
                    .data;
                let mut theme = parse_theme(data, &tn)?;
                let (_, tinfo) = cx.load(&tn)?;
                theme.info.rels = tinfo.rels;
                pres.themes.push(theme);
                theme_index.insert(tn.to_ascii_lowercase(), pres.themes.len() - 1);
                pres.themes.len() - 1
            };
            m.theme = idx;
        }
        let mut layout_refs: Vec<(u32, String)> = Vec::new();
        m.extra = walk(&root, MASTER, |c| {
            match c.name.as_str() {
                "p:cSld" => m.data = parse_csld(c),
                // §19.3.1.6 clrMap
                "p:clrMap" => m.clr_map = c.attrs.clone(),
                // §19.3.1.41 sldLayoutIdLst
                "p:sldLayoutIdLst" => {
                    for l in c.elements() {
                        layout_refs.push((
                            attr_int(l, "id").unwrap_or(0),
                            l.attr("r:id").unwrap_or_default().to_owned(),
                        ));
                    }
                }
                // §19.3.1.52 txStyles
                "p:txStyles" => {
                    let mut ts = TextStyles::default();
                    ts.extra = walk(c, TX_STYLES, |s| {
                        let ls = Some(parse_list_style(s));
                        match s.name.as_str() {
                            "p:titleStyle" => ts.title = ls,
                            "p:bodyStyle" => ts.body = ls,
                            "p:otherStyle" => ts.other = ls,
                            _ => return false,
                        }
                        true
                    });
                    m.text_styles = ts;
                }
                _ => return false,
            }
            true
        });
        let master_index = pres.masters.len();
        for (lid, lrid) in layout_refs {
            let Some(ln) = target_by_id(&m.info, &lrid) else {
                continue;
            };
            let idx = load_layout(&mut cx, &mut pres, &mut layout_index, &ln, master_index)?;
            m.layouts.push(LayoutRef {
                layout: idx,
                id: lid,
                rel_id: lrid,
            });
        }
        pres.masters.push(m);
    }

    for (id, rid) in &slide_ids {
        let Some(name) = target_by_id(&pres.info, rid) else {
            continue;
        };
        let (root, info) = cx.load(&name)?;
        let mut s = Slide {
            slide_id: *id,
            pres_rel_id: rid.clone(),
            info,
            ..Slide::default()
        };
        let mut bag = AttrBag::of(&root);
        s.show = bag.bool("show");
        s.info.root_attrs = bag.rest();
        s.extra = walk(&root, SLD, |c| {
            match c.name.as_str() {
                "p:cSld" => s.data = parse_csld(c),
                "p:clrMapOvr" => match parse_clr_map_ovr(c) {
                    Some(m) => s.clr_map_ovr = Some(m),
                    None => return false,
                },
                _ => return false,
            }
            true
        });
        // §19.3.1.50 transition (possibly inside mc:AlternateContent), §19.3.1.48 timing.
        if let Some(i) = s.extra.iter().position(|x| anim::is_transition(&x.xml)) {
            let x = s.extra.remove(i);
            s.transition = anim::parse_transition(&x.xml, x.key);
        }
        if let Some(i) = s.extra.iter().position(|x| x.xml.name == "p:timing") {
            let x = s.extra.remove(i);
            s.timing = Some(anim::parse_timing(&x.xml, x.key));
        }
        if let Some(ln) = target_of(&s.info, |r| rel_is(&r.rel_type, "slideLayout")) {
            s.layout = load_layout(&mut cx, &mut pres, &mut layout_index, &ln, 0)?;
        }
        if let Some(nn) = target_of(&s.info, |r| rel_is(&r.rel_type, "notesSlide")) {
            if pkg.part(&nn).is_some() {
                let (root, info) = cx.load(&nn)?;
                let mut n = Notes {
                    info,
                    ..Notes::default()
                };
                n.info.root_attrs = root.attrs.clone();
                n.extra = walk(&root, NOTES, |c| {
                    match c.name.as_str() {
                        "p:cSld" => n.data = parse_csld(c),
                        "p:clrMapOvr" => match parse_clr_map_ovr(c) {
                            Some(m) => n.clr_map_ovr = Some(m),
                            None => return false,
                        },
                        _ => return false,
                    }
                    true
                });
                s.notes = Some(n);
            }
        }
        pres.slides.push(s);
    }

    // Everything not modelled stays verbatim.
    let ct = pkg.content_types().ok();
    for p in pkg.parts() {
        if cx.consumed.contains(&p.name.to_ascii_lowercase()) {
            continue;
        }
        pres.parts.push(RawPart {
            name: p.name.clone(),
            content_type: ct.as_ref().and_then(|c| c.get(&p.name)).map(str::to_owned),
            data: p.data.clone(),
        });
    }
    Ok(pres)
}

fn load_layout(
    cx: &mut Ctx<'_>,
    pres: &mut Presentation,
    index: &mut HashMap<String, usize>,
    name: &str,
    master: usize,
) -> Result<usize> {
    if let Some(i) = index.get(&name.to_ascii_lowercase()) {
        return Ok(*i);
    }
    let (root, info) = cx.load(name)?;
    let mut l = SlideLayout {
        master,
        info,
        ..SlideLayout::default()
    };
    let mut bag = AttrBag::of(&root);
    l.kind = bag.take("type");
    l.show_master_sp = bag.bool("showMasterSp");
    l.info.root_attrs = bag.rest();
    l.extra = walk(&root, LAYOUT, |c| {
        match c.name.as_str() {
            "p:cSld" => l.data = parse_csld(c),
            "p:clrMapOvr" => match parse_clr_map_ovr(c) {
                Some(m) => l.clr_map_ovr = Some(m),
                None => return false,
            },
            _ => return false,
        }
        true
    });
    pres.layouts.push(l);
    let i = pres.layouts.len() - 1;
    index.insert(name.to_ascii_lowercase(), i);
    Ok(i)
}
