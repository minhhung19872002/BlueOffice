//! Text and colour of animated shapes in the slide show.
//!
//! - Effects run word by word or letter by letter (ECMA-376 Part 1, §19.5 iterate `wd` / `lt`):
//!   the shape's layer is cut into the drawing without its text and one small display list per
//!   letter (with the index of its word), so the show plays the effect on each piece in turn.
//! - Colour changes (§19.5.2 animClr of `fillcolor`, `stroke.color`, `style.color`): the shape is
//!   drawn again in the colour the behaviour ends on; the show fades that drawing in.
//! - Morph by word / by character ([MS-PPTX] p159:morph@option): the same pieces of two slides
//!   are paired by their text.

use bo_model::slide::{Color, Fill, Line, Presentation, ShapeNode, TextBody, TextRun};
use bo_render::{Command, Glyph, PageList};
use serde::Serialize;

/// Most letters a layer is cut into (beyond that, the text plays as one object).
pub const MAX_PIECES: usize = 600;

/// The shape `id` of a slide (anywhere in the shape tree).
fn node_by_id(pres: &Presentation, slide: usize, id: u32) -> Option<&ShapeNode> {
    let s = pres.slides.get(slide)?;
    let mut hit = None;
    bo_pptx::visit(&s.data.shapes, &mut |n| {
        if hit.is_none() && n.nv().is_some_and(|nv| nv.id == id) {
            hit = Some(n);
        }
    });
    hit
}

/// Text of a shape and the shapes in it.
fn node_text(n: &ShapeNode) -> String {
    let mut out = Vec::new();
    bo_pptx::visit(std::slice::from_ref(n), &mut |c| {
        if let ShapeNode::Shape(sp) = c {
            if let Some(t) = &sp.text {
                out.push(super::text::body_text(t));
            }
        }
    });
    out.join("\n")
}

/// Words (`wd`) or letters (`lt`) of `text`: what an iterated effect runs on (at least 1).
#[must_use]
pub fn count_pieces(text: &str, kind: &str) -> u32 {
    let n = match kind {
        "wd" => text.split_whitespace().count(),
        "lt" => text.chars().filter(|c| !c.is_whitespace()).count(),
        _ => 1,
    };
    u32::try_from(n.clamp(1, MAX_PIECES)).unwrap_or(1)
}

/// Words / letters of shape `id` (§19.5 iterate type `kind`).
#[must_use]
pub fn piece_count(pres: &Presentation, slide: usize, id: u32, kind: &str) -> u32 {
    node_by_id(pres, slide, id).map_or(1, |n| count_pieces(&node_text(n), kind))
}

/// One letter of a layer.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextPiece {
    /// The letter alone (same page size as the layer).
    pub page: PageList,
    /// Its box `[x, y, w, h]` on the slide, points.
    pub rect: [f64; 4],
    /// Index of its word.
    pub word: usize,
    /// Index of the letter.
    pub letter: usize,
    /// The letter (or ligature) drawn.
    pub text: String,
}

/// A layer cut into its drawing without text and its letters.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextSplit {
    /// Everything but the text.
    pub body: PageList,
    /// The letters in reading order.
    pub pieces: Vec<TextPiece>,
    /// Number of words.
    pub words: usize,
}

type Mat = [f64; 6];
const ID: Mat = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `m ∘ n` (apply `n` first), affine `[a, b, c, d, e, f]` (`x' = a·x + c·y + e`).
fn mul(m: Mat, n: Mat) -> Mat {
    [
        m[0] * n[0] + m[2] * n[1],
        m[1] * n[0] + m[3] * n[1],
        m[0] * n[2] + m[2] * n[3],
        m[1] * n[2] + m[3] * n[3],
        m[0] * n[4] + m[2] * n[5] + m[4],
        m[1] * n[4] + m[3] * n[5] + m[5],
    ]
}

fn apply(m: Mat, x: f64, y: f64) -> (f64, f64) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

fn pop_of(c: &Command) -> Option<Command> {
    match c {
        Command::PushTransform { .. } => Some(Command::PopTransform),
        Command::PushClip { .. } | Command::PushClipPath { .. } => Some(Command::PopClip),
        Command::PushOpacity { .. } => Some(Command::PopOpacity),
        _ => None,
    }
}

/// Box of glyphs `gs` of a run at (`x`, `y`), size `size`, through `m`: `[x0, y0, x1, y1]`.
fn glyph_box(m: Mat, x: f64, y: f64, size: f64, gs: &[Glyph], next_x: Option<f64>) -> [f64; 4] {
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for (k, g) in gs.iter().enumerate() {
        let gx = f64::from(g.1);
        let right = gs
            .get(k + 1)
            .map(|n| f64::from(n.1))
            .or(next_x)
            .filter(|r| *r > gx)
            .unwrap_or(gx + size * 0.6);
        let gy = f64::from(g.2);
        for (px, py) in [
            (x + gx, y + gy - size * 0.9),
            (x + right, y + gy - size * 0.9),
            (x + gx, y + gy + size * 0.25),
            (x + right, y + gy + size * 0.25),
        ] {
            let (u, v) = apply(m, px, py);
            b = [b[0].min(u), b[1].min(v), b[2].max(u), b[3].max(v)];
        }
    }
    b
}

/// Cut a layer into its drawing without text and its letters (`None`: no text, or more than
/// [`MAX_PIECES`] letters).
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn split_text(page: &PageList) -> Option<TextSplit> {
    let mut body = Vec::with_capacity(page.commands.len());
    // Open pushes with the transform before each.
    let mut open: Vec<(Command, Mat)> = Vec::new();
    let mut m = ID;
    let mut pieces: Vec<TextPiece> = Vec::new();
    let (mut words, mut in_word) = (0usize, false);
    let mut last_line: Option<(f64, f64)> = None;
    for c in &page.commands {
        match c {
            Command::PushTransform { m: t } => {
                open.push((c.clone(), m));
                m = mul(m, t.map(f64::from));
                body.push(c.clone());
            }
            Command::PushClip { .. }
            | Command::PushClipPath { .. }
            | Command::PushOpacity { .. } => {
                open.push((c.clone(), m));
                body.push(c.clone());
            }
            Command::PopTransform | Command::PopClip | Command::PopOpacity => {
                if let Some((_, before)) = open.pop() {
                    m = before;
                }
                body.push(c.clone());
            }
            Command::Glyphs {
                face,
                size,
                x,
                y,
                color,
                text,
                glyphs,
            } => {
                // A run on another line (or further left) starts a new word.
                let (x0, y0) = apply(m, f64::from(*x), f64::from(*y));
                if last_line.is_some_and(|(lx, ly)| (ly - y0).abs() > 0.5 || x0 < lx) {
                    in_word = false;
                }
                let mut k = 0;
                while k < glyphs.len() {
                    let cl = glyphs[k].3;
                    let mut e = k + 1;
                    while e < glyphs.len() && glyphs[e].3 == cl {
                        e += 1;
                    }
                    let start = cl as usize;
                    let end = glyphs.get(e).map_or(text.len(), |g| g.3 as usize);
                    let piece_text = text
                        .get(start.min(end)..end.max(start))
                        .or_else(|| text.get(start..))
                        .unwrap_or("")
                        .to_owned();
                    if piece_text.chars().all(char::is_whitespace) {
                        in_word = false;
                        k = e;
                        continue;
                    }
                    if !in_word {
                        words += 1;
                        in_word = true;
                    }
                    if pieces.len() >= MAX_PIECES {
                        return None;
                    }
                    // Ends with a space (one cluster holding the space): the word ends here.
                    if piece_text.ends_with(char::is_whitespace) {
                        in_word = false;
                    }
                    let gs: Vec<Glyph> = glyphs[k..e]
                        .iter()
                        .map(|g| Glyph(g.0, g.1, g.2, g.3.saturating_sub(cl)))
                        .collect();
                    let next_x = glyphs.get(e).map(|g| f64::from(g.1));
                    let bx = glyph_box(
                        m,
                        f64::from(*x),
                        f64::from(*y),
                        f64::from(*size),
                        &glyphs[k..e],
                        next_x,
                    );
                    let mut cmds: Vec<Command> = open.iter().map(|(p, _)| p.clone()).collect();
                    cmds.push(Command::Glyphs {
                        face: *face,
                        size: *size,
                        x: *x,
                        y: *y,
                        color: *color,
                        text: piece_text.clone(),
                        glyphs: gs,
                    });
                    cmds.extend(open.iter().rev().filter_map(|(p, _)| pop_of(p)));
                    pieces.push(TextPiece {
                        page: PageList {
                            width: page.width,
                            height: page.height,
                            commands: cmds,
                        },
                        rect: [
                            bx[0],
                            bx[1],
                            (bx[2] - bx[0]).max(0.0),
                            (bx[3] - bx[1]).max(0.0),
                        ],
                        word: words - 1,
                        letter: pieces.len(),
                        text: piece_text,
                    });
                    k = e;
                }
                last_line = Some((x0, y0));
            }
            other => body.push(other.clone()),
        }
    }
    if pieces.is_empty() {
        return None;
    }
    Some(TextSplit {
        body: PageList {
            width: page.width,
            height: page.height,
            commands: body,
        },
        pieces,
        words,
    })
}

/// Pieces of a split grouped by word (`by_word`) or one per letter: (text, page, box).
#[must_use]
pub fn grouped(split: &TextSplit, by_word: bool) -> Vec<(String, PageList, [f64; 4])> {
    if !by_word {
        return split
            .pieces
            .iter()
            .map(|p| (p.text.clone(), p.page.clone(), p.rect))
            .collect();
    }
    let mut out: Vec<(String, PageList, [f64; 4])> = Vec::new();
    let mut cur_word = usize::MAX;
    for p in &split.pieces {
        if p.word == cur_word {
            if let Some((t, page, r)) = out.last_mut() {
                t.push_str(&p.text);
                page.commands.extend(p.page.commands.iter().cloned());
                let (x1, y1) = (
                    (r[0] + r[2]).max(p.rect[0] + p.rect[2]),
                    (r[1] + r[3]).max(p.rect[1] + p.rect[3]),
                );
                r[0] = r[0].min(p.rect[0]);
                r[1] = r[1].min(p.rect[1]);
                r[2] = x1 - r[0];
                r[3] = y1 - r[1];
            }
        } else {
            cur_word = p.word;
            out.push((p.text.clone(), p.page.clone(), p.rect));
        }
    }
    out
}

/// Pair pieces of two slides by equal text, in reading order (each piece at most once): indices
/// (from, to).
#[must_use]
pub fn pair_by_text(a: &[String], b: &[String]) -> Vec<(usize, usize)> {
    let mut used = vec![false; b.len()];
    let mut out = Vec::new();
    for (i, t) in a.iter().enumerate() {
        // The first unused piece with the same text after the last paired one, else any.
        let after = out.last().map_or(0, |&(_, j): &(usize, usize)| j + 1);
        let hit = (after..b.len())
            .chain(0..after.min(b.len()))
            .find(|&j| !used[j] && b[j] == *t);
        if let Some(j) = hit {
            used[j] = true;
            out.push((i, j));
        }
    }
    out
}

fn solid(rgb: u32) -> Fill {
    Fill::Solid(Color::srgb(rgb))
}

fn tint_text(t: &mut TextBody, rgb: u32) {
    for p in &mut t.paragraphs {
        for r in &mut p.runs {
            match r {
                TextRun::Run { props, .. }
                | TextRun::Field { props, .. }
                | TextRun::Break { props } => props.fill = Some(solid(rgb)),
                TextRun::Other(_) => {}
            }
        }
        if let Some(e) = &mut p.end_props {
            e.fill = Some(solid(rgb));
        }
    }
}

/// Shape `n` (and the shapes in it) in the colour a §19.5.2 animClr of `attr` ends on: the fill
/// (`fillcolor`, `fill.color`), the outline (`stroke.color`) or the text (`style.color`).
pub fn tint(n: &mut ShapeNode, attr: &str, rgb: u32) {
    match n {
        ShapeNode::Group(g) => {
            for c in &mut g.children {
                tint(c, attr, rgb);
            }
        }
        ShapeNode::Shape(sp) => match attr {
            "fillcolor" | "fill.color" => sp.props.fill = Some(solid(rgb)),
            "stroke.color" => {
                sp.props.line.get_or_insert_with(Line::default).fill = Some(solid(rgb));
            }
            "style.color" => {
                if let Some(t) = &mut sp.text {
                    tint_text(t, rgb);
                }
            }
            _ => {}
        },
        ShapeNode::Connector(c) if attr == "stroke.color" => {
            c.props.line.get_or_insert_with(Line::default).fill = Some(solid(rgb));
        }
        _ => {}
    }
}

/// [`tint`] the shape `id` inside top-level node `n` (`false`: not there).
pub fn tint_id(n: &mut ShapeNode, id: u32, attr: &str, rgb: u32) -> bool {
    if n.nv().is_some_and(|nv| nv.id == id) {
        tint(n, attr, rgb);
        return true;
    }
    if let ShapeNode::Group(g) = n {
        return g.children.iter_mut().any(|c| tint_id(c, id, attr, rgb));
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_render::Rgba;

    fn run(x: f32, y: f32, text: &str) -> Command {
        // One glyph per byte, 10 pt apart.
        Command::Glyphs {
            face: 0,
            size: 10.0,
            x,
            y,
            color: Rgba(0, 0, 0, 255),
            text: text.into(),
            glyphs: (0..text.len())
                .map(|i| Glyph(1, i as f32 * 10.0, 0.0, i as u32))
                .collect(),
        }
    }

    // source: words are the letters between spaces; a run on a new line starts a new word; the
    // pieces keep the transform they are drawn in.
    #[test]
    fn layers_split_into_letters_and_words() {
        let page = PageList {
            width: 100.0,
            height: 100.0,
            commands: vec![
                Command::FillRect {
                    x: 0.0,
                    y: 0.0,
                    w: 10.0,
                    h: 10.0,
                    color: Rgba(1, 2, 3, 255),
                },
                Command::PushTransform {
                    m: [1.0, 0.0, 0.0, 1.0, 5.0, 0.0],
                },
                run(0.0, 20.0, "ab c"),
                run(40.0, 20.0, "d"),
                run(0.0, 40.0, "ef"),
                Command::PopTransform,
            ],
        };
        let s = split_text(&page).unwrap();
        let txt: Vec<&str> = s.pieces.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(txt, vec!["a", "b", "c", "d", "e", "f"]);
        let w: Vec<usize> = s.pieces.iter().map(|p| p.word).collect();
        // "ab" | "cd" (the run "d" continues "c" on the same line) | "ef" (next line).
        assert_eq!(w, vec![0, 0, 1, 1, 2, 2]);
        assert_eq!(s.words, 3);
        // The body keeps the rectangle and the transform, without text.
        assert_eq!(s.body.commands.len(), 3);
        assert!(s
            .body
            .commands
            .iter()
            .all(|c| !matches!(c, Command::Glyphs { .. })));
        // "b" drawn at x 10 + 5 (transform), 10 wide.
        let b = s.pieces[1].rect;
        assert!(
            (b[0] - 15.0).abs() < 1e-6 && (b[2] - 10.0).abs() < 1e-6,
            "{b:?}"
        );
        assert!(matches!(
            s.pieces[1].page.commands[0],
            Command::PushTransform { .. }
        ));
        assert!(matches!(
            s.pieces[1].page.commands[2],
            Command::PopTransform
        ));
        let g = grouped(&s, true);
        assert_eq!(
            g.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(),
            vec!["ab", "cd", "ef"]
        );
        assert!((g[0].2[2] - 20.0).abs() < 1e-6);
        assert_eq!(count_pieces("Xin chào  bạn", "wd"), 3);
        assert_eq!(count_pieces("Xin chào", "lt"), 7);
        assert_eq!(count_pieces("", "lt"), 1);
    }

    #[test]
    fn pieces_pair_by_text_in_order() {
        let s = |v: &[&str]| v.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            pair_by_text(&s(&["a", "b", "a", "z"]), &s(&["b", "a", "a"])),
            vec![(0, 1), (1, 0), (2, 2)]
        );
    }
}
