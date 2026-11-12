//! BlueDoc pages of a print job: always the print layout; tracked changes
//! (ECMA-376 Part 1, §17.13.5) and comments (§17.13.4) printed with their
//! markup (change bars, balloons) or left out; an optional document
//! properties page (core properties, Part 2 / ISO 29500-2 §11).
//!
//! Choices of ours: with markup on, every page gets the same markup area width
//! (the widest one) so all pages print at one scale; the properties page comes
//! after the chosen pages and is not part of the page range.

use super::range::PageKey;
use super::{choose, Logical, PrintOptions};
use crate::markup::MarkupMode;
use crate::slide_export::text;
use crate::views::ViewMode;
use crate::{Engine, Result};
use bo_layout::{Balloon, PageMarkup};
use bo_render::{Command, PageList, PathSeg, Rgba, Stroke};
use bo_text::FontManager;

/// Balloon text size, author line size and line pitch (points).
const BALLOON_SIZE: f32 = 7.5;
const PITCH: f32 = 10.0;
const PAD: f32 = 4.0;

fn hex(s: &str) -> Rgba {
    let h = s.trim_start_matches('#');
    let c = |i: usize| h.get(i..i + 2).and_then(|v| u8::from_str_radix(v, 16).ok());
    match (c(0), c(2), c(4)) {
        (Some(r), Some(g), Some(b)) => Rgba(r, g, b, 255),
        _ => Rgba(89, 89, 89, 255),
    }
}

/// A colour mixed with white (`k` = share of the colour).
fn tint(c: Rgba, k: f32) -> Rgba {
    let m = |v: u8| {
        let x = f32::from(v) * k + 255.0 * (1.0 - k);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let x = x.round().clamp(0.0, 255.0) as u8;
        x
    };
    Rgba(m(c.0), m(c.1), m(c.2), 255)
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Vec<PathSeg> {
    vec![
        PathSeg::M(x, y),
        PathSeg::L(x + w, y),
        PathSeg::L(x + w, y + h),
        PathSeg::L(x, y + h),
        PathSeg::Z,
    ]
}

fn stroke(width: f32, color: Rgba, dash: Vec<f32>) -> Stroke {
    Stroke {
        width,
        color,
        dash,
        cap: "butt".into(),
        join: "miter".into(),
    }
}

/// Lines of `text` wrapped to `width`, at most `max` (the last cut with "…").
fn lines(fonts: &mut FontManager, text: &str, size: f32, width: f32, max: usize) -> Vec<String> {
    let Some(id) = text::face(fonts) else {
        return Vec::new();
    };
    let mut v = text::wrap(fonts, id, &text.replace('\r', ""), size, width);
    if v.len() > max {
        v.truncate(max);
        if let Some(l) = v.last_mut() {
            l.push('…');
        }
    }
    v
}

/// Draw one text line with its baseline at (`x`, `y`).
fn line(
    fonts: &mut FontManager,
    out: &mut Vec<Command>,
    s: &str,
    size: f32,
    x: f32,
    y: f32,
    color: Rgba,
) {
    let Some(id) = text::face(fonts) else {
        return;
    };
    if s.is_empty() {
        return;
    }
    let (mut c, _) = text::run(fonts, id, s, size, x, y);
    if let Command::Glyphs { color: col, .. } = &mut c {
        *col = color;
    }
    out.push(c);
}

/// Change bars, connectors and balloons of a page (the view's markup, see
/// `bo_layout::page_markup`).
fn draw_markup(fonts: &mut FontManager, out: &mut Vec<Command>, m: &PageMarkup, page_w: f32) {
    for b in &m.bars {
        out.push(Command::Line {
            x1: b.x,
            y1: b.top,
            x2: b.x,
            y2: b.bottom,
            width: 1.0,
            color: hex(&b.color),
            dash: None,
        });
    }
    for b in &m.balloons {
        draw_balloon(fonts, out, b, page_w);
    }
}

fn draw_balloon(fonts: &mut FontManager, out: &mut Vec<Command>, b: &Balloon, page_w: f32) {
    let color = hex(&b.color);
    if b.icon {
        out.push(Command::Path {
            segs: rect(b.x, b.y, b.w, b.h),
            fill: Some(bo_render::Paint::Solid {
                color: tint(color, 0.35),
            }),
            stroke: Some(stroke(0.75, color, Vec::new())),
        });
        return;
    }
    // Connector: from the anchor along the text line to the page edge, then to the balloon.
    let target_y = b.y + PAD + PITCH / 2.0;
    out.push(Command::Path {
        segs: vec![
            PathSeg::M(b.anchor_x, b.anchor_y),
            PathSeg::L(page_w, b.anchor_y),
            PathSeg::L(b.x, target_y),
        ],
        fill: None,
        stroke: Some(stroke(0.5, color, vec![2.0, 2.0])),
    });
    out.push(Command::Path {
        segs: rect(b.x, b.y, b.w, b.h),
        fill: Some(bo_render::Paint::Solid {
            color: tint(color, if b.done { 0.08 } else { 0.18 }),
        }),
        stroke: Some(stroke(0.75, color, Vec::new())),
    });
    let x = b.x + PAD;
    let w = (b.w - 2.0 * PAD).max(BALLOON_SIZE);
    let mut y = b.y + PAD + BALLOON_SIZE;
    line(fonts, out, &b.author, BALLOON_SIZE, x, y, color);
    let room = ((b.h - 2.0 * PAD) / PITCH).floor().max(1.0) as usize;
    for l in lines(
        fonts,
        &b.text,
        BALLOON_SIZE,
        w,
        room.saturating_sub(1).max(1),
    ) {
        y += PITCH;
        line(fonts, out, &l, BALLOON_SIZE, x, y, Rgba(0, 0, 0, 255));
    }
}

impl Engine {
    /// BlueDoc pages of a print job, in print layout, with or without markup.
    pub(crate) fn doc_print_pages(&mut self, o: &PrintOptions) -> Result<Vec<Logical>> {
        let saved_view = self.view_layout;
        let saved_markup = self.markup.clone();
        let to_print = saved_view.mode != ViewMode::Print;
        // Markup off: the text as if every change were accepted, nothing marked.
        let unmark = !o.doc.markup
            && saved_markup.mode != MarkupMode::None
            && saved_markup.mode != MarkupMode::Original;
        if to_print {
            self.view_command(&crate::edit::Command::SetViewLayout {
                mode: ViewMode::Print,
                width: None,
                height: None,
            })?;
        }
        // Hidden text shown with the formatting marks is not printed (§17.3.2.41 vanish).
        let marks = self.show_marks;
        if unmark || marks {
            if unmark {
                self.markup.mode = MarkupMode::None;
                self.view = self.markup.layout_view();
            }
            self.show_marks = false;
            self.view_changed = true;
            self.relayout()?;
        }
        let pages = self.doc_pages_now(o);
        if marks && !unmark {
            self.show_marks = true;
            self.view_changed = true;
            self.relayout()?;
        }
        if unmark {
            self.show_marks = marks;
            self.markup = saved_markup;
            self.view = self.markup.layout_view();
            self.view_changed = true;
            self.relayout()?;
        }
        if to_print {
            self.view_command(&crate::edit::Command::SetViewLayout {
                mode: saved_view.mode,
                width: Some(saved_view.width),
                height: Some(saved_view.height),
            })?;
        }
        pages
    }

    fn doc_pages_now(&mut self, o: &PrintOptions) -> Result<Vec<Logical>> {
        let keys: Vec<PageKey> = self
            .layout
            .document()
            .pages
            .iter()
            .map(|p| PageKey {
                number: p.number,
                section: p.section,
            })
            .collect();
        let chosen = choose(o, keys.len(), Some(&keys))?;
        let area = if o.doc.markup {
            (0..keys.len())
                .filter_map(|i| self.page_markup(i).map(|m| m.width))
                .fold(0.0_f32, f32::max)
        } else {
            0.0
        };
        let mut out = Vec::with_capacity(chosen.len());
        for i in chosen {
            let mut page = self.page(i)?.clone();
            // §17.2.1 background: the page colour is the first command (bo-render); left
            // out unless asked for.
            let coloured = self
                .layout
                .document()
                .pages
                .get(i)
                .is_some_and(|p| p.decor.background.is_some());
            if coloured && !o.doc.background {
                if let Some(Command::FillRect { x, y, w, h, .. }) = page.commands.first() {
                    if *x == 0.0 && *y == 0.0 && *w == page.width && *h == page.height {
                        page.commands.remove(0);
                    }
                }
            }
            if o.doc.markup {
                if let Some(m) = self.page_markup(i).cloned() {
                    draw_markup(&mut self.fonts, &mut page.commands, &m, page.width);
                }
                page.width += area;
            }
            out.push((i + 1, page));
        }
        if o.doc.properties {
            let size = self
                .layout
                .document()
                .pages
                .first()
                .map_or((595.0, 842.0), |p| (p.width, p.height));
            let page = self.properties_page(o, size);
            out.push((out.len() + 1, page));
        }
        Ok(out)
    }

    /// A page listing the document properties ("label: value", one per line).
    fn properties_page(&mut self, o: &PrintOptions, (w, h): (f32, f32)) -> PageList {
        const KEYS: [&str; 14] = [
            "title",
            "subject",
            "author",
            "keywords",
            "comments",
            "category",
            "template",
            "createdate",
            "savedate",
            "lastsavedby",
            "revnum",
            "filename",
            "pages",
            "words",
        ];
        let mut props = self
            .session
            .as_ref()
            .map(|s| s.field_env().props.clone())
            .unwrap_or_default();
        props.insert(
            "pages".into(),
            self.layout.document().pages.len().to_string(),
        );
        if let Some(d) = self.document() {
            props.insert("words".into(), self.words.get(d).to_string());
        }
        let mut commands = Vec::new();
        let (margin, size, pitch) = (72.0, 11.0, 18.0);
        let mut y = margin + size;
        for k in KEYS {
            let Some(v) = props.get(k).filter(|v| !v.trim().is_empty()) else {
                continue;
            };
            let label = o.doc.labels.get(k).map_or(k, String::as_str);
            for (j, l) in lines(
                &mut self.fonts,
                &format!("{label}: {v}"),
                size,
                w - 2.0 * margin,
                4,
            )
            .iter()
            .enumerate()
            {
                if y > h - margin {
                    break;
                }
                let x = if j == 0 { margin } else { margin + 18.0 };
                line(
                    &mut self.fonts,
                    &mut commands,
                    l,
                    size,
                    x,
                    y,
                    Rgba(0, 0, 0, 255),
                );
                y += pitch;
            }
        }
        PageList {
            width: w,
            height: h,
            commands,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours() {
        assert_eq!(hex("#1a2B3c"), Rgba(0x1a, 0x2b, 0x3c, 255));
        assert_eq!(hex("bad"), Rgba(89, 89, 89, 255));
        // Half red, half white: 255·0.5 + 255·0.5 = 255; 0·0.5 + 255·0.5 = 127.5 → 128.
        assert_eq!(tint(Rgba(255, 0, 0, 255), 0.5), Rgba(255, 128, 128, 255));
    }
}
