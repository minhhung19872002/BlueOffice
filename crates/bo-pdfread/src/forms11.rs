//! BlueReader round 11: form filling and Fill & Sign helpers.
//!
//! - Tab order of the form (ISO 32000-1 §12.5.2 page `/Tabs`: R row, C column, S structure /
//!   unspecified = the `/Annots` order), one stop per radio group.
//! - Calculated fields: the calculation order `/AcroForm /CO` (§12.7.2) and each field's
//!   calculate action `/AA /C` (§12.6.3 Table 196). Only the simple `AFSimple_Calculate(op,
//!   fields)` call of the public Acrobat JavaScript reference is understood (SUM, PRD, AVG,
//!   MIN, MAX); no script is ever run.
//! - NoZoom annotations (§12.5.3 Table 165 bit 4), whose drawn icon is smaller than `/Rect`.
//! - Signatures and initials placed by Fill & Sign: `/Stamp` annotations (§12.5.6.12) whose
//!   appearance is typed text in an embedded font, drawn strokes or a picture, marked with
//!   `/BOFillSign true` so that they can be flattened into the page on save.
use crate::annot::{pdf_date, text_obj};
use crate::forms::{FieldKind, FormField};
use crate::interp::{apply, invert};
use crate::objects::NewImage;
use crate::{resolve, Annot, PdfDocument};
use lopdf::{Dictionary, Object, ObjectId, Stream};

/// Key marking the comments Fill & Sign placed (a private key; viewers ignore it).
const FILL_KEY: &[u8] = b"BOFillSign";

/// What a placed signature shows.
#[derive(Debug, Clone, PartialEq)]
pub enum SignatureInk {
    /// Typed text in a bundled font (face index of the fallback fonts).
    Typed {
        /// The text.
        text: String,
        /// Bundled face (2 Tinos Italic, 6 Liberation Sans Italic, 10 Carlito Italic, 14
        /// Caladea Italic).
        face: usize,
    },
    /// Strokes drawn in a unit box (0..1 both ways, top down).
    Strokes(Vec<Vec<[f32; 2]>>),
    /// A JPEG or PNG picture.
    Image(Vec<u8>),
}

/// One calculated field: its name, the operation and the fields it reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Calculation {
    /// The calculated field.
    pub target: String,
    /// SUM, PRD, AVG, MIN or MAX.
    pub op: String,
    /// The fields it reads.
    pub sources: Vec<String>,
}

/// Width of `text` at size 1 in bundled face `face` (em units), for a typed signature's box.
#[must_use]
pub fn typed_width(text: &str, face: usize) -> f32 {
    let Some(data) = crate::fallback::face_data(face) else {
        return text.chars().count() as f32 * 0.5;
    };
    let Ok(f) = ttf_parser::Face::parse(data, 0) else {
        return text.chars().count() as f32 * 0.5;
    };
    let upem = f32::from(f.units_per_em().max(1));
    text.chars()
        .map(|c| {
            f.glyph_index(c)
                .and_then(|g| f.glyph_hor_advance(g))
                .map_or(0.5, |a| f32::from(a) / upem)
        })
        .sum()
}

/// Glyph outlines of `text` in bundled face `face` at size 1 (em units, y down, baseline at
/// 0), so that a typed signature's preview is drawn exactly as it will be placed.
#[must_use]
pub fn typed_outline(text: &str, face: usize) -> Vec<bo_render::PathSeg> {
    use bo_render::PathSeg;
    struct B {
        out: Vec<PathSeg>,
        x: f32,
        k: f32,
    }
    impl ttf_parser::OutlineBuilder for B {
        fn move_to(&mut self, x: f32, y: f32) {
            self.out.push(PathSeg::M(self.x + x * self.k, -y * self.k));
        }
        fn line_to(&mut self, x: f32, y: f32) {
            self.out.push(PathSeg::L(self.x + x * self.k, -y * self.k));
        }
        fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
            let k = self.k;
            self.out
                .push(PathSeg::Q(self.x + x1 * k, -y1 * k, self.x + x * k, -y * k));
        }
        fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
            let k = self.k;
            self.out.push(PathSeg::C(
                self.x + x1 * k,
                -y1 * k,
                self.x + x2 * k,
                -y2 * k,
                self.x + x * k,
                -y * k,
            ));
        }
        fn close(&mut self) {
            self.out.push(PathSeg::Z);
        }
    }
    let Some(f) = crate::fallback::face_data(face).and_then(|d| ttf_parser::Face::parse(d, 0).ok())
    else {
        return Vec::new();
    };
    let k = 1.0 / f32::from(f.units_per_em().max(1));
    let mut b = B {
        out: Vec::new(),
        x: 0.0,
        k,
    };
    for c in text.chars() {
        let Some(g) = f.glyph_index(c) else {
            b.x += 0.5;
            continue;
        };
        f.outline_glyph(g, &mut b);
        b.x += f32::from(f.glyph_hor_advance(g).unwrap_or(0)) * k;
    }
    b.out
}

/// A number as the reference shows a calculated value: integers plain, else up to six
/// decimals without trailing zeros (5 / 3 → 1.666667).
#[must_use]
pub fn calc_text(v: f64) -> String {
    if !v.is_finite() {
        return String::new();
    }
    if (v - v.round()).abs() < 1e-9 {
        return format!("{}", v.round() as i64);
    }
    let s = format!("{v:.6}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// A field value read as a number (empty or not a number: 0).
fn number(s: &str) -> f64 {
    let t: String = s
        .chars()
        .filter(|c| !matches!(c, ',' | ' ' | '$'))
        .collect();
    t.parse().unwrap_or(0.0)
}

/// Parse `AFSimple_Calculate("SUM", new Array ("a", "b"))` or `AFSimple_Calculate("SUM",
/// "a, b")` out of a calculate script.
#[must_use]
pub fn parse_simple_calculate(js: &str) -> Option<(String, Vec<String>)> {
    let at = js.find("AFSimple_Calculate")?;
    let rest = &js[at + "AFSimple_Calculate".len()..];
    let open = rest.find('(')?;
    let body = &rest[open + 1..];
    // Quoted strings in order, up to the call's closing parenthesis.
    let mut strings = Vec::new();
    let mut depth = 0i32;
    let mut cur: Option<(char, String)> = None;
    for c in body.chars() {
        match &mut cur {
            Some((q, s)) => {
                if c == *q {
                    strings.push(std::mem::take(s));
                    cur = None;
                } else {
                    s.push(c);
                }
            }
            None => match c {
                '"' | '\'' => cur = Some((c, String::new())),
                '(' => depth += 1,
                ')' => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                }
                _ => {}
            },
        }
    }
    let mut it = strings.into_iter();
    let op = it.next()?.trim().to_ascii_uppercase();
    if !matches!(op.as_str(), "SUM" | "PRD" | "AVG" | "MIN" | "MAX") {
        return None;
    }
    let mut sources = Vec::new();
    for s in it {
        for part in s.split(',') {
            let p = part.trim();
            if !p.is_empty() {
                sources.push(p.to_owned());
            }
        }
    }
    (!sources.is_empty()).then_some((op, sources))
}

/// Apply `op` to `values`.
fn apply_op(op: &str, values: &[f64]) -> f64 {
    match op {
        "PRD" => values.iter().product(),
        "AVG" if !values.is_empty() => values.iter().sum::<f64>() / values.len() as f64,
        "MIN" => values.iter().copied().fold(f64::INFINITY, f64::min),
        "MAX" => values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        _ => values.iter().sum(),
    }
}

impl FormField {
    /// Both are the same widget (one box on a page).
    #[must_use]
    pub fn same_widget(&self, o: &FormField) -> bool {
        self.widget == o.widget
    }

    /// Both belong to the same field (a radio group's buttons do).
    #[must_use]
    pub fn same_field(&self, o: &FormField) -> bool {
        self.field == o.field
    }
}

impl PdfDocument {
    /// The annotation has the NoZoom flag: it is drawn at a fixed size from its top left
    /// corner (sticky notes).
    #[must_use]
    pub fn annotation_no_zoom(&self, a: &Annot) -> bool {
        a.id.and_then(|id| self.doc.get_dictionary(id).ok())
            .and_then(|d| d.get(b"F").ok())
            .and_then(|f| f.as_i64().ok())
            .is_some_and(|f| f & 8 != 0)
    }

    /// References in page `index`'s `/Annots`, in order.
    fn annots11(&self, index: usize) -> Vec<ObjectId> {
        let Some(info) = self.pages.get(index) else {
            return Vec::new();
        };
        self.doc
            .get_dictionary(info.id)
            .ok()
            .and_then(|d| d.get(b"Annots").ok())
            .map(|o| resolve(&self.doc, o))
            .and_then(|o| o.as_array().ok())
            .map(|a| a.iter().filter_map(|o| o.as_reference().ok()).collect())
            .unwrap_or_default()
    }

    /// The fields in the order Tab visits them: page by page; on a page by `/Tabs` (R rows,
    /// C columns, else the `/Annots` order); one stop per radio group (its chosen button, else
    /// the first); read-only, hidden, push button and signature fields are skipped.
    #[must_use]
    pub fn form_tab_order(&self) -> Vec<FormField> {
        let fields = self.form_fields();
        let mut out: Vec<FormField> = Vec::new();
        for page in 0..self.pages.len() {
            let order = self.annots11(page);
            let tabs = self
                .pages
                .get(page)
                .and_then(|i| self.doc.get_dictionary(i.id).ok())
                .and_then(|d| d.get(b"Tabs").ok())
                .and_then(|t| t.as_name().ok())
                .map(<[u8]>::to_vec)
                .unwrap_or_default();
            let hidden = |w: ObjectId| {
                self.doc
                    .get_dictionary(w)
                    .ok()
                    .and_then(|d| d.get(b"F").ok())
                    .and_then(|f| f.as_i64().ok())
                    .is_some_and(|f| f & 2 != 0)
            };
            let mut on_page: Vec<&FormField> = fields
                .iter()
                .filter(|f| {
                    f.page == page
                        && !f.read_only
                        && !matches!(f.kind, FieldKind::Button | FieldKind::Signature)
                        && !hidden(f.widget)
                })
                .collect();
            let pos = |f: &FormField| {
                order
                    .iter()
                    .position(|w| *w == f.widget)
                    .unwrap_or(usize::MAX)
            };
            match tabs.as_slice() {
                b"R" => on_page.sort_by(|a, b| {
                    (a.rect[1], a.rect[0])
                        .partial_cmp(&(b.rect[1], b.rect[0]))
                        .unwrap_or(std::cmp::Ordering::Equal)
                }),
                b"C" => on_page.sort_by(|a, b| {
                    (a.rect[0], a.rect[1])
                        .partial_cmp(&(b.rect[0], b.rect[1]))
                        .unwrap_or(std::cmp::Ordering::Equal)
                }),
                _ => on_page.sort_by_key(|f| pos(f)),
            }
            for f in on_page {
                if f.kind == FieldKind::Radio {
                    if let Some(k) = out.iter().position(|o| o.field == f.field) {
                        // The chosen button stands for the group.
                        if f.checked && !out[k].checked {
                            out[k] = f.clone();
                        }
                        continue;
                    }
                }
                out.push(f.clone());
            }
        }
        out
    }

    /// The calculate script of field `id` (`/AA /C /JS`, a string or a stream).
    fn calc_script(&self, id: ObjectId) -> Option<String> {
        let d = self.doc.get_dictionary(id).ok()?;
        let aa = resolve(&self.doc, d.get(b"AA").ok()?).as_dict().ok()?;
        let c = resolve(&self.doc, aa.get(b"C").ok()?).as_dict().ok()?;
        match resolve(&self.doc, c.get(b"JS").ok()?) {
            Object::String(s, _) => Some(crate::text_string(s)),
            Object::Stream(s) => crate::filters::stream_bytes(&self.doc, s)
                .ok()
                .map(|b| String::from_utf8_lossy(&b).into_owned()),
            _ => None,
        }
    }

    /// The calculated fields in calculation order (`/CO`), each with its simple calculation.
    #[must_use]
    pub fn calculations(&self) -> Vec<Calculation> {
        let doc = &self.doc;
        let Some(co) = doc
            .catalog()
            .ok()
            .and_then(|c| c.get(b"AcroForm").ok())
            .and_then(|a| resolve(doc, a).as_dict().ok())
            .and_then(|a| a.get(b"CO").ok())
            .and_then(|o| resolve(doc, o).as_array().ok())
        else {
            return Vec::new();
        };
        let fields = self.form_fields();
        co.iter()
            .filter_map(|o| o.as_reference().ok())
            .filter_map(|id| {
                let (op, sources) = parse_simple_calculate(&self.calc_script(id)?)?;
                let target = fields.iter().find(|f| f.field == id)?.name.clone();
                Some(Calculation {
                    target,
                    op,
                    sources,
                })
            })
            .collect()
    }

    /// Work out every calculated field (in `/CO` order, so that a total of totals sees the new
    /// sums) and write the values that changed. Returns how many changed.
    pub fn calculate_fields(&mut self) -> Result<usize, String> {
        let calcs = self.calculations();
        let mut changed = 0;
        for c in calcs {
            let fields = self.form_fields();
            let values: Vec<f64> = c
                .sources
                .iter()
                .map(|s| {
                    let prefix = format!("{s}.");
                    fields
                        .iter()
                        .filter(|f| f.name == *s || f.name.starts_with(&prefix))
                        .find(|f| matches!(f.kind, FieldKind::Text | FieldKind::Combo))
                        .map_or(0.0, |f| number(&f.value))
                })
                .collect();
            let text = calc_text(apply_op(&c.op, &values));
            let Some(target) = fields.into_iter().find(|f| f.name == c.target) else {
                continue;
            };
            if target.value != text {
                // Calculated fields are often read-only to the user, not to the calculation.
                let f = FormField {
                    read_only: false,
                    ..target
                };
                self.set_field_text(&f, &text)?;
                changed += 1;
            }
        }
        Ok(changed)
    }

    /// Place a signature (or initials) showing `ink` in `rect` (page points: left, top, right,
    /// bottom) of `page`, in colour `color`. It is one undo step and marked as Fill & Sign.
    pub fn add_signature_stamp(
        &mut self,
        page: usize,
        rect: [f32; 4],
        ink: &SignatureInk,
        color: [u8; 3],
        author: &str,
    ) -> Result<(), String> {
        if self.doc.is_encrypted() && self.doc.encryption_state.is_none() {
            return Err("comments cannot be added to an encrypted document".into());
        }
        let info = self.pages.get(page).ok_or("no such page")?.clone();
        let base = crate::page_matrix(info.crop, info.rotate);
        let inv = invert(base).ok_or("page matrix")?;
        let (x0, y0) = apply(inv, rect[0], rect[1]);
        let (x1, y1) = apply(inv, rect[2], rect[3]);
        let (l, b, r, t) = (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1));
        let (w, h) = ((r - l).max(1.0), (t - b).max(1.0));
        let [cr, cg, cb] = color.map(|v| f32::from(v) / 255.0);
        self.begin("signature");
        let result = (|| -> Result<(), String> {
            let mut res = Dictionary::new();
            let content = match ink {
                SignatureInk::Image(bytes) => {
                    let img = self.image_xobject(NewImage(bytes))?;
                    let mut xo = Dictionary::new();
                    xo.set("Im0", Object::Reference(img));
                    res.set("XObject", Object::Dictionary(xo));
                    format!("q {w:.3} 0 0 {h:.3} 0 0 cm /Im0 Do Q")
                }
                SignatureInk::Strokes(strokes) => {
                    let lw = (h * 0.035).max(0.8);
                    let mut s = format!("q {cr:.3} {cg:.3} {cb:.3} RG {lw:.2} w 1 J 1 j\n");
                    for st in strokes {
                        for (k, p) in st.iter().enumerate() {
                            let x = p[0].clamp(0.0, 1.0) * w;
                            let y = (1.0 - p[1].clamp(0.0, 1.0)) * h;
                            s.push_str(&format!(
                                "{x:.2} {y:.2} {}\n",
                                if k == 0 { "m" } else { "l" }
                            ));
                        }
                        if st.len() == 1 {
                            s.push_str(&format!(
                                "{:.2} {:.2} l\n",
                                st[0][0].clamp(0.0, 1.0) * w + 0.01,
                                (1.0 - st[0][1].clamp(0.0, 1.0)) * h
                            ));
                        }
                        s.push_str("S\n");
                    }
                    s.push('Q');
                    s
                }
                SignatureInk::Typed { text, face } => {
                    let (font, glyphs) = self.embed_text(*face, text)?;
                    let mut fonts = Dictionary::new();
                    fonts.set("BoS", Object::Reference(font));
                    res.set("Font", Object::Dictionary(fonts));
                    let em = typed_width(text, *face).max(0.1);
                    let size = (h * 0.72).min(w * 0.96 / em);
                    let x = (w - em * size) / 2.0;
                    let y = (h - size * 0.7) / 2.0;
                    let mut hex = String::from("<");
                    for g in &glyphs {
                        hex.push_str(&format!("{g:02X}"));
                    }
                    hex.push('>');
                    format!(
                        "BT /BoS {size:.2} Tf {cr:.3} {cg:.3} {cb:.3} rg {x:.2} {y:.2} Td {hex} Tj ET"
                    )
                }
            };
            let mut fd = Dictionary::new();
            fd.set("Type", Object::Name(b"XObject".to_vec()));
            fd.set("Subtype", Object::Name(b"Form".to_vec()));
            fd.set("BBox", vec![0.into(), 0.into(), w.into(), h.into()]);
            fd.set("Resources", Object::Dictionary(res));
            let ap = self.add(Object::Stream(Stream::new(fd, content.into_bytes())));
            let mut d = Dictionary::new();
            d.set("Type", Object::Name(b"Annot".to_vec()));
            d.set("Subtype", Object::Name(b"Stamp".to_vec()));
            d.set("Name", Object::Name(b"Signature".to_vec()));
            d.set("F", 4);
            d.set("P", Object::Reference(info.id));
            d.set("T", text_obj(author));
            let now = pdf_date();
            d.set("CreationDate", text_obj(&now));
            d.set("M", text_obj(&now));
            d.set("Rect", vec![l.into(), b.into(), r.into(), t.into()]);
            d.set(
                "C",
                vec![Object::Real(cr), Object::Real(cg), Object::Real(cb)],
            );
            d.set(FILL_KEY, true);
            let mut apd = Dictionary::new();
            apd.set("N", Object::Reference(ap));
            d.set("AP", Object::Dictionary(apd));
            let id = self.add(Object::Dictionary(d));
            self.push_annot(page, id)
        })();
        self.commit();
        result
    }

    /// How many entries page `page`'s `/Annots` has (to mark what is added next).
    #[must_use]
    pub fn annot_count(&self, page: usize) -> usize {
        self.annots11(page).len()
    }

    /// Mark the comments of `page` from `/Annots` entry `from` on as placed by Fill & Sign
    /// (inside the step that added them when one is open).
    pub fn mark_fill_sign(&mut self, page: usize, from: usize) -> Result<(), String> {
        let ids: Vec<ObjectId> = self.annots11(page).into_iter().skip(from).collect();
        if ids.is_empty() {
            return Ok(());
        }
        self.begin("fill & sign");
        for id in ids {
            self.touch(id);
            if let Ok(d) = self.doc.get_dictionary_mut(id) {
                d.set(FILL_KEY, true);
            }
        }
        self.commit();
        Ok(())
    }

    /// The comment was placed by Fill & Sign.
    #[must_use]
    pub fn is_fill_sign(&self, a: &Annot) -> bool {
        a.id.and_then(|id| self.doc.get_dictionary(id).ok())
            .and_then(|d| d.get(FILL_KEY).ok())
            .and_then(|o| o.as_bool().ok())
            .unwrap_or(false)
    }

    /// Draw every Fill & Sign mark and signature into its page and remove the comments, so
    /// they can no longer be moved (Fill & Sign's flatten on save). Returns how many.
    pub fn flatten_fill_sign(&mut self) -> Result<usize, String> {
        let pages: Vec<(usize, Vec<ObjectId>)> = (0..self.pages.len())
            .map(|p| {
                let ids = self
                    .annots11(p)
                    .into_iter()
                    .filter(|id| {
                        self.doc
                            .get_dictionary(*id)
                            .ok()
                            .and_then(|d| d.get(FILL_KEY).ok())
                            .and_then(|o| o.as_bool().ok())
                            .unwrap_or(false)
                    })
                    .collect::<Vec<_>>();
                (p, ids)
            })
            .filter(|(_, ids)| !ids.is_empty())
            .collect();
        if pages.is_empty() {
            return Ok(0);
        }
        self.begin("flatten fill & sign");
        let r = (|| {
            let mut n = 0;
            for (page, ids) in pages {
                let mut tail = String::new();
                for &a in &ids {
                    let Some((ap, m)) = self.widget_placement(a) else {
                        continue;
                    };
                    let name = self.add_resource(page, b"XObject", "BoFs", ap)?;
                    tail.push_str(&format!(
                        "\nq {} {} {} {} {} {} cm /{} Do Q\n",
                        m[0],
                        m[1],
                        m[2],
                        m[3],
                        m[4],
                        m[5],
                        String::from_utf8_lossy(&name)
                    ));
                    n += 1;
                }
                let (sid, content) = self.page_stream(page)?;
                self.write_page(sid, content, Vec::new(), tail.as_bytes());
                // Their pop-up notes go too.
                let popups: Vec<ObjectId> = self
                    .annots11(page)
                    .into_iter()
                    .filter(|p| {
                        self.doc
                            .get_dictionary(*p)
                            .ok()
                            .and_then(|d| d.get(b"Parent").ok())
                            .and_then(|o| o.as_reference().ok())
                            .is_some_and(|parent| ids.contains(&parent))
                    })
                    .collect();
                let gone: Vec<ObjectId> = ids.iter().copied().chain(popups).collect();
                self.remove_annots(page, &gone)?;
            }
            Ok(n)
        })();
        self.commit();
        r
    }
}

#[cfg(test)]
#[path = "forms11_tests.rs"]
mod tests;
