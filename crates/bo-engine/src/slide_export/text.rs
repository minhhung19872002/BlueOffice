//! Plain text drawn on handout and notes pages: one shaped run per line, in the
//! body font substitute (Calibri → Carlito, else the last-resort family).

use super::handout::Frame;
use super::masters::Align;
use bo_render::{Command, Glyph, Rgba};
use bo_text::{FaceId, FontManager};

/// Text inset of a text box (§21.1.2.1.1 `bodyPr` lIns/rIns 91 440 EMU, tIns/bIns 45 720 EMU).
const INSET_X: f32 = 7.2;
const INSET_Y: f32 = 3.6;

fn face(fonts: &mut FontManager) -> Option<FaceId> {
    fonts
        .resolve("Calibri", false, false)
        .or_else(|| fonts.resolve("Liberation Sans", false, false))
}

/// Shaped run of `text` with its origin at (`x`, `y` = baseline); returns its width.
fn run(
    fonts: &mut FontManager,
    id: FaceId,
    text: &str,
    size: f32,
    x: f32,
    y: f32,
) -> (Command, f32) {
    let upem = f32::from(fonts.info(id).units_per_em.max(1));
    let k = size / upem;
    let shaped = fonts.shape(id, text);
    let mut pen = 0.0;
    let glyphs = shaped
        .iter()
        .map(|g| {
            let out = Glyph(
                g.glyph_id,
                pen + g.x_offset as f32 * k,
                -(g.y_offset as f32) * k,
                g.cluster,
            );
            pen += g.x_advance as f32 * k;
            out
        })
        .collect();
    (
        Command::Glyphs {
            face: id.0,
            size,
            x,
            y,
            color: Rgba(0, 0, 0, 255),
            text: text.to_owned(),
            glyphs,
        },
        pen,
    )
}

/// One line of text in `frame`, aligned horizontally, at the top or bottom.
pub fn line_in_box(
    fonts: &mut FontManager,
    out: &mut Vec<Command>,
    text: &str,
    frame: &Frame,
    align: Align,
    bottom: bool,
    size: f32,
) {
    let Some(id) = face(fonts) else {
        return;
    };
    let text = text.replace(['\n', '\r', '\t'], " ");
    let width = fonts.measure(id, &text, size);
    let left = frame.x + INSET_X;
    let room = (frame.w - 2.0 * INSET_X).max(0.0);
    let x = match align {
        Align::Left => left,
        Align::Center => left + (room - width) / 2.0,
        Align::Right => left + room - width,
    };
    // Baseline: one em below the top inset, or a fifth of an em above the bottom inset.
    let y = if bottom {
        frame.y + frame.h - INSET_Y - size * 0.25
    } else {
        frame.y + INSET_Y + size
    };
    out.push(run(fonts, id, &text, size, x, y).0);
}

/// Greedy line breaking of `text` at spaces to `width` (a word wider than the
/// line is split between characters).
pub fn wrap(fonts: &mut FontManager, id: FaceId, text: &str, size: f32, width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if line.is_empty() || fonts.measure(id, &candidate, size) <= width {
                line = candidate;
            } else {
                lines.push(std::mem::take(&mut line));
                line = word.to_owned();
            }
            // A single word too long for the line: break it by characters.
            while fonts.measure(id, &line, size) > width && line.chars().count() > 1 {
                let mut cut = String::new();
                for c in line.chars() {
                    let mut next = cut.clone();
                    next.push(c);
                    if !cut.is_empty() && fonts.measure(id, &next, size) > width {
                        break;
                    }
                    cut = next;
                }
                let rest = line[cut.len()..].to_owned();
                lines.push(cut);
                line = rest;
            }
        }
        lines.push(line);
    }
    lines
}

/// Speaker notes wrapped into `frame` from the top, 1.2 × size apart; lines
/// that do not fit are dropped and the last shown one ends with "…".
pub fn wrap_into(
    fonts: &mut FontManager,
    out: &mut Vec<Command>,
    text: &str,
    frame: &Frame,
    size: f32,
) {
    let Some(id) = face(fonts) else {
        return;
    };
    let text = text.replace('\r', "");
    if text.trim().is_empty() {
        return;
    }
    let width = (frame.w - 2.0 * INSET_X).max(size);
    let mut lines = wrap(fonts, id, &text, size, width);
    let pitch = size * 1.2;
    let fit = (((frame.h - 2.0 * INSET_Y) / pitch).floor().max(1.0)) as usize;
    if lines.len() > fit {
        lines.truncate(fit);
        if let Some(last) = lines.last_mut() {
            last.push('…');
        }
    }
    for (i, l) in lines.iter().enumerate() {
        if l.is_empty() {
            continue;
        }
        let y = frame.y + INSET_Y + size + i as f32 * pitch;
        out.push(run(fonts, id, l, size, frame.x + INSET_X, y).0);
    }
}
