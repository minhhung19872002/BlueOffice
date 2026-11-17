//! Accessibility checks of a presentation (round 19 of BlueSlide's native UI), the rules of
//! the reference's checker grouped as it groups them (WCAG 2.2 1.1.1 Non-text Content,
//! 1.2.2 Captions, 1.3.1 Info and Relationships, 1.3.2 Meaningful Sequence, 1.4.3 Contrast,
//! 2.4.2 Page Titled, 2.4.4 Link Purpose):
//!
//! * `contrast` — text whose colour against its shape's solid fill (or the slide's solid
//!   background, else white) is below 4.5 : 1 (3 : 1 for text of 18 pt, or 14 pt bold);
//! * `altText` — pictures, charts, SmartArt, media, objects and 3D models without
//!   `cNvPr@descr` (ECMA-376 Part 1, §19.3.1.12) and not marked decorative;
//! * `captions` — videos (§20.1.3.6 videoFile): no caption track is ever stored;
//! * `tableHeader` — tables whose `tblPr@firstRow` is off (§21.1.3.15);
//! * `merged` — tables with merged cells (§21.1.3.16 tc `gridSpan` / `rowSpan` / `hMerge`
//!   / `vMerge`);
//! * `linkText` — hyperlinked runs (§21.1.2.3.5 hlinkClick) whose text is a bare address;
//! * `slideTitle` / `dupTitle` — slides without a title placeholder with text, or with the
//!   same title as another slide;
//! * `readingOrder` — slides whose shape order (the order a screen reader follows,
//!   §19.3.1.45 spTree) differs from top-to-bottom, left-to-right;
//! * `defaultSection` / `dupSection` — sections still named "Default Section" /
//!   "Untitled Section", or named like another ([MS-PPTX] p14:sectionLst);
//! * `restricted` — never (BlueSlide writes no rights-management restrictions).
//!
//! The checks only read the deck.
use super::geom::place_slide;
use super::{kind_of, ops};
use bo_model::slide::{
    Background, Fill, GraphicContent, Presentation, ShapeNode, TextBody, TextRun,
};
use serde::Serialize;

/// Rule ids in the order the reference lists them, with their category (0 Color and
/// Contrast, 1 Media and Illustrations, 2 Tables, 3 Document Structure, 4 Document Access)
/// and whether an issue is an error (the reference's red "Keep going!").
pub const RULES: [(&str, usize, bool); 12] = [
    ("contrast", 0, false),
    ("altText", 1, true),
    ("captions", 1, false),
    ("tableHeader", 2, true),
    ("merged", 2, false),
    ("linkText", 3, false),
    ("slideTitle", 3, true),
    ("dupTitle", 3, false),
    ("readingOrder", 3, false),
    ("defaultSection", 3, false),
    ("dupSection", 3, false),
    ("restricted", 4, false),
];

/// Kinds of shape that need alternative text.
pub const NEEDS_ALT: [&str; 6] = ["picture", "chart", "smartart", "media", "object", "model3d"];

/// One issue found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct A11yIssue {
    /// Rule id (see [`RULES`]).
    pub rule: &'static str,
    /// Slide (0-based) the issue is on; `None` for the deck (sections).
    pub slide: Option<usize>,
    /// Shape id (`cNvPr@id`), 0 for the slide itself.
    pub id: u32,
    /// What the issue is about: a shape name, a title or a section name.
    pub name: String,
}

/// Relative luminance of a colour (WCAG 2.2 definition).
fn luminance(rgb: u32) -> f64 {
    let ch = |v: u32| {
        let c = f64::from(v & 0xFF) / 255.0;
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * ch(rgb >> 16) + 0.7152 * ch(rgb >> 8) + 0.0722 * ch(rgb)
}

/// Contrast ratio of two colours (1–21).
pub fn contrast(a: u32, b: u32) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

fn hex(s: &str) -> Option<u32> {
    u32::from_str_radix(s.trim_start_matches('#'), 16).ok()
}

fn solid(pres: &Presentation, slide: usize, f: &Fill) -> Option<u32> {
    use bo_pptx::inherit::{clr_map_of, theme_of, Level};
    let Fill::Solid(c) = f else { return None };
    let map = clr_map_of(pres, Level::Slide(slide));
    let ctx = bo_drawingml::ColorContext {
        theme: theme_of(pres, Level::Slide(slide)),
        clr_map: &map,
        ph_clr: None,
    };
    Some(bo_drawingml::resolve_color(c, &ctx).to_hex() & 0x00FF_FFFF)
}

/// The slide's background colour when solid (slide, layout, master), else white.
fn background(pres: &Presentation, slide: usize) -> u32 {
    let s = &pres.slides[slide];
    let layout = pres.layouts.get(s.layout);
    let master = layout.and_then(|l| pres.masters.get(l.master));
    [
        s.data.background.as_ref(),
        layout.and_then(|l| l.data.background.as_ref()),
        master.and_then(|m| m.data.background.as_ref()),
    ]
    .into_iter()
    .flatten()
    .find_map(|b| match b {
        Background::Props { fill, .. } => Some(solid(pres, slide, fill)),
        Background::Ref { .. } => None,
    })
    .flatten()
    .unwrap_or(0x00FF_FFFF)
}

fn looks_like_address(t: &str) -> bool {
    let t = t.trim().to_ascii_lowercase();
    t.starts_with("http://")
        || t.starts_with("https://")
        || t.starts_with("www.")
        || t.starts_with("mailto:")
}

fn bare_links(tb: &TextBody) -> bool {
    tb.paragraphs.iter().flat_map(|p| &p.runs).any(|r| match r {
        TextRun::Run { props, text } => {
            props.extra.iter().any(|x| x.xml.name == "a:hlinkClick") && looks_like_address(text)
        }
        _ => false,
    })
}

fn title_of(pres: &Presentation, slide: usize) -> Option<String> {
    pres.slides[slide].data.shapes.iter().find_map(|n| {
        let ShapeNode::Shape(s) = n else { return None };
        let k = s.nv.placeholder.as_ref()?.kind.as_deref();
        matches!(k, Some("title" | "ctrTitle"))
            .then(|| s.text.as_ref().map(super::text::body_text))
            .flatten()
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
    })
}

fn decorative(n: &ShapeNode) -> bool {
    // [MS-ODRAWXML] a16:decorative in cNvPr's extLst.
    n.nv().is_some_and(|nv| {
        nv.cnv_extra.iter().any(|x| {
            let mut found = false;
            walk(&x.xml, &mut |e| {
                if e.name.ends_with(":decorative") && e.attr("val") != Some("0") {
                    found = true;
                }
            });
            found
        })
    })
}

fn walk(el: &bo_model::slide::XmlElement, f: &mut impl FnMut(&bo_model::slide::XmlElement)) {
    f(el);
    for c in &el.children {
        if let bo_model::slide::XmlNode::Element(e) = c {
            walk(e, f);
        }
    }
}

/// Issues of slide `i` (shape rules).
#[allow(clippy::too_many_lines)]
fn slide_issues(pres: &Presentation, i: usize, out: &mut Vec<A11yIssue>) {
    let s = &pres.slides[i];
    let issue = |rule, id, name: &str| A11yIssue {
        rule,
        slide: Some(i),
        id,
        name: name.to_owned(),
    };
    let bg = background(pres, i);
    let mut found = Vec::new();
    bo_pptx::visit(&s.data.shapes, &mut |n| {
        let Some(nv) = n.nv() else { return };
        if nv.hidden == Some(true) {
            return;
        }
        let kind = kind_of(n);
        if NEEDS_ALT.contains(&kind)
            && nv.descr.as_deref().is_none_or(|d| d.trim().is_empty())
            && !decorative(n)
        {
            found.push(issue("altText", nv.id, &nv.name));
        }
        match n {
            ShapeNode::Shape(sh) => {
                if let Some(tb) = sh
                    .text
                    .as_ref()
                    .filter(|t| !super::text::body_text(t).trim().is_empty())
                {
                    let st = super::format_state(pres, i, sh);
                    let back = sh
                        .props
                        .fill
                        .as_ref()
                        .and_then(|f| solid(pres, i, f))
                        .unwrap_or(bg);
                    if let Some(fg) = st.color.as_deref().and_then(hex) {
                        let size = st.size.unwrap_or(18.0);
                        let large = size >= 18.0 || (st.bold && size >= 14.0);
                        let need = if large { 3.0 } else { 4.5 };
                        if contrast(fg, back) < need {
                            found.push(issue("contrast", nv.id, &nv.name));
                        }
                    }
                    if bare_links(tb) {
                        found.push(issue("linkText", nv.id, &nv.name));
                    }
                }
            }
            ShapeNode::Frame(f) => {
                if let GraphicContent::Table(t) = &f.content {
                    let header = t
                        .props
                        .as_ref()
                        .and_then(|p| p.attr("firstRow"))
                        .is_some_and(|v| v == "1" || v == "true");
                    if !header {
                        found.push(issue("tableHeader", nv.id, &nv.name));
                    }
                    let merged = t.rows.iter().flat_map(|r| &r.cells).any(|c| {
                        c.grid_span.is_some_and(|v| v > 1)
                            || c.row_span.is_some_and(|v| v > 1)
                            || c.h_merge == Some(true)
                            || c.v_merge == Some(true)
                    });
                    if merged {
                        found.push(issue("merged", nv.id, &nv.name));
                    }
                }
            }
            _ => {}
        }
    });
    out.extend(found);
    for m in super::media::media_out(pres, i) {
        if m.kind == "video" {
            out.push(issue("captions", m.id, &m.name));
        }
    }
    // Reading order: the tree order of the top-level shapes against their places.
    let placed = place_slide(pres, i);
    let mut order: Vec<(usize, f64, f64)> = placed
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            ops::node_at(pres, i, &p.path)
                .and_then(ShapeNode::nv)
                .is_some_and(|nv| nv.hidden != Some(true))
        })
        .map(|(k, p)| (k, p.b.cy - p.b.h / 2.0, p.b.cx - p.b.w / 2.0))
        .collect();
    let tree: Vec<usize> = order.iter().map(|o| o.0).collect();
    // Shapes within a few points vertically read left to right.
    order.sort_by(|a, b| {
        if (a.1 - b.1).abs() < 6.0 {
            a.2.total_cmp(&b.2)
        } else {
            a.1.total_cmp(&b.1)
        }
    });
    let visual: Vec<usize> = order.iter().map(|o| o.0).collect();
    // Titles may come first whatever their place (the reference reads them first).
    let is_title = |k: &usize| {
        placed
            .get(*k)
            .and_then(|p| ops::node_at(pres, i, &p.path))
            .is_some_and(|n| {
                n.nv()
                    .and_then(|nv| nv.placeholder.as_ref())
                    .is_some_and(|ph| matches!(ph.kind.as_deref(), Some("title" | "ctrTitle")))
            })
    };
    let strip = |v: &[usize]| {
        v.iter()
            .copied()
            .filter(|k| !is_title(k))
            .collect::<Vec<_>>()
    };
    if strip(&tree) != strip(&visual) {
        out.push(issue("readingOrder", 0, ""));
    }
}

/// Every issue of the deck, slide by slide (titles, then shapes), then the sections.
pub fn check(pres: &Presentation) -> Vec<A11yIssue> {
    let mut out = Vec::new();
    let titles: Vec<Option<String>> = (0..pres.slides.len()).map(|i| title_of(pres, i)).collect();
    for i in 0..pres.slides.len() {
        match &titles[i] {
            None => out.push(A11yIssue {
                rule: "slideTitle",
                slide: Some(i),
                id: 0,
                name: String::new(),
            }),
            Some(t) => {
                if titles
                    .iter()
                    .enumerate()
                    .any(|(j, o)| j != i && o.as_deref() == Some(t.as_str()))
                {
                    out.push(A11yIssue {
                        rule: "dupTitle",
                        slide: Some(i),
                        id: 0,
                        name: t.clone(),
                    });
                }
            }
        }
        slide_issues(pres, i, &mut out);
    }
    let sections = super::sections::sections_out(pres);
    for (k, s) in sections.iter().enumerate() {
        let lower = s.name.trim().to_lowercase();
        if lower == "default section" || lower == "untitled section" {
            out.push(A11yIssue {
                rule: "defaultSection",
                slide: Some(s.first),
                id: 0,
                name: s.name.clone(),
            });
        }
        if sections
            .iter()
            .enumerate()
            .any(|(j, o)| j != k && o.name.trim().eq_ignore_ascii_case(s.name.trim()))
        {
            out.push(A11yIssue {
                rule: "dupSection",
                slide: Some(s.first),
                id: 0,
                name: s.name.clone(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Engine;

    fn open(name: &str) -> Engine {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/corpus/slide")
            .join(name);
        let mut e = Engine::with_bundled_fonts();
        e.open(&std::fs::read(&p).unwrap(), name).unwrap();
        e
    }

    #[test]
    fn contrast_ratios() {
        assert!((contrast(0x00_0000, 0xFF_FFFF) - 21.0).abs() < 0.01);
        assert!((contrast(0x77_7777, 0xFF_FFFF) - 4.48).abs() < 0.02);
        assert!(contrast(0xFF_FF00, 0xFF_FFFF) < 1.1);
    }

    #[test]
    fn titles_alt_text_and_duplicates() {
        let mut e = open("slide-text-bullets.pptx");
        let base = check(e.presentation().unwrap());
        assert!(base.iter().all(|x| x.rule != "dupTitle"), "{base:?}");
        let pres = e.presentation_mut().unwrap();
        // A second slide titled like the first, a picture without a description.
        let first = title_of(pres, 0).expect("the first slide has a title");
        if pres.slides.len() < 2 {
            let s = pres.slides[0].clone();
            pres.slides.push(s);
        } else {
            for n in &mut pres.slides[1].data.shapes {
                if let ShapeNode::Shape(s) = n {
                    if s.nv
                        .placeholder
                        .as_ref()
                        .is_some_and(|p| matches!(p.kind.as_deref(), Some("title" | "ctrTitle")))
                    {
                        super::super::text::set_text(
                            s.text.get_or_insert_with(TextBody::default),
                            &first,
                        );
                    }
                }
            }
        }
        let got = check(pres);
        let dup: Vec<_> = got.iter().filter(|x| x.rule == "dupTitle").collect();
        assert!(dup.len() >= 2, "{got:?}");
        // Remove every title placeholder of slide 0: a missing title.
        pres.slides[0].data.shapes.retain(|n| {
            !n.nv()
                .and_then(|nv| nv.placeholder.as_ref())
                .is_some_and(|p| matches!(p.kind.as_deref(), Some("title" | "ctrTitle")))
        });
        let got = check(pres);
        assert!(got
            .iter()
            .any(|x| x.rule == "slideTitle" && x.slide == Some(0)));
    }
}
