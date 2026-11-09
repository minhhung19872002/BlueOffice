//! BlueSlide checks over the PresentationML model (ECMA-376 Part 1, §19):
//! inspector categories, accessibility rules and compatibility features, plus
//! what the model-level removals take away.

use super::a11y;
use super::compat::{self, BOTH, ODF};
use super::package::{self, PackageScan, Scrub};
use super::{
    looks_like_url, sample, Action, Category, Feature, Finding, FixKind, Issue, Report, Rule,
    Severity, Target,
};
use bo_model::slide::{
    ColorKind, Fill, GraphicContent, NonVisual, Presentation, ShapeNode, Slide, XmlElement,
};
use std::collections::BTreeMap;

/// Default text size of slide text when the run does not set one (18 pt).
const DEFAULT_SIZE: f64 = 18.0;

/// Package scan of the parts the deck keeps verbatim.
pub fn scan(pres: &Presentation) -> PackageScan {
    package::scan(
        pres.parts
            .iter()
            .map(|p| (p.name.as_str(), p.data.as_slice())),
    )
}

/// Inspector, accessibility and compatibility report of a deck.
pub fn report(pres: &Presentation, pending: &Scrub) -> Report {
    let pkg = scan(pres);
    Report::new(
        inspector(pres, &pkg),
        accessibility(pres, &pkg),
        compatibility(pres, &pkg),
        pending,
    )
}

/// `(x, y, w, h)` of a top-level node in EMU.
fn node_box(n: &ShapeNode) -> Option<(i64, i64, i64, i64)> {
    let x = match n {
        ShapeNode::Shape(s) => s.props.xfrm.as_ref(),
        ShapeNode::Picture(p) => p.props.xfrm.as_ref(),
        ShapeNode::Group(g) => g.props.xfrm.as_ref(),
        ShapeNode::Connector(c) => c.props.xfrm.as_ref(),
        ShapeNode::Frame(f) => Some(&f.xfrm),
        ShapeNode::Other(_) => None,
    }?;
    let (ox, oy) = x.off?;
    let (cx, cy) = x.ext.unwrap_or((0, 0));
    Some((ox, oy, cx, cy))
}

/// The node lies entirely outside the slide.
fn off_slide(pres: &Presentation, n: &ShapeNode) -> bool {
    let (w, h) = (pres.slide_size.cx, pres.slide_size.cy);
    node_box(n).is_some_and(|(x, y, cx, cy)| x + cx <= 0 || y + cy <= 0 || x >= w || y >= h)
}

fn invisible(n: &ShapeNode) -> bool {
    n.nv().is_some_and(|nv| nv.hidden == Some(true))
}

/// Date, footer and slide number placeholders (§19.7.10 `ST_PlaceholderType`).
fn footer_placeholder(n: &ShapeNode) -> bool {
    n.nv()
        .and_then(|nv| nv.placeholder.as_ref())
        .and_then(|p| p.kind.as_deref())
        .is_some_and(|k| matches!(k, "dt" | "ftr" | "sldNum"))
}

fn notes_text(s: &Slide) -> String {
    s.notes
        .as_ref()
        .map(bo_model::slide::Notes::text)
        .unwrap_or_default()
}

/// Indices of the top-level nodes of `slide` that the removal of `cats` takes away.
pub fn removable_nodes(pres: &Presentation, slide: usize, cats: &[Category]) -> Vec<usize> {
    let Some(s) = pres.slides.get(slide) else {
        return Vec::new();
    };
    s.data
        .shapes
        .iter()
        .enumerate()
        .filter(|(_, n)| {
            (cats.contains(&Category::OffSlide) && off_slide(pres, n))
                || (cats.contains(&Category::InvisibleObjects) && invisible(n))
                || (cats.contains(&Category::HeadersFooters) && footer_placeholder(n))
        })
        .map(|(i, _)| i)
        .collect()
}

/// Slides left out of the slide show (`sld/@show = 0`).
pub fn hidden_slides(pres: &Presentation) -> Vec<usize> {
    pres.slides
        .iter()
        .enumerate()
        .filter(|(_, s)| s.show == Some(false))
        .map(|(i, _)| i)
        .collect()
}

/// Slides with speaker notes text.
pub fn slides_with_notes(pres: &Presentation) -> Vec<usize> {
    pres.slides
        .iter()
        .enumerate()
        .filter(|(_, s)| !notes_text(s).trim().is_empty())
        .map(|(i, _)| i)
        .collect()
}

fn title_of(s: &Slide) -> Option<(usize, String)> {
    s.data.shapes.iter().enumerate().find_map(|(i, n)| match n {
        ShapeNode::Shape(sh)
            if sh
                .nv
                .placeholder
                .as_ref()
                .and_then(|p| p.kind.as_deref())
                .is_some_and(|k| k == "title" || k == "ctrTitle") =>
        {
            Some((
                i,
                sh.text
                    .as_ref()
                    .map(bo_model::slide::TextBody::text)
                    .unwrap_or_default(),
            ))
        }
        _ => None,
    })
}

fn inspector(pres: &Presentation, pkg: &PackageScan) -> Vec<Finding> {
    let all = [
        Category::OffSlide,
        Category::InvisibleObjects,
        Category::HeadersFooters,
    ];
    let mut counts = [0usize; 3];
    for i in 0..pres.slides.len() {
        for (k, c) in all.iter().enumerate() {
            counts[k] += removable_nodes(pres, i, &[*c]).len();
        }
    }
    let hidden = hidden_slides(pres);
    let notes = slides_with_notes(pres);
    let mut out = vec![
        Finding::new(Category::Comments, pkg.deck_comments, Action::OnSave)
            .with_details(pkg.deck_comment_authors.iter().cloned()),
    ];
    out.extend([
        Finding::new(Category::Properties, pkg.properties.len(), Action::OnSave).with_details(
            pkg.properties
                .iter()
                .map(|(k, v)| format!("{k}: {}", sample(v))),
        ),
        Finding::new(
            Category::CustomProperties,
            pkg.custom_properties,
            Action::OnSave,
        )
        .with_details(pkg.custom_names.iter().cloned()),
        Finding::new(Category::CustomXml, pkg.custom_xml, Action::OnSave),
        Finding::new(
            Category::ServerProperties,
            pkg.server_properties,
            Action::OnSave,
        ),
        Finding::new(Category::Macros, pkg.macros, Action::OnSave),
        Finding::new(Category::HiddenSlides, hidden.len(), Action::Remove)
            .with_details(hidden.iter().map(|i| (i + 1).to_string())),
        Finding::new(Category::OffSlide, counts[0], Action::Remove),
        Finding::new(Category::Notes, notes.len(), Action::Remove)
            .with_details(notes.iter().map(|i| (i + 1).to_string())),
        Finding::new(Category::InvisibleObjects, counts[1], Action::Remove),
        Finding::new(Category::HeadersFooters, counts[2], Action::Remove),
        Finding::new(Category::Embedded, pkg.embedded, Action::None)
            .with_details(pkg.embedded_names.iter().cloned()),
    ]);
    out
}

fn decorative(nv: &NonVisual) -> bool {
    fn find(e: &XmlElement) -> bool {
        (e.local() == "decorative" && e.attr("val").is_some_and(|v| v == "1" || v == "true"))
            || e.elements().any(find)
    }
    nv.cnv_extra.iter().any(|x| find(&x.xml))
}

fn srgb(f: Option<&Fill>) -> Option<(u8, u8, u8)> {
    match f? {
        Fill::Solid(c) if c.mods.is_empty() => match c.kind {
            ColorKind::Srgb(v) => Some(a11y::rgb_of(v)),
            _ => None,
        },
        _ => None,
    }
}

fn slide_bg(s: &Slide) -> Option<(u8, u8, u8)> {
    match s.data.background.as_ref()? {
        bo_model::slide::Background::Props { fill, .. } => srgb(Some(fill)),
        bo_model::slide::Background::Ref { .. } => None,
    }
}

fn table_first_row(props: Option<&XmlElement>) -> bool {
    props
        .and_then(|p| p.attr("firstRow"))
        .is_some_and(|v| v == "1" || v == "true")
}

fn has_link(props: &bo_model::slide::RunProps) -> bool {
    props.extra.iter().any(|x| x.xml.local() == "hlinkClick")
}

fn accessibility(pres: &Presentation, pkg: &PackageScan) -> Vec<Issue> {
    let mut out = Vec::new();
    let mut titles: BTreeMap<String, usize> = BTreeMap::new();
    for (si, s) in pres.slides.iter().enumerate() {
        let title = title_of(s);
        match &title {
            Some((_, t)) if !t.trim().is_empty() => {
                let key = t
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .to_lowercase();
                if let Some(first) = titles.get(&key) {
                    out.push(Issue::new(
                        Rule::DuplicateTitle,
                        Severity::Warning,
                        Target::Slide { slide: si },
                        format!("{} · {}", first + 1, sample(t)),
                    ));
                } else {
                    titles.insert(key, si);
                }
            }
            _ => out.push(Issue::new(
                Rule::SlideTitle,
                Severity::Error,
                Target::Slide { slide: si },
                "",
            )),
        }
        if let Some((ti, t)) = &title {
            let first_visible = s.data.shapes.iter().position(|n| !invisible(n));
            if !t.trim().is_empty() && first_visible.is_some_and(|f| f < *ti) {
                out.push(Issue::new(
                    Rule::ReadingOrder,
                    Severity::Tip,
                    Target::Slide { slide: si },
                    sample(t),
                ));
            }
        }
        let bg = slide_bg(s);
        for n in &s.data.shapes {
            let Some(nv) = n.nv() else { continue };
            let target = Target::Shape {
                slide: si,
                id: nv.id,
            };
            let no_alt = nv.descr.as_deref().is_none_or(|d| d.trim().is_empty()) && !decorative(nv);
            match n {
                ShapeNode::Picture(_) | ShapeNode::Group(_) if no_alt => {
                    out.push(
                        Issue::new(Rule::AltText, Severity::Error, target, nv.name.clone())
                            .fix(FixKind::AltText),
                    );
                }
                ShapeNode::Frame(f) => match &f.content {
                    GraphicContent::Table(t) => {
                        if t.rows.len() >= 2 && !table_first_row(t.props.as_ref()) {
                            out.push(
                                Issue::new(
                                    Rule::TableHeader,
                                    Severity::Warning,
                                    target.clone(),
                                    nv.name.clone(),
                                )
                                .fix(FixKind::HeaderRow),
                            );
                        }
                        let grid: Vec<Vec<bool>> = t
                            .rows
                            .iter()
                            .map(|r| {
                                r.cells
                                    .iter()
                                    .map(|c| {
                                        c.text.as_ref().is_some_and(|b| !b.text().trim().is_empty())
                                    })
                                    .collect()
                            })
                            .collect();
                        if a11y::blank_line(&grid) {
                            out.push(Issue::new(
                                Rule::LayoutTable,
                                Severity::Tip,
                                target,
                                nv.name.clone(),
                            ));
                        }
                    }
                    GraphicContent::Other(_) if no_alt => {
                        out.push(
                            Issue::new(Rule::AltText, Severity::Error, target, nv.name.clone())
                                .fix(FixKind::AltText),
                        );
                    }
                    GraphicContent::Other(_) => {}
                },
                ShapeNode::Shape(sh) => {
                    let Some(body) = &sh.text else { continue };
                    let back = srgb(sh.props.fill.as_ref()).or(bg);
                    let mut flagged = false;
                    for p in &body.paragraphs {
                        for r in &p.runs {
                            let bo_model::slide::TextRun::Run { props, text } = r else {
                                continue;
                            };
                            if text.trim().is_empty() {
                                continue;
                            }
                            if has_link(props) && looks_like_url(text) {
                                out.push(Issue::new(
                                    Rule::LinkText,
                                    Severity::Warning,
                                    target.clone(),
                                    sample(text),
                                ));
                            }
                            if flagged {
                                continue;
                            }
                            if let (Some(fg), Some(bk)) = (srgb(props.fill.as_ref()), back) {
                                let size = props.sz.map_or(DEFAULT_SIZE, |s| f64::from(s) / 100.0);
                                if let Some(ratio) =
                                    a11y::low_contrast(fg, bk, size, props.b == Some(true))
                                {
                                    let mut i = Issue::new(
                                        Rule::Contrast,
                                        Severity::Warning,
                                        target.clone(),
                                        sample(text),
                                    );
                                    i.ratio = Some(ratio);
                                    out.push(i);
                                    flagged = true;
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    if pkg.title.is_empty() {
        out.push(Issue::new(
            Rule::DocumentTitle,
            Severity::Tip,
            Target::Document,
            "",
        ));
    }
    out
}

fn json_has(v: &impl serde::Serialize, needle: &str) -> bool {
    serde_json::to_string(v).is_ok_and(|s| s.contains(needle))
}

fn compatibility(pres: &Presentation, pkg: &PackageScan) -> Vec<super::CompatItem> {
    let (mut smart, mut charts, mut modern, mut tables, mut pictures, mut equations, mut models) =
        (0, 0, 0, 0, 0, 0, 0);
    let (mut transitions, mut morph, mut animations) = (0, 0, 0);
    for s in &pres.slides {
        for n in &s.data.shapes {
            match n {
                ShapeNode::Frame(f) => match &f.content {
                    GraphicContent::Table(_) => tables += 1,
                    GraphicContent::Other(_) if f.uri.contains("diagram") => smart += 1,
                    GraphicContent::Other(_) if f.uri.contains("chartex") => modern += 1,
                    GraphicContent::Other(_) if f.uri.contains("chart") => charts += 1,
                    GraphicContent::Other(_) => {}
                },
                ShapeNode::Picture(_) => pictures += 1,
                _ => {}
            }
            if json_has(n, "oMath") {
                equations += 1;
            }
            if json_has(n, "model3d") {
                models += 1;
            }
        }
        if s.transition.is_some() {
            transitions += 1;
        }
        if json_has(&s.transition, "orph") || json_has(&s.extra, "morph") {
            morph += 1;
        }
        if s.timing.is_some() {
            animations += 1;
        }
    }
    let sections: usize = pres
        .extra
        .iter()
        .map(|e| {
            fn count(e: &XmlElement) -> usize {
                usize::from(e.local() == "section") + e.elements().map(count).sum::<usize>()
            }
            count(&e.xml)
        })
        .sum();
    vec![
        compat::item(Feature::SmartArt, smart, BOTH),
        compat::item(Feature::ModernCharts, modern.max(pkg.chart_ex), BOTH),
        compat::item(Feature::Charts, charts, ODF),
        compat::item(Feature::Tables, tables, ODF),
        compat::item(Feature::Drawings, pictures, ODF),
        compat::item(Feature::Media, pkg.media, BOTH),
        compat::item(Feature::Equations, equations, BOTH),
        compat::item(Feature::Models3d, models, BOTH),
        compat::item(Feature::Transitions, transitions, ODF),
        compat::item(Feature::Morph, morph, BOTH),
        compat::item(Feature::Animations, animations, ODF),
        compat::item(Feature::Sections, sections, BOTH),
        compat::item(Feature::CommentThreads, pkg.deck_comments, BOTH),
    ]
}

/// `tblPr` with `firstRow="1"` (§21.1.3.15: the first row is a header row).
pub fn with_first_row(props: Option<&XmlElement>) -> XmlElement {
    let mut p = props.cloned().unwrap_or_else(|| XmlElement::new("a:tblPr"));
    match p.attrs.iter_mut().find(|(k, _)| k == "firstRow") {
        Some(a) => "1".clone_into(&mut a.1),
        None => p.attrs.push(("firstRow".into(), "1".into())),
    }
    p
}
