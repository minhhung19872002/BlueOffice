//! Page objects for BlueEditor's Edit Text / Edit Object (ISO 32000-1 §8.2 graphics objects,
//! §9.4 text objects): the content stream is run once more with every top-level operator's
//! byte range recorded, giving text runs (one per text-showing operator), images, paths, forms
//! and shadings with their boxes. Edits splice the page's content stream:
//!
//! - a deleted text run becomes a `TJ` that only moves the text position by the run's
//!   advance, so the text after it stays where it was;
//! - moved, resized or edited text is written again at the end of the stream with its full
//!   text state (font, size, spacing, colour, matrix), in the run's own font when it has
//!   glyphs for every character, otherwise in an embedded Unicode font ([`crate::fallback`]);
//! - moved or resized images, paths and forms are wrapped in `q … cm … Q`;
//! - new images are image XObjects (JPEG as `DCTDecode`, PNG decoded, alpha as `/SMask`).
//!
//! The page's content becomes one stream wrapped in `q … Q` on its first edit, so the graphics
//! state at its end is the page's initial one.
#![allow(clippy::single_match_else)]
use crate::interp::{invert, mul, Interp, Shared, TextState, IDENTITY, M};
use crate::{filters, inherited, page_content, page_matrix, resolve, PdfDocument, RenderOptions};
use bo_render::{Command, PathSeg, Rgba};
use lopdf::{Dictionary, Object, ObjectId, Stream};
use std::fmt::Write as _;

/// What a page object is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    /// A text run (one text-showing operator).
    Text,
    /// An image XObject or inline image.
    Image,
    /// A filled and/or stroked path.
    Path,
    /// A form XObject.
    Form,
    /// A shading fill (`sh`).
    Shading,
}

/// A top-level operator that drew something, as the interpreter saw it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Recorded {
    pub kind: ObjectKind,
    /// Byte range in the page content (all content streams joined).
    pub span: (usize, usize),
    /// Display list commands and shown characters it produced.
    pub commands: (usize, usize),
    pub chars: (usize, usize),
    /// Current transformation matrix (user space to page points) at the operator.
    pub ctm: M,
    pub op: Vec<u8>,
    pub args: Vec<Object>,
    pub text: Option<TextState>,
}

/// A page object with its box in page points (origin top-left, y down).
#[derive(Debug, Clone, PartialEq)]
pub struct PageObject {
    /// What it is.
    pub kind: ObjectKind,
    /// Left, top, right, bottom.
    pub bbox: [f32; 4],
    /// Text of a text run.
    pub text: String,
    /// Boxes of the run's characters (for a caret).
    pub char_boxes: Vec<[f32; 4]>,
    /// Font size on the page (text runs).
    pub font_size: f32,
    /// Fill colour (text runs).
    pub color: [u8; 3],
    pub(crate) rec: Recorded,
}

/// Text runs of one font, size and colour that follow each other on one baseline (a line or
/// a part of one): what Edit Text edits as a unit.
#[derive(Debug, Clone, PartialEq)]
pub struct TextLine {
    /// Page object indices of its runs.
    pub ids: Vec<usize>,
    /// Left, top, right, bottom in page points.
    pub bbox: [f32; 4],
    /// Its text (word gaps without a space character become spaces).
    pub text: String,
    /// One box per character of `text`.
    pub char_boxes: Vec<[f32; 4]>,
    /// Font size on the page.
    pub font_size: f32,
    /// Fill colour.
    pub color: [u8; 3],
    /// Page object index of each character of `text` (a word gap counts to the run before).
    pub runs: Vec<usize>,
}

/// Group consecutive text runs into lines.
pub fn text_lines(objs: &[PageObject]) -> Vec<TextLine> {
    let mut out: Vec<TextLine> = Vec::new();
    let mut last: Option<(usize, Vec<u8>, f32, [u8; 3])> = None;
    for (i, o) in objs.iter().enumerate() {
        let Some(ts) = o.rec.text.as_ref().filter(|_| o.kind == ObjectKind::Text) else {
            last = None;
            continue;
        };
        if o.text.is_empty() {
            continue;
        }
        let key = (ts.font.clone(), o.font_size, o.color);
        let joins = match (&last, out.last()) {
            (Some((li, f, s, c)), Some(line)) if *li + 1 == i || line.ids.last() == Some(li) => {
                let em = s.max(1.0);
                let same = *f == key.0 && (s - key.1).abs() < 0.01 && *c == key.2;
                let base = (line.bbox[3] - o.bbox[3]).abs() < 0.25 * em;
                let gap = o.bbox[0] - line.bbox[2];
                same && base && gap > -0.3 * em && gap < 1.2 * em
            }
            _ => false,
        };
        if joins {
            if let Some(line) = out.last_mut() {
                let gap = o.bbox[0] - line.bbox[2];
                let spaced = line.text.ends_with(' ') || o.text.starts_with(' ');
                if gap > 0.2 * o.font_size.max(1.0) && !spaced {
                    line.text.push(' ');
                    line.char_boxes
                        .push([line.bbox[2], o.bbox[1], o.bbox[0], o.bbox[3]]);
                    let prev = line.ids.last().copied().unwrap_or(i);
                    line.runs.push(prev);
                }
                line.ids.push(i);
                line.text.push_str(&o.text);
                line.runs
                    .extend(std::iter::repeat_n(i, o.text.chars().count()));
                line.char_boxes.extend(o.char_boxes.iter().copied());
                line.bbox = [
                    line.bbox[0].min(o.bbox[0]),
                    line.bbox[1].min(o.bbox[1]),
                    line.bbox[2].max(o.bbox[2]),
                    line.bbox[3].max(o.bbox[3]),
                ];
            }
        } else {
            out.push(TextLine {
                ids: vec![i],
                bbox: o.bbox,
                text: o.text.clone(),
                char_boxes: o.char_boxes.clone(),
                font_size: o.font_size,
                color: o.color,
                runs: vec![i; o.text.chars().count()],
            });
        }
        last = Some((i, key.0, key.1, key.2));
    }
    // Characters that are several code points (ligatures) keep one box each.
    for l in &mut out {
        let n = l.text.chars().count();
        l.char_boxes
            .resize(n, [l.bbox[2], l.bbox[1], l.bbox[2], l.bbox[3]]);
        let last = l.runs.last().or(l.ids.last()).copied().unwrap_or(0);
        l.runs.resize(n, last);
    }
    out
}

/// Pixel size of JPEG or PNG bytes.
pub fn image_size(b: &[u8]) -> Option<(u32, u32)> {
    if b.starts_with(&[0x89, b'P', b'N', b'G']) && b.len() >= 24 {
        let w = u32::from_be_bytes(b[16..20].try_into().ok()?);
        let h = u32::from_be_bytes(b[20..24].try_into().ok()?);
        return Some((w, h));
    }
    if b.starts_with(&[0xff, 0xd8]) {
        let mut d = zune_jpeg::JpegDecoder::new(b);
        d.decode_headers().ok()?;
        let (w, h) = d.dimensions()?;
        return Some((w as u32, h as u32));
    }
    None
}

/// A picture to place: JPEG or PNG file bytes.
#[derive(Debug, Clone, Copy)]
pub struct NewImage<'a>(pub &'a [u8]);

fn union(a: Option<[f32; 4]>, b: [f32; 4]) -> Option<[f32; 4]> {
    if !b.iter().all(|v| v.is_finite()) {
        return a;
    }
    Some(match a {
        None => b,
        Some(a) => [
            a[0].min(b[0]),
            a[1].min(b[1]),
            a[2].max(b[2]),
            a[3].max(b[3]),
        ],
    })
}

fn pt_box(m: M, pts: &[(f32, f32)]) -> Option<[f32; 4]> {
    let mut r: Option<[f32; 4]> = None;
    for &(x, y) in pts {
        let (u, v) = crate::interp::apply(m, x, y);
        r = union(r, [u, v, u, v]);
    }
    r
}

/// Bounds of display list commands in page points.
fn command_bounds(cmds: &[Command]) -> Option<[f32; 4]> {
    let mut stack = vec![IDENTITY];
    let mut r: Option<[f32; 4]> = None;
    for c in cmds {
        let m = *stack.last().unwrap_or(&IDENTITY);
        let b = match c {
            Command::Path { segs, stroke, .. } => {
                let pad = stroke.as_ref().map_or(0.0, |s| s.width / 2.0);
                let mut pts = Vec::new();
                for s in segs {
                    match *s {
                        PathSeg::M(x, y) | PathSeg::L(x, y) => pts.push((x, y)),
                        PathSeg::Q(a, b, x, y) => pts.extend([(a, b), (x, y)]),
                        PathSeg::C(a, b, c, d, x, y) => pts.extend([(a, b), (c, d), (x, y)]),
                        PathSeg::Z => {}
                    }
                }
                pt_box(m, &pts).map(|b| [b[0] - pad, b[1] - pad, b[2] + pad, b[3] + pad])
            }
            Command::Image { x, y, w, h, .. } | Command::FillRect { x, y, w, h, .. } => {
                pt_box(m, &[(*x, *y), (x + w, *y), (*x, y + h), (x + w, y + h)])
            }
            Command::Line { x1, y1, x2, y2, .. } => pt_box(m, &[(*x1, *y1), (*x2, *y2)]),
            Command::Glyphs {
                x, y, size, glyphs, ..
            } => {
                let end = glyphs.last().map_or(0.0, |g| g.1) + size * 0.6;
                pt_box(m, &[(*x, y - size * 0.8), (x + end, y + size * 0.25)])
            }
            Command::PushTransform { m: t } => {
                stack.push(mul(*t, m));
                None
            }
            Command::PopTransform => {
                if stack.len() > 1 {
                    stack.pop();
                }
                None
            }
            _ => None,
        };
        if let Some(b) = b {
            r = union(r, b);
        }
    }
    r
}

pub(crate) fn num(v: f32) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".into()
    } else {
        s.to_owned()
    }
}

pub(crate) fn matrix(m: M) -> String {
    m.iter().map(|v| num(*v)).collect::<Vec<_>>().join(" ")
}

pub(crate) fn rgb(c: Rgba) -> String {
    format!(
        "{} {} {}",
        num(f32::from(c.0) / 255.0),
        num(f32::from(c.1) / 255.0),
        num(f32::from(c.2) / 255.0)
    )
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2 + 2);
    s.push('<');
    for b in bytes {
        let _ = write!(s, "{b:02X}");
    }
    s.push('>');
    s
}

fn obj_bytes(o: &Object) -> Vec<u8> {
    let mut out = Vec::new();
    crate::write::object(&mut out, o);
    out
}

/// Advance of a text run along its baseline in text space units.
fn advance(ts: &TextState) -> f32 {
    let (a, b) = (ts.tm[0], ts.tm[1]);
    let n2 = a * a + b * b;
    if n2 < 1e-12 {
        return 0.0;
    }
    let (de, df) = (ts.tm_after[4] - ts.tm[4], ts.tm_after[5] - ts.tm[5]);
    (de * a + df * b) / n2
}

/// Operators that keep a deleted run's effect on the text position.
pub(crate) fn deleted_run(rec: &Recorded) -> String {
    let Some(ts) = &rec.text else {
        return String::new();
    };
    let k = ts.size * ts.th;
    let shift = if k.abs() > 1e-6 {
        format!("[{}] TJ", num(-advance(ts) * 1000.0 / k))
    } else {
        String::new()
    };
    match rec.op.as_slice() {
        b"'" => format!("T* {shift}"),
        b"\"" => {
            let aw = rec.args.first().map_or_else(
                || "0".into(),
                |o| String::from_utf8_lossy(&obj_bytes(o)).into_owned(),
            );
            let ac = rec.args.get(1).map_or_else(
                || "0".into(),
                |o| String::from_utf8_lossy(&obj_bytes(o)).into_owned(),
            );
            format!("{aw} Tw {ac} Tc T* {shift}")
        }
        _ => shift,
    }
}

/// The showing operator of a run with its own operands (`'` and `"` become `Tj`).
fn show_operator(rec: &Recorded) -> Vec<u8> {
    let mut out = Vec::new();
    match rec.op.as_slice() {
        b"TJ" | b"Tj" => {
            if let Some(a) = rec.args.first() {
                out.extend(obj_bytes(a));
            }
            out.push(b' ');
            out.extend_from_slice(&rec.op);
        }
        b"'" => {
            if let Some(a) = rec.args.first() {
                out.extend(obj_bytes(a));
            }
            out.extend_from_slice(b" Tj");
        }
        _ => {
            if let Some(a) = rec.args.get(2) {
                out.extend(obj_bytes(a));
            }
            out.extend_from_slice(b" Tj");
        }
    }
    out
}

/// A text object with full state at the end of the content (`ctm` relative to user space).
pub(crate) fn text_object(ts: &TextState, font: &[u8], ctm: M, tm: M, show: &[u8]) -> Vec<u8> {
    let mut font_name = Vec::new();
    crate::write::object(&mut font_name, &Object::Name(font.to_vec()));
    let mut s = format!("\nq {} cm BT ", matrix(ctm)).into_bytes();
    s.extend(font_name);
    s.extend(
        format!(
            " {} Tf {} Tc {} Tw {} Tz {} TL {} Ts {} Tr {} rg {} RG {} Tm ",
            num(ts.size),
            num(ts.tc),
            num(ts.tw),
            num(ts.th * 100.0),
            num(ts.tl),
            num(ts.rise),
            ts.mode,
            rgb(ts.fill),
            rgb(ts.stroke),
            matrix(tm)
        )
        .into_bytes(),
    );
    s.extend_from_slice(show);
    s.extend_from_slice(b" ET Q\n");
    s
}

impl PdfDocument {
    /// Text runs, images, paths, forms and shadings of page `index` in content order.
    pub fn page_objects(&mut self, index: usize) -> Result<Vec<PageObject>, String> {
        self.objects_in(index, None)
    }

    /// Page objects of `content` run with page `index`'s resources (the page's own content
    /// when `None`).
    pub(crate) fn objects_in(
        &mut self,
        index: usize,
        content: Option<&[u8]>,
    ) -> Result<Vec<PageObject>, String> {
        let info = self.pages.get(index).ok_or("no such page")?.clone();
        let base = page_matrix(info.crop, info.rotate);
        let doc = &self.doc;
        let page = doc.get_dictionary(info.id).map_err(|e| e.to_string())?;
        let resources = inherited(doc, page, b"Resources").and_then(|o| o.as_dict().ok());
        let own;
        let content: &[u8] = match content {
            Some(c) => c,
            None => {
                own = page_content(doc, page);
                &own
            }
        };
        let mut shared = Shared {
            doc,
            fonts: &mut self.fonts,
            font_cache: &mut self.font_cache,
            images: &mut self.images,
            hidden: &self.hidden,
            opts: RenderOptions {
                annotations: false,
                ..RenderOptions::default()
            },
        };
        let mut it = Interp::new(
            &mut shared,
            base,
            resources,
            (info.width, info.height),
            format!("o{index}"),
        );
        it.out.objects = Some(Vec::new());
        it.run(content);
        it.finish();
        let out = std::mem::take(&mut it.out);
        let inv_base = invert(base).unwrap_or(IDENTITY);
        let mut list = Vec::new();
        for rec in out.objects.unwrap_or_default() {
            let chars = out.chars.get(rec.chars.0..rec.chars.1).unwrap_or(&[]);
            let cmds = out
                .commands
                .get(rec.commands.0..rec.commands.1)
                .unwrap_or(&[]);
            let mut bbox = None;
            for c in chars {
                bbox = union(bbox, c.rect);
            }
            if bbox.is_none() {
                bbox = command_bounds(cmds);
            }
            let Some(bbox) = bbox else { continue };
            let (font_size, color) = match &rec.text {
                Some(ts) => {
                    // Size on the page: the text matrix and CTM scale the font size.
                    let m = mul(ts.tm, mul(rec.ctm, inv_base));
                    let s = (m[2] * m[2] + m[3] * m[3]).sqrt() * ts.size;
                    (s, [ts.fill.0, ts.fill.1, ts.fill.2])
                }
                None => (0.0, [0, 0, 0]),
            };
            list.push(PageObject {
                kind: rec.kind,
                bbox,
                text: chars.iter().map(|c| c.text.as_str()).collect(),
                char_boxes: chars.iter().map(|c| c.rect).collect(),
                font_size,
                color,
                rec,
            });
        }
        Ok(list)
    }

    /// The page's content as one stream wrapped in `q … Q` (made on the first edit); its id
    /// and decoded bytes.
    pub(crate) fn page_stream(&mut self, index: usize) -> Result<(ObjectId, Vec<u8>), String> {
        let info = self.pages.get(index).ok_or("no such page")?.clone();
        let page = self
            .doc
            .get_dictionary(info.id)
            .map_err(|e| e.to_string())?;
        if let Ok(Object::Reference(id)) = page.get(b"Contents") {
            if self.edit.own_streams.contains(id) {
                let s = self.doc.get_object(*id).and_then(Object::as_stream);
                let bytes = s
                    .ok()
                    .and_then(|s| filters::stream_bytes(&self.doc, s).ok())
                    .ok_or("page content")?;
                return Ok((*id, bytes));
            }
        }
        let mut content = b"q\n".to_vec();
        content.extend(page_content(&self.doc, page));
        content.extend_from_slice(b"\nQ\n");
        self.touch(info.id);
        let id = self.add(Object::Stream(Stream::new(Dictionary::new(), Vec::new())));
        self.edit.own_streams.insert(id);
        if let Ok(d) = self.doc.get_dictionary_mut(info.id) {
            d.set("Contents", Object::Reference(id));
        }
        self.set_stream(id, content.clone());
        Ok((id, content))
    }

    pub(crate) fn set_stream(&mut self, id: ObjectId, content: Vec<u8>) {
        self.touch(id);
        if let Ok(Object::Stream(s)) = self.doc.get_object_mut(id) {
            s.dict.remove(b"DecodeParms");
            if content.len() > 64 {
                s.dict.set("Filter", Object::Name(b"FlateDecode".to_vec()));
                s.set_content(miniz_oxide::deflate::compress_to_vec_zlib(&content, 6));
            } else {
                s.dict.remove(b"Filter");
                s.set_content(content);
            }
        }
    }

    /// Normalise the page, analyse it and check `ids`.
    fn edit_page(
        &mut self,
        index: usize,
        ids: &[usize],
    ) -> Result<(ObjectId, Vec<u8>, Vec<PageObject>), String> {
        let (sid, content) = self.page_stream(index)?;
        let objs = self.page_objects(index)?;
        if ids.iter().any(|&i| i >= objs.len()) {
            return Err("no such object".into());
        }
        Ok((sid, content, objs))
    }

    /// Splice `edits` (byte range, replacement) into `content`, append `tail` and store it.
    pub(crate) fn write_page(
        &mut self,
        sid: ObjectId,
        mut content: Vec<u8>,
        mut edits: Vec<((usize, usize), Vec<u8>)>,
        tail: &[u8],
    ) {
        edits.sort_by_key(|e| std::cmp::Reverse(e.0 .0));
        for ((a, b), with) in edits {
            if a <= b && b <= content.len() {
                content.splice(a..b, with);
            }
        }
        content.extend_from_slice(tail);
        self.set_stream(sid, content);
    }

    /// Base matrix of page `index` (user space to page points) and its inverse.
    fn bases(&self, index: usize) -> Result<(M, M), String> {
        let info = self.pages.get(index).ok_or("no such page")?;
        let base = page_matrix(info.crop, info.rotate);
        Ok((base, invert(base).ok_or("page matrix")?))
    }

    /// Delete page objects `ids` of page `index`.
    pub fn delete_objects(&mut self, index: usize, ids: &[usize]) -> Result<(), String> {
        self.begin("delete objects");
        let r = self.delete_inner(index, ids);
        self.commit();
        r
    }

    fn delete_inner(&mut self, index: usize, ids: &[usize]) -> Result<(), String> {
        let (sid, content, objs) = self.edit_page(index, ids)?;
        let edits = ids
            .iter()
            .map(|&i| {
                let rec = &objs[i].rec;
                let with = if rec.kind == ObjectKind::Text {
                    deleted_run(rec).into_bytes()
                } else {
                    Vec::new()
                };
                (rec.span, with)
            })
            .collect();
        self.write_page(sid, content, edits, b"");
        Ok(())
    }

    /// Transform page objects `ids` of page `index` by `m` in page points (a move is
    /// `[1, 0, 0, 1, dx, dy]`, a resize scales about a corner).
    pub fn transform_objects(
        &mut self,
        index: usize,
        ids: &[usize],
        m: [f32; 6],
    ) -> Result<(), String> {
        self.begin("move objects");
        let r = self.transform_inner(index, ids, m);
        self.commit();
        r
    }

    fn transform_inner(&mut self, index: usize, ids: &[usize], m: M) -> Result<(), String> {
        let (base, inv_base) = self.bases(index)?;
        let (sid, content, objs) = self.edit_page(index, ids)?;
        // The transform in default user space.
        let m_user = mul(mul(base, m), inv_base);
        let mut edits = Vec::new();
        let mut tail = Vec::new();
        for &i in ids {
            let rec = &objs[i].rec;
            let ctm_u = mul(rec.ctm, inv_base);
            let inv_ctm = invert(ctm_u).ok_or("singular matrix")?;
            match &rec.text {
                Some(ts) => {
                    edits.push((rec.span, deleted_run(rec).into_bytes()));
                    let tm = mul(mul(mul(ts.tm, ctm_u), m_user), inv_ctm);
                    tail.extend(text_object(ts, &ts.font, ctm_u, tm, &show_operator(rec)));
                }
                None => {
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

    /// Replace the text of run `id` on page `index` with `text`, in the run's font when it has
    /// a glyph for every character, otherwise in an embedded Unicode font. The run is written
    /// again at the end of the page; returns its new object index.
    pub fn replace_text(&mut self, index: usize, id: usize, text: &str) -> Result<usize, String> {
        self.replace_text_runs(index, &[id], text)
    }

    /// Replace the text runs `ids` (a [`TextLine`]) with one run of `text` written where the
    /// first run starts, in its font and state; returns the new run's object index.
    pub fn replace_text_runs(
        &mut self,
        index: usize,
        ids: &[usize],
        text: &str,
    ) -> Result<usize, String> {
        self.begin("edit text");
        let r = self.replace_text_inner(index, ids, text);
        self.commit();
        r
    }

    fn replace_text_inner(
        &mut self,
        index: usize,
        ids: &[usize],
        text: &str,
    ) -> Result<usize, String> {
        let (_, inv_base) = self.bases(index)?;
        let objs = self.page_objects(index)?;
        let id = *ids.first().ok_or("no text")?;
        let rec = objs.get(id).ok_or("no such object")?.rec.clone();
        let ts = rec.text.clone().ok_or("not a text run")?;
        let encoded = self.encode_in_font(index, &ts.font, text);
        let (font, bytes) = match encoded {
            Some(b) => (ts.font.clone(), b),
            None => {
                let face = self.fallback_face(index, &ts.font);
                let (fid, bytes) = self.embed_text(face, text)?;
                (self.add_resource(index, b"Font", "BoF", fid)?, bytes)
            }
        };
        let (sid, content, objs) = self.edit_page(index, ids)?;
        let rec = objs[id].rec.clone();
        let ctm_u = mul(rec.ctm, inv_base);
        let mut show = hex(&bytes).into_bytes();
        show.extend_from_slice(b" Tj");
        let mut tail = Vec::new();
        if !text.is_empty() {
            tail = text_object(&ts, &font, ctm_u, ts.tm, &show);
        }
        let edits = ids
            .iter()
            .filter(|&&i| objs[i].kind == ObjectKind::Text)
            .map(|&i| (objs[i].rec.span, deleted_run(&objs[i].rec).into_bytes()))
            .collect();
        self.write_page(sid, content, edits, &tail);
        let objs = self.page_objects(index)?;
        Ok(objs
            .iter()
            .rposition(|o| o.kind == ObjectKind::Text)
            .unwrap_or(0))
    }

    /// Add a text run at `origin` (baseline start, page points) in `size` points and `color`;
    /// returns its object index.
    pub fn add_text(
        &mut self,
        index: usize,
        origin: [f32; 2],
        text: &str,
        size: f32,
        color: [u8; 3],
    ) -> Result<usize, String> {
        self.begin("add text");
        let r = (|| {
            let (base, inv_base) = self.bases(index)?;
            let (fid, bytes) = self.embed_text(crate::fallback::SANS, text)?;
            let font = self.add_resource(index, b"Font", "BoF", fid)?;
            let (sid, content) = self.page_stream(index)?;
            // Text space upright on the page in any page rotation: x right, y up on screen.
            let _ = base;
            let tm = mul([1.0, 0.0, 0.0, -1.0, origin[0], origin[1]], inv_base);
            let c = Rgba(color[0], color[1], color[2], 255);
            let ts = TextState {
                font: font.clone(),
                size,
                tm,
                tm_after: tm,
                tc: 0.0,
                tw: 0.0,
                th: 1.0,
                tl: size * 1.2,
                rise: 0.0,
                mode: 0,
                fill: c,
                stroke: c,
            };
            let mut show = hex(&bytes).into_bytes();
            show.extend_from_slice(b" Tj");
            let tail = text_object(&ts, &font, IDENTITY, tm, &show);
            self.write_page(sid, content, Vec::new(), &tail);
            let objs = self.page_objects(index)?;
            Ok(objs
                .iter()
                .rposition(|o| o.kind == ObjectKind::Text)
                .unwrap_or(0))
        })();
        self.commit();
        r
    }

    /// Place a picture into `rect` (left, top, right, bottom in page points) of page `index`.
    pub fn add_image(
        &mut self,
        index: usize,
        rect: [f32; 4],
        image: NewImage<'_>,
    ) -> Result<(), String> {
        self.begin("add image");
        let r = (|| {
            let (_, inv_base) = self.bases(index)?;
            let xid = self.image_xobject(image)?;
            let name = self.add_resource(index, b"XObject", "BoIm", xid)?;
            let (sid, content) = self.page_stream(index)?;
            let n = [
                rect[2] - rect[0],
                0.0,
                0.0,
                rect[1] - rect[3],
                rect[0],
                rect[3],
            ];
            let u = mul(n, inv_base);
            let mut tail = format!("\nq {} cm ", matrix(u)).into_bytes();
            crate::write::object(&mut tail, &Object::Name(name));
            tail.extend_from_slice(b" Do Q\n");
            self.write_page(sid, content, Vec::new(), &tail);
            Ok(())
        })();
        self.commit();
        r
    }

    /// Replace image object `id` of page `index` with another picture in the same place.
    pub fn replace_image(
        &mut self,
        index: usize,
        id: usize,
        image: NewImage<'_>,
    ) -> Result<(), String> {
        self.begin("replace image");
        let r = (|| {
            let xid = self.image_xobject(image)?;
            let name = self.add_resource(index, b"XObject", "BoIm", xid)?;
            let (sid, content, objs) = self.edit_page(index, &[id])?;
            let rec = &objs[id].rec;
            if rec.kind != ObjectKind::Image {
                return Err("not an image".into());
            }
            let mut with = Vec::new();
            crate::write::object(&mut with, &Object::Name(name));
            with.extend_from_slice(b" Do");
            let span = rec.span;
            self.write_page(sid, content, vec![(span, with)], b"");
            Ok(())
        })();
        self.commit();
        r
    }

    /// Redact page `index` under `rects` (page points): the glyphs whose boxes touch a
    /// rectangle are removed from their runs (the other glyphs keep their places), images,
    /// paths and forms touching one are removed, comments over one are deleted, and the areas
    /// are painted black. Returns how many objects were changed.
    pub fn apply_redactions(&mut self, index: usize, rects: &[[f32; 4]]) -> Result<usize, String> {
        if rects.is_empty() {
            return Ok(0);
        }
        self.begin("redact");
        let r = self.redact_inner(index, rects);
        self.commit();
        r
    }

    fn redact_inner(&mut self, index: usize, rects: &[[f32; 4]]) -> Result<usize, String> {
        let hits = |b: [f32; 4]| {
            rects
                .iter()
                .any(|r| b[0] < r[2] && b[2] > r[0] && b[1] < r[3] && b[3] > r[1])
        };
        let (_, inv_base) = self.bases(index)?;
        let (sid, content, objs) = self.edit_page(index, &[])?;
        let mut edits = Vec::new();
        let mut changed = 0;
        for o in &objs {
            if !hits(o.bbox) {
                continue;
            }
            changed += 1;
            let rec = &o.rec;
            let with = match &rec.text {
                Some(ts) => self
                    .redacted_run(index, rec, ts, &hits)
                    .unwrap_or_else(|| deleted_run(rec).into_bytes()),
                None => Vec::new(),
            };
            edits.push((rec.span, with));
        }
        let mut tail = String::from("\nq 0 g\n");
        for r in rects {
            let a = crate::interp::apply(inv_base, r[0], r[1]);
            let b = crate::interp::apply(inv_base, r[2], r[3]);
            let (x0, y0) = (a.0.min(b.0), a.1.min(b.1));
            let (x1, y1) = (a.0.max(b.0), a.1.max(b.1));
            tail.push_str(&format!(
                "{} {} {} {} re f\n",
                num(x0),
                num(y0),
                num(x1 - x0),
                num(y1 - y0)
            ));
        }
        tail.push_str("Q\n");
        self.write_page(sid, content, edits, tail.as_bytes());
        // Comments over the areas go too.
        let annots: Vec<ObjectId> = self
            .annotations()
            .into_iter()
            .filter(|a| a.page == index && hits(a.rect))
            .filter_map(|a| a.id)
            .collect();
        changed += annots.len();
        self.remove_annots(index, &annots)?;
        Ok(changed)
    }

    /// A text run without the glyphs `hits` selects: a `TJ` whose numbers keep the other
    /// glyphs in place. `None` when the run cannot be split (Type 3 or vertical fonts).
    fn redacted_run(
        &mut self,
        index: usize,
        rec: &Recorded,
        ts: &TextState,
        hits: &dyn Fn([f32; 4]) -> bool,
    ) -> Option<Vec<u8>> {
        let info = self.pages.get(index)?.clone();
        let page = self.doc.get_dictionary(info.id).ok()?;
        let res = inherited(&self.doc, page, b"Resources")?.as_dict().ok()?;
        let fonts = resolve(&self.doc, res.get(b"Font").ok()?).as_dict().ok()?;
        let fo = fonts.get(&ts.font).ok()?.clone();
        let font = self.font_cache.get(&self.doc, &fo, &mut self.fonts)?;
        if font.type3.is_some() || font.vertical || ts.size.abs() < 1e-6 {
            return None;
        }
        let items: Vec<Object> = match rec.op.as_slice() {
            b"TJ" => rec.args.first()?.as_array().ok()?.clone(),
            b"\"" => vec![rec.args.get(2)?.clone()],
            _ => vec![rec.args.first()?.clone()],
        };
        let fs = ts.size;
        let th = ts.th;
        let (asc, desc) = (font.ascent / 1000.0, font.descent / 1000.0);
        let mut tx = 0.0_f32;
        let mut out: Vec<Object> = Vec::new();
        let mut kept: Vec<u8> = Vec::new();
        let mut gap = 0.0_f32;
        let flush = |out: &mut Vec<Object>, kept: &mut Vec<u8>, gap: &mut f32| {
            if !kept.is_empty() {
                out.push(Object::String(
                    std::mem::take(kept),
                    lopdf::StringFormat::Hexadecimal,
                ));
            }
            if gap.abs() > 1e-4 {
                out.push(Object::Real(*gap));
                *gap = 0.0;
            }
        };
        for it in &items {
            match it {
                Object::String(bytes, _) => {
                    for (code, len) in font.codes(bytes) {
                        let w0 = font.width(code) / 1000.0;
                        let space = code == 32 && len == 1;
                        let adv = (w0 * fs + ts.tc + if space { ts.tw } else { 0.0 }) * th;
                        let trm = mul(
                            [fs * th, 0.0, 0.0, fs, 0.0, ts.rise],
                            mul(mul([1.0, 0.0, 0.0, 1.0, tx, 0.0], ts.tm), rec.ctm),
                        );
                        let c = [
                            crate::interp::apply(trm, 0.0, desc),
                            crate::interp::apply(trm, w0.max(0.01), asc),
                        ];
                        let b = [
                            c[0].0.min(c[1].0),
                            c[0].1.min(c[1].1),
                            c[0].0.max(c[1].0),
                            c[0].1.max(c[1].1),
                        ];
                        tx += adv;
                        // The code's bytes in this string, in order.
                        let code_bytes = {
                            let n = len.min(4);
                            (0..n)
                                .rev()
                                .map(|k| (code >> (8 * k)) as u8)
                                .collect::<Vec<u8>>()
                        };
                        if hits(b) {
                            if !kept.is_empty() {
                                out.push(Object::String(
                                    std::mem::take(&mut kept),
                                    lopdf::StringFormat::Hexadecimal,
                                ));
                            }
                            gap -= adv * 1000.0 / (fs * th);
                        } else {
                            if gap.abs() > 1e-4 {
                                flush(&mut out, &mut kept, &mut gap);
                            }
                            kept.extend(code_bytes);
                        }
                    }
                }
                o => {
                    let n = o.as_float().unwrap_or(0.0);
                    tx -= n / 1000.0 * fs * th;
                    if !kept.is_empty() {
                        out.push(Object::String(
                            std::mem::take(&mut kept),
                            lopdf::StringFormat::Hexadecimal,
                        ));
                    }
                    gap += n;
                }
            }
        }
        flush(&mut out, &mut kept, &mut gap);
        let mut s = Vec::new();
        match rec.op.as_slice() {
            b"'" => s.extend_from_slice(b"T* "),
            b"\"" => {
                let aw = rec.args.first().map(obj_bytes).unwrap_or_default();
                let ac = rec.args.get(1).map(obj_bytes).unwrap_or_default();
                s.extend(aw);
                s.extend_from_slice(b" Tw ");
                s.extend(ac);
                s.extend_from_slice(b" Tc T* ");
            }
            _ => {}
        }
        s.extend(obj_bytes(&Object::Array(out)));
        s.extend_from_slice(b" TJ");
        Some(s)
    }

    /// Start a group of edits that undo as one (typing into a text run).
    pub fn begin_group(&mut self, label: &str) {
        self.begin(label);
        self.edit.group = true;
    }

    /// End the group started by [`PdfDocument::begin_group`].
    pub fn end_group(&mut self) {
        self.edit.group = false;
        self.commit();
    }

    /// Encode `text` in page `index`'s font resource `name` when it has every glyph.
    pub(crate) fn encode_in_font(
        &mut self,
        index: usize,
        name: &[u8],
        text: &str,
    ) -> Option<Vec<u8>> {
        let info = self.pages.get(index)?.clone();
        let page = self.doc.get_dictionary(info.id).ok()?;
        let res = inherited(&self.doc, page, b"Resources")?.as_dict().ok()?;
        let fonts = resolve(&self.doc, res.get(b"Font").ok()?).as_dict().ok()?;
        let fo = fonts.get(name).ok()?.clone();
        let key = fo.as_reference().ok();
        let enc = match key.and_then(|k| self.edit.encoders.get(&k).cloned()) {
            Some(e) => e,
            None => {
                let font = self.font_cache.get(&self.doc, &fo, &mut self.fonts)?;
                let e = font.encoder().map(std::rc::Rc::new);
                if let Some(k) = key {
                    self.edit.encoders.insert(k, e.clone());
                }
                e
            }
        }?;
        let mut out = Vec::new();
        for c in text.chars() {
            out.extend_from_slice(enc.get(&c)?);
        }
        Some(out)
    }

    /// Bundled face (serif / sans, bold, italic) standing in for font resource `name`.
    pub(crate) fn fallback_face(&self, index: usize, name: &[u8]) -> usize {
        let Some(info) = self.pages.get(index) else {
            return crate::fallback::SANS;
        };
        let doc = &self.doc;
        let Some(font) = doc
            .get_dictionary(info.id)
            .ok()
            .and_then(|p| inherited(doc, p, b"Resources"))
            .and_then(|r| r.as_dict().ok())
            .and_then(|r| r.get(b"Font").ok())
            .and_then(|f| resolve(doc, f).as_dict().ok())
            .and_then(|f| f.get(name).ok())
            .and_then(|f| resolve(doc, f).as_dict().ok())
        else {
            return crate::fallback::SANS;
        };
        let base = font
            .get(b"BaseFont")
            .ok()
            .and_then(|b| b.as_name().ok())
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .unwrap_or_default();
        // The descriptor tells bold and italic when the name does not (§9.8.2): ForceBold or
        // a weight ≥ 600 or a thick stem, and the Italic flag.
        let desc = font
            .get(b"DescendantFonts")
            .ok()
            .and_then(|d| resolve(doc, d).as_array().ok())
            .and_then(|a| a.first())
            .and_then(|d| resolve(doc, d).as_dict().ok())
            .unwrap_or(font)
            .get(b"FontDescriptor")
            .ok()
            .and_then(|d| resolve(doc, d).as_dict().ok());
        let num = |k: &[u8]| {
            desc.and_then(|d| d.get(k).ok())
                .and_then(|o| resolve(doc, o).as_float().ok())
                .unwrap_or(0.0)
        };
        let flags = num(b"Flags") as i64;
        let bold = flags & (1 << 18) != 0 || num(b"FontWeight") >= 600.0 || num(b"StemV") >= 120.0;
        let italic = flags & (1 << 6) != 0 || num(b"ItalicAngle").abs() > 1.0;
        let face = crate::fallback::face_for(&base);
        let (base_face, b0, i0) = ((face / 4) * 4, face & 1, face & 2);
        base_face + (b0 | usize::from(bold)) + (i0 | (2 * usize::from(italic)))
    }

    /// Add `id` to page `index`'s resources under `category` (`Font`, `XObject`) with a new
    /// name starting with `prefix`; returns the name.
    pub(crate) fn add_resource(
        &mut self,
        index: usize,
        category: &[u8],
        prefix: &str,
        id: ObjectId,
    ) -> Result<Vec<u8>, String> {
        let page_id = self.pages.get(index).ok_or("no such page")?.id;
        let name = format!("{prefix}{}", id.0).into_bytes();
        // The page's own resources (copied from an ancestor when inherited).
        let page = self
            .doc
            .get_dictionary(page_id)
            .map_err(|e| e.to_string())?;
        let res = match page.get(b"Resources") {
            Ok(Object::Reference(r)) => Some(*r),
            Ok(_) => None,
            Err(_) => {
                let v = crate::edit::inherited_raw(&self.doc, page_id, b"Resources")
                    .unwrap_or_else(|| Object::Dictionary(Dictionary::new()));
                self.touch(page_id);
                if let Ok(d) = self.doc.get_dictionary_mut(page_id) {
                    d.set("Resources", v.clone());
                }
                v.as_reference().ok()
            }
        };
        let holder = res.unwrap_or(page_id);
        // The category dictionary may itself be an indirect object.
        let cat_ref = {
            let d = self.doc.get_dictionary(holder).map_err(|e| e.to_string())?;
            let r = if res.is_some() {
                Some(d)
            } else {
                d.get(b"Resources").ok().and_then(|o| o.as_dict().ok())
            };
            r.and_then(|r| r.get(category).ok())
                .and_then(|o| o.as_reference().ok())
        };
        if let Some(c) = cat_ref {
            self.touch(c);
            let d = self.doc.get_dictionary_mut(c).map_err(|e| e.to_string())?;
            d.set(name.clone(), Object::Reference(id));
            return Ok(name);
        }
        self.touch(holder);
        let d = self
            .doc
            .get_dictionary_mut(holder)
            .map_err(|e| e.to_string())?;
        let res_dict = if res.is_some() {
            d
        } else {
            match d.get_mut(b"Resources") {
                Ok(Object::Dictionary(r)) => r,
                _ => return Err("page resources".into()),
            }
        };
        match res_dict.get_mut(category) {
            Ok(Object::Dictionary(c)) => c.set(name.clone(), Object::Reference(id)),
            _ => {
                let mut c = Dictionary::new();
                c.set(name.clone(), Object::Reference(id));
                res_dict.set(category.to_vec(), c);
            }
        }
        Ok(name)
    }

    /// An image XObject (and soft mask) from JPEG or PNG bytes.
    fn image_xobject(&mut self, image: NewImage<'_>) -> Result<ObjectId, String> {
        let b = image.0;
        let mut d = Dictionary::new();
        d.set("Type", Object::Name(b"XObject".to_vec()));
        d.set("Subtype", Object::Name(b"Image".to_vec()));
        d.set("BitsPerComponent", 8);
        if b.starts_with(&[0xff, 0xd8]) {
            let mut probe = zune_jpeg::JpegDecoder::new(b);
            probe.decode_headers().map_err(|e| format!("JPEG: {e:?}"))?;
            let (w, h) = probe.dimensions().ok_or("JPEG size")?;
            let comps = probe
                .get_input_colorspace()
                .map_or(3, |c| c.num_components());
            let cs: &[u8] = match comps {
                1 => b"DeviceGray",
                4 => b"DeviceCMYK",
                _ => b"DeviceRGB",
            };
            d.set("Width", w as i64);
            d.set("Height", h as i64);
            d.set("ColorSpace", Object::Name(cs.to_vec()));
            d.set("Filter", Object::Name(b"DCTDecode".to_vec()));
            if comps == 4 {
                d.set(
                    "Decode",
                    vec![
                        1.into(),
                        0.into(),
                        1.into(),
                        0.into(),
                        1.into(),
                        0.into(),
                        1.into(),
                        0.into(),
                    ],
                );
            }
            return Ok(self.add(Object::Stream(Stream::new(d, b.to_vec()))));
        }
        let dec = png::Decoder::new(std::io::Cursor::new(b));
        let mut reader = dec.read_info().map_err(|e| format!("PNG: {e}"))?;
        let mut buf = vec![0; reader.output_buffer_size().ok_or("PNG size")?];
        let frame = reader
            .next_frame(&mut buf)
            .map_err(|e| format!("PNG: {e}"))?;
        let (w, h) = (frame.width as usize, frame.height as usize);
        if w == 0 || h == 0 || w * h > 1 << 28 {
            return Err("PNG size".into());
        }
        let data = &buf[..frame.buffer_size()];
        let line = frame.line_size;
        let mut rgb = Vec::with_capacity(w * h * 3);
        let mut alpha = Vec::with_capacity(w * h);
        let mut has_alpha = false;
        for y in 0..h {
            let row = &data[y * line..(y + 1) * line];
            for x in 0..w {
                let (r, g, bl, a) = match (frame.color_type, frame.bit_depth) {
                    (png::ColorType::Rgba, png::BitDepth::Eight) => {
                        (row[x * 4], row[x * 4 + 1], row[x * 4 + 2], row[x * 4 + 3])
                    }
                    (png::ColorType::Rgb, png::BitDepth::Eight) => {
                        (row[x * 3], row[x * 3 + 1], row[x * 3 + 2], 255)
                    }
                    (png::ColorType::GrayscaleAlpha, png::BitDepth::Eight) => {
                        (row[x * 2], row[x * 2], row[x * 2], row[x * 2 + 1])
                    }
                    (png::ColorType::Grayscale, png::BitDepth::Eight) => {
                        (row[x], row[x], row[x], 255)
                    }
                    _ => return Err("PNG: only 8-bit images".into()),
                };
                rgb.extend_from_slice(&[r, g, bl]);
                alpha.push(a);
                has_alpha |= a != 255;
            }
        }
        d.set("Width", w as i64);
        d.set("Height", h as i64);
        d.set("ColorSpace", Object::Name(b"DeviceRGB".to_vec()));
        d.set("Filter", Object::Name(b"FlateDecode".to_vec()));
        if has_alpha {
            let mut m = Dictionary::new();
            m.set("Type", Object::Name(b"XObject".to_vec()));
            m.set("Subtype", Object::Name(b"Image".to_vec()));
            m.set("Width", w as i64);
            m.set("Height", h as i64);
            m.set("ColorSpace", Object::Name(b"DeviceGray".to_vec()));
            m.set("BitsPerComponent", 8);
            m.set("Filter", Object::Name(b"FlateDecode".to_vec()));
            let mid = self.add(Object::Stream(Stream::new(
                m,
                miniz_oxide::deflate::compress_to_vec_zlib(&alpha, 6),
            )));
            d.set("SMask", Object::Reference(mid));
        }
        Ok(self.add(Object::Stream(Stream::new(
            d,
            miniz_oxide::deflate::compress_to_vec_zlib(&rgb, 6),
        ))))
    }
}
