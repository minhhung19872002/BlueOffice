//! Paragraph layout: itemization, shaping, line breaking, alignment, line height.

use crate::types::{Fragment, GlyphPos, Line};
use crate::LayoutError;
use bo_model::{Alignment, Color, Document, LineSpacing, Paragraph, RunContent, Twips, Underline};
use bo_text::{FaceId, FontManager, ShapedGlyph};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// Word-processor defaults when neither the run nor `docDefaults` specify a value
/// (ECMA-376 Part 1, §17.7.5.5: 10 pt; the application default face).
const DEFAULT_FONT: &str = "Times New Roman";
const DEFAULT_SIZE_PT: f32 = 10.0;
/// Default tab stop interval (ECMA-376 Part 1, §17.15.1.25 defaultTabStop default 720 twips).
const DEFAULT_TAB_PT: f32 = 36.0;

const LINE_SEP: char = '\u{2028}';
const PAGE_SEP: char = '\u{000C}';

/// Cache of laid-out paragraphs (see crate docs, "Incremental layout").
#[derive(Default)]
pub struct LayoutCache {
    map: HashMap<u64, Arc<ParaLayout>>,
    hits: u64,
    misses: u64,
}

impl LayoutCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self::default()
    }
    /// (hits, misses) since creation.
    pub fn stats(&self) -> (u64, u64) {
        (self.hits, self.misses)
    }
    /// Drop everything (e.g. after fonts change).
    pub fn clear(&mut self) {
        self.map.clear();
    }
}

/// Lines of one paragraph, positioned relative to the content box.
#[derive(Debug)]
pub(crate) struct ParaLayout {
    pub lines: Vec<RelLine>,
    pub space_before: f32,
    pub space_after: f32,
    pub page_break_before: bool,
}

#[derive(Debug)]
pub(crate) struct RelLine {
    pub height: f32,
    /// Baseline offset from line top.
    pub baseline: f32,
    pub fragments: Vec<Fragment>, // x relative to content left, y = 0
    pub page_break_after: bool,
}

impl RelLine {
    pub(crate) fn place(&self, left: f32, top: f32) -> Line {
        let baseline = top + self.baseline;
        Line {
            top,
            height: self.height,
            baseline,
            fragments: self
                .fragments
                .iter()
                .map(|f| Fragment {
                    x: f.x + left,
                    y: baseline,
                    ..f.clone()
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
struct Style {
    face: FaceId,
    size: f32,
    color: Color,
    underline: Underline,
    strike: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Text,
    Space,
    Tab,
    LineBreak,
    PageBreak,
}

#[derive(Debug, Clone)]
struct Piece {
    style: usize,
    face: FaceId,
    kind: Kind,
    text: String,
    glyphs: Arc<[ShapedGlyph]>,
    width: f32,
}

#[derive(Debug, Clone)]
struct Word {
    pieces: Vec<Piece>,
}

impl Word {
    /// Width that must fit on the line (trailing spaces / breaks excluded), starting at `x`.
    fn fit_width(&self, x: f32, tab_origin: f32) -> f32 {
        let mut cx = x;
        let mut last_ink = x;
        for p in &self.pieces {
            match p.kind {
                Kind::Tab => {
                    cx = next_tab(cx, tab_origin);
                    last_ink = cx;
                }
                Kind::Space => cx += p.width,
                Kind::Text => {
                    cx += p.width;
                    last_ink = cx;
                }
                Kind::LineBreak | Kind::PageBreak => {}
            }
        }
        last_ink - x
    }
    fn brk(&self) -> Option<Kind> {
        self.pieces
            .iter()
            .rev()
            .find(|p| matches!(p.kind, Kind::LineBreak | Kind::PageBreak))
            .map(|p| p.kind)
    }
}

fn next_tab(x: f32, origin: f32) -> f32 {
    let rel = x - origin;
    origin + ((rel / DEFAULT_TAB_PT).floor() + 1.0) * DEFAULT_TAB_PT
}

fn hash_inputs(doc: &Document, p: &Paragraph, width: f32) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    p.hash(&mut h);
    doc.default_run.hash(&mut h);
    doc.default_para.hash(&mut h);
    width.to_bits().hash(&mut h);
    h.finish()
}

pub(crate) fn layout_paragraph(
    doc: &Document,
    p: &Paragraph,
    width: f32,
    fonts: &mut FontManager,
    cache: &mut LayoutCache,
) -> Result<Arc<ParaLayout>, LayoutError> {
    let key = hash_inputs(doc, p, width);
    if let Some(l) = cache.map.get(&key) {
        cache.hits += 1;
        return Ok(l.clone());
    }
    cache.misses += 1;
    let l = Arc::new(layout_uncached(doc, p, width, fonts)?);
    cache.map.insert(key, l.clone());
    Ok(l)
}

fn layout_uncached(
    doc: &Document,
    p: &Paragraph,
    width: f32,
    fonts: &mut FontManager,
) -> Result<ParaLayout, LayoutError> {
    let props = p.props.inherit(&doc.default_para);
    let indent_left = props.indent_left.unwrap_or(Twips(0)).to_pt();
    let indent_right = props.indent_right.unwrap_or(Twips(0)).to_pt();
    let first = props.indent_first_line.unwrap_or(Twips(0)).to_pt();
    let align = props.align.unwrap_or_default();
    let spacing = props.line_spacing.unwrap_or_default();

    // 1. Resolve styles and build the paragraph text with style spans.
    let mut styles: Vec<Style> = Vec::new();
    let mut text = String::new();
    let mut spans: Vec<(std::ops::Range<usize>, usize)> = Vec::new();
    for run in &p.runs {
        let rp = run.props.inherit(&doc.default_run);
        let face = fonts
            .resolve(
                rp.font.as_deref().unwrap_or(DEFAULT_FONT),
                rp.bold.unwrap_or(false),
                rp.italic.unwrap_or(false),
            )
            .ok_or(LayoutError::NoFonts)?;
        styles.push(Style {
            face,
            size: rp.size.map_or(DEFAULT_SIZE_PT, bo_model::HalfPoints::to_pt),
            color: rp.color.unwrap_or(Color::BLACK),
            underline: rp.underline.unwrap_or(Underline::None),
            strike: rp.strike.unwrap_or(false),
        });
        let si = styles.len() - 1;
        let start = text.len();
        for c in &run.content {
            match c {
                RunContent::Text(t) => text.push_str(&bo_text::nfc(t)),
                RunContent::Tab => text.push('\t'),
                RunContent::LineBreak => text.push(LINE_SEP),
                RunContent::PageBreak => text.push(PAGE_SEP),
            }
        }
        if text.len() > start {
            spans.push((start..text.len(), si));
        }
    }
    // Style of the paragraph mark (empty paragraphs still take a line).
    let mark_style = match styles.last() {
        Some(s) => s.clone(),
        None => {
            let rp = &doc.default_run;
            let face = fonts
                .resolve(
                    rp.font.as_deref().unwrap_or(DEFAULT_FONT),
                    rp.bold.unwrap_or(false),
                    rp.italic.unwrap_or(false),
                )
                .ok_or(LayoutError::NoFonts)?;
            Style {
                face,
                size: rp.size.map_or(DEFAULT_SIZE_PT, bo_model::HalfPoints::to_pt),
                color: Color::BLACK,
                underline: Underline::None,
                strike: false,
            }
        }
    };
    if styles.is_empty() {
        styles.push(mark_style.clone());
    }

    // 2. Split into words at UAX #14 break opportunities, then into pieces.
    let mut words: Vec<Word> = Vec::new();
    let mut prev = 0;
    for (idx, _opp) in unicode_linebreak::linebreaks(&text) {
        if idx > prev {
            words.push(build_word(&text, prev..idx, &spans, &styles, fonts));
        }
        prev = idx;
    }

    // 3. Greedy line filling.
    let mut lines: Vec<Vec<Piece>> = Vec::new();
    let mut line_breaks: Vec<Option<Kind>> = Vec::new();
    let mut cur: Vec<Piece> = Vec::new();
    let mut x = 0.0f32;
    let avail = |first_line: bool| {
        (width - indent_left - indent_right - if first_line { first } else { 0.0 }).max(1.0)
    };
    let mut queue: std::collections::VecDeque<Word> = words.into();
    while let Some(word) = queue.pop_front() {
        let line_start = if lines.is_empty() { first } else { 0.0 };
        let max = avail(lines.is_empty());
        let w = word.fit_width(x, -line_start);
        if x + w > max + 0.01 && !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
            line_breaks.push(None);
            x = 0.0;
            queue.push_front(word);
            continue;
        }
        if cur.is_empty() && w > max + 0.01 && word_char_count(&word) > 1 {
            // Emergency break: explode the word into grapheme-sized words.
            for w2 in explode(&word, fonts).into_iter().rev() {
                queue.push_front(w2);
            }
            continue;
        }
        for piece in &word.pieces {
            x = match piece.kind {
                Kind::Tab => next_tab(x, -line_start),
                _ => x + piece.width,
            };
        }
        let brk = word.brk();
        cur.extend(word.pieces);
        if brk.is_some() {
            lines.push(std::mem::take(&mut cur));
            line_breaks.push(brk);
            x = 0.0;
        }
    }
    if !cur.is_empty() || lines.is_empty() {
        lines.push(cur);
        line_breaks.push(None);
    }
    let n_lines = lines.len();

    // 4. Position pieces → fragments, apply alignment and line height.
    let mut out = Vec::with_capacity(n_lines);
    for (li, pieces) in lines.into_iter().enumerate() {
        let is_first = li == 0;
        let is_last = li + 1 == n_lines;
        let start_x = indent_left + if is_first { first } else { 0.0 };
        let max = avail(is_first);
        let brk = line_breaks[li];

        // Natural width excluding trailing spaces/breaks.
        let mut xs = Vec::with_capacity(pieces.len());
        let mut cx = 0.0f32;
        let mut ink_end = 0.0f32;
        let mut last_tab_idx: Option<usize> = None;
        for (i, p) in pieces.iter().enumerate() {
            xs.push(cx);
            cx = match p.kind {
                Kind::Tab => {
                    last_tab_idx = Some(i);
                    next_tab(cx, -(start_x - indent_left))
                }
                _ => cx + p.width,
            };
            if matches!(p.kind, Kind::Text | Kind::Tab) {
                ink_end = cx;
            }
        }
        let slack = (max - ink_end).max(0.0);
        let justify = align == Alignment::Justify && !is_last && brk.is_none();
        // Spaces after the last tab and before the last ink participate in justification.
        let ink_last_piece = pieces.iter().rposition(|p| p.kind == Kind::Text);
        let space_count: usize = if justify {
            pieces
                .iter()
                .enumerate()
                .filter(|(i, p)| {
                    p.kind == Kind::Space
                        && last_tab_idx.is_none_or(|t| *i > t)
                        && ink_last_piece.is_some_and(|l| *i < l)
                })
                .map(|(_, p)| p.text.chars().count())
                .sum()
        } else {
            0
        };
        let per_space = if space_count > 0 { slack / space_count as f32 } else { 0.0 };
        let offset = match align {
            Alignment::Center => slack / 2.0,
            Alignment::Right => slack,
            _ => 0.0,
        };

        let mut ascent = 0.0f32;
        let mut descent = 0.0f32;
        let mut natural = 0.0f32;
        let mut frags: Vec<Fragment> = Vec::new();
        let mut extra = 0.0f32;
        for (i, p) in pieces.iter().enumerate() {
            let st = &styles[p.style];
            let info = fonts.info(p.face);
            let scale = st.size / f32::from(info.units_per_em);
            let asc = f32::from(info.ascender) * scale;
            let desc = -f32::from(info.descender) * scale;
            ascent = ascent.max(asc);
            descent = descent.max(desc);
            natural = natural.max(asc + desc + f32::from(info.line_gap) * scale);
            if matches!(p.kind, Kind::LineBreak | Kind::PageBreak | Kind::Tab) {
                continue;
            }
            let justify_here = per_space > 0.0
                && p.kind == Kind::Space
                && last_tab_idx.is_none_or(|t| i > t)
                && ink_last_piece.is_some_and(|l| i < l);
            let mut glyphs = Vec::with_capacity(p.glyphs.len());
            let mut gx = 0.0f32;
            for g in p.glyphs.iter() {
                glyphs.push(GlyphPos {
                    id: g.glyph_id,
                    x: gx + g.x_offset as f32 * scale,
                    y: -(g.y_offset as f32) * scale,
                    cluster: g.cluster,
                });
                gx += g.x_advance as f32 * scale;
                if justify_here {
                    gx += per_space;
                }
            }
            let frag_x = start_x + offset + xs[i] + extra;
            if justify_here {
                extra += per_space * p.text.chars().count() as f32;
            }
            // Merge with the previous fragment when face/style continue.
            if let Some(last) = frags.last_mut() {
                if last.face == p.face
                    && last.size == st.size
                    && last.color == st.color
                    && last.underline == st.underline
                    && last.strike == st.strike
                    && (last.x + last.width - frag_x).abs() < 0.001
                {
                    let dx = frag_x - last.x;
                    let base = last.text.len() as u32;
                    last.glyphs.extend(glyphs.into_iter().map(|g| GlyphPos {
                        x: g.x + dx,
                        cluster: g.cluster + base,
                        ..g
                    }));
                    last.width = dx + gx;
                    last.text.push_str(&p.text);
                    continue;
                }
            }
            frags.push(Fragment {
                face: p.face,
                size: st.size,
                x: frag_x,
                y: 0.0,
                width: gx,
                color: st.color,
                underline: st.underline,
                strike: st.strike,
                text: p.text.clone(),
                glyphs,
            });
        }
        if pieces.iter().all(|p| p.kind != Kind::Text && p.kind != Kind::Space) {
            let info = fonts.info(mark_style.face);
            let scale = mark_style.size / f32::from(info.units_per_em);
            ascent = ascent.max(f32::from(info.ascender) * scale);
            descent = descent.max(-f32::from(info.descender) * scale);
            natural = natural.max(
                (f32::from(info.ascender) - f32::from(info.descender) + f32::from(info.line_gap)) * scale,
            );
        }
        // Line height (ECMA-376 Part 1, §17.3.1.33 spacing/@line, @lineRule).
        let (height, baseline) = match spacing {
            LineSpacing::Auto(v) => {
                let h = natural * v as f32 / 240.0;
                (h, (h - natural).max(0.0) + ascent)
            }
            LineSpacing::Exact(t) => {
                let h = t.to_pt();
                (h, h - descent)
            }
            LineSpacing::AtLeast(t) => {
                let h = natural.max(t.to_pt());
                (h, h - descent)
            }
        };
        out.push(RelLine {
            height,
            baseline,
            fragments: frags,
            page_break_after: brk == Some(Kind::PageBreak),
        });
    }

    Ok(ParaLayout {
        lines: out,
        space_before: props.space_before.unwrap_or(Twips(0)).to_pt(),
        space_after: props.space_after.unwrap_or(Twips(0)).to_pt(),
        page_break_before: props.page_break_before.unwrap_or(false),
    })
}

fn word_char_count(w: &Word) -> usize {
    w.pieces
        .iter()
        .filter(|p| p.kind == Kind::Text)
        .map(|p| p.text.chars().count())
        .sum()
}

fn kind_of(c: char) -> Kind {
    match c {
        ' ' | '\u{3000}' => Kind::Space,
        '\t' => Kind::Tab,
        LINE_SEP | '\n' | '\r' => Kind::LineBreak,
        PAGE_SEP => Kind::PageBreak,
        _ => Kind::Text,
    }
}

fn build_word(
    text: &str,
    range: std::ops::Range<usize>,
    spans: &[(std::ops::Range<usize>, usize)],
    styles: &[Style],
    fonts: &mut FontManager,
) -> Word {
    let mut pieces = Vec::new();
    for (span, si) in spans {
        let s = span.start.max(range.start);
        let e = span.end.min(range.end);
        if s >= e {
            continue;
        }
        let sub = &text[s..e];
        // Group consecutive chars of the same kind.
        let mut groups: Vec<(Kind, usize, usize)> = Vec::new();
        for (i, c) in sub.char_indices() {
            let k = kind_of(c);
            let end = i + c.len_utf8();
            match groups.last_mut() {
                Some(g) if g.0 == k && matches!(k, Kind::Text | Kind::Space) => g.2 = end,
                _ => groups.push((k, i, end)),
            }
        }
        for (kind, gs, ge) in groups {
            let gtext = &sub[gs..ge];
            match kind {
                Kind::Text | Kind::Space => {
                    for (r, face) in fonts.itemize(styles[*si].face, gtext) {
                        let t = &gtext[r];
                        let glyphs = fonts.shape(face, t);
                        let upem = f32::from(fonts.info(face).units_per_em);
                        let width = glyphs.iter().map(|g| g.x_advance).sum::<i32>() as f32
                            * styles[*si].size
                            / upem;
                        pieces.push(Piece {
                            style: *si,
                            face,
                            kind,
                            text: t.to_owned(),
                            glyphs,
                            width,
                        });
                    }
                }
                _ => pieces.push(Piece {
                    style: *si,
                    face: styles[*si].face,
                    kind,
                    text: String::new(),
                    glyphs: Arc::from(Vec::new()),
                    width: 0.0,
                }),
            }
        }
    }
    Word { pieces }
}

/// Split a word into one word per grapheme cluster (emergency breaking).
fn explode(word: &Word, fonts: &mut FontManager) -> Vec<Word> {
    use unicode_segmentation::UnicodeSegmentation;
    let mut out = Vec::new();
    for p in &word.pieces {
        if p.kind != Kind::Text {
            match out.last_mut() {
                Some(Word { pieces }) => pieces.push(p.clone()),
                None => out.push(Word { pieces: vec![p.clone()] }),
            }
            continue;
        }
        let size_per_unit = if p.glyphs.is_empty() {
            0.0
        } else {
            p.width / p.glyphs.iter().map(|g| g.x_advance).sum::<i32>().max(1) as f32
        };
        for g in p.text.graphemes(true) {
            let glyphs = fonts.shape(p.face, g);
            let width = glyphs.iter().map(|x| x.x_advance).sum::<i32>() as f32 * size_per_unit;
            out.push(Word {
                pieces: vec![Piece {
                    text: g.to_owned(),
                    glyphs,
                    width,
                    ..p.clone()
                }],
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_linebreak::BreakOpportunity;

    #[test]
    fn tab_stops() {
        assert_eq!(next_tab(0.0, 0.0), 36.0);
        assert_eq!(next_tab(36.0, 0.0), 72.0);
        assert_eq!(next_tab(10.0, -18.0), 18.0);
    }

    #[test]
    fn break_opportunities_are_allowed_or_mandatory() {
        let v: Vec<_> = unicode_linebreak::linebreaks("a b\u{2028}c").collect();
        assert_eq!(v[0], (2, BreakOpportunity::Allowed));
        assert_eq!(v[1].1, BreakOpportunity::Mandatory);
    }
}
