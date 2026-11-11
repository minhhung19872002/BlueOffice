//! Header and footer of slides (Insert › Header & Footer): the date, footer and slide number
//! placeholders of a slide (ECMA-376 Part 1, §19.3.1.36 ph `@type` `dt` / `ftr` / `sldNum`,
//! §19.7.10 ST_PlaceholderType). A slide shows one of them only when it has its own placeholder
//! of that type; position and formatting come from the matching placeholder of its layout. The
//! slide number is a `slidenum` field and an automatic date a `datetime1` field (§21.1.2.2.4
//! fld); a fixed date is plain text.

use super::ops::Tx;
use super::{build, next_shape_id, refuse, SlideSel, R};
use bo_model::slide::{Presentation, RunProps, ShapeNode, TextRun};
use serde::{Deserialize, Serialize};

/// The three kinds handled, with the name given to a new placeholder.
const KINDS: [(&str, &str); 3] = [
    ("dt", "Date Placeholder"),
    ("ftr", "Footer Placeholder"),
    ("sldNum", "Slide Number Placeholder"),
];

/// Date of the header and footer.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HfDate {
    /// Text shown (the current date as the user's locale writes it, or the fixed text).
    pub text: String,
    /// Updated when the deck is opened (`datetime1` field) rather than fixed.
    #[serde(default)]
    pub auto: bool,
}

/// What the header and footer of slides show.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HfSpec {
    /// Date (absent: none).
    #[serde(default)]
    pub date: Option<HfDate>,
    /// Slide number.
    #[serde(default)]
    pub slide_number: bool,
    /// Footer text (absent: none).
    #[serde(default)]
    pub footer: Option<String>,
    /// Leave slides of the title slide layout without them.
    #[serde(default)]
    pub not_on_title: bool,
}

/// Header and footer as the current slide has them (the dialog's starting state).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HfInfo {
    /// Date, when the slide shows one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<HfDate>,
    /// The slide shows its number.
    pub slide_number: bool,
    /// Footer text, when the slide shows a footer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footer: Option<String>,
    /// Some slide shows a header / footer element while every title slide shows none.
    pub not_on_title: bool,
}

fn ph_kind(n: &ShapeNode) -> Option<&str> {
    n.nv()?.placeholder.as_ref()?.kind.as_deref()
}

fn is_title_slide(pres: &Presentation, i: usize) -> bool {
    pres.slides
        .get(i)
        .and_then(|s| pres.layouts.get(s.layout))
        .is_some_and(|l| l.kind.as_deref() == Some("title"))
}

fn has_any(pres: &Presentation, i: usize) -> bool {
    pres.slides.get(i).is_some_and(|s| {
        s.data
            .shapes
            .iter()
            .any(|n| ph_kind(n).is_some_and(|k| KINDS.iter().any(|(x, _)| *x == k)))
    })
}

/// The header and footer of the current slide.
pub fn info(pres: &Presentation, slide: usize) -> HfInfo {
    let mut out = HfInfo {
        date: None,
        slide_number: false,
        footer: None,
        not_on_title: false,
    };
    if let Some(s) = pres.slides.get(slide) {
        for n in &s.data.shapes {
            let ShapeNode::Shape(sh) = n else { continue };
            let text = sh.text.as_ref().map(bo_model::slide::TextBody::text);
            match ph_kind(n) {
                Some("dt") => {
                    let auto = sh.text.as_ref().is_some_and(|t| {
                        t.paragraphs.iter().flat_map(|p| &p.runs).any(|r| {
                            matches!(r, TextRun::Field { kind: Some(k), .. } if k.starts_with("datetime"))
                        })
                    });
                    out.date = Some(HfDate {
                        text: text.unwrap_or_default(),
                        auto,
                    });
                }
                Some("ftr") => out.footer = Some(text.unwrap_or_default()),
                Some("sldNum") => out.slide_number = true,
                _ => {}
            }
        }
    }
    let n = pres.slides.len();
    let titles: Vec<usize> = (0..n).filter(|&i| is_title_slide(pres, i)).collect();
    out.not_on_title = !titles.is_empty()
        && titles.iter().all(|&i| !has_any(pres, i))
        && (0..n).any(|i| !titles.contains(&i) && has_any(pres, i));
    out
}

/// A field run (§21.1.2.2.4 fld) of `kind` with its cached text.
fn field(kind: &str, slide: usize, text: &str) -> TextRun {
    // §21.1.2.2.4 fld@id: a GUID; derived from the slide and the field kind.
    let tag = kind
        .bytes()
        .fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)));
    TextRun::Field {
        id: format!("{{{tag:08X}-B0F1-4E5A-9C3D-{slide:012X}}}"),
        kind: Some(kind.to_owned()),
        props: RunProps::default(),
        ppr: None,
        text: text.to_owned(),
    }
}

/// The runs of a placeholder of `kind` on slide `i`.
fn runs_for(kind: &str, i: usize, spec: &HfSpec) -> Vec<TextRun> {
    match kind {
        "dt" => match &spec.date {
            Some(d) if d.auto => vec![field("datetime1", i, &d.text)],
            Some(d) => vec![TextRun::Run {
                props: RunProps::default(),
                text: d.text.clone(),
            }],
            None => Vec::new(),
        },
        "ftr" => vec![TextRun::Run {
            props: RunProps::default(),
            text: spec.footer.clone().unwrap_or_default(),
        }],
        // The cached text is the number itself; it is drawn from the slide's position.
        _ => vec![field("slidenum", i, &(i + 1).to_string())],
    }
}

/// Set the text of a header / footer placeholder, keeping the formatting of its first run.
fn set_runs(n: &mut ShapeNode, runs: Vec<TextRun>) {
    let ShapeNode::Shape(sh) = n else { return };
    let tb = sh
        .text
        .get_or_insert_with(bo_model::slide::TextBody::default);
    let props = tb
        .paragraphs
        .iter()
        .flat_map(|p| &p.runs)
        .find_map(|r| match r {
            TextRun::Run { props, .. } | TextRun::Field { props, .. } => Some(props.clone()),
            _ => None,
        });
    let runs = runs
        .into_iter()
        .map(|mut r| {
            if let (Some(p), TextRun::Run { props, .. } | TextRun::Field { props, .. }) =
                (&props, &mut r)
            {
                p.clone_into(props);
            }
            r
        })
        .collect();
    if tb.paragraphs.is_empty() {
        tb.paragraphs.push(bo_model::slide::Paragraph::default());
    }
    tb.paragraphs.truncate(1);
    tb.paragraphs[0].runs = runs;
}

/// Apply the header and footer to `slides` (all of them when `all`, else the current one).
pub fn cmd_header_footer(
    tx: &mut Tx<'_>,
    sel: &SlideSel,
    spec: &HfSpec,
    slides: Option<Vec<usize>>,
    all: bool,
) -> R<()> {
    let n = tx.pres.slides.len();
    let targets: Vec<usize> = if all {
        (0..n).collect()
    } else {
        slides.unwrap_or_else(|| vec![sel.slide])
    };
    if targets.is_empty() || targets.iter().any(|&i| i >= n) {
        return refuse("noSlide");
    }
    for i in targets {
        let title = is_title_slide(tx.pres, i);
        let layout = tx.pres.slides[i].layout;
        for (kind, label) in KINDS {
            let want = !(spec.not_on_title && title)
                && match kind {
                    "dt" => spec.date.is_some(),
                    "ftr" => spec.footer.is_some(),
                    _ => spec.slide_number,
                };
            let at = tx.pres.slides[i]
                .data
                .shapes
                .iter()
                .position(|s| ph_kind(s) == Some(kind));
            match (want, at) {
                (true, Some(k)) => {
                    let mut node = tx.pres.slides[i].data.shapes[k].clone();
                    set_runs(&mut node, runs_for(kind, i, spec));
                    tx.replace_node(i, &[k], node)?;
                }
                (true, None) => {
                    // Only where the layout has the placeholder to inherit the box from.
                    let Some(idx) = tx.pres.layouts.get(layout).and_then(|l| {
                        l.data.shapes.iter().find_map(|s| {
                            let ph = s.nv()?.placeholder.as_ref()?;
                            (ph.kind.as_deref() == Some(kind)).then_some(ph.idx)
                        })
                    }) else {
                        continue;
                    };
                    let id = next_shape_id(tx.pres, i);
                    let Some(mut node) =
                        build::placeholder(id, Some(kind), idx, &format!("{label} {}", id - 1))
                    else {
                        continue;
                    };
                    set_runs(&mut node, runs_for(kind, i, spec));
                    let len = tx.pres.slides[i].data.shapes.len();
                    tx.insert_nodes(i, &[], len, vec![node])?;
                }
                (false, Some(k)) => {
                    tx.remove_node(i, &[k])?;
                }
                (false, None) => {}
            }
        }
    }
    Ok(())
}
