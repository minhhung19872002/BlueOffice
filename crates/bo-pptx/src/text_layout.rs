//! Text body layout for slides: line breaking, bullets, spacing, columns,
//! vertical anchoring and autofit (ECMA-376 Part 1, §21.1.2).
//!
//! Output commands are in the text box's local coordinate system (points,
//! origin at the shape's top-left corner); the caller applies the shape transform.

use crate::inherit::TextStyleStack;
use bo_drawingml::color::{resolve, ColorContext};
use bo_drawingml::theme::resolve_typeface;
use bo_model::slide::{
    Autofit, BodyProps, Bullet, Fill, Line as LineProps, ParaProps, RunProps, Spacing, TextBody,
    TextRun, Theme,
};
use bo_render::{Command, Glyph, PathSeg, Rgba};
use bo_text::{FaceId, FontManager};
use std::sync::Arc;

const EMU_PT: f64 = 12_700.0;

/// Inputs shared by every text body of a slide.
pub(crate) struct TextEnv<'a> {
    pub theme: Option<&'a Theme>,
    pub colors: ColorContext<'a>,
    /// 1-based slide number for `slidenum` fields.
    pub slide_number: usize,
}

#[derive(Clone)]
struct Style {
    face: FaceId,
    size: f64,
    color: Rgba,
    underline: bool,
    strike: bool,
    baseline: f64,
    /// Gradient / pattern / picture fill or outline of the glyphs (WordArt, CORE-SHAPE-002).
    fx: Option<Arc<RunFx>>,
}

/// Text fill and outline beyond a plain colour (§21.1.2.3.9 rPr: `a:ln`, `a:gradFill`,
/// `a:pattFill`, `a:blipFill`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RunFx {
    /// Non-solid fill of the glyphs (`None`: the run colour, already on the path).
    pub fill: Option<Fill>,
    /// Glyph outline (`a:ln` with a visible fill).
    pub line: Option<LineProps>,
}

/// A run drawn as glyph outlines (a `Command::Path` at `index` of the output) whose fill
/// or outline the caller paints ([`RunFx`]).
#[derive(Debug, Clone)]
pub(crate) struct TextFx {
    /// Index of the placeholder path in the output commands.
    pub index: usize,
    /// What to paint.
    pub fx: Arc<RunFx>,
}

/// The [`RunFx`] of run properties, if the run needs more than a glyph colour.
fn run_fx(rp: &RunProps) -> Option<Arc<RunFx>> {
    let fill = match &rp.fill {
        Some(f @ (Fill::Gradient(_) | Fill::Pattern(_) | Fill::Picture(_))) => Some(f.clone()),
        _ => None,
    };
    let line = rp
        .line
        .as_ref()
        .filter(|l| l.fill.as_ref().is_some_and(|f| !matches!(f, Fill::None)))
        .cloned();
    (fill.is_some() || line.is_some()).then(|| Arc::new(RunFx { fill, line }))
}

/// Outline of positioned glyphs as one path (points, y down): run origin (`x`, `y`),
/// glyph offsets as in [`Command::Glyphs`].
pub(crate) fn glyph_path(
    fonts: &FontManager,
    face: FaceId,
    size: f64,
    (x, y): (f64, f64),
    glyphs: &[Glyph],
) -> Vec<PathSeg> {
    use bo_text::PathCmd;
    let upem = f64::from(fonts.info(face).units_per_em.max(1));
    let k = size / upem;
    let mut segs = Vec::new();
    for g in glyphs {
        let (ox, oy) = (x + f64::from(g.1), y + f64::from(g.2));
        let p = |px: f32, py: f32| {
            (
                (ox + f64::from(px) * k) as f32,
                (oy - f64::from(py) * k) as f32,
            )
        };
        for c in fonts.outline(face, g.0) {
            segs.push(match c {
                PathCmd::MoveTo(a, b) => {
                    let (a, b) = p(a, b);
                    PathSeg::M(a, b)
                }
                PathCmd::LineTo(a, b) => {
                    let (a, b) = p(a, b);
                    PathSeg::L(a, b)
                }
                PathCmd::QuadTo(a, b, c2, d) => {
                    let (a, b) = p(a, b);
                    let (c2, d) = p(c2, d);
                    PathSeg::Q(a, b, c2, d)
                }
                PathCmd::CubicTo(a, b, c2, d, e, f) => {
                    let (a, b) = p(a, b);
                    let (c2, d) = p(c2, d);
                    let (e, f) = p(e, f);
                    PathSeg::C(a, b, c2, d, e, f)
                }
                PathCmd::Close => PathSeg::Z,
            });
        }
    }
    segs
}

#[derive(Clone)]
struct Piece {
    text: String,
    style: Style,
    width: f64,
    /// Trailing whitespace width (not counted at line end).
    trailing: f64,
    /// Source range in the paragraph's editing text (characters; breaks count 1).
    src: (usize, usize),
    /// An equation (`a14:m`, [MS-ODRAWXML]) laid out by `bo-layout` instead of text.
    math: Option<std::sync::Arc<bo_layout::EquationGlyphs>>,
}

struct Line {
    pieces: Vec<Piece>,
    width: f64,
    descent: f64,
    height: f64,
    indent: f64,
    bullet: Option<Piece>,
    bullet_x: f64,
    align: String,
    avail: f64,
    space_before: f64,
    /// Ended by a line break (§21.1.2.2.1 br).
    forced: bool,
}

fn to_rgba(c: bo_drawingml::Rgba) -> Rgba {
    let [r, g, b, a] = c.to_u8();
    Rgba(r, g, b, a)
}

fn run_color(rp: &RunProps, env: &TextEnv<'_>) -> Rgba {
    match &rp.fill {
        Some(Fill::Solid(c)) => to_rgba(resolve(c, &env.colors)),
        Some(Fill::Gradient(g)) => g.stops.first().map_or(Rgba(0, 0, 0, 255), |s| {
            to_rgba(resolve(&s.color, &env.colors))
        }),
        Some(Fill::None) => Rgba(0, 0, 0, 0),
        _ => to_rgba(resolve(&bo_model::slide::Color::scheme("tx1"), &env.colors)),
    }
}

fn style_for(
    rp: &RunProps,
    scale: f64,
    env: &TextEnv<'_>,
    fonts: &mut FontManager,
) -> Option<Style> {
    let typeface = rp.latin.as_ref().map_or("+mn-lt", |f| f.typeface.as_str());
    let family = resolve_typeface(env.theme, typeface);
    let family = if family.is_empty() { "Calibri" } else { family };
    let bold = rp.b.unwrap_or(false);
    let italic = rp.i.unwrap_or(false);
    let face = fonts.resolve(family, bold, italic)?;
    let base = f64::from(rp.sz.unwrap_or(1800)) / 100.0;
    let baseline = f64::from(rp.baseline.unwrap_or(0)) / 100_000.0;
    // §21.1.2.3.9: super/subscript text is drawn smaller.
    let size = if baseline == 0.0 {
        base
    } else {
        base * 2.0 / 3.0
    } * scale;
    Some(Style {
        face,
        size: size.max(1.0),
        color: run_color(rp, env),
        underline: rp.u.as_deref().is_some_and(|u| u != "none"),
        strike: rp.strike.as_deref().is_some_and(|s| s != "noStrike"),
        baseline: baseline * base * scale,
        fx: run_fx(rp),
    })
}

fn apply_caps(rp: &RunProps, s: &str) -> String {
    match rp.cap.as_deref() {
        Some("all" | "small") => s.to_uppercase(),
        _ => s.to_owned(),
    }
}

fn metrics(fonts: &FontManager, st: &Style) -> (f64, f64) {
    let info = fonts.info(st.face);
    let upem = f64::from(info.units_per_em.max(1));
    let asc = f64::from(info.ascender) / upem * st.size;
    let desc = -f64::from(info.descender) / upem * st.size;
    (asc, desc.max(0.0))
}

fn measure(fonts: &mut FontManager, st: &Style, text: &str) -> f64 {
    let mut w = 0.0;
    for (range, face) in fonts.itemize(st.face, text) {
        w += f64::from(fonts.measure(face, &text[range], st.size as f32));
    }
    w
}

fn spacing(s: Option<Spacing>, size: f64) -> f64 {
    match s {
        Some(Spacing::Percent(p)) => f64::from(p) / 100_000.0 * size * 1.2,
        Some(Spacing::Points(p)) => f64::from(p) / 100.0,
        None => 0.0,
    }
}

/// §21.1.2.4.1 buAutoNum schemes (common subset).
fn auto_number(scheme: &str, n: i32) -> String {
    fn roman(mut n: i32) -> String {
        const T: [(i32, &str); 13] = [
            (1000, "m"),
            (900, "cm"),
            (500, "d"),
            (400, "cd"),
            (100, "c"),
            (90, "xc"),
            (50, "l"),
            (40, "xl"),
            (10, "x"),
            (9, "ix"),
            (5, "v"),
            (4, "iv"),
            (1, "i"),
        ];
        let mut s = String::new();
        for (v, r) in T {
            while n >= v {
                s.push_str(r);
                n -= v;
            }
        }
        s
    }
    fn alpha(n: i32) -> String {
        let mut n = n.max(1) - 1;
        let mut s = String::new();
        loop {
            s.insert(0, char::from(b'a' + (n % 26) as u8));
            n = n / 26 - 1;
            if n < 0 {
                break;
            }
        }
        s
    }
    let (body, rest) = if let Some(r) = scheme.strip_prefix("arabic") {
        (n.to_string(), r)
    } else if let Some(r) = scheme.strip_prefix("romanUc") {
        (roman(n).to_uppercase(), r)
    } else if let Some(r) = scheme.strip_prefix("romanLc") {
        (roman(n), r)
    } else if let Some(r) = scheme.strip_prefix("alphaUc") {
        (alpha(n).to_uppercase(), r)
    } else if let Some(r) = scheme.strip_prefix("alphaLc") {
        (alpha(n), r)
    } else {
        (n.to_string(), "Period")
    };
    match rest {
        "Period" => format!("{body}."),
        "ParenR" => format!("{body})"),
        "ParenBoth" => format!("({body})"),
        "Minus" => format!("{body} -"),
        _ => body,
    }
}

struct ParaLayout {
    lines: Vec<Line>,
    space_after: f64,
}

#[allow(clippy::too_many_arguments)]
fn layout_paragraph(
    para: &bo_model::slide::Paragraph,
    ppr: &ParaProps,
    width: f64,
    wrap: bool,
    scale: f64,
    ln_reduction: f64,
    auto_num: Option<i32>,
    env: &TextEnv<'_>,
    fonts: &mut FontManager,
    first: bool,
) -> ParaLayout {
    let mar_l = ppr.mar_l.unwrap_or(0) as f64 / EMU_PT;
    let indent = ppr.indent.unwrap_or(0) as f64 / EMU_PT;
    let mar_r = ppr.mar_r.unwrap_or(0) as f64 / EMU_PT;
    // Flatten runs into (text, style, forced break after, source range) spans.
    let mut spans: Vec<(String, Style, bool, (usize, usize))> = Vec::new();
    // Equations by span index (the span's text is U+FFFC).
    let mut maths: Vec<(usize, std::sync::Arc<bo_layout::EquationGlyphs>)> = Vec::new();
    let mut first_rp: Option<RunProps> = None;
    let mut src = 0usize;
    for r in &para.runs {
        match r {
            TextRun::Run { props, text: raw }
            | TextRun::Field {
                props, text: raw, ..
            } => {
                let rp = TextStyleStack::run(ppr, props);
                let text = match r {
                    TextRun::Field { kind: Some(k), .. } if k == "slidenum" => {
                        env.slide_number.to_string()
                    }
                    _ => apply_caps(&rp, raw),
                };
                let len = raw.chars().count();
                if first_rp.is_none() {
                    first_rp = Some(rp.clone());
                }
                if let Some(st) = style_for(&rp, scale, env, fonts) {
                    spans.push((bo_text::nfc(&text), st, false, (src, src + len)));
                }
                src += len;
            }
            TextRun::Break { props } => {
                let rp = TextStyleStack::run(ppr, props);
                if let Some(st) = style_for(&rp, scale, env, fonts) {
                    spans.push((String::new(), st, true, (src, src + 1)));
                }
                src += 1;
            }
            // [MS-ODRAWXML] a14:m: an Office Math zone inside the paragraph.
            TextRun::Other(el) if el.local() == "m" => {
                let rp =
                    TextStyleStack::run(ppr, first_rp.as_ref().unwrap_or(&RunProps::default()));
                let Some(st) = style_for(&rp, scale, env, fonts) else {
                    continue;
                };
                let Some(zone) = el
                    .elements()
                    .find(|e| e.local() == "oMathPara" || e.local() == "oMath")
                else {
                    continue;
                };
                let mut xml = String::new();
                bo_drawingml::dom::write_element(zone, &mut xml);
                if let Some(g) = bo_layout::layout_equation(&xml, st.size as f32, fonts, st.face) {
                    maths.push((spans.len(), std::sync::Arc::new(g)));
                    spans.push(("\u{FFFC}".to_owned(), st, false, (src, src)));
                }
            }
            TextRun::Other(_) => {}
        }
    }
    let end_rp = TextStyleStack::run(ppr, para.end_props.as_ref().unwrap_or(&RunProps::default()));
    let end_style = style_for(first_rp.as_ref().unwrap_or(&end_rp), scale, env, fonts);
    let has_text = spans.iter().any(|(t, _, _, _)| !t.trim().is_empty());

    // Bullet (§21.1.2.4).
    let mut bullet: Option<Piece> = None;
    if has_text {
        let btext = match &ppr.bullet {
            Some(Bullet::Char(c)) => Some(c.clone()),
            Some(Bullet::AutoNum { scheme, .. }) => {
                Some(auto_number(scheme, auto_num.unwrap_or(1)))
            }
            _ => None,
        };
        if let (Some(bt), Some(base)) = (btext, first_rp.as_ref()) {
            let mut brp = base.clone();
            brp.u = None;
            brp.strike = None;
            if let Some(Some(f)) = &ppr.bu_font {
                brp.latin = Some(f.clone());
            }
            if let Some(Some(c)) = &ppr.bu_clr {
                brp.fill = Some(Fill::Solid(c.clone()));
            }
            let base_sz = base.sz.unwrap_or(1800);
            brp.sz = Some(match ppr.bu_sz {
                Some(Some(Spacing::Percent(p))) => {
                    (i64::from(base_sz) * i64::from(p) / 100_000) as i32
                }
                Some(Some(Spacing::Points(p))) => p,
                _ => base_sz,
            });
            if let Some(st) = style_for(&brp, scale, env, fonts) {
                let w = measure(fonts, &st, &bt);
                bullet = Some(Piece {
                    text: bt,
                    style: st,
                    width: w,
                    trailing: 0.0,
                    src: (0, 0),
                    math: None,
                });
            }
        }
    }

    // Break opportunities (UAX #14) over the whole paragraph text.
    let mut full = String::new();
    let mut span_starts = Vec::new();
    for (t, _, _, _) in &spans {
        span_starts.push(full.len());
        full.push_str(t);
    }
    let breaks: Vec<usize> = unicode_linebreak::linebreaks(&full)
        .map(|(i, _)| i)
        .collect();

    // Words: pieces between break opportunities, split by span.
    let mut words: Vec<(Vec<Piece>, bool)> = Vec::new(); // bool = forced break after word
    let mut cur: Vec<Piece> = Vec::new();
    for (si, (t, st, forced, range)) in spans.iter().enumerate() {
        // An equation is a word of its own.
        if let Some((_, g)) = maths.iter().find(|(k, _)| *k == si) {
            if !cur.is_empty() {
                words.push((std::mem::take(&mut cur), false));
            }
            words.push((
                vec![Piece {
                    text: t.clone(),
                    style: st.clone(),
                    width: f64::from(g.w),
                    trailing: 0.0,
                    src: *range,
                    math: Some(g.clone()),
                }],
                false,
            ));
            continue;
        }
        let start = span_starts[si];
        let mut pos = 0;
        let map = SrcMap::new(t, *range);
        for &b in breaks.iter().filter(|&&b| b > start && b < start + t.len()) {
            let seg = &t[pos..b - start];
            push_piece(&mut cur, seg, st, fonts, map.range(pos, b - start));
            words.push((std::mem::take(&mut cur), false));
            pos = b - start;
        }
        push_piece(&mut cur, &t[pos..], st, fonts, map.range(pos, t.len()));
        if *forced {
            words.push((std::mem::take(&mut cur), true));
        }
    }
    if !cur.is_empty() {
        words.push((cur, false));
    }

    let algn = ppr.algn.clone().unwrap_or_else(|| "l".into());
    let pct = match ppr.ln_spc {
        Some(Spacing::Percent(p)) => Some((f64::from(p) / 100_000.0 - ln_reduction).max(0.1)),
        Some(Spacing::Points(_)) => None,
        None => Some((1.0 - ln_reduction).max(0.1)),
    };
    let first_size = spans
        .first()
        .map(|s| s.1.size)
        .or(end_style.as_ref().map(|s| s.size))
        .unwrap_or(18.0);
    let space_before = if first {
        0.0
    } else {
        spacing(ppr.spc_bef, first_size)
    };
    let space_after = spacing(ppr.spc_aft, first_size);

    let mut lines: Vec<Line> = Vec::new();
    let mut line_pieces: Vec<Piece> = Vec::new();
    let mut line_w = 0.0;
    let new_line_avail = |n: usize| {
        let start = if n == 0 {
            if bullet.is_some() {
                mar_l.max(mar_l + indent + bullet.as_ref().map_or(0.0, |b| b.width))
            } else {
                mar_l + indent
            }
        } else {
            mar_l
        };
        (start, (width - start - mar_r).max(1.0))
    };
    let finish = |pieces: Vec<Piece>, lines: &mut Vec<Line>, fonts: &FontManager, forced: bool| {
        let n = lines.len();
        let (start, avail) = new_line_avail(n);
        let mut asc: f64 = 0.0;
        let mut desc: f64 = 0.0;
        for p in &pieces {
            let (a, d) = match &p.math {
                Some(g) => (f64::from(g.asc), f64::from(g.desc)),
                None => metrics(fonts, &p.style),
            };
            asc = asc.max(a);
            desc = desc.max(d);
        }
        if pieces.is_empty() || pieces.iter().all(|p| p.text.is_empty()) {
            if let Some(es) = &end_style {
                let (a, d) = metrics(fonts, es);
                asc = asc.max(a);
                desc = desc.max(d);
            }
        }
        let natural = asc + desc;
        let height = match (pct, ppr.ln_spc) {
            (Some(p), _) => natural * p,
            (None, Some(Spacing::Points(v))) => f64::from(v) / 100.0,
            _ => natural,
        };
        let width: f64 =
            pieces.iter().map(|p| p.width).sum::<f64>() - pieces.last().map_or(0.0, |p| p.trailing);
        lines.push(Line {
            pieces,
            width,
            descent: desc,
            height,
            indent: start,
            bullet: None,
            bullet_x: mar_l + indent,
            align: String::new(),
            avail,
            space_before: 0.0,
            forced,
        });
    };
    for (word, forced) in words {
        let ww: f64 = word.iter().map(|p| p.width).sum();
        let wtrail = word.last().map_or(0.0, |p| p.trailing);
        let (_, avail) = new_line_avail(lines.len());
        if wrap && !line_pieces.is_empty() && line_w + ww - wtrail > avail {
            finish(std::mem::take(&mut line_pieces), &mut lines, fonts, false);
            line_w = 0.0;
        }
        line_w += ww;
        line_pieces.extend(word);
        if forced {
            finish(std::mem::take(&mut line_pieces), &mut lines, fonts, true);
            line_w = 0.0;
        }
    }
    if !line_pieces.is_empty() || lines.is_empty() {
        finish(line_pieces, &mut lines, fonts, false);
    }
    if let Some(l) = lines.first_mut() {
        l.bullet = bullet;
        l.space_before = space_before;
    }
    for l in &mut lines {
        l.align.clone_from(&algn);
    }
    ParaLayout { lines, space_after }
}

/// Maps byte offsets of a span's displayed (normalised) text to characters of
/// its source run; proportional when normalisation changed the length.
struct SrcMap {
    bytes: Vec<usize>,
    range: (usize, usize),
}

impl SrcMap {
    fn new(text: &str, range: (usize, usize)) -> SrcMap {
        let mut bytes: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
        bytes.push(text.len());
        SrcMap { bytes, range }
    }
    /// Source character offset of byte `b`.
    fn at(&self, b: usize) -> usize {
        let ci = self.bytes.partition_point(|&x| x < b);
        let n = self.bytes.len() - 1;
        let len = self.range.1 - self.range.0;
        if n == len {
            self.range.0 + ci
        } else {
            self.range.0 + (ci * len + n / 2).checked_div(n).unwrap_or(0)
        }
    }
    fn range(&self, a: usize, b: usize) -> (usize, usize) {
        (self.at(a), self.at(b))
    }
}

fn push_piece(
    cur: &mut Vec<Piece>,
    seg: &str,
    st: &Style,
    fonts: &mut FontManager,
    src: (usize, usize),
) {
    if seg.is_empty() {
        return;
    }
    let width = measure(fonts, st, seg);
    let trimmed = seg.trim_end();
    let trailing = if trimmed.len() == seg.len() {
        0.0
    } else {
        width - measure(fonts, st, trimmed)
    };
    cur.push(Piece {
        text: seg.to_owned(),
        style: st.clone(),
        width,
        trailing,
        src,
        math: None,
    });
}

/// A line placed in the text box.
struct PlacedLine {
    para: usize,
    line: usize,
    top: f64,
    baseline: f64,
    /// Column origin.
    x0: f64,
    /// Start of the first piece (after alignment).
    x_start: f64,
}

/// A laid-out text body (shared by drawing and caret geometry).
struct Arranged {
    layout: Vec<ParaLayout>,
    lines: Vec<PlacedLine>,
    /// Height of the text (all columns' tallest) plus the top and bottom insets.
    content_h: f64,
    scale: f64,
    reduction: f64,
}

#[allow(clippy::too_many_lines)]
fn arrange(
    body: &TextBody,
    bp: &BodyProps,
    stack: &TextStyleStack,
    rect: [f64; 4],
    env: &TextEnv<'_>,
    fonts: &mut FontManager,
    from_full_scale: bool,
) -> Arranged {
    // §21.1.2.1.1 bodyPr insets (defaults 0.1" left/right, 0.05" top/bottom).
    let t_ins = bp.t_ins.unwrap_or(45_720) as f64 / EMU_PT;
    let b_ins = bp.b_ins.unwrap_or(45_720) as f64 / EMU_PT;
    let l = rect[0] + bp.l_ins.unwrap_or(91_440) as f64 / EMU_PT;
    let t = rect[1] + t_ins;
    let r = rect[2] - bp.r_ins.unwrap_or(91_440) as f64 / EMU_PT;
    let b = rect[3] - b_ins;
    let ncol = bp.num_col.unwrap_or(1).clamp(1, 16) as usize;
    let spc_col = bp.spc_col.unwrap_or(0) as f64 / EMU_PT;
    let col_w = ((r - l) - spc_col * (ncol - 1) as f64) / ncol as f64;
    let wrap = bp.wrap.as_deref() != Some("none");
    let box_h = (b - t).max(0.0);

    let (mut scale, mut reduction, shrink) = match &bp.autofit {
        Some(Autofit::Normal { .. }) if from_full_scale => (1.0, 0.0, true),
        Some(Autofit::Normal {
            font_scale,
            ln_spc_reduction,
        }) => (
            f64::from(font_scale.unwrap_or(100_000)) / 100_000.0,
            f64::from(ln_spc_reduction.unwrap_or(0)) / 100_000.0,
            true,
        ),
        _ => (1.0, 0.0, false),
    };

    let mut layout = layout_all(body, stack, col_w, wrap, scale, reduction, env, fonts);
    // §21.1.2.1.3 normAutofit: shrink further while the text overflows.
    if shrink {
        let mut guard = 0;
        while columns_needed(&layout, box_h) > ncol && guard < 12 {
            scale *= 0.9;
            reduction = (reduction + 0.05).min(0.2);
            layout = layout_all(body, stack, col_w, wrap, scale, reduction, env, fonts);
            guard += 1;
        }
    }

    // Distribute lines into columns: (column, y, para, line).
    let mut cols: Vec<Vec<(f64, usize, usize)>> = vec![Vec::new()];
    let mut y = 0.0;
    for (pi, para) in layout.iter().enumerate() {
        for (li, line) in para.lines.iter().enumerate() {
            let before = if li == 0 { line.space_before } else { 0.0 };
            if ncol > 1 && y + before + line.height > box_h && y > 0.0 && cols.len() < ncol {
                cols.push(Vec::new());
                y = 0.0;
            } else {
                y += before;
            }
            if let Some(c) = cols.last_mut() {
                c.push((y, pi, li));
            }
            y += line.height;
        }
        if pi + 1 < layout.len() {
            y += para.space_after;
        }
    }
    let content_h = |c: &Vec<(f64, usize, usize)>| {
        c.last()
            .map_or(0.0, |(y, pi, li)| y + layout[*pi].lines[*li].height)
    };
    let tallest = cols.iter().map(content_h).fold(0.0, f64::max);
    // §21.1.2.1.1 anchor
    let dy = match bp.anchor.as_deref() {
        Some("ctr") => (box_h - tallest) / 2.0,
        Some("b") => box_h - tallest,
        _ => 0.0,
    };
    let mut lines = Vec::new();
    for (ci, col) in cols.iter().enumerate() {
        let x0 = l + ci as f64 * (col_w + spc_col);
        for &(ly, pi, li) in col {
            let line = &layout[pi].lines[li];
            let top = t + dy + ly;
            let baseline = top + line.height - line.descent;
            // Unwrapped text (wrap="none") aligns within the box, not the unbounded line.
            let avail = if wrap {
                line.avail
            } else {
                (col_w - line.indent).max(0.0)
            };
            let extra = (avail - line.width).max(0.0);
            let x_start = x0
                + line.indent
                + match line.align.as_str() {
                    "ctr" => extra / 2.0,
                    "r" => extra,
                    _ => 0.0,
                };
            lines.push(PlacedLine {
                para: pi,
                line: li,
                top,
                baseline,
                x0,
                x_start,
            });
        }
    }
    Arranged {
        layout,
        lines,
        content_h: tallest + t_ins + b_ins,
        scale,
        reduction,
    }
}

/// Lay out and draw a text body inside `rect` = [l, t, r, b] (points, local).
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_text(
    body: &TextBody,
    bp: &BodyProps,
    stack: &TextStyleStack,
    rect: [f64; 4],
    env: &TextEnv<'_>,
    fonts: &mut FontManager,
    out: &mut Vec<Command>,
) {
    draw_text_fx(body, bp, stack, rect, env, fonts, out);
}

/// [`draw_text`], also returning the runs drawn as outlines whose fill / outline the
/// caller paints (until then they show in their run colour).
pub(crate) fn draw_text_fx(
    body: &TextBody,
    bp: &BodyProps,
    stack: &TextStyleStack,
    rect: [f64; 4],
    env: &TextEnv<'_>,
    fonts: &mut FontManager,
    out: &mut Vec<Command>,
) -> Vec<TextFx> {
    let mut fx = Vec::new();
    // Nothing to draw: no text and no equation (a14:m) in any paragraph.
    let has_math = |p: &bo_model::slide::Paragraph| {
        p.runs
            .iter()
            .any(|r| matches!(r, TextRun::Other(el) if el.local() == "m"))
    };
    if body
        .paragraphs
        .iter()
        .all(|p| p.text().trim().is_empty() && !has_math(p))
    {
        return fx;
    }
    let a = arrange(body, bp, stack, rect, env, fonts, false);
    for pl in &a.lines {
        let line = &a.layout[pl.para].lines[pl.line];
        if let Some(bu) = &line.bullet {
            emit(fonts, bu, pl.x0 + line.bullet_x, pl.baseline, out, &mut fx);
        }
        let mut x = pl.x_start;
        for p in &line.pieces {
            emit(fonts, p, x, pl.baseline, out, &mut fx);
            x += p.width;
        }
    }
    fx
}

/// A line of a text body for caret placement: offsets are characters of the
/// editing text (paragraphs joined by `\n`, line breaks one character).
#[derive(Debug, Clone, PartialEq)]
pub struct CaretLine {
    /// Paragraph index within the text body.
    pub para: usize,
    /// Offsets `start..=end` covered by the line.
    pub start: usize,
    /// Last offset covered by the line (inclusive).
    pub end: usize,
    /// Top of the line, in points.
    pub top: f64,
    /// Line height, in points.
    pub height: f64,
    /// Baseline y, in points.
    pub baseline: f64,
    /// Caret x for every offset `start..=end`.
    pub xs: Vec<f64>,
}

/// Caret geometry of a text body (local points, like [`draw_text`]).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextGeometry {
    /// Laid-out lines in order.
    pub lines: Vec<CaretLine>,
    /// Text height plus top/bottom insets (what `spAutoFit` sizes the box to).
    pub content_height: f64,
    /// Font scale and line-spacing reduction the box needs (normAutofit), from 100 %.
    pub font_scale: f64,
    /// Line spacing reduction as a fraction (0.2 = 20 %).
    pub ln_spc_reduction: f64,
}

impl TextGeometry {
    /// Line index and x of the caret at `offset` (a caret at a soft wrap goes
    /// to the start of the next line).
    pub fn caret(&self, offset: usize) -> Option<(usize, f64)> {
        let mut found = None;
        for (i, l) in self.lines.iter().enumerate() {
            if offset >= l.start && offset <= l.end {
                let x = l.xs.get(offset - l.start).copied().unwrap_or(0.0);
                found = Some((i, x));
                if offset < l.end {
                    break;
                }
            }
        }
        found.or_else(|| {
            self.lines
                .last()
                .map(|l| (self.lines.len() - 1, l.xs.last().copied().unwrap_or(0.0)))
        })
    }

    /// Offset nearest to a local point.
    pub fn offset_at(&self, x: f64, y: f64) -> usize {
        let Some(line) = self
            .lines
            .iter()
            .find(|l| y < l.top + l.height)
            .or(self.lines.last())
        else {
            return 0;
        };
        let mut best = (line.start, f64::MAX);
        for (i, cx) in line.xs.iter().enumerate() {
            let d = (cx - x).abs();
            if d < best.1 {
                best = (line.start + i, d);
            }
        }
        best.0
    }
}

/// Lay out a text body and return caret positions instead of drawing.
#[allow(clippy::too_many_arguments)]
pub fn text_geometry(
    body: &TextBody,
    bp: &BodyProps,
    stack: &TextStyleStack,
    rect: [f64; 4],
    env: &TextEnv<'_>,
    fonts: &mut FontManager,
) -> TextGeometry {
    let a = arrange(body, bp, stack, rect, env, fonts, true);
    // Global offset of each paragraph's first character.
    let mut base = Vec::with_capacity(body.paragraphs.len());
    let mut acc = 0;
    for p in &body.paragraphs {
        base.push(acc);
        acc += para_len(p) + 1;
    }
    // Start offset of each line within its paragraph.
    let mut starts: Vec<Vec<usize>> = Vec::with_capacity(a.layout.len());
    for para in &a.layout {
        let mut v = Vec::with_capacity(para.lines.len());
        let mut next = 0;
        for line in &para.lines {
            let start = line.pieces.first().map_or(next, |p| p.src.0);
            v.push(start);
            let end = line.pieces.last().map_or(start, |p| p.src.1);
            next = if line.forced { end + 1 } else { end };
        }
        starts.push(v);
    }
    let mut lines = Vec::with_capacity(a.lines.len());
    for pl in &a.lines {
        let line = &a.layout[pl.para].lines[pl.line];
        let start = starts[pl.para][pl.line];
        let end = line.pieces.last().map_or(start, |p| p.src.1).max(start);
        let mut xs = vec![pl.x_start; end - start + 1];
        let mut x = pl.x_start;
        for p in &line.pieces {
            let n = p.src.1.saturating_sub(p.src.0);
            let chars: Vec<(usize, char)> = p.text.char_indices().collect();
            for k in 0..=n {
                let off = p.src.0 + k;
                if off < start || off > end {
                    continue;
                }
                // Displayed characters for k source characters.
                let shown = (k * chars.len() + n / 2).checked_div(n).unwrap_or(0);
                let byte = chars.get(shown).map_or(p.text.len(), |c| c.0);
                let w = if shown >= chars.len() {
                    p.width
                } else {
                    measure(fonts, &p.style, &p.text[..byte])
                };
                xs[off - start] = x + w;
            }
            x += p.width;
        }
        let gb = base.get(pl.para).copied().unwrap_or(0);
        lines.push(CaretLine {
            para: pl.para,
            start: gb + start,
            end: gb + end,
            top: pl.top,
            height: line.height,
            baseline: pl.baseline,
            xs,
        });
    }
    TextGeometry {
        lines,
        content_height: a.content_h,
        font_scale: a.scale,
        ln_spc_reduction: a.reduction,
    }
}

/// Length of a paragraph in the editing text.
fn para_len(p: &bo_model::slide::Paragraph) -> usize {
    p.runs
        .iter()
        .map(|r| match r {
            TextRun::Run { text, .. } | TextRun::Field { text, .. } => text.chars().count(),
            TextRun::Break { .. } => 1,
            TextRun::Other(_) => 0,
        })
        .sum()
}

fn columns_needed(layout: &[ParaLayout], box_h: f64) -> usize {
    let mut cols = 1;
    let mut y = 0.0;
    for (pi, para) in layout.iter().enumerate() {
        for (li, line) in para.lines.iter().enumerate() {
            let before = if li == 0 { line.space_before } else { 0.0 };
            if y + before + line.height > box_h + 0.5 && y > 0.0 {
                cols += 1;
                y = 0.0;
            } else {
                y += before;
            }
            y += line.height;
        }
        if pi + 1 < layout.len() {
            y += para.space_after;
        }
    }
    if y > box_h + 0.5 {
        cols += 1;
    }
    cols
}

#[allow(clippy::too_many_arguments)]
fn layout_all(
    body: &TextBody,
    stack: &TextStyleStack,
    col_w: f64,
    wrap: bool,
    scale: f64,
    reduction: f64,
    env: &TextEnv<'_>,
    fonts: &mut FontManager,
) -> Vec<ParaLayout> {
    let mut out = Vec::new();
    // Auto-numbering counters per level (§21.1.2.4.1).
    let mut counters: [Option<(String, i32)>; 9] = Default::default();
    for (i, p) in body.paragraphs.iter().enumerate() {
        let ppr = stack.paragraph(p);
        let lvl = ppr.lvl.unwrap_or(0).min(8) as usize;
        let num = match &ppr.bullet {
            Some(Bullet::AutoNum { scheme, start_at }) if !p.text().trim().is_empty() => {
                let n = match &counters[lvl] {
                    Some((s, n)) if s == scheme => n + 1,
                    _ => start_at.unwrap_or(1),
                };
                counters[lvl] = Some((scheme.clone(), n));
                for c in counters.iter_mut().skip(lvl + 1) {
                    *c = None;
                }
                Some(n)
            }
            _ => {
                if !p.text().trim().is_empty() {
                    counters[lvl] = None;
                }
                None
            }
        };
        let width = if wrap { col_w } else { f64::MAX / 4.0 };
        out.push(layout_paragraph(
            p,
            &ppr,
            width,
            wrap,
            scale,
            reduction,
            num,
            env,
            fonts,
            i == 0,
        ));
    }
    out
}

fn emit(
    fonts: &mut FontManager,
    p: &Piece,
    x: f64,
    baseline: f64,
    out: &mut Vec<Command>,
    fx: &mut Vec<TextFx>,
) {
    if let Some(g) = &p.math {
        for (rx, ry, face, size, text, glyphs) in &g.runs {
            out.push(Command::Glyphs {
                face: face.0,
                size: *size,
                x: x as f32 + rx,
                y: baseline as f32 + ry,
                color: p.style.color,
                text: text.clone(),
                glyphs: glyphs
                    .iter()
                    .map(|&(id, dx, dy, cluster)| Glyph(id, dx, dy, cluster))
                    .collect(),
            });
        }
        for &(x1, y1, x2, y2, width) in &g.rules {
            out.push(Command::Line {
                x1: x as f32 + x1,
                y1: baseline as f32 + y1,
                x2: x as f32 + x2,
                y2: baseline as f32 + y2,
                width,
                color: p.style.color,
                dash: None,
            });
        }
        return;
    }
    let text = p.text.trim_end_matches(['\n', '\r']);
    if text.is_empty() {
        return;
    }
    let st = &p.style;
    let y = baseline - st.baseline;
    let mut gx = 0.0_f64;
    for (range, face) in fonts.itemize(st.face, text) {
        let sub = &text[range.clone()];
        let upem = f64::from(fonts.info(face).units_per_em.max(1));
        let k = st.size / upem;
        let shaped = fonts.shape(face, sub);
        let mut glyphs = Vec::with_capacity(shaped.len());
        let mut adv = 0.0_f64;
        for g in shaped.iter() {
            glyphs.push(Glyph(
                g.glyph_id,
                ((adv + f64::from(g.x_offset)) * k) as f32,
                (-f64::from(g.y_offset) * k) as f32,
                g.cluster,
            ));
            adv += f64::from(g.x_advance);
        }
        if let (Some(f), false) = (&st.fx, sub.trim().is_empty()) {
            // WordArt fill / outline: the glyph outlines as a path, painted by the caller.
            fx.push(TextFx {
                index: out.len(),
                fx: f.clone(),
            });
            out.push(Command::Path {
                segs: glyph_path(fonts, face, st.size, (x + gx, y), &glyphs),
                fill: (st.color.3 > 0).then_some(bo_render::Paint::Solid { color: st.color }),
                stroke: None,
            });
        } else if !sub.trim().is_empty() && st.color.3 > 0 {
            out.push(Command::Glyphs {
                face: face.0,
                size: st.size as f32,
                x: (x + gx) as f32,
                y: y as f32,
                color: st.color,
                text: sub.to_owned(),
                glyphs,
            });
        }
        gx += adv * k;
    }
    let info = fonts.info(st.face);
    let upem = f64::from(info.units_per_em.max(1));
    let k = st.size / upem;
    let thick = (f64::from(info.underline_thickness) * k).max(0.5);
    let w = p.width - p.trailing;
    // §21.1.2.3.9 rPr@u / rPr@strike
    if st.underline {
        let uy = y - f64::from(info.underline_position) * k;
        out.push(line_cmd(x, uy, x + w, thick, st.color));
    }
    if st.strike {
        let sy = y - f64::from(info.strikeout_position) * k;
        out.push(line_cmd(x, sy, x + w, thick, st.color));
    }
}

fn line_cmd(x1: f64, y: f64, x2: f64, width: f64, color: Rgba) -> Command {
    Command::Line {
        x1: x1 as f32,
        y1: y as f32,
        x2: x2 as f32,
        y2: y as f32,
        width: width as f32,
        color,
        dash: None,
    }
}

#[cfg(test)]
mod tests {
    use super::auto_number;

    #[test]
    fn auto_numbers() {
        assert_eq!(auto_number("arabicPeriod", 3), "3.");
        assert_eq!(auto_number("arabicParenR", 2), "2)");
        assert_eq!(auto_number("romanUcPeriod", 4), "IV.");
        assert_eq!(auto_number("romanLcParenBoth", 9), "(ix)");
        assert_eq!(auto_number("alphaLcParenR", 27), "aa)");
        assert_eq!(auto_number("alphaUcPeriod", 2), "B.");
    }
}
