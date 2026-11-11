//! Inline objects built from text: phonetic guides (ECMA-376 Part 1, §17.3.3.25 ruby)
//! and two lines in one (§17.3.2.10 eastAsianLayout `w:combine`).
//!
//! Both are laid out like equations: an inline box (placeholder picture of the box
//! width and its height above the baseline) whose drawing holds the glyph runs, with
//! the depth below the baseline returned separately.

use crate::drawing::Drawing;
use crate::paragraph::shape_run;
use crate::style::{run_style, slot_of, RunStyle};
use crate::types::{Fragment, GlyphPos, Line, LineSource};
use bo_model::slide::XmlElement;
use bo_model::{Color, Document, Emu, HalfPoints, Image, RunProps};
use bo_text::{FaceId, FontManager};
use std::sync::Arc;

/// A shaped line of text: fragments with x relative to the line start and y = 0 on
/// the baseline; its width, ascent and descent.
struct TextLine {
    frags: Vec<Fragment>,
    width: f32,
    ascent: f32,
    descent: f32,
}

fn face_for(st: &RunStyle, c: char) -> FaceId {
    st.faces[slot_of(c, st.hint_east_asia, st.complex) as usize]
}

/// Shape runs of (text, style) one after the other.
fn shape_line(fonts: &mut FontManager, runs: &[(String, RunStyle)]) -> TextLine {
    let mut out = TextLine {
        frags: Vec::new(),
        width: 0.0,
        ascent: 0.0,
        descent: 0.0,
    };
    for (text, st) in runs {
        let Some(first) = text.chars().next() else {
            continue;
        };
        let primary = face_for(st, first);
        for (r, face) in fonts.itemize(primary, text) {
            let t = &text[r];
            let glyphs = shape_run(fonts, face, t, st);
            let info = fonts.info(face);
            let k = st.size / f32::from(info.units_per_em);
            out.ascent = out.ascent.max(f32::from(info.ascender) * k);
            out.descent = out.descent.max(-f32::from(info.descender) * k);
            let mut gx = 0.0;
            let mut gs = Vec::with_capacity(glyphs.len());
            for g in glyphs.iter() {
                gs.push(GlyphPos {
                    id: g.glyph_id,
                    x: gx + g.x_offset as f32 * k,
                    y: -(g.y_offset as f32) * k,
                    cluster: g.cluster,
                });
                gx += g.x_advance as f32 * k + st.spacing;
            }
            out.frags.push(Fragment {
                face,
                size: st.size,
                x: out.width,
                y: 0.0,
                width: gx,
                color: st.color,
                underline: st.underline,
                strike: st.strike,
                double_strike: st.double_strike,
                text: t.to_owned().into(),
                glyphs: gs.into(),
                link: None,
                fx: st.fx.clone(),
            });
            out.width += gx;
        }
    }
    out
}

/// The inline box: placeholder picture (`w` × `asc`), drawing with the fragments moved
/// to (`dx`, `asc + dy`) per line, depth `desc`.
fn boxed(
    lines: Vec<(TextLine, f32, f32)>,
    w: f32,
    asc: f32,
    desc: f32,
    what: &str,
) -> (Image, Arc<Drawing>, f32) {
    let mut fragments = Vec::new();
    for (line, dx, dy) in lines {
        for mut f in line.frags {
            f.x += dx;
            f.y = asc + dy;
            fragments.push(f);
        }
    }
    let line = Line {
        top: 0.0,
        height: 0.0,
        baseline: asc,
        fragments,
        source: LineSource::Other,
        start: 0,
        end: 0,
        carets: crate::types::Carets::default(),
        rotation: None,
    };
    let emu = |pt: f32| Emu((pt * 12_700.0).round() as i64);
    let image = Image {
        media: String::new(),
        width: emu(w),
        height: emu(asc),
        anchor: None,
        crop: None,
        rotation: 0,
        description: what.into(),
        shape: None,
        src: None,
        format: None,
        ole: None,
    };
    (
        image,
        Arc::new(Drawing {
            items: Vec::new(),
            lines: vec![line],
        }),
        desc,
    )
}

fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn val(e: &XmlElement) -> Option<&str> {
    e.attrs
        .iter()
        .find(|(k, _)| local(k) == "val")
        .map(|(_, v)| v.as_str())
}

fn child<'e>(e: &'e XmlElement, name: &str) -> Option<&'e XmlElement> {
    e.elements().find(|c| local(&c.name) == name)
}

/// The formatting of a run inside a ruby that layout uses (§17.3.2.28 rPr subset:
/// fonts, size, bold, italic, colour).
fn inner_props(r: &XmlElement) -> RunProps {
    let mut p = RunProps::default();
    let Some(rpr) = child(r, "rPr") else {
        return p;
    };
    for e in rpr.elements() {
        let on = || !matches!(val(e), Some("0" | "false" | "off"));
        match local(&e.name) {
            "rFonts" => {
                for (k, v) in &e.attrs {
                    match local(k) {
                        "ascii" => p.font = Some(v.clone()),
                        "hAnsi" => p.font_h_ansi = Some(v.clone()),
                        "eastAsia" => p.font_east_asia = Some(v.clone()),
                        "cs" => p.font_cs = Some(v.clone()),
                        _ => {}
                    }
                }
            }
            "sz" => {
                p.size = val(e)
                    .and_then(|v| v.parse::<u32>().ok())
                    .map(|v| HalfPoints(v.clamp(1, 3276)));
            }
            "b" => p.bold = Some(on()),
            "i" => p.italic = Some(on()),
            "color" => p.color = val(e).and_then(Color::from_hex),
            _ => {}
        }
    }
    p
}

/// Text runs of a `w:rt` / `w:rubyBase` (§17.3.3.24, §17.3.3.27): (text, direct props).
fn runs_of(container: &XmlElement) -> Vec<(String, RunProps)> {
    container
        .elements()
        .filter(|r| local(&r.name) == "r")
        .map(|r| {
            let text: String = r
                .elements()
                .filter(|t| local(&t.name) == "t")
                .map(XmlElement::text)
                .collect();
            (text, inner_props(r))
        })
        .collect()
}

/// Phonetic guide (§17.3.3.25 ruby): the guide text (§17.3.3.24 rt) above the base text
/// (§17.3.3.27 rubyBase), both centred on the wider of the two unless `w:rubyAlign`
/// (§17.3.3.26) says left or right. The guide baseline is `w:hpsRaise` half-points
/// (§17.3.3.12) above the base baseline, or just clears the base text when absent;
/// sizes default to `w:hps` / `w:hpsBaseText` (§17.3.3.11, §17.3.3.13).
pub(crate) fn ruby_object(
    xml: &str,
    outer: &RunProps,
    doc: &Document,
    fonts: &mut FontManager,
) -> Option<(Image, Arc<Drawing>, f32)> {
    let el = bo_drawingml::dom::parse(xml.as_bytes(), "/word/document.xml").ok()?;
    let pr = child(&el, "rubyPr");
    let pr_val = |name: &str| pr.and_then(|p| child(p, name)).and_then(val);
    let hps = |name: &str| {
        pr_val(name)
            .and_then(|v| v.parse::<u32>().ok())
            .map(|v| HalfPoints(v.clamp(1, 3276)))
    };
    let mut styled = |container: Option<&XmlElement>, size: Option<HalfPoints>| {
        let mut v = Vec::new();
        for (text, mut direct) in container.map(runs_of).unwrap_or_default() {
            if direct.size.is_none() {
                direct.size = size;
            }
            if let Some(st) = run_style(doc, fonts, &direct.inherit(outer)) {
                v.push((text, st));
            }
        }
        v
    };
    let outer_size = outer.size.unwrap_or(HalfPoints(20));
    let guide_runs = styled(
        child(&el, "rt"),
        Some(hps("hps").unwrap_or(HalfPoints((outer_size.0 / 2).max(1)))),
    );
    let base_runs = styled(child(&el, "rubyBase"), hps("hpsBaseText"));
    let guide = shape_line(fonts, &guide_runs);
    let base = shape_line(fonts, &base_runs);
    let w = guide.width.max(base.width);
    let place = |lw: f32| match pr_val("rubyAlign") {
        Some("left") => 0.0,
        Some("right") => w - lw,
        _ => (w - lw) / 2.0,
    };
    let raise = hps("hpsRaise").map_or(base.ascent + guide.descent, HalfPoints::to_pt);
    let asc = base.ascent.max(raise + guide.ascent);
    let desc = base.descent;
    let (gx, bx) = (place(guide.width), place(base.width));
    Some(boxed(
        vec![(base, bx, 0.0), (guide, gx, -raise)],
        w,
        asc,
        desc,
        "ruby",
    ))
}

/// Two lines in one (§17.3.2.10 eastAsianLayout `w:combine`): the text split in two
/// halves set at half size, one above the other within the run's own line height
/// (the halves tile ascent + descent), optionally between full-size brackets
/// (§17.18.8 `ST_CombineBrackets`).
pub(crate) fn combine_object(
    text: &str,
    st: &RunStyle,
    fonts: &mut FontManager,
) -> Option<(Image, Arc<Drawing>, f32)> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return None;
    }
    let split = chars.len().div_ceil(2);
    let top: String = chars[..split].iter().collect();
    let bottom: String = chars[split..].iter().collect();
    let mut small = st.clone();
    small.size = st.size / 2.0;
    let face = face_for(st, chars[0]);
    let info = fonts.info(face);
    let k = st.size / f32::from(info.units_per_em);
    let (a, d) = (f32::from(info.ascender) * k, -f32::from(info.descender) * k);
    let t = shape_line(fonts, &[(top, small.clone())]);
    let b = shape_line(fonts, &[(bottom, small)]);
    let inner = t.width.max(b.width);
    // Top half: [-a, (d - a) / 2]; bottom half: [(d - a) / 2, d]; each holds a line at
    // half size whose ascent is a / 2 and descent d / 2.
    let (ty, by) = (-a + a / 2.0, d - d / 2.0);
    let mut lines = Vec::new();
    let mut x = 0.0;
    let brackets = st.combine_brackets;
    if let Some((open, _)) = brackets {
        let l = shape_line(fonts, &[(open.to_string(), st.clone())]);
        x = l.width;
        lines.push((l, 0.0, 0.0));
    }
    let (tw, bw) = (t.width, b.width);
    lines.push((t, x + (inner - tw) / 2.0, ty));
    lines.push((b, x + (inner - bw) / 2.0, by));
    x += inner;
    if let Some((_, close)) = brackets {
        let r = shape_line(fonts, &[(close.to_string(), st.clone())]);
        let w = r.width;
        lines.push((r, x, 0.0));
        x += w;
    }
    Some(boxed(lines, x, a, d, "combined characters"))
}
