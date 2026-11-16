//! Edit Object's Split commands on text objects (ISO 32000-1 §9.4 text objects, §14.6 marked
//! content, §8.5 path construction):
//!
//! - split a paragraph into its lines: each line's runs are wrapped in a marked-content sequence
//!   `/BOLine <</N n>> BDC … EMC` with its own number, and paragraph grouping never joins lines
//!   of different numbers, so every line is an object of its own (the drawing is unchanged);
//! - remove kerning: kerning-sized numbers of `TJ` arrays (§9.4.3) are dropped, word gaps made
//!   with larger numbers stay;
//! - convert to shapes: the runs are replaced by their glyph outlines filled as paths, in the
//!   run's fill colour.
//!
//! And the Effect section on other objects: constant opacity (§11.6.4.4 `/ca` `/CA` through an
//! ExtGState, `gs`) and shear (`cm`, §8.3.4) about the object's box.
use crate::interp::mul;
use crate::objects::{deleted_run, num, obj_bytes, ObjectKind, PageObject};
use crate::paragraph::TextBlock;
use crate::PdfDocument;
use bo_render::{Command, Paint, PathSeg};
use lopdf::Object;
use std::fmt::Write as _;

/// `TJ` numbers (thousandths of an em) at most this large are kerning, larger ones word gaps.
const KERNING: f32 = 100.0;

/// Lines of `block`: its run ids grouped by baseline, top to bottom.
fn block_lines(objs: &[PageObject], block: &TextBlock) -> Vec<Vec<usize>> {
    let size = block.font_size.max(1.0);
    let mut lines: Vec<(f32, Vec<usize>)> = Vec::new();
    for &i in &block.ids {
        let Some(o) = objs.get(i) else { continue };
        let y = o.bbox[3];
        match lines.iter_mut().find(|l| (l.0 - y).abs() < 0.5 * size) {
            Some(l) => l.1.push(i),
            None => lines.push((y, vec![i])),
        }
    }
    lines.sort_by(|a, b| a.0.total_cmp(&b.0));
    lines.into_iter().map(|l| l.1).collect()
}

/// A `TJ` array without kerning-sized numbers; `None` when it has none.
fn unkerned(args: &[Object]) -> Option<Vec<u8>> {
    let Some(Object::Array(items)) = args.first() else {
        return None;
    };
    let small = |o: &Object| match o {
        Object::Integer(n) => (*n as f32).abs() <= KERNING,
        Object::Real(r) => r.abs() <= KERNING,
        _ => false,
    };
    if !items
        .iter()
        .any(|o| small(o) && !matches!(o, Object::Integer(0)))
    {
        return None;
    }
    let kept: Vec<Object> = items.iter().filter(|o| !small(o)).cloned().collect();
    let mut out = obj_bytes(&Object::Array(kept));
    out.extend_from_slice(b" TJ");
    Some(out)
}

/// Content of path `segs` (page points) in user space through `inv_base`.
fn path_ops(segs: &[PathSeg], inv: [f32; 6], out: &mut String) {
    let p = |x: f32, y: f32| {
        let q = mul([1.0, 0.0, 0.0, 1.0, x, y], inv);
        format!("{} {}", num(q[4]), num(q[5]))
    };
    let mut cur = (0.0, 0.0);
    for s in segs {
        let _ = match *s {
            PathSeg::M(x, y) => {
                cur = (x, y);
                writeln!(out, "{} m", p(x, y))
            }
            PathSeg::L(x, y) => {
                cur = (x, y);
                writeln!(out, "{} l", p(x, y))
            }
            PathSeg::C(a, b, c, d, x, y) => {
                cur = (x, y);
                writeln!(out, "{} {} {} c", p(a, b), p(c, d), p(x, y))
            }
            PathSeg::Q(a, b, x, y) => {
                // Quadratic to cubic: control points two thirds of the way to (a, b).
                let c1 = (
                    cur.0 + 2.0 / 3.0 * (a - cur.0),
                    cur.1 + 2.0 / 3.0 * (b - cur.1),
                );
                let c2 = (x + 2.0 / 3.0 * (a - x), y + 2.0 / 3.0 * (b - y));
                cur = (x, y);
                writeln!(out, "{} {} {} c", p(c1.0, c1.1), p(c2.0, c2.1), p(x, y))
            }
            PathSeg::Z => writeln!(out, "h"),
        };
    }
}

impl PdfDocument {
    /// Whether text runs `ids` of page `index` have kerning that
    /// [`PdfDocument::remove_kerning`] would drop.
    pub fn has_kerning(&mut self, index: usize, ids: &[usize]) -> bool {
        let Ok(objs) = self.page_objects(index) else {
            return false;
        };
        ids.iter().any(|&i| {
            objs.get(i)
                .is_some_and(|o| o.rec.op == b"TJ" && unkerned(&o.rec.args).is_some())
        })
    }

    /// Split paragraph `block` of page `index` into one text object per line; returns the
    /// number of lines (one undo step; nothing changes for a single line).
    pub fn split_text_lines(&mut self, index: usize, block: &TextBlock) -> Result<usize, String> {
        self.begin("split text");
        let r = self.split_inner(index, block);
        self.commit();
        r
    }

    fn split_inner(&mut self, index: usize, block: &TextBlock) -> Result<usize, String> {
        let (sid, content, objs) = self.edit_page(index, &block.ids)?;
        let lines = block_lines(&objs, block);
        if lines.len() < 2 {
            return Ok(lines.len());
        }
        let first = objs.iter().filter_map(|o| o.rec.mark).max().unwrap_or(0) + 1;
        let mut edits = Vec::new();
        for (n, line) in (first..).zip(&lines) {
            for &i in line {
                let rec = &objs[i].rec;
                let mut with = format!("/BOLine <</N {n}>> BDC ").into_bytes();
                with.extend_from_slice(content.get(rec.span.0..rec.span.1).unwrap_or(&[]));
                with.extend_from_slice(b" EMC");
                edits.push((rec.span, with));
            }
        }
        self.write_page(sid, content, edits, &[]);
        Ok(lines.len())
    }

    /// Drop kerning from text runs `ids` of page `index`; returns how many runs changed (one
    /// undo step).
    pub fn remove_kerning(&mut self, index: usize, ids: &[usize]) -> Result<usize, String> {
        self.begin("remove kerning");
        let r = (|| {
            let (sid, content, objs) = self.edit_page(index, ids)?;
            let edits: Vec<_> = ids
                .iter()
                .filter_map(|&i| {
                    let rec = &objs[i].rec;
                    (rec.op == b"TJ")
                        .then(|| unkerned(&rec.args))
                        .flatten()
                        .map(|w| (rec.span, w))
                })
                .collect();
            let n = edits.len();
            if n > 0 {
                self.write_page(sid, content, edits, &[]);
            }
            Ok(n)
        })();
        self.commit();
        r
    }

    /// Replace text runs `ids` of page `index` by their glyph outlines filled in the run's
    /// colour (one undo step).
    pub fn text_to_shapes(&mut self, index: usize, ids: &[usize]) -> Result<(), String> {
        self.begin("convert to shapes");
        let r = (|| {
            let (_, inv_base) = self.bases(index)?;
            let (sid, content, objs) = self.edit_page(index, ids)?;
            let drawn = self.objects_drawn(index, None, true)?;
            if drawn.len() != objs.len() {
                return Err("the page changed".into());
            }
            let mut edits = Vec::new();
            let mut tail = String::from("q\n");
            for &i in ids {
                let rec = &objs[i].rec;
                if rec.kind != ObjectKind::Text {
                    continue;
                }
                edits.push((rec.span, deleted_run(rec).into_bytes()));
                for c in &drawn[i].1 {
                    if let Command::Path { segs, fill, .. } = c {
                        let col = match fill {
                            Some(Paint::Solid { color }) => *color,
                            _ => rec
                                .text
                                .as_ref()
                                .map_or(bo_render::Rgba(0, 0, 0, 255), |t| t.fill),
                        };
                        let _ = writeln!(
                            tail,
                            "{} {} {} rg",
                            num(f32::from(col.0) / 255.0),
                            num(f32::from(col.1) / 255.0),
                            num(f32::from(col.2) / 255.0)
                        );
                        path_ops(segs, inv_base, &mut tail);
                        tail.push_str("f\n");
                    }
                }
            }
            tail.push_str("Q\n");
            if edits.is_empty() {
                return Err("no text".into());
            }
            self.write_page(sid, content, edits, tail.as_bytes());
            Ok(())
        })();
        self.commit();
        r
    }

    /// Draw page objects `ids` of page `index` with constant opacity `alpha` (0–1; one undo
    /// step).
    pub fn set_object_opacity(
        &mut self,
        index: usize,
        ids: &[usize],
        alpha: f32,
    ) -> Result<(), String> {
        self.begin("opacity");
        let r = (|| {
            let a = alpha.clamp(0.0, 1.0);
            let (sid, content, objs) = self.edit_page(index, ids)?;
            let mut gs = lopdf::Dictionary::new();
            gs.set("Type", Object::Name(b"ExtGState".to_vec()));
            gs.set("ca", Object::Real(a));
            gs.set("CA", Object::Real(a));
            let id = self.add(Object::Dictionary(gs));
            let name = self.add_resource(index, b"ExtGState", "BoOp", id)?;
            let name = String::from_utf8_lossy(&name).into_owned();
            let mut edits = Vec::new();
            for &i in ids {
                let rec = &objs[i].rec;
                if rec.kind == ObjectKind::Text {
                    continue;
                }
                let mut with = format!(
                    "q /{name} gs
"
                )
                .into_bytes();
                with.extend_from_slice(content.get(rec.span.0..rec.span.1).unwrap_or(&[]));
                with.extend_from_slice(
                    b"
Q",
                );
                edits.push((rec.span, with));
            }
            if edits.is_empty() {
                return Err("no object".into());
            }
            self.write_page(sid, content, edits, &[]);
            Ok(())
        })();
        self.commit();
        r
    }

    /// Opacity an object draws with (1 when it sets none): the `/ca` of the ExtGState applied
    /// right before it inside its own `q … Q`, as [`PdfDocument::set_object_opacity`] writes.
    pub fn object_opacity(&mut self, index: usize, id: usize) -> f32 {
        let Ok(objs) = self.page_objects(index) else {
            return 1.0;
        };
        let Some(o) = objs.get(id) else { return 1.0 };
        // The bytes `page_objects` ran (spans index into them).
        let content = match self.pages.get(index).map(|i| i.id) {
            Some(pid) => match self.doc.get_dictionary(pid) {
                Ok(page) => crate::page_content(&self.doc, page),
                Err(_) => return 1.0,
            },
            None => return 1.0,
        };
        let before = &content[..o.rec.span.0.min(content.len())];
        let tail = &before[before.len().saturating_sub(40)..];
        let tail = String::from_utf8_lossy(tail);
        let Some(name) = tail
            .rsplit_once(" gs")
            .and_then(|(h, rest)| rest.trim().is_empty().then_some(h))
            .and_then(|h| h.rsplit_once('/'))
            .map(|(_, n)| n.trim().to_owned())
        else {
            return 1.0;
        };
        self.ext_gstate_alpha(index, name.as_bytes()).unwrap_or(1.0)
    }

    fn ext_gstate_alpha(&self, index: usize, name: &[u8]) -> Option<f32> {
        let info = self.pages.get(index)?;
        let doc = &self.doc;
        let page = doc.get_dictionary(info.id).ok()?;
        let res = crate::inherited(doc, page, b"Resources")?.as_dict().ok()?;
        let ext = crate::resolve(doc, res.get(b"ExtGState").ok()?)
            .as_dict()
            .ok()?;
        let gs = crate::resolve(doc, ext.get(name).ok()?).as_dict().ok()?;
        match gs.get(b"ca").ok()? {
            Object::Real(r) => Some(*r),
            Object::Integer(i) => Some(*i as f32),
            _ => None,
        }
    }

    /// Shear page objects `ids` of page `index` across by `degrees` about the bottom of their
    /// joint box (page points; one undo step).
    pub fn shear_objects(
        &mut self,
        index: usize,
        ids: &[usize],
        bbox: [f32; 4],
        degrees: f32,
    ) -> Result<(), String> {
        let t = degrees.to_radians().tan();
        // Page y grows downwards: x' = x - t (y - bottom).
        let m = [1.0, 0.0, -t, 1.0, t * bbox[3], 0.0];
        let moves: Vec<(usize, [f32; 6])> = ids.iter().map(|&i| (i, m)).collect();
        self.transform_each(index, &moves)
    }
}
