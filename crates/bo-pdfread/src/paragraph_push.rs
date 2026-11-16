//! What moves when a paragraph being edited grows or shrinks by whole lines.
//!
//! Observed in the reference editor (black-box, `docs/ui/REPORT-editor.md` round 7): Edit Text
//! groups paragraphs that follow each other closely in one column into one box; when the
//! paragraph being typed in gets more lines, the paragraphs after it in that box move down with
//! it while typing (and up again when it loses lines). Everything else — rules, images, a
//! paragraph further down after a larger gap — stays where it is and is overlapped. With
//! Reflow Editing on, everything below the paragraph moves instead.
//!
//! The followers are written like a move of page objects (`transform_objects`): text runs are
//! neutralised where they were and written again at the end of the prefix, other objects are
//! wrapped in `q … cm … Q` in place. The prefix still leaves the graphics state as it found it
//! (the typing fast path relies on that).
use crate::interp::{invert, mul, TextState, M};
use crate::objects::{deleted_run, matrix, show_operator, text_object, ObjectKind, PageObject};
use crate::paragraph::{text_blocks, BlockEdit};
use crate::PdfDocument;

/// What follows a paragraph being edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushMode {
    /// The paragraphs after it in its box (the reference editor's default).
    Box,
    /// Everything below it on the page (Reflow Editing on).
    Below,
}

/// One object that moves with the paragraph.
#[derive(Debug, Clone)]
pub(crate) struct Follower {
    span: (usize, usize),
    /// Its CTM relative to default user space and the inverse.
    ctm_u: M,
    inv_ctm: M,
    /// A text run: its state, the operators that neutralise it and its showing operator.
    text: Option<(TextState, Vec<u8>, Vec<u8>)>,
}

/// The followers of a block edit and the prefix they were found in.
#[derive(Debug, Clone)]
pub(crate) struct Push {
    original: Vec<u8>,
    pub(crate) followers: Vec<Follower>,
    base: M,
    inv_base: M,
    /// Lines of the paragraph when the edit began.
    lines0: usize,
    /// Shift now applied to the followers (page points, along the block's line direction).
    applied: f32,
    /// Box of the paragraphs after it in its box where they were (Box mode, page points).
    group: Option<[f32; 4]>,
}

impl Push {
    /// The prefix with the followers moved by `d` page points.
    fn content(&self, d: (f32, f32)) -> Vec<u8> {
        if self.followers.is_empty() || (d.0.abs() < 1e-4 && d.1.abs() < 1e-4) {
            return self.original.clone();
        }
        let m_user = mul(
            mul(self.base, [1.0, 0.0, 0.0, 1.0, d.0, d.1]),
            self.inv_base,
        );
        let mut edits: Vec<((usize, usize), Vec<u8>)> = Vec::new();
        let mut tail = Vec::new();
        for f in &self.followers {
            if let Some((ts, gone, show)) = &f.text {
                edits.push((f.span, gone.clone()));
                let tm = mul(mul(mul(ts.tm, f.ctm_u), m_user), f.inv_ctm);
                tail.extend(text_object(ts, &ts.font, f.ctm_u, tm, show));
            } else {
                let x = mul(mul(f.ctm_u, m_user), f.inv_ctm);
                let mut with = format!("q {} cm\n", matrix(x)).into_bytes();
                with.extend_from_slice(self.original.get(f.span.0..f.span.1).unwrap_or(&[]));
                with.extend_from_slice(b"\nQ");
                edits.push((f.span, with));
            }
        }
        let mut out = self.original.clone();
        edits.sort_by_key(|e| std::cmp::Reverse(e.0 .0));
        for ((a, b), with) in edits {
            if a <= b && b <= out.len() {
                out.splice(a..b, with);
            }
        }
        out.extend(tail);
        out
    }
}

/// Indices of the objects of `objs` that follow a paragraph with box `bbox` and font size
/// `size` in `mode`.
fn followers_of(objs: &[PageObject], bbox: [f32; 4], size: f32, mode: PushMode) -> Vec<usize> {
    let shown = |o: &PageObject| o.kind != ObjectKind::Text || !o.text.trim().is_empty();
    match mode {
        PushMode::Below => objs
            .iter()
            .enumerate()
            .filter(|(_, o)| shown(o) && o.bbox[1] >= bbox[3] - 1.0)
            .map(|(i, _)| i)
            .collect(),
        PushMode::Box => {
            // Paragraphs below in the same column, each starting within two lines of the
            // one before (the reference editor's box); the first larger gap ends the box.
            let mut below: Vec<_> = text_blocks(objs)
                .into_iter()
                .filter(|b| b.bbox[1] >= bbox[3] - 0.25 * size)
                .filter(|b| b.bbox[0] < bbox[2] && b.bbox[2] > bbox[0])
                .collect();
            below.sort_by(|a, b| a.bbox[1].total_cmp(&b.bbox[1]));
            let mut bottom = bbox[3];
            let mut out = Vec::new();
            for b in below {
                if b.bbox[1] - bottom > 2.0 * size.max(b.font_size) {
                    break;
                }
                bottom = bottom.max(b.bbox[3]);
                out.extend(b.ids.iter().copied().filter(|&i| shown(&objs[i])));
            }
            out
        }
    }
}

impl BlockEdit {
    /// Shift now applied to what follows the paragraph (page points; 0 without followers).
    #[must_use]
    pub fn follower_shift(&self) -> f32 {
        self.push.as_ref().map_or(0.0, |p| p.applied)
    }

    /// The editing frame (page points): the paragraph and the paragraphs after it in its box
    /// where they are now — the reference editor frames the whole box.
    #[must_use]
    pub fn frame(&self) -> [f32; 4] {
        let b = self.bbox;
        let Some((g, d)) = self.push.as_ref().and_then(|p| Some((p.group?, p.applied))) else {
            return b;
        };
        let (vx, vy) = self.across();
        let g = [g[0] + vx * d, g[1] + vy * d, g[2] + vx * d, g[3] + vy * d];
        [
            b[0].min(g[0]),
            b[1].min(g[1]),
            b[2].max(g[2]),
            b[3].max(g[3]),
        ]
    }

    /// Width the paragraph's lines wrap at (points).
    #[must_use]
    pub fn wrap_width(&self) -> f32 {
        self.width
    }

    /// Wrap the lines at `w` points (dragging the frame's side; at least 12 points). Lay out
    /// again with [`PdfDocument::block_edit_update`].
    pub fn set_wrap_width(&mut self, w: f32) {
        if w.is_finite() {
            self.width = w.max(12.0);
        }
    }
}

impl PdfDocument {
    /// Make the paragraphs after the one `e` edits (or everything below it) move with it when
    /// it gains or loses lines; takes effect from the next [`PdfDocument::block_edit_update`].
    pub fn block_edit_push(&mut self, e: &mut BlockEdit, mode: PushMode) -> Result<(), String> {
        let (base, inv_base) = self.bases(e.page)?;
        let objs = self.objects_in(e.page, Some(&e.prefix))?;
        let size = e.styles.first().map_or(10.0, |s| s.size).max(1.0);
        let ids = followers_of(&objs, e.bbox, size, mode);
        let group = (mode == PushMode::Box)
            .then(|| {
                ids.iter().map(|&i| objs[i].bbox).reduce(|a, b| {
                    [
                        a[0].min(b[0]),
                        a[1].min(b[1]),
                        a[2].max(b[2]),
                        a[3].max(b[3]),
                    ]
                })
            })
            .flatten();
        let followers = ids
            .into_iter()
            .filter_map(|i| {
                let rec = &objs[i].rec;
                let ctm_u = mul(rec.ctm, inv_base);
                let inv_ctm = invert(ctm_u)?;
                let text = rec.text.as_ref().map(|ts| {
                    (
                        ts.clone(),
                        deleted_run(rec).into_bytes(),
                        show_operator(rec),
                    )
                });
                Some(Follower {
                    span: rec.span,
                    ctm_u,
                    inv_ctm,
                    text,
                })
            })
            .collect();
        e.push = Some(Push {
            original: e.prefix.clone(),
            followers,
            base,
            inv_base,
            lines0: e.lines,
            applied: 0.0,
            group,
        });
        Ok(())
    }
}

/// Move the followers of `e` by the lines it gained or lost since the edit began (rewrites
/// its prefix when that changed).
pub(crate) fn update(e: &mut BlockEdit) {
    let (vx, vy) = e.across();
    let Some(p) = e.push.as_mut() else {
        return;
    };
    #[allow(clippy::cast_precision_loss)]
    let shift = (e.lines as f32 - p.lines0 as f32) * e.pitch;
    if (shift - p.applied).abs() < 1e-3 {
        return;
    }
    p.applied = shift;
    e.prefix = p.content((vx * shift, vy * shift));
}
