//! Edit Object arrangement (ISO 32000-1 §8.4.4 `cm`, §9.4.2 `Tm`): copies of page objects
//! (Ctrl+drag), and the matrices of rotating, flipping and aligning objects.
//!
//! A copy of an image, path, form or shading is written right after the original inside the
//! same graphics state (`<original> q <m> cm <original> Q`), so it draws with the same colours
//! and clip, one step above it; a copied text run is written at the end of the content with
//! its full text state, like a moved one.
#![allow(clippy::single_match_else)]
use crate::interp::{invert, mul};
use crate::objects::{deleted_run, matrix, show_operator, text_object, ObjectKind};
use crate::PdfDocument;

/// Matrix turning page points by `quarter_turns` × 90° clockwise about `(cx, cy)` (page
/// points, y down).
pub fn rotation_about(quarter_turns: i32, cx: f32, cy: f32) -> [f32; 6] {
    let (s, c) = match quarter_turns.rem_euclid(4) {
        0 => (0.0, 1.0),
        1 => (1.0, 0.0),
        2 => (0.0, -1.0),
        _ => (-1.0, 0.0),
    };
    // (x, y) → (c x − s y, s x + c y): clockwise on a y-down page.
    [c, s, -s, c, cx - c * cx + s * cy, cy - s * cx - c * cy]
}

/// Matrix mirroring page points about the vertical (`horizontal` flip) or horizontal line
/// through `(cx, cy)`.
pub fn flip_about(horizontal: bool, cx: f32, cy: f32) -> [f32; 6] {
    if horizontal {
        [-1.0, 0.0, 0.0, 1.0, 2.0 * cx, 0.0]
    } else {
        [1.0, 0.0, 0.0, -1.0, 0.0, 2.0 * cy]
    }
}

impl PdfDocument {
    /// Add copies of page objects `ids` of page `index`, transformed by `m` in page points
    /// (a Ctrl+drag copy is `[1, 0, 0, 1, dx, dy]`); one undo step.
    pub fn copy_objects(&mut self, index: usize, ids: &[usize], m: [f32; 6]) -> Result<(), String> {
        self.begin("copy objects");
        let r = self.copy_inner(index, ids, m);
        self.commit();
        r
    }

    /// Transform each listed page object of page `index` by its own matrix (page points):
    /// aligning or distributing several objects; one undo step.
    pub fn transform_each(
        &mut self,
        index: usize,
        moves: &[(usize, [f32; 6])],
    ) -> Result<(), String> {
        self.begin("arrange objects");
        let r = self.each_inner(index, moves);
        self.commit();
        r
    }

    fn each_inner(&mut self, index: usize, moves: &[(usize, [f32; 6])]) -> Result<(), String> {
        let (base, inv_base) = self.bases(index)?;
        let ids: Vec<usize> = moves.iter().map(|m| m.0).collect();
        let (sid, content, objs) = self.edit_page(index, &ids)?;
        let mut edits = Vec::new();
        let mut tail = Vec::new();
        for &(i, m) in moves {
            let rec = &objs[i].rec;
            let m_user = mul(mul(base, m), inv_base);
            let ctm_u = mul(rec.ctm, inv_base);
            let inv_ctm = invert(ctm_u).ok_or("singular matrix")?;
            match (&rec.text, rec.kind) {
                (Some(ts), ObjectKind::Text) => {
                    edits.push((rec.span, deleted_run(rec).into_bytes()));
                    let tm = mul(mul(mul(ts.tm, ctm_u), m_user), inv_ctm);
                    tail.extend(text_object(ts, &ts.font, ctm_u, tm, &show_operator(rec)));
                }
                _ => {
                    let x = mul(mul(ctm_u, m_user), inv_ctm);
                    let mut with = format!("q {} cm\n", matrix(x)).into_bytes();
                    with.extend_from_slice(content.get(rec.span.0..rec.span.1).unwrap_or(&[]));
                    with.extend_from_slice(b"\nQ");
                    edits.push((rec.span, with));
                }
            }
        }
        self.write_page(sid, content, edits, &tail);
        Ok(())
    }

    fn copy_inner(&mut self, index: usize, ids: &[usize], m: [f32; 6]) -> Result<(), String> {
        let (base, inv_base) = self.bases(index)?;
        let (sid, content, objs) = self.edit_page(index, ids)?;
        let m_user = mul(mul(base, m), inv_base);
        let mut edits = Vec::new();
        let mut tail = Vec::new();
        for &i in ids {
            let rec = &objs[i].rec;
            let ctm_u = mul(rec.ctm, inv_base);
            let inv_ctm = invert(ctm_u).ok_or("singular matrix")?;
            match (&rec.text, rec.kind) {
                (Some(ts), ObjectKind::Text) => {
                    let tm = mul(mul(mul(ts.tm, ctm_u), m_user), inv_ctm);
                    tail.extend(text_object(ts, &ts.font, ctm_u, tm, &show_operator(rec)));
                }
                _ => {
                    let x = mul(mul(ctm_u, m_user), inv_ctm);
                    let own = content.get(rec.span.0..rec.span.1).unwrap_or(&[]);
                    let mut with = own.to_vec();
                    with.extend(format!("\nq {} cm\n", matrix(x)).into_bytes());
                    with.extend_from_slice(own);
                    with.extend_from_slice(b"\nQ");
                    edits.push((rec.span, with));
                }
            }
        }
        self.write_page(sid, content, edits, &tail);
        Ok(())
    }
}
