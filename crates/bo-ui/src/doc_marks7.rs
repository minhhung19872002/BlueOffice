//! Formatting marks of the native view (round 7): ¶ after paragraphs, · for spaces, → for
//! tabs, ¤ at cell and row ends, ¬ for optional hyphens, the anchor of floating objects —
//! the engine's marks of each page (`Engine::formatting_marks`, view only, never printed),
//! filtered by the kinds File › Options › Display turns on. Drawn over the page image in the
//! text colour, as the reference draws them.
use super::DocView;
use crate::{Font, Painter};
use bo_engine::views::{FormatMark, FormatMarkKind, MarkKinds};
use bo_render::{PathSeg, Rgba};

/// Marks of the pages in view and the kinds they were read with.
#[derive(Default)]
pub(crate) struct Marks {
    pages: std::collections::HashMap<usize, Vec<FormatMark>>,
    kinds: MarkKinds,
}

const INK: Rgba = Rgba(0, 0, 0, 255);

impl DocView {
    /// Read the marks of `pages` again (those redrawn, or all when the kinds changed).
    pub(crate) fn refresh_marks(&mut self, pages: &[usize], wanted: &[usize]) {
        let kinds = self.engine.formatting_mark_kinds();
        if !kinds.any() {
            self.marks.pages.clear();
            self.marks.kinds = kinds;
            return;
        }
        let all = kinds != self.marks.kinds;
        self.marks.kinds = kinds;
        self.marks.pages.retain(|i, _| wanted.contains(i));
        for &i in wanted {
            if all || pages.contains(&i) || !self.marks.pages.contains_key(&i) {
                let m = self.engine.formatting_marks(i);
                self.marks.pages.insert(i, m);
            }
        }
    }

    /// Draw the marks of page slot `(index, x, y)`.
    pub(crate) fn paint_marks(&self, p: &mut dyn Painter, (index, sx, sy): (usize, i32, i32)) {
        let Some(marks) = self.marks.pages.get(&index) else {
            return;
        };
        let s = self.scale();
        let (x0, y0) = (sx as f32, sy as f32);
        // Object anchors on their own show for the selected object only, as the reference.
        let anchors = self.engine.shows_formatting_marks()
            || self.format.picture.is_some()
            || self.format.picture_tools;
        for m in marks
            .iter()
            .filter(|m| anchors || m.kind != FormatMarkKind::Anchor)
        {
            let size = (m.size * s).max(4.0);
            let font = Font {
                family: "Segoe UI",
                size,
                weight: 400,
                italic: false,
                underline: false,
            };
            let base = y0 + m.baseline * s;
            // Top of a Segoe UI line box whose baseline is `base` (ascent 1.079 em).
            let top = (base - size * 1.079).round();
            let x = x0 + m.x * s;
            let w = m.w * s;
            match m.kind {
                FormatMarkKind::Space | FormatMarkKind::NoBreakSpace => {
                    let k = (size * 0.09).max(1.0).round();
                    let cx = (x + w / 2.0 - k / 2.0).round();
                    let cy = (base - size * 0.33).round();
                    if m.kind == FormatMarkKind::Space {
                        p.fill_rect(cx, cy, k, k, INK);
                    } else {
                        p.text_font("°", cx - size * 0.1, top, &font, INK);
                    }
                }
                FormatMarkKind::Tab => {
                    // An arrow across the middle of the tab's span.
                    let len = w.min(size * 0.75).max(size * 0.4);
                    let ax = (x + (w - len) / 2.0).round();
                    let ay = (base - size * 0.3).round() + 0.5;
                    let head = size * 0.18;
                    p.path(
                        &[
                            PathSeg::M(ax, ay),
                            PathSeg::L(ax + len, ay),
                            PathSeg::M(ax + len - head, ay - head),
                            PathSeg::L(ax + len, ay),
                            PathSeg::L(ax + len - head, ay + head),
                        ],
                        None,
                        Some((INK, (size * 0.06).max(1.0))),
                    );
                }
                FormatMarkKind::Paragraph => p.text_font("¶", x.round(), top, &font, INK),
                FormatMarkKind::CellEnd | FormatMarkKind::RowEnd => {
                    p.text_font("¤", x.round(), top, &font, INK);
                }
                FormatMarkKind::LineBreak => p.text_font("↵", x.round(), top, &font, INK),
                FormatMarkKind::OptionalHyphen => {
                    // ¬ drawn as a bar with a down tick at the hyphen's height, centred on
                    // the break (the layout gives it no width).
                    let w = size * 0.32;
                    let (l, y) = ((x - w / 2.0).round(), (base - size * 0.3).round() + 0.5);
                    let t = (size * 0.07).max(1.0);
                    p.path(
                        &[
                            PathSeg::M(l, y),
                            PathSeg::L(l + w, y),
                            PathSeg::L(l + w, y + size * 0.16),
                        ],
                        None,
                        Some((INK, t)),
                    );
                }
                FormatMarkKind::PageBreak | FormatMarkKind::ColumnBreak => {
                    let label = if m.kind == FormatMarkKind::PageBreak {
                        "Page Break"
                    } else {
                        "Column Break"
                    };
                    let small = Font {
                        size: size * 0.75,
                        ..font
                    };
                    p.text_font(&format!("……{label}……¶"), x.round(), top, &small, INK);
                }
                FormatMarkKind::Anchor => {
                    // An anchor shape (own drawing): ring, shank and arms.
                    let k = size * 0.4;
                    let cx = x.round() + k;
                    let ty = (y0 + m.top * s).round() + k * 0.4;
                    p.path(
                        &[
                            PathSeg::M(cx, ty + k * 0.5),
                            PathSeg::L(cx, ty + k * 2.2),
                            PathSeg::M(cx - k * 0.8, ty + k * 1.6),
                            PathSeg::Q(cx - k * 0.6, ty + k * 2.3, cx, ty + k * 2.3),
                            PathSeg::Q(cx + k * 0.6, ty + k * 2.3, cx + k * 0.8, ty + k * 1.6),
                            PathSeg::M(cx - k * 0.5, ty + k * 0.9),
                            PathSeg::L(cx + k * 0.5, ty + k * 0.9),
                        ],
                        None,
                        Some((INK, (size * 0.06).max(1.0))),
                    );
                    p.path(
                        &[
                            PathSeg::M(cx + k * 0.25, ty + k * 0.25),
                            PathSeg::Q(cx + k * 0.25, ty, cx, ty),
                            PathSeg::Q(cx - k * 0.25, ty, cx - k * 0.25, ty + k * 0.25),
                            PathSeg::Q(cx - k * 0.25, ty + k * 0.5, cx, ty + k * 0.5),
                            PathSeg::Q(cx + k * 0.25, ty + k * 0.5, cx + k * 0.25, ty + k * 0.25),
                        ],
                        None,
                        Some((INK, (size * 0.05).max(1.0))),
                    );
                }
            }
        }
    }
}
