//! Presenting a deck (SLIDE-SHOW-001): custom shows, show setup, rehearsed
//! timings, kept ink and hyperlinks that start a custom show.
//!
//! - Custom shows: `p:custShowLst` (ECMA-376 Part 1, §19.2.1.7) of `p:custShow`
//!   (§19.2.1.6), each a named list of slides by relationship id (`p:sldLst`,
//!   `p:sld`), kept among the preserved children of `presentation.xml`.
//! - Show setup: `p:showPr` (§19.2.1.30) in the presentation properties part
//!   (`p:presentationPr`, §19.2.1.27; part §13.3.7).
//! - Rehearsed timings: `p:transition@advTm` (§19.3.1.50) of each slide, in ms.
//! - Ink kept at the end of a show: one freeform shape per stroke (§19.3.1.43 sp
//!   with §20.1.9.8 custGeom), so any reader shows it.
//! - Hyperlinks: `a:hlinkClick` (§21.1.2.3.5) whose `action` is a `ppaction://`
//!   URL (`customshow?id=…&return=…`, `hlinkshowjump?jump=…`, `hlinksldjump`).
//!
//! Everything goes through invertible ops (one undo step per command); parts
//! and elements that a command does not change keep their bytes.

use super::geom::EMU_PT;
use super::ops::{PresFields, SlideOp, Tx};
use super::{refuse, SlideSel, R};
use bo_drawingml::dom;
use bo_model::slide::{
    Extra, Presentation, RawPart, Rel, ShapeNode, Transition, XmlElement, XmlNode,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Relationship type of the presentation properties part (§13.3.7).
const REL_PRES_PROPS: &str = "presProps";
/// Content type of the presentation properties part (§13.3.7).
const CT_PRES_PROPS: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.presProps+xml";
const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const NS_P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
/// Sort key of `p:custShowLst` in CT_Presentation (§19.2.1.26: 9th child, as
/// `dom::key_for` numbers the sequence).
const KEY_CUST_SHOW_LST: u16 = 2 * 8 + 2;
/// `kiosk@restart` default (§19.2.1.17): 300 000 ms.
const DEFAULT_RESTART_MS: u32 = 300_000;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A custom show as seen by the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomShowOut {
    /// `custShow@id` (ST_Index).
    pub id: u32,
    /// `custShow@name`.
    pub name: String,
    /// 0-based slides in playing order (references to missing slides are left out).
    pub slides: Vec<usize>,
}

/// How the show is presented (EG_ShowType of §19.2.1.30).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ShowType {
    /// §19.2.1.28 present: a speaker presents full screen.
    #[default]
    Speaker,
    /// §19.2.1.3 browse: browsed by one person in a window.
    Browse,
    /// §19.2.1.17 kiosk: runs unattended, restarts after inactivity.
    Kiosk,
}

/// Which slides the show plays (EG_SlideListChoice of §19.2.1.30).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SlideRange {
    /// §19.2.1.32 sldAll.
    #[default]
    All,
    /// §19.2.1.38 sldRg: 1-based slide numbers `st`..=`end`.
    Range {
        /// First slide number.
        from: u32,
        /// Last slide number.
        to: u32,
    },
    /// §19.2.1.5 custShow: a custom show by id.
    Custom {
        /// `custShow@id`.
        id: u32,
    },
}

/// §19.2.1.30 showPr as seen and set by the UI (defaults: the schema's).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShowSetup {
    /// Presented by a speaker, browsed in a window or at a kiosk.
    pub show_type: ShowType,
    /// `browse@showScrollbar` (default true).
    pub scrollbar: bool,
    /// `kiosk@restart`: inactivity before the show restarts, ms (default 300 000).
    pub restart_ms: u32,
    /// `loop`: start again after the last slide (default false).
    #[serde(rename = "loop")]
    pub looped: bool,
    /// `showNarration` (default false).
    pub narration: bool,
    /// `showAnimation` (default true).
    pub animation: bool,
    /// `useTimings`: advance after the slides' `advTm` (default true).
    pub use_timings: bool,
    /// Slides to play.
    pub range: SlideRange,
    /// `penClr` as `RRGGBB` when it is an sRGB colour.
    pub pen_color: Option<String>,
}

impl Default for ShowSetup {
    fn default() -> Self {
        ShowSetup {
            show_type: ShowType::Speaker,
            scrollbar: true,
            restart_ms: DEFAULT_RESTART_MS,
            looped: false,
            narration: false,
            animation: true,
            use_timings: true,
            range: SlideRange::All,
            pen_color: None,
        }
    }
}

/// What the Slide Show tab and the show need about the deck.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresentOut {
    /// Custom shows in file order.
    pub custom_shows: Vec<CustomShowOut>,
    /// Show setup.
    pub setup: ShowSetup,
    /// Slides left out of the show (`sld@show="0"`, §19.3.1.38).
    pub hidden: Vec<usize>,
    /// `advTm` of each slide in ms (`None`: no automatic advance).
    pub timings: Vec<Option<u32>>,
}

/// Rehearsed time of one slide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideTiming {
    /// 0-based slide index.
    pub slide: usize,
    /// Advance after this many ms; `None` removes the automatic advance.
    #[serde(default)]
    pub ms: Option<u32>,
}

/// One ink stroke kept from the show, in slide points.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InkStroke {
    /// Points of the stroke (`[x, y]`, slide points).
    pub points: Vec<[f64; 2]>,
    /// `RRGGBB`.
    pub color: String,
    /// Line width in points.
    pub width: f64,
    /// Highlighter: drawn half transparent.
    #[serde(default)]
    pub highlighter: bool,
}

/// Ink of one slide.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InkSlide {
    /// 0-based slide index.
    pub slide: usize,
    /// Strokes in drawing order.
    pub strokes: Vec<InkStroke>,
}

/// Where a hyperlink of the selected shapes leads in the show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LinkAction {
    /// No hyperlink (removes it).
    None,
    /// Play custom show `id`, then come back when `ret`.
    CustomShow {
        /// `custShow@id`.
        id: u32,
        /// Come back to the slide of the link after the custom show.
        #[serde(default, rename = "return")]
        ret: bool,
    },
    /// Go to another slide (0-based).
    Slide {
        /// 0-based slide index.
        slide: usize,
    },
    /// Next slide.
    Next,
    /// Previous slide.
    Prev,
    /// First slide.
    First,
    /// Last slide.
    Last,
    /// Last slide viewed.
    LastViewed,
    /// End the show.
    End,
    /// A URL or mail address (external relationship without an `action`).
    Url {
        /// Target of the relationship, as written.
        url: String,
    },
    /// Another presentation (`ppaction://hlinkpres`).
    Presentation {
        /// Target of the relationship.
        path: String,
        /// 1-based slide of that deck (`slideindex`).
        #[serde(default)]
        slide: Option<u32>,
    },
    /// Another file (`ppaction://hlinkfile`).
    File {
        /// Target of the relationship.
        path: String,
    },
    /// A program (`ppaction://program`). Kept in the file, never run.
    Program {
        /// Target of the relationship.
        path: String,
    },
    /// A macro (`ppaction://macro`). Kept in the file, never run.
    Macro {
        /// Macro name.
        name: String,
    },
    /// An action BlueSlide does not know. Kept in the file, never run.
    Unknown {
        /// The `action` URI.
        action: String,
    },
    /// A zoom (SLIDE-ZOOM-001, [MS-PPTX] §2.8.3.1): play `slides` (a slide, or a section's
    /// slides), zooming in from the zoom's box; with `ret`, come back to the zoom's slide
    /// afterwards.
    Zoom {
        /// 0-based slides in playing order.
        slides: Vec<usize>,
        /// `zmPr@returnToParent`.
        #[serde(default, rename = "return")]
        ret: bool,
        /// `zmPr@transitionDur` in ms; `None`: the destination's own transition.
        #[serde(default)]
        transition_ms: Option<u32>,
    },
}

pub use super::action::LinkOut;

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

fn bool_attr(el: &XmlElement, name: &str, default: bool) -> bool {
    el.attr(name).and_then(dom::parse_bool).unwrap_or(default)
}

/// Value of the relationship id attribute (`r:id`, whatever its prefix).
fn rel_id_attr(el: &XmlElement) -> Option<&str> {
    el.attrs
        .iter()
        .find(|(k, _)| k.ends_with(":id") && !k.starts_with("xmlns"))
        .map(|(_, v)| v.as_str())
}

fn cust_list_index(pres: &Presentation) -> Option<usize> {
    pres.extra
        .iter()
        .position(|x| x.xml.local() == "custShowLst")
}

/// Custom shows (§19.2.1.7) with their slides resolved to indices.
pub fn custom_shows(pres: &Presentation) -> Vec<CustomShowOut> {
    let Some(i) = cust_list_index(pres) else {
        return Vec::new();
    };
    pres.extra[i]
        .xml
        .elements()
        .filter(|c| c.local() == "custShow")
        .map(|c| CustomShowOut {
            id: c
                .attr("id")
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0),
            name: c.attr("name").unwrap_or_default().to_owned(),
            slides: c
                .elements()
                .filter(|l| l.local() == "sldLst")
                .flat_map(XmlElement::elements)
                .filter(|s| s.local() == "sld")
                .filter_map(|s| {
                    let rid = rel_id_attr(s)?;
                    pres.slides.iter().position(|x| x.pres_rel_id == rid)
                })
                .collect(),
        })
        .collect()
}

/// Part name of the presentation properties part, when related.
fn pres_props_name(pres: &Presentation) -> Option<String> {
    pres.info
        .rels
        .iter()
        .find(|r| !r.external && r.rel_type.ends_with(&format!("/{REL_PRES_PROPS}")))
        .map(|r| bo_opc::resolve_target(&pres.info.part_name, &r.target))
}

fn pres_props_root(pres: &Presentation) -> Option<XmlElement> {
    let name = pres_props_name(pres)?;
    let data = pres.part_data(&name)?;
    dom::parse(data, &name).ok()
}

fn show_pr(root: &XmlElement) -> Option<&XmlElement> {
    root.elements().find(|c| c.local() == "showPr")
}

/// `RRGGBB` of a penClr holding an sRGB colour (§20.1.2.3.32 srgbClr).
fn pen_color(el: &XmlElement) -> Option<String> {
    el.elements()
        .find(|c| c.local() == "srgbClr")
        .and_then(|c| c.attr("val"))
        .map(str::to_ascii_uppercase)
}

/// §19.2.1.30 showPr → [`ShowSetup`].
pub fn parse_setup(sp: &XmlElement) -> ShowSetup {
    let mut s = ShowSetup {
        looped: bool_attr(sp, "loop", false),
        narration: bool_attr(sp, "showNarration", false),
        animation: bool_attr(sp, "showAnimation", true),
        use_timings: bool_attr(sp, "useTimings", true),
        ..ShowSetup::default()
    };
    for c in sp.elements() {
        match c.local() {
            // §19.2.1.28 present
            "present" => s.show_type = ShowType::Speaker,
            // §19.2.1.3 browse
            "browse" => {
                s.show_type = ShowType::Browse;
                s.scrollbar = bool_attr(c, "showScrollbar", true);
            }
            // §19.2.1.17 kiosk
            "kiosk" => {
                s.show_type = ShowType::Kiosk;
                s.restart_ms = c
                    .attr("restart")
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(DEFAULT_RESTART_MS);
            }
            // §19.2.1.32 sldAll
            "sldAll" => s.range = SlideRange::All,
            // §19.2.1.38 sldRg (CT_IndexRange st / end)
            "sldRg" => {
                let n = |k: &str| c.attr(k).and_then(|v| v.trim().parse::<u32>().ok());
                if let (Some(from), Some(to)) = (n("st"), n("end")) {
                    s.range = SlideRange::Range { from, to };
                }
            }
            // §19.2.1.5 custShow
            "custShow" => {
                if let Some(id) = c.attr("id").and_then(|v| v.trim().parse().ok()) {
                    s.range = SlideRange::Custom { id };
                }
            }
            // §19.2.1.25 penClr
            "penClr" => s.pen_color = pen_color(c),
            _ => {}
        }
    }
    s
}

/// The show setup of the deck (schema defaults without a showPr).
pub fn show_setup(pres: &Presentation) -> ShowSetup {
    pres_props_root(pres)
        .as_ref()
        .and_then(show_pr)
        .map(parse_setup)
        .unwrap_or_default()
}

/// Everything the Slide Show tab needs.
pub fn present_out(pres: &Presentation) -> PresentOut {
    PresentOut {
        custom_shows: custom_shows(pres),
        setup: show_setup(pres),
        hidden: pres
            .slides
            .iter()
            .enumerate()
            .filter(|(_, s)| s.show == Some(false))
            .map(|(i, _)| i)
            .collect(),
        timings: pres
            .slides
            .iter()
            .map(|s| s.transition.as_ref().and_then(|t| t.advance_after_ms))
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Relationship ids already in use by the presentation part.
fn used_rel_ids(pres: &Presentation) -> HashSet<String> {
    let mut used: HashSet<String> = pres.info.rels.iter().map(|r| r.id.clone()).collect();
    used.extend(pres.slides.iter().map(|s| s.pres_rel_id.clone()));
    used.extend(pres.masters.iter().map(|m| m.pres_rel_id.clone()));
    if let Some(i) = cust_list_index(pres) {
        for c in pres.extra[i].xml.elements() {
            for l in c.elements() {
                for s in l.elements() {
                    if let Some(id) = rel_id_attr(s) {
                        used.insert(id.to_owned());
                    }
                }
            }
        }
    }
    used
}

fn fresh_rel_id(used: &HashSet<String>) -> String {
    // One of the first `used.len() + 1` ids is free.
    (1..=used.len() + 1)
        .map(|n| format!("rId{n}"))
        .find(|id| !used.contains(id))
        .unwrap_or_default()
}

/// The relationship id of slide `i` from the presentation part. A slide added in
/// this session gets one now — with its relationship, so the writer keeps the id.
fn ensure_slide_rel(tx: &mut Tx<'_>, i: usize) -> R<String> {
    let s = &tx.pres.slides[i];
    if !s.pres_rel_id.is_empty() {
        return Ok(s.pres_rel_id.clone());
    }
    let id = fresh_rel_id(&used_rel_ids(tx.pres));
    let old = s.clone();
    let mut new = old.clone();
    new.pres_rel_id.clone_from(&id);
    tx.op(SlideOp::Slides {
        index: i,
        remove: vec![old],
        insert: vec![new],
    })?;
    let before = PresFields::of(tx.pres);
    let mut after = before.clone();
    // ECMA-376 Part 2, §9.3: the writer points the target at the slide part.
    after.rels.push(Rel {
        id: id.clone(),
        rel_type: format!("{}slide", bo_pptx::RELATIONSHIP_NS),
        target: format!("slides/slide{}.xml", i + 1),
        external: false,
    });
    tx.op(SlideOp::Pres {
        before: Box::new(before),
        after: Box::new(after),
    })?;
    Ok(id)
}

/// Replace (`Some`) or remove (`None`) `p:custShowLst`.
fn store_cust_list(tx: &mut Tx<'_>, list: Option<XmlElement>) -> R<()> {
    let before = PresFields::of(tx.pres);
    let mut after = before.clone();
    let at = after
        .extra
        .iter()
        .position(|x| x.xml.local() == "custShowLst");
    match (at, list) {
        (Some(i), Some(l)) => after.extra[i].xml = l,
        (Some(i), None) => {
            after.extra.remove(i);
        }
        (None, Some(l)) => after.extra.push(Extra {
            key: KEY_CUST_SHOW_LST,
            xml: l,
        }),
        (None, None) => {}
    }
    if after == before {
        return Ok(());
    }
    tx.op(SlideOp::Pres {
        before: Box::new(before),
        after: Box::new(after),
    })?;
    Ok(())
}

fn el(name: &str, attrs: &[(&str, String)]) -> XmlElement {
    XmlElement {
        name: name.to_owned(),
        attrs: attrs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect(),
        children: Vec::new(),
    }
}

fn set_attr(e: &mut XmlElement, k: &str, v: Option<String>) {
    let at = e.attrs.iter().position(|(n, _)| n == k);
    match (at, v) {
        (Some(i), Some(v)) => e.attrs[i].1 = v,
        (Some(i), None) => {
            e.attrs.remove(i);
        }
        (None, Some(v)) => e.attrs.push((k.to_owned(), v)),
        (None, None) => {}
    }
}

/// §19.2.1.35 sldLst of slide relationship ids (§19.2.1.33 sld).
fn sld_lst(prefix: &str, rids: &[String]) -> XmlElement {
    let mut l = el(&format!("{prefix}sldLst"), &[]);
    for rid in rids {
        l.children.push(XmlNode::Element(el(
            &format!("{prefix}sld"),
            &[("r:id", rid.clone())],
        )));
    }
    l
}

/// Create (`id` = `None`) or change a custom show.
pub fn cmd_set_custom_show(
    tx: &mut Tx<'_>,
    id: Option<u32>,
    name: &str,
    slides: &[usize],
) -> R<()> {
    let name = name.trim();
    if name.is_empty() {
        return refuse("emptyName");
    }
    if slides.iter().any(|&i| i >= tx.pres.slides.len()) {
        return refuse("noSlide");
    }
    let shows = custom_shows(tx.pres);
    if let Some(id) = id {
        if !shows.iter().any(|s| s.id == id) {
            return refuse("noCustomShow");
        }
    }
    if shows
        .iter()
        .any(|s| Some(s.id) != id && s.name.trim().eq_ignore_ascii_case(name))
    {
        return refuse("duplicateName");
    }
    let mut rids = Vec::with_capacity(slides.len());
    for &i in slides {
        rids.push(ensure_slide_rel(tx, i)?);
    }
    let mut list = cust_list_index(tx.pres).map_or_else(
        || el("p:custShowLst", &[]),
        |i| tx.pres.extra[i].xml.clone(),
    );
    let prefix = list
        .name
        .split_once(':')
        .map_or(String::new(), |(p, _)| format!("{p}:"));
    if let Some(id) = id {
        {
            for c in &mut list.children {
                let XmlNode::Element(show) = c else { continue };
                if show.local() != "custShow"
                    || show.attr("id").and_then(|v| v.trim().parse().ok()) != Some(id)
                {
                    continue;
                }
                set_attr(show, "name", Some(name.to_owned()));
                let lst = sld_lst(&prefix, &rids);
                // §19.2.1.6: sldLst is the first child; extLst after it is kept.
                match show
                    .children
                    .iter()
                    .position(|n| matches!(n, XmlNode::Element(e) if e.local() == "sldLst"))
                {
                    Some(i) => show.children[i] = XmlNode::Element(lst),
                    None => show.children.insert(0, XmlNode::Element(lst)),
                }
            }
        }
    } else {
        {
            let next = shows.iter().map(|s| s.id + 1).max().unwrap_or(0);
            let mut show = el(
                &format!("{prefix}custShow"),
                &[("name", name.to_owned()), ("id", next.to_string())],
            );
            show.children
                .push(XmlNode::Element(sld_lst(&prefix, &rids)));
            list.children.push(XmlNode::Element(show));
        }
    }
    store_cust_list(tx, Some(list))
}

/// Delete custom show `id`; a show setup playing it plays every slide again.
pub fn cmd_delete_custom_show(tx: &mut Tx<'_>, id: u32) -> R<()> {
    let Some(i) = cust_list_index(tx.pres) else {
        return refuse("noCustomShow");
    };
    let mut list = tx.pres.extra[i].xml.clone();
    let n = list.children.len();
    list.children.retain(|c| {
        !matches!(c, XmlNode::Element(e) if e.local() == "custShow"
            && e.attr("id").and_then(|v| v.trim().parse().ok()) == Some(id))
    });
    if list.children.len() == n {
        return refuse("noCustomShow");
    }
    let empty = list.elements().all(|e| e.local() != "custShow");
    store_cust_list(tx, (!empty).then_some(list))?;
    let setup = show_setup(tx.pres);
    if setup.range == (SlideRange::Custom { id }) {
        store_setup(
            tx,
            &ShowSetup {
                range: SlideRange::All,
                ..setup
            },
        )?;
    }
    Ok(())
}

/// Build the new `p:showPr`, keeping what the setup does not model (extLst,
/// unknown attributes, a non-sRGB pen colour) from `old`.
fn build_show_pr(
    old: Option<&XmlElement>,
    s: &ShowSetup,
    prefix: &str,
    a_declared: bool,
) -> XmlElement {
    let mut sp = old
        .cloned()
        .unwrap_or_else(|| el(&format!("{prefix}showPr"), &[]));
    let flag =
        |v: bool, default: bool| (v != default).then(|| if v { "1" } else { "0" }.to_owned());
    // Attributes equal to the schema default are left out.
    set_attr(&mut sp, "loop", flag(s.looped, false));
    set_attr(&mut sp, "showNarration", flag(s.narration, false));
    set_attr(&mut sp, "showAnimation", flag(s.animation, true));
    set_attr(&mut sp, "useTimings", flag(s.use_timings, true));
    let old_pen = old
        .and_then(|o| o.elements().find(|c| c.local() == "penClr"))
        .cloned();
    let ext = old
        .and_then(|o| o.elements().find(|c| c.local() == "extLst"))
        .cloned();
    let mut kids: Vec<XmlElement> = Vec::new();
    kids.push(match s.show_type {
        ShowType::Speaker => el(&format!("{prefix}present"), &[]),
        ShowType::Browse => {
            let mut b = el(&format!("{prefix}browse"), &[]);
            set_attr(&mut b, "showScrollbar", flag(s.scrollbar, true));
            b
        }
        ShowType::Kiosk => {
            let mut k = el(&format!("{prefix}kiosk"), &[]);
            set_attr(
                &mut k,
                "restart",
                (s.restart_ms != DEFAULT_RESTART_MS).then(|| s.restart_ms.to_string()),
            );
            k
        }
    });
    kids.push(match &s.range {
        SlideRange::All => el(&format!("{prefix}sldAll"), &[]),
        SlideRange::Range { from, to } => el(
            &format!("{prefix}sldRg"),
            &[("st", from.to_string()), ("end", to.to_string())],
        ),
        SlideRange::Custom { id } => el(&format!("{prefix}custShow"), &[("id", id.to_string())]),
    });
    match (&old_pen, &s.pen_color) {
        (Some(p), want) if pen_color(p) == *want || want.is_none() && pen_color(p).is_none() => {
            kids.push(p.clone());
        }
        (_, Some(hex)) => {
            let mut c = el("a:srgbClr", &[("val", hex.to_ascii_uppercase())]);
            if !a_declared {
                c.attrs.push(("xmlns:a".into(), NS_A.into()));
            }
            let mut pen = el(&format!("{prefix}penClr"), &[]);
            pen.children.push(XmlNode::Element(c));
            kids.push(pen);
        }
        _ => {}
    }
    if let Some(e) = ext {
        kids.push(e);
    }
    sp.children = kids.into_iter().map(XmlNode::Element).collect();
    sp
}

/// Write `setup` into the presentation properties part (created when missing).
fn store_setup(tx: &mut Tx<'_>, setup: &ShowSetup) -> R<()> {
    if show_setup(tx.pres) == *setup
        && pres_props_root(tx.pres)
            .as_ref()
            .and_then(show_pr)
            .is_some()
    {
        return Ok(());
    }
    let name = pres_props_name(tx.pres);
    let root = pres_props_root(tx.pres);
    let mut root = root.unwrap_or_else(|| {
        el(
            "p:presentationPr",
            &[
                ("xmlns:a", NS_A.to_owned()),
                ("xmlns:r", NS_R.to_owned()),
                ("xmlns:p", NS_P.to_owned()),
            ],
        )
    });
    let prefix = root
        .name
        .split_once(':')
        .map_or(String::new(), |(p, _)| format!("{p}:"));
    let a_declared = root.attrs.iter().any(|(k, v)| k == "xmlns:a" && v == NS_A);
    let at = root
        .children
        .iter()
        .position(|n| matches!(n, XmlNode::Element(e) if e.local() == "showPr"));
    let sp = build_show_pr(
        at.and_then(|i| match &root.children[i] {
            XmlNode::Element(e) => Some(e),
            XmlNode::Text(_) => None,
        }),
        setup,
        &prefix,
        a_declared,
    );
    if let Some(i) = at {
        root.children[i] = XmlNode::Element(sp);
    } else {
        // §19.2.1.27: showPr comes after htmlPubPr / webPr / prnPr, before clrMru / extLst.
        let before = root.children.iter().position(
            |n| matches!(n, XmlNode::Element(e) if matches!(e.local(), "clrMru" | "extLst")),
        );
        match before {
            Some(i) => root.children.insert(i, XmlNode::Element(sp)),
            None => root.children.push(XmlNode::Element(sp)),
        }
    }
    let data = dom::to_part_xml(&root);
    if let Some(name) = name.filter(|n| tx.pres.part_data(n).is_some()) {
        let i = tx
            .pres
            .parts
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case(&name))
            .unwrap_or_default();
        let old = tx.pres.parts[i].clone();
        let new = RawPart {
            data,
            ..old.clone()
        };
        tx.op(SlideOp::Parts {
            index: i,
            remove: vec![old],
            insert: vec![new],
        })?;
        return Ok(());
    }
    // A new part, related from the presentation part (§13.3.7).
    let part = if tx.pres.part_data("/ppt/presProps.xml").is_none() {
        "/ppt/presProps.xml".to_owned()
    } else {
        super::build::unique_part(tx.pres, "/ppt/presProps", "xml")
    };
    let at = tx.pres.parts.len();
    tx.op(SlideOp::Parts {
        index: at,
        remove: vec![],
        insert: vec![RawPart {
            name: part.clone(),
            content_type: Some(CT_PRES_PROPS.into()),
            data,
        }],
    })?;
    let before = PresFields::of(tx.pres);
    let mut after = before.clone();
    after
        .rels
        .retain(|r| !r.rel_type.ends_with(&format!("/{REL_PRES_PROPS}")));
    after.rels.push(Rel {
        id: fresh_rel_id(&used_rel_ids(tx.pres)),
        rel_type: format!("{}{REL_PRES_PROPS}", bo_pptx::RELATIONSHIP_NS),
        target: super::clip::relative_target(&tx.pres.info.part_name, &part),
        external: false,
    });
    tx.op(SlideOp::Pres {
        before: Box::new(before),
        after: Box::new(after),
    })?;
    Ok(())
}

/// Change the show setup.
pub fn cmd_set_show_setup(tx: &mut Tx<'_>, setup: &ShowSetup) -> R<()> {
    match &setup.range {
        SlideRange::Range { from, to } if *from == 0 || to < from => return refuse("badRange"),
        SlideRange::Custom { id } if !custom_shows(tx.pres).iter().any(|s| s.id == *id) => {
            return refuse("noCustomShow")
        }
        _ => {}
    }
    if let Some(c) = &setup.pen_color {
        if c.len() != 6 || !c.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return refuse("badColor");
        }
    }
    store_setup(tx, setup)
}

/// Set the rehearsed (or typed) timings of slides; `use_timings` also turns
/// the setup's "use timings" on or off.
pub fn cmd_set_timings(
    tx: &mut Tx<'_>,
    timings: &[SlideTiming],
    use_timings: Option<bool>,
) -> R<()> {
    let n = tx.pres.slides.len();
    if timings.iter().any(|t| t.slide >= n) {
        return refuse("noSlide");
    }
    for t in timings {
        tx.fields(t.slide, |f| {
            let mut tr = f.transition.clone().unwrap_or_default();
            // §19.3.1.50 transition@advTm: ms before advancing to the next slide.
            tr.advance_after_ms = t.ms;
            let bare = Transition {
                source: None,
                ..tr.clone()
            } == Transition::default();
            f.transition = if bare && f.transition.is_none() {
                None
            } else {
                Some(tr)
            };
        })?;
    }
    if let Some(on) = use_timings {
        let setup = show_setup(tx.pres);
        if setup.use_timings != on {
            store_setup(
                tx,
                &ShowSetup {
                    use_timings: on,
                    ..setup
                },
            )?;
        }
    }
    Ok(())
}

/// Give the selected shapes a show hyperlink (or remove it): the click action
/// of [`super::action`] without a tooltip, sound or highlight.
pub fn cmd_set_show_link(tx: &mut Tx<'_>, sel: &SlideSel, action: &LinkAction) -> R<()> {
    super::action::set_action(
        tx,
        sel,
        None,
        super::action::Trigger::Click,
        &super::action::ActionSpec {
            target: action.clone(),
            tooltip: None,
            highlight: false,
            sound: super::action::SoundSpec::None,
        },
    )
}

/// Freeform shape of one ink stroke (§19.3.1.43 sp, §20.1.9.8 custGeom).
fn ink_shape(id: u32, name: &str, st: &InkStroke) -> Option<ShapeNode> {
    let pts: Vec<[f64; 2]> = st
        .points
        .iter()
        .copied()
        .filter(|p| p[0].is_finite() && p[1].is_finite())
        .collect();
    let first = *pts.first()?;
    let (mut x0, mut y0, mut x1, mut y1) = (first[0], first[1], first[0], first[1]);
    for p in &pts {
        x0 = x0.min(p[0]);
        y0 = y0.min(p[1]);
        x1 = x1.max(p[0]);
        y1 = y1.max(p[1]);
    }
    let emu = |pt: f64| (pt * EMU_PT).round() as i64;
    let (cx, cy) = (emu(x1 - x0).max(1), emu(y1 - y0).max(1));
    let mut path = format!(
        r#"<a:moveTo><a:pt x="{}" y="{}"/></a:moveTo>"#,
        emu(first[0] - x0),
        emu(first[1] - y0)
    );
    // A dot (one point) is drawn as a segment to itself so the round cap shows.
    let rest: Vec<[f64; 2]> = if pts.len() == 1 {
        vec![first]
    } else {
        pts[1..].to_vec()
    };
    for p in rest {
        path.push_str(&format!(
            r#"<a:lnTo><a:pt x="{}" y="{}"/></a:lnTo>"#,
            emu(p[0] - x0),
            emu(p[1] - y0)
        ));
    }
    let color = if st.color.len() == 6 && st.color.chars().all(|c| c.is_ascii_hexdigit()) {
        st.color.to_ascii_uppercase()
    } else {
        "000000".to_owned()
    };
    // §20.1.2.3.1 alpha: the highlighter is half transparent.
    let alpha = if st.highlighter {
        r#"<a:alpha val="50000"/>"#
    } else {
        ""
    };
    let width = emu(st.width.clamp(0.25, 100.0));
    let xml = format!(
        concat!(
            r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{name}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>"#,
            r#"<p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm>"#,
            r#"<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/><a:rect l="l" t="t" r="r" b="b"/>"#,
            r#"<a:pathLst><a:path w="{cx}" h="{cy}" fill="none">{path}</a:path></a:pathLst></a:custGeom>"#,
            r#"<a:noFill/><a:ln w="{width}" cap="rnd"><a:solidFill><a:srgbClr val="{color}">{alpha}</a:srgbClr></a:solidFill>"#,
            r#"<a:round/></a:ln></p:spPr></p:sp>"#
        ),
        id = id,
        name = super::build::esc(name),
        x = emu(x0),
        y = emu(y0),
        cx = cx,
        cy = cy,
        path = path,
        width = width,
        color = color,
        alpha = alpha,
    );
    super::build::node(&xml)
}

/// Keep the ink drawn during the show: each stroke becomes a freeform shape
/// on top of its slide (one undo step for all slides).
pub fn cmd_keep_ink(tx: &mut Tx<'_>, ink: &[InkSlide], name: &str) -> R<()> {
    if ink.iter().any(|s| s.slide >= tx.pres.slides.len()) {
        return refuse("noSlide");
    }
    let base = if name.trim().is_empty() {
        "Ink"
    } else {
        name.trim()
    };
    for s in ink {
        let mut id = super::next_shape_id(tx.pres, s.slide);
        let mut nodes = Vec::new();
        for st in &s.strokes {
            if let Some(n) = ink_shape(id, &format!("{base} {id}"), st) {
                nodes.push(n);
                id += 1;
            }
        }
        if nodes.is_empty() {
            continue;
        }
        let at = tx.pres.slides[s.slide].data.shapes.len();
        tx.insert_nodes(s.slide, &[], at, nodes)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_pr_attributes_and_children() {
        // §19.2.1.30 example shapes: kiosk restarting after 60 s, slides 2–4, red pen.
        let x = br#"<p:showPr xmlns:p="p" xmlns:a="a" loop="1" showNarration="1" useTimings="0"><p:kiosk restart="60000"/><p:sldRg st="2" end="4"/><p:penClr><a:srgbClr val="ff0000"/></p:penClr></p:showPr>"#;
        let sp = dom::parse(x, "t").unwrap();
        let s = parse_setup(&sp);
        assert_eq!(s.show_type, ShowType::Kiosk);
        assert_eq!(s.restart_ms, 60_000);
        assert!(s.looped && s.narration && s.animation && !s.use_timings);
        assert_eq!(s.range, SlideRange::Range { from: 2, to: 4 });
        assert_eq!(s.pen_color.as_deref(), Some("FF0000"));
        // Written back: defaults are left out, the order follows the schema.
        let back = build_show_pr(Some(&sp), &s, "p:", true);
        let names: Vec<&str> = back.elements().map(XmlElement::local).collect();
        assert_eq!(names, ["kiosk", "sldRg", "penClr"]);
        assert_eq!(parse_setup(&back), s);
        assert_eq!(back.attr("showAnimation"), None);
    }

    #[test]
    fn schema_defaults_without_attributes() {
        let sp = dom::parse(br#"<p:showPr xmlns:p="p"/>"#, "t").unwrap();
        assert_eq!(parse_setup(&sp), ShowSetup::default());
        let b = dom::parse(br#"<p:showPr xmlns:p="p"><p:browse showScrollbar="0"/><p:custShow id="3"/></p:showPr>"#, "t").unwrap();
        let s = parse_setup(&b);
        assert_eq!(s.show_type, ShowType::Browse);
        assert!(!s.scrollbar);
        assert_eq!(s.range, SlideRange::Custom { id: 3 });
    }

    #[test]
    fn ink_stroke_geometry() {
        // Points (10,20) → (40,60): box 30 × 40 pt at (10, 20); 1 pt = 12 700 EMU.
        let st = InkStroke {
            points: vec![[10.0, 20.0], [40.0, 60.0]],
            color: "00ff00".into(),
            width: 2.0,
            highlighter: true,
        };
        let Some(ShapeNode::Shape(s)) = ink_shape(5, "Ink 5", &st) else {
            panic!("shape");
        };
        let x = s.props.xfrm.unwrap();
        assert_eq!(x.off, Some((127_000, 254_000)));
        assert_eq!(x.ext, Some((381_000, 508_000)));
        assert_eq!(s.nv.id, 5);
    }
}
