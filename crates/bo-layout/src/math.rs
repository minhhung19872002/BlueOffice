//! Office Math (OMML, ECMA-376 Part 1, §22.1) laid out as an inline object.
//!
//! A `m:oMath` / `m:oMathPara` element (kept verbatim in the model as [`bo_model::RunContent::Markup`]) is
//! laid out into a box with an ascent and a descent around the text baseline, holding glyph runs and
//! rules (fraction bars, radical signs, over/under bars). The layout follows the usual typesetting of
//! mathematics: fractions centred on the math axis, scripts shrunk and shifted, n-ary operators enlarged
//! with limits above/below or beside, delimiters grown to their content. Letters of `m:r` runs are set
//! in italic unless `m:sty` says otherwise (§22.1.2.90).
//!
//! With a math font (an OpenType MATH table: the bundled STIX Two Math, or an installed math font) the
//! layout follows the table: `MathConstants` for shifts, gaps and rule thicknesses, `MathVariants` size
//! variants and glyph assemblies for delimiters, radicals and big operators, italic letters from the
//! Mathematical Alphanumeric Symbols block (Unicode §22.2). Without one, the text font is used with
//! approximate proportions.

use bo_model::slide::{XmlElement, XmlNode};
use bo_text::math::{MathConstants, Stretched};
use bo_text::{FaceId, FontManager};

/// A glyph run of the math box (y = baseline offset, positive downwards).
#[derive(Debug, Clone)]
pub(crate) struct MathRun {
    pub x: f32,
    pub y: f32,
    pub face: FaceId,
    pub size: f32,
    pub text: String,
    /// (glyph id, x offset from `x`, y offset from `y`, cluster).
    pub glyphs: Vec<(u16, f32, f32, u32)>,
    pub width: f32,
}

/// A laid-out math expression: `asc` above and `desc` below its baseline.
#[derive(Debug, Clone, Default)]
pub(crate) struct MathBox {
    pub w: f32,
    pub asc: f32,
    pub desc: f32,
    pub runs: Vec<MathRun>,
    /// Rules (x1, y1, x2, y2, width), y relative to the baseline.
    pub rules: Vec<(f32, f32, f32, f32, f32)>,
}

impl MathBox {
    /// The box moved right by `dx` and down by `dy` (its extents follow, measured from the old baseline).
    fn shifted(mut self, dx: f32, dy: f32) -> MathBox {
        self.asc = (self.asc - dy).max(0.0);
        self.desc = (self.desc + dy).max(0.0);
        for r in &mut self.runs {
            r.x += dx;
            r.y += dy;
        }
        for r in &mut self.rules {
            r.0 += dx;
            r.2 += dx;
            r.1 += dy;
            r.3 += dy;
        }
        self
    }

    /// Adds `b` with its baseline at `dy` and its left edge at `dx`.
    fn put(&mut self, b: MathBox, dx: f32, dy: f32) {
        self.asc = self.asc.max(b.asc - dy);
        self.desc = self.desc.max(b.desc + dy);
        self.w = self.w.max(dx + b.w);
        let b = b.shifted(dx, dy);
        self.runs.extend(b.runs);
        self.rules.extend(b.rules);
    }

    /// Horizontal concatenation on a common baseline.
    fn row(items: Vec<MathBox>) -> MathBox {
        let mut out = MathBox::default();
        let mut x = 0.0;
        for b in items {
            let w = b.w;
            out.put(b, x, 0.0);
            x += w;
        }
        out.w = x;
        out
    }
}

/// Fonts and size of the expression being laid out.
pub(crate) struct MathCtx<'a> {
    pub fonts: &'a mut FontManager,
    /// Upright face (the run's) and an italic face of the same family.
    pub upright: FaceId,
    pub italic: FaceId,
    /// Math font and its MATH constants, when one is available.
    pub math: Option<(FaceId, MathConstants)>,
    /// Display style (`m:oMathPara`): full-size fractions, large operators.
    pub display: bool,
}

/// The Mathematical Italic letter for `c` (Unicode Mathematical Alphanumeric Symbols, with the
/// italic small h at U+210E PLANCK CONSTANT), `None` for other characters.
fn math_italic(c: char) -> Option<char> {
    let u = c as u32;
    let m = match c {
        'h' => 0x210E,
        'a'..='z' => 0x1D44E + (u - 'a' as u32),
        'A'..='Z' => 0x1D434 + (u - 'A' as u32),
        'α'..='ω' => 0x1D6FC + (u - 'α' as u32),
        'Α'..='Ω' if c != '\u{3A2}' => 0x1D6E2 + (u - 'Α' as u32),
        _ => return None,
    };
    char::from_u32(m)
}

/// Size factor of scripts and limits (one level; deeper levels are not shrunk further below 60 %).
const SCRIPT: f32 = 0.7;

impl MathCtx<'_> {
    /// Height of the math axis above the baseline (where fraction bars sit).
    fn axis(size: f32) -> f32 {
        size * 0.25
    }

    /// A MATH constant at `size` (points).
    fn mv(c: &MathConstants, v: i16, size: f32) -> f32 {
        f32::from(v) * size / f32::from(c.units_per_em.max(1))
    }

    /// Height of the math axis at `size`.
    fn axis_at(&self, size: f32) -> f32 {
        match &self.math {
            Some((_, c)) => Self::mv(c, c.axis_height, size),
            None => Self::axis(size),
        }
    }

    /// Size of scripts and limits of a base at `size`.
    fn script_size(&self, size: f32) -> f32 {
        match &self.math {
            Some((_, c)) if c.script_percent_scale_down > 0 => {
                size * f32::from(c.script_percent_scale_down) / 100.0
            }
            _ => (size * SCRIPT).max(size.min(6.0)),
        }
    }

    /// Runs `f` in text style (inside fractions, scripts and limits).
    fn text_style<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let saved = std::mem::replace(&mut self.display, false);
        let r = f(self);
        self.display = saved;
        r
    }

    /// A text run in the math font: letters as Mathematical Italic unless `upright`, box from the ink.
    fn math_text(&mut self, s: &str, size: f32, upright: bool, mf: FaceId) -> MathBox {
        let mapped: String = s
            .chars()
            .map(|c| match math_italic(c) {
                Some(m) if !upright && self.fonts.has_glyph(mf, m) => m,
                _ => c,
            })
            .collect();
        let mut out = MathBox::default();
        let mut x = 0.0f32;
        for (r, face) in self.fonts.itemize(mf, &mapped) {
            let piece = &mapped[r];
            let shaped = self.fonts.shape(face, piece);
            let scale = size / f32::from(self.fonts.info(face).units_per_em);
            let mut gx = 0.0f32;
            let mut glyphs = Vec::with_capacity(shaped.len());
            for g in shaped.iter() {
                let dy = -(g.y_offset as f32) * scale;
                let e = self.fonts.glyph_extents(face, g.glyph_id);
                out.asc = out.asc.max(f32::from(e.y_max) * scale - dy);
                out.desc = out.desc.max(-f32::from(e.y_min) * scale + dy);
                glyphs.push((g.glyph_id, gx + g.x_offset as f32 * scale, dy, g.cluster));
                gx += g.x_advance as f32 * scale;
            }
            out.runs.push(MathRun {
                x,
                y: 0.0,
                face,
                size,
                text: piece.to_owned(),
                glyphs,
                width: gx,
            });
            x += gx;
        }
        out.w = x;
        out
    }

    /// The glyph of `c` in the math font grown to `target` points (vertically or horizontally), as a box
    /// on the baseline with its ink extents; also the italic correction (points) of the chosen glyph.
    fn stretched(&mut self, c: &str, size: f32, target: f32, vertical: bool) -> (MathBox, f32) {
        let Some((mf, k)) = self.math else {
            return (self.text(c, size, true), 0.0);
        };
        let Some(ch) = c.chars().next() else {
            return (MathBox::default(), 0.0);
        };
        let base = self.fonts.glyph_of(mf, ch);
        if base == 0 || c.chars().count() != 1 {
            return (self.math_text(c, size, true, mf), 0.0);
        }
        let scale = size / f32::from(k.units_per_em.max(1));
        let units = (target / scale).ceil() as i32;
        let parts = match self.fonts.stretch(mf, base, units, vertical) {
            Stretched::Glyph(g) => vec![(g, 0i32)],
            Stretched::Assembly { parts, .. } => parts,
        };
        let mut out = MathBox::default();
        let mut glyphs = Vec::with_capacity(parts.len());
        let mut width = 0.0f32;
        let mut ic = 0.0;
        for &(g, off) in &parts {
            let e = self.fonts.glyph_extents(mf, g);
            let (gx, gy) = if vertical {
                (0.0, -(off as f32) * scale)
            } else {
                (off as f32 * scale, 0.0)
            };
            out.asc = out.asc.max(f32::from(e.y_max) * scale - gy);
            out.desc = out.desc.max(-f32::from(e.y_min) * scale + gy);
            width = width.max(gx + f32::from(e.advance) * scale);
            ic = f32::from(self.fonts.italic_correction(mf, g)) * scale;
            glyphs.push((g, gx, gy, 0));
        }
        out.runs.push(MathRun {
            x: 0.0,
            y: 0.0,
            face: mf,
            size,
            text: c.to_owned(),
            glyphs,
            width,
        });
        out.w = width;
        (out, ic)
    }

    /// A text run: italic letters unless `upright`.
    fn text(&mut self, s: &str, size: f32, upright: bool) -> MathBox {
        if let Some((mf, _)) = self.math {
            return self.math_text(s, size, upright, mf);
        }
        let mut out = MathBox::default();
        let mut x = 0.0f32;
        // Letters italic (§22.1.2.90 sty default "i" for single letters), digits/operators upright.
        let mut segs: Vec<(String, bool)> = Vec::new();
        for c in s.chars() {
            let it = !upright && c.is_alphabetic();
            match segs.last_mut() {
                Some((t, i)) if *i == it => t.push(c),
                _ => segs.push((c.to_string(), it)),
            }
        }
        for (seg, it) in segs {
            let primary = if it { self.italic } else { self.upright };
            for (r, face) in self.fonts.itemize(primary, &seg) {
                let piece = &seg[r];
                let shaped = self.fonts.shape(face, piece);
                let info = self.fonts.info(face);
                let scale = size / f32::from(info.units_per_em);
                let asc = f32::from(info.ascender) * scale;
                let desc = -f32::from(info.descender) * scale;
                let mut gx = 0.0f32;
                let mut glyphs = Vec::with_capacity(shaped.len());
                for g in shaped.iter() {
                    glyphs.push((
                        g.glyph_id,
                        gx + g.x_offset as f32 * scale,
                        -(g.y_offset as f32) * scale,
                        g.cluster,
                    ));
                    gx += g.x_advance as f32 * scale;
                }
                out.runs.push(MathRun {
                    x,
                    y: 0.0,
                    face,
                    size,
                    text: piece.to_owned(),
                    glyphs,
                    width: gx,
                });
                // Glyph boxes of math text: the font's typical heights, not the line gap.
                out.asc = out.asc.max(asc * 0.8);
                out.desc = out.desc.max(desc * 0.8);
                x += gx;
            }
        }
        out.w = x;
        out
    }

    /// An operator or delimiter character grown to cover `asc` / `desc` (centred on the axis).
    fn stretchy(&mut self, c: &str, size: f32, asc: f32, desc: f32) -> MathBox {
        if self.math.is_some() {
            // Symmetric around the axis, covering the content (as TeX and the MATH table intend).
            let axis = self.axis_at(size);
            let target = 2.0 * (asc - axis).max(desc + axis);
            let (b, _) = self.stretched(c, size, target, true);
            let mid = (b.desc - b.asc) / 2.0;
            return b.shifted(0.0, -axis - mid);
        }
        let base = self.text(c, size, true);
        let need = (asc + desc).max(1.0);
        let have = (base.asc + base.desc).max(1.0);
        let factor = (need / have).max(1.0);
        let b = if factor > 1.05 {
            self.text(c, size * factor, true)
        } else {
            base
        };
        // Centre on the content's middle.
        let mid = (desc - asc) / 2.0;
        let bmid = (b.desc - b.asc) / 2.0;
        b.shifted(0.0, mid - bmid)
    }

    fn children(&mut self, el: &XmlElement, size: f32) -> MathBox {
        let items: Vec<MathBox> = el.elements().filter_map(|c| self.node(c, size)).collect();
        MathBox::row(items)
    }

    fn arg(&mut self, el: &XmlElement, name: &str, size: f32) -> MathBox {
        match el.kid(name) {
            Some(c) => self.children(c, size),
            None => MathBox::default(),
        }
    }

    fn prop<'e>(el: &'e XmlElement, pr: &str, name: &str) -> Option<&'e str> {
        el.kid(pr)?.kid(name)?.val()
    }

    /// One OMML element (§22.1.2); `None` for property elements.
    #[allow(clippy::too_many_lines)]
    fn node(&mut self, el: &XmlElement, size: f32) -> Option<MathBox> {
        let small = self.script_size(size);
        if let Some((_, c)) = self.math {
            if let Some(b) = self.math_node(el, size, small, &c) {
                return Some(b);
            }
        }
        Some(match el.local() {
            n if n.ends_with("Pr") => return None,
            // §22.1.2.98 r: text run (m:t); §22.1.2.90 sty p/b = upright.
            "r" => {
                let text: String = el
                    .elements()
                    .filter(|e| e.local() == "t")
                    .map(text_of)
                    .collect();
                let sty = el
                    .kid("rPr")
                    .and_then(|p| p.kid("sty"))
                    .and_then(|s| s.val());
                let nor = el.kid("rPr").and_then(|p| p.kid("nor")).is_some();
                let upright = nor || matches!(sty, Some("p" | "b"));
                // Operators get a little room (thick spaces around relations, §22.1.2 spacing).
                let spaced: String = text
                    .chars()
                    .flat_map(|c| {
                        if "=<>≤≥≠≈±∓+−×÷→←⇒⇔∈∉⊂⊆∪∩".contains(c)
                        {
                            vec!['\u{2005}', c, '\u{2005}']
                        } else {
                            vec![c]
                        }
                    })
                    .collect();
                self.text(&spaced, size, upright)
            }
            // §22.1.2.36 f: fraction.
            "f" => {
                let ty = Self::prop(el, "fPr", "type").unwrap_or("bar");
                let num = self.arg(el, "num", size * 0.9);
                let den = self.arg(el, "den", size * 0.9);
                if ty == "lin" || ty == "skw" {
                    let slash = self.text("/", size, true);
                    return Some(MathBox::row(vec![num, slash, den]));
                }
                let w = num.w.max(den.w) + size * 0.2;
                let axis = Self::axis(size);
                let gap = size * 0.12;
                let mut out = MathBox {
                    w,
                    ..MathBox::default()
                };
                let num_base = -axis - gap - num.desc;
                let den_base = -axis + gap + den.asc;
                let (nw, dw) = (num.w, den.w);
                out.put(num, (w - nw) / 2.0, num_base);
                out.put(den, (w - dw) / 2.0, den_base);
                if ty != "noBar" {
                    out.rules.push((
                        size * 0.05,
                        -axis,
                        w - size * 0.05,
                        -axis,
                        (size * 0.05).max(0.5),
                    ));
                }
                out
            }
            // §22.1.2.112 sSup, §22.1.2.110 sSub, §22.1.2.111 sSubSup, §22.1.2.99 sPre.
            "sSup" | "sSub" | "sSubSup" | "sPre" => {
                let e = self.arg(el, "e", size);
                let sup = el.kid("sup").map(|c| self.children(c, small));
                let sub = el.kid("sub").map(|c| self.children(c, small));
                let sup_shift = (e.asc - small * 0.3).max(size * 0.35);
                let sub_shift = (e.desc + small * 0.1).max(size * 0.15);
                let mut scripts = MathBox::default();
                let sw = sup
                    .as_ref()
                    .map_or(0.0, |b| b.w)
                    .max(sub.as_ref().map_or(0.0, |b| b.w));
                if let Some(s) = sup {
                    scripts.put(s, 0.0, -sup_shift);
                }
                if let Some(s) = sub {
                    scripts.put(s, 0.0, sub_shift);
                }
                scripts.w = sw + size * 0.05;
                if el.local() == "sPre" {
                    MathBox::row(vec![scripts, e])
                } else {
                    MathBox::row(vec![e, scripts])
                }
            }
            // §22.1.2.88 rad: radical with optional degree.
            "rad" => {
                let e = self.arg(el, "e", size);
                let hide = Self::prop(el, "radPr", "degHide")
                    .is_some_and(|v| v == "1" || v == "on" || v == "true");
                let deg = if hide {
                    None
                } else {
                    el.kid("deg").map(|c| self.children(c, size * 0.55))
                };
                let lw = (size * 0.05).max(0.5);
                let top = -(e.asc + size * 0.15);
                let bottom = e.desc;
                let sign_w = size * 0.55;
                let dx = deg.as_ref().map_or(0.0, |d| (d.w - sign_w * 0.4).max(0.0));
                let mut out = MathBox::default();
                if let Some(d) = deg {
                    let dh = d.desc;
                    out.put(d, 0.0, top * 0.45 - dh);
                }
                let x0 = dx;
                // √ sign: short rise, long stroke down, long stroke up, then the bar over the radicand.
                out.rules.push((
                    x0,
                    -Self::axis(size) * 0.6,
                    x0 + sign_w * 0.25,
                    -Self::axis(size),
                    lw,
                ));
                out.rules.push((
                    x0 + sign_w * 0.25,
                    -Self::axis(size),
                    x0 + sign_w * 0.5,
                    bottom,
                    lw * 1.6,
                ));
                out.rules
                    .push((x0 + sign_w * 0.5, bottom, x0 + sign_w, top, lw));
                let ew = e.w;
                out.rules
                    .push((x0 + sign_w, top, x0 + sign_w + ew + size * 0.1, top, lw));
                out.put(e, x0 + sign_w + size * 0.05, 0.0);
                out.asc = out.asc.max(-top + lw);
                out.desc = out.desc.max(bottom);
                out.w = x0 + sign_w + ew + size * 0.15;
                out
            }
            // §22.1.2.70 nary: big operator with limits.
            "nary" => {
                let chr = Self::prop(el, "naryPr", "chr").unwrap_or("∫").to_owned();
                let hide = |n: &str| {
                    Self::prop(el, "naryPr", n)
                        .is_some_and(|v| v == "1" || v == "on" || v == "true")
                };
                let integral = matches!(chr.as_str(), "∫" | "∬" | "∭" | "∮" | "∯" | "∰");
                let loc = Self::prop(el, "naryPr", "limLoc").unwrap_or(if integral {
                    "subSup"
                } else {
                    "undOvr"
                });
                let op = self.text(&chr, size * 1.5, true).shifted(0.0, size * 0.1);
                let e = self.arg(el, "e", size);
                let sub = if hide("subHide") {
                    None
                } else {
                    el.kid("sub").map(|c| self.children(c, small))
                };
                let sup = if hide("supHide") {
                    None
                } else {
                    el.kid("sup").map(|c| self.children(c, small))
                };
                let mut opbox = MathBox::default();
                if loc == "undOvr" {
                    let w =
                        op.w.max(sub.as_ref().map_or(0.0, |b| b.w))
                            .max(sup.as_ref().map_or(0.0, |b| b.w));
                    let (oasc, odesc, ow) = (op.asc, op.desc, op.w);
                    opbox.put(op, (w - ow) / 2.0, 0.0);
                    if let Some(s) = sup {
                        let (sw, sd) = (s.w, s.desc);
                        opbox.put(s, (w - sw) / 2.0, -oasc - sd - size * 0.05);
                    }
                    if let Some(s) = sub {
                        let (sw, sa) = (s.w, s.asc);
                        opbox.put(s, (w - sw) / 2.0, odesc + sa + size * 0.05);
                    }
                    opbox.w = w;
                } else {
                    let (oasc, odesc, ow) = (op.asc, op.desc, op.w);
                    opbox.put(op, 0.0, 0.0);
                    let mut sw: f32 = 0.0;
                    if let Some(s) = sup {
                        sw = sw.max(s.w);
                        opbox.put(s, ow, -oasc + small * 0.6);
                    }
                    if let Some(s) = sub {
                        sw = sw.max(s.w);
                        opbox.put(s, ow - size * 0.1, odesc);
                    }
                    opbox.w = ow + sw;
                }
                opbox.w += size * 0.15;
                MathBox::row(vec![opbox, e])
            }
            // §22.1.2.24 d: delimiters around one or more m:e separated by sepChr.
            "d" => {
                let attr = |n: &str| Self::prop(el, "dPr", n).map(str::to_owned);
                let beg = attr("begChr").unwrap_or_else(|| "(".into());
                let end = attr("endChr").unwrap_or_else(|| ")".into());
                let sep = attr("sepChr").unwrap_or_else(|| "|".into());
                let parts: Vec<MathBox> = el
                    .elements()
                    .filter(|c| c.local() == "e")
                    .map(|c| self.children(c, size))
                    .collect();
                let mut inner_items = Vec::new();
                let n = parts.len();
                for (i, p) in parts.into_iter().enumerate() {
                    inner_items.push(p);
                    if i + 1 < n {
                        let s = self.text(&sep, size, true);
                        inner_items.push(s);
                    }
                }
                let inner = MathBox::row(inner_items);
                let (a, d) = (inner.asc.max(size * 0.7), inner.desc.max(size * 0.2));
                let mut items = Vec::new();
                if !beg.is_empty() {
                    items.push(self.stretchy(&beg, size, a, d));
                }
                items.push(inner);
                if !end.is_empty() {
                    items.push(self.stretchy(&end, size, a, d));
                }
                MathBox::row(items)
            }
            // §22.1.2.60 m: matrix of m:mr rows of m:e cells.
            "m" => {
                let rows: Vec<Vec<MathBox>> = el
                    .elements()
                    .filter(|r| r.local() == "mr")
                    .map(|r| {
                        r.elements()
                            .filter(|c| c.local() == "e")
                            .map(|c| self.children(c, size))
                            .collect()
                    })
                    .collect();
                let axis = self.axis_at(size);
                Self::grid(rows, size, axis, true)
            }
            // §22.1.2.34 eqArr: equations stacked, left aligned.
            "eqArr" => {
                let rows: Vec<Vec<MathBox>> = el
                    .elements()
                    .filter(|c| c.local() == "e")
                    .map(|c| vec![self.children(c, size)])
                    .collect();
                let axis = self.axis_at(size);
                Self::grid(rows, size, axis, false)
            }
            // §22.1.2.39 func: function name upright, thin space, argument.
            "func" => {
                let name = el
                    .kid("fName")
                    .map(|c| self.upright_children(c, size))
                    .unwrap_or_default();
                let gap = MathBox {
                    w: size * 0.17,
                    ..MathBox::default()
                };
                let e = self.arg(el, "e", size);
                MathBox::row(vec![name, gap, e])
            }
            // §22.1.2.1 acc: accent over the base.
            "acc" => {
                let chr = Self::prop(el, "accPr", "chr")
                    .unwrap_or("\u{0302}")
                    .to_owned();
                let e = self.arg(el, "e", size);
                let shown: String = match chr.as_str() {
                    "\u{0302}" => "^".into(),
                    "\u{0303}" => "~".into(),
                    "\u{0307}" => "˙".into(),
                    "\u{0308}" => "¨".into(),
                    "\u{20D7}" => "→".into(),
                    "\u{0305}" | "\u{0304}" => "¯".into(),
                    other => other.into(),
                };
                let a = self.text(&shown, size * 0.8, true);
                let mut out = MathBox::default();
                let (ew, easc, aw, adesc) = (e.w, e.asc, a.w, a.desc);
                out.put(e, 0.0, 0.0);
                out.put(a, (ew - aw).max(0.0) / 2.0, -easc - adesc + size * 0.2);
                out.w = ew.max(aw);
                out
            }
            // §22.1.2.7 bar: line over or under the base.
            "bar" => {
                let top = Self::prop(el, "barPr", "pos").is_some_and(|p| p == "top");
                let mut e = self.arg(el, "e", size);
                let lw = (size * 0.05).max(0.5);
                if top {
                    let y = -e.asc - size * 0.08;
                    e.rules.push((0.0, y, e.w, y, lw));
                    e.asc += size * 0.1;
                } else {
                    let y = e.desc + size * 0.08;
                    e.rules.push((0.0, y, e.w, y, lw));
                    e.desc += size * 0.1;
                }
                e
            }
            // §22.1.2.54 limLow / §22.1.2.56 limUpp: limit under or over the base.
            "limLow" | "limUpp" => {
                let e = self.arg(el, "e", size);
                let lim = self.arg(el, "lim", small);
                let w = e.w.max(lim.w);
                let mut out = MathBox::default();
                let (ew, easc, edesc, lw, lasc, ldesc) =
                    (e.w, e.asc, e.desc, lim.w, lim.asc, lim.desc);
                out.put(e, (w - ew) / 2.0, 0.0);
                if el.local() == "limLow" {
                    out.put(lim, (w - lw) / 2.0, edesc + lasc + size * 0.05);
                } else {
                    out.put(lim, (w - lw) / 2.0, -easc - ldesc - size * 0.05);
                }
                out.w = w;
                out
            }
            // §22.1.2.41 groupChr: brace under/over the base.
            "groupChr" => {
                let chr = Self::prop(el, "groupChrPr", "chr")
                    .unwrap_or("⏟")
                    .to_owned();
                let above = Self::prop(el, "groupChrPr", "pos").is_some_and(|p| p == "top");
                let e = self.arg(el, "e", size);
                let g = self.text(&chr, size, true);
                let mut out = MathBox::default();
                let (ew, easc, edesc, gw, gasc, gdesc) = (e.w, e.asc, e.desc, g.w, g.asc, g.desc);
                out.put(e, 0.0, 0.0);
                let y = if above { -easc - gdesc } else { edesc + gasc };
                out.put(g, (ew - gw).max(0.0) / 2.0, y);
                out.w = ew.max(gw);
                out
            }
            // §22.1.2.13 borderBox: frame around the base.
            "borderBox" => {
                let mut e = self.arg(el, "e", size);
                let lw = (size * 0.05).max(0.5);
                let pad = size * 0.1;
                let e2 = std::mem::take(&mut e);
                let mut out = MathBox::default();
                let (w, a, d) = (e2.w + 2.0 * pad, e2.asc + pad, e2.desc + pad);
                out.put(e2, pad, 0.0);
                out.rules.extend([
                    (0.0, -a, w, -a, lw),
                    (0.0, d, w, d, lw),
                    (0.0, -a, 0.0, d, lw),
                    (w, -a, w, d, lw),
                ]);
                out.w = w;
                out.asc = a;
                out.desc = d;
                out
            }
            // §22.1.2.81 phant: invisible base (keeps its size unless zeroed).
            "phant" => {
                let mut e = self.arg(el, "e", size);
                if Self::prop(el, "phantPr", "show")
                    .is_none_or(|v| v == "0" || v == "off" || v == "false")
                {
                    e.runs.clear();
                    e.rules.clear();
                }
                e
            }
            // Containers (oMath, oMathPara, e, box, …): their children in a row.
            _ => self.children(el, size),
        })
    }

    /// MATH-table layout of fractions, scripts, radicals, big operators, bars and limits; `None` for the
    /// other elements (laid out by [`MathCtx::node`]).
    #[allow(clippy::too_many_lines)]
    fn math_node(
        &mut self,
        el: &XmlElement,
        size: f32,
        small: f32,
        c: &MathConstants,
    ) -> Option<MathBox> {
        let mv = |v: i16| Self::mv(c, v, size);
        let display = self.display;
        Some(match el.local() {
            // §22.1.2.36 f (bar fractions; linear and skewed ones take the generic path).
            "f" if !matches!(Self::prop(el, "fPr", "type"), Some("lin" | "skw")) => {
                let no_bar = Self::prop(el, "fPr", "type") == Some("noBar");
                let part = if display { size } else { small };
                let (num, den) =
                    self.text_style(|s| (s.arg(el, "num", part), s.arg(el, "den", part)));
                let axis = mv(c.axis_height);
                let t = if no_bar {
                    0.0
                } else {
                    mv(c.fraction_rule_thickness)
                };
                let (up, down, gn, gd) = if display {
                    (
                        mv(c.fraction_numerator_display_style_shift_up),
                        mv(c.fraction_denominator_display_style_shift_down),
                        mv(c.fraction_num_display_style_gap_min),
                        mv(c.fraction_denom_display_style_gap_min),
                    )
                } else {
                    (
                        mv(c.fraction_numerator_shift_up),
                        mv(c.fraction_denominator_shift_down),
                        mv(c.fraction_numerator_gap_min),
                        mv(c.fraction_denominator_gap_min),
                    )
                };
                // Baselines (y down): the shift, or more when the gap to the bar would be too small.
                let num_base = (-up).min(-axis - t / 2.0 - gn - num.desc);
                let den_base = down.max(-axis + t / 2.0 + gd + den.asc);
                let pad = size * 0.1;
                let w = num.w.max(den.w) + 2.0 * pad;
                let mut out = MathBox {
                    w,
                    ..MathBox::default()
                };
                let (nw, dw) = (num.w, den.w);
                out.put(num, (w - nw) / 2.0, num_base);
                out.put(den, (w - dw) / 2.0, den_base);
                if !no_bar {
                    out.rules.push((pad / 2.0, -axis, w - pad / 2.0, -axis, t));
                }
                out
            }
            // §22.1.2.112 sSup, §22.1.2.110 sSub, §22.1.2.111 sSubSup, §22.1.2.99 sPre.
            "sSup" | "sSub" | "sSubSup" | "sPre" => {
                let e = self.arg(el, "e", size);
                let (sup, sub) = self.text_style(|s| {
                    (
                        el.kid("sup").map(|k| s.children(k, small)),
                        el.kid("sub").map(|k| s.children(k, small)),
                    )
                });
                let (sup_shift, sub_shift) =
                    Self::script_shifts(c, size, &e, sup.as_ref(), sub.as_ref());
                let mut scripts = MathBox::default();
                let sw = sup
                    .as_ref()
                    .map_or(0.0, |b| b.w)
                    .max(sub.as_ref().map_or(0.0, |b| b.w));
                if let Some(s) = sup {
                    scripts.put(s, 0.0, -sup_shift);
                }
                if let Some(s) = sub {
                    scripts.put(s, 0.0, sub_shift);
                }
                scripts.w = sw + mv(c.space_after_script);
                if el.local() == "sPre" {
                    MathBox::row(vec![scripts, e])
                } else {
                    MathBox::row(vec![e, scripts])
                }
            }
            // §22.1.2.88 rad: the radical glyph grown to the radicand, the overbar from its end.
            "rad" => {
                let e = self.arg(el, "e", size);
                let hide = Self::prop(el, "radPr", "degHide")
                    .is_some_and(|v| v == "1" || v == "on" || v == "true");
                let deg = if hide {
                    None
                } else {
                    el.kid("deg")
                        .map(|k| self.text_style(|s| s.children(k, s.script_size(small))))
                };
                let gap = if display {
                    mv(c.radical_display_style_vertical_gap)
                } else {
                    mv(c.radical_vertical_gap)
                };
                let t = mv(c.radical_rule_thickness);
                let (sign, _) = self.stretched("√", size, e.asc + e.desc + gap + t, true);
                // Top of the sign's ink on the top of the bar.
                let top = -(e.asc + gap + t);
                let sasc = sign.asc;
                let sign = sign.shifted(0.0, top + sasc);
                let (sa, sd, sw) = (top.abs(), sign.desc, sign.w);
                let mut out = MathBox::default();
                let mut x = 0.0;
                if let Some(d) = deg {
                    // Degree raised by a percentage of the sign's height, kerned before and after.
                    let raise =
                        f32::from(c.radical_degree_bottom_raise_percent) / 100.0 * (sa + sd);
                    let dw = d.w;
                    let kb = mv(c.radical_kern_before_degree);
                    out.put(d, kb, sd - raise);
                    x = (kb + dw + mv(c.radical_kern_after_degree)).max(0.0);
                }
                out.put(sign, x, 0.0);
                let ew = e.w;
                out.put(e, x + sw, 0.0);
                out.rules
                    .push((x + sw, top + t / 2.0, x + sw + ew, top + t / 2.0, t));
                out.asc = out.asc.max(sa + mv(c.radical_extra_ascender));
                out.w = x + sw + ew;
                out
            }
            // §22.1.2.70 nary: the operator (display size in display style) with limits or scripts.
            "nary" => {
                let chr = Self::prop(el, "naryPr", "chr").unwrap_or("∫").to_owned();
                let hide = |n: &str| {
                    Self::prop(el, "naryPr", n)
                        .is_some_and(|v| v == "1" || v == "on" || v == "true")
                };
                let integral = matches!(chr.as_str(), "∫" | "∬" | "∭" | "∮" | "∯" | "∰");
                let loc = Self::prop(el, "naryPr", "limLoc")
                    .unwrap_or(if integral { "subSup" } else { "undOvr" })
                    .to_owned();
                let min = if display {
                    f32::from(c.display_operator_min_height) * size
                        / f32::from(c.units_per_em.max(1))
                } else {
                    0.0
                };
                let (op, ic) = self.stretched(&chr, size, min, true);
                // Centred on the axis.
                let axis = mv(c.axis_height);
                let mid = (op.desc - op.asc) / 2.0;
                let op = op.shifted(0.0, -axis - mid);
                let e = self.arg(el, "e", size);
                let (sub, sup) = self.text_style(|s| {
                    (
                        if hide("subHide") {
                            None
                        } else {
                            el.kid("sub").map(|k| s.children(k, small))
                        },
                        if hide("supHide") {
                            None
                        } else {
                            el.kid("sup").map(|k| s.children(k, small))
                        },
                    )
                });
                let mut opbox = MathBox::default();
                let (oasc, odesc, ow) = (op.asc, op.desc, op.w);
                if loc == "undOvr" {
                    let w = ow
                        .max(sub.as_ref().map_or(0.0, |b| b.w))
                        .max(sup.as_ref().map_or(0.0, |b| b.w));
                    opbox.put(op, (w - ow) / 2.0, 0.0);
                    if let Some(s) = sup {
                        let rise = (mv(c.upper_limit_gap_min) + s.desc)
                            .max(mv(c.upper_limit_baseline_rise_min));
                        let sw = s.w;
                        opbox.put(s, (w - sw) / 2.0 + ic / 2.0, -oasc - rise);
                    }
                    if let Some(s) = sub {
                        let drop = (mv(c.lower_limit_gap_min) + s.asc)
                            .max(mv(c.lower_limit_baseline_drop_min));
                        let sw = s.w;
                        opbox.put(s, (w - sw) / 2.0 - ic / 2.0, odesc + drop);
                    }
                    opbox.w = w;
                } else {
                    let (sup_shift, sub_shift) = Self::script_shifts(
                        c,
                        size,
                        &opbox_of(oasc, odesc),
                        sup.as_ref(),
                        sub.as_ref(),
                    );
                    opbox.put(op, 0.0, 0.0);
                    let mut sw: f32 = 0.0;
                    if let Some(s) = sup {
                        sw = sw.max(s.w + ic);
                        opbox.put(s, ow + ic, -sup_shift);
                    }
                    if let Some(s) = sub {
                        sw = sw.max(s.w);
                        opbox.put(s, ow, sub_shift);
                    }
                    opbox.w = ow + sw + mv(c.space_after_script);
                }
                opbox.w += size * 0.1;
                MathBox::row(vec![opbox, e])
            }
            // §22.1.2.1 acc: the accent (grown horizontally when the font has variants) centred on the
            // base's ink, raised only by what the base rises above AccentBaseHeight.
            "acc" => {
                let chr = Self::prop(el, "accPr", "chr")
                    .unwrap_or("\u{0302}")
                    .to_owned();
                let e = self.arg(el, "e", size);
                let (a, _) = self.stretched(&chr, size, e.w, false);
                let (ax0, ax1) = self.ink_x(&a);
                let lift = (e.asc - mv(c.accent_base_height)).max(0.0);
                let (ew, aasc) = (e.w, a.asc);
                let mut out = MathBox::default();
                out.put(e, 0.0, 0.0);
                out.put(a, ew / 2.0 - f32::midpoint(ax0, ax1), -lift);
                out.asc = out.asc.max(aasc + lift);
                out.w = ew;
                out
            }
            // §22.1.2.7 bar: over- or underbar with the MATH gaps and thickness.
            "bar" => {
                let top = Self::prop(el, "barPr", "pos").is_some_and(|p| p == "top");
                let mut e = self.arg(el, "e", size);
                if top {
                    let t = mv(c.overbar_rule_thickness);
                    let y = -e.asc - mv(c.overbar_vertical_gap) - t / 2.0;
                    e.rules.push((0.0, y, e.w, y, t));
                    e.asc += mv(c.overbar_vertical_gap) + t + mv(c.overbar_extra_ascender);
                } else {
                    let t = mv(c.underbar_rule_thickness);
                    let y = e.desc + mv(c.underbar_vertical_gap) + t / 2.0;
                    e.rules.push((0.0, y, e.w, y, t));
                    e.desc += mv(c.underbar_vertical_gap) + t + mv(c.underbar_extra_descender);
                }
                e
            }
            // §22.1.2.54 limLow / §22.1.2.56 limUpp with the limit gaps of the MATH table.
            "limLow" | "limUpp" => {
                let e = self.arg(el, "e", size);
                let lim = self.text_style(|s| s.arg(el, "lim", small));
                let w = e.w.max(lim.w);
                let mut out = MathBox::default();
                let (ew, easc, edesc, lw, lasc, ldesc) =
                    (e.w, e.asc, e.desc, lim.w, lim.asc, lim.desc);
                out.put(e, (w - ew) / 2.0, 0.0);
                if el.local() == "limLow" {
                    let drop =
                        (mv(c.lower_limit_gap_min) + lasc).max(mv(c.lower_limit_baseline_drop_min));
                    out.put(lim, (w - lw) / 2.0, edesc + drop);
                } else {
                    let rise = (mv(c.upper_limit_gap_min) + ldesc)
                        .max(mv(c.upper_limit_baseline_rise_min));
                    out.put(lim, (w - lw) / 2.0, -easc - rise);
                }
                out.w = w;
                out
            }
            _ => return None,
        })
    }

    /// Left and right ink edges (points) of the glyphs of `b`.
    fn ink_x(&self, b: &MathBox) -> (f32, f32) {
        let (mut x0, mut x1) = (f32::MAX, f32::MIN);
        for r in &b.runs {
            let scale = r.size / f32::from(self.fonts.info(r.face).units_per_em.max(1));
            for &(g, gx, _, _) in &r.glyphs {
                let e = self.fonts.glyph_extents(r.face, g);
                x0 = x0.min(r.x + gx + f32::from(e.x_min) * scale);
                x1 = x1.max(r.x + gx + f32::from(e.x_max) * scale);
            }
        }
        if x0 > x1 {
            (0.0, b.w)
        } else {
            (x0, x1)
        }
    }

    /// Superscript shift up and subscript shift down (points) for a base `e` (OpenType MATH: the
    /// shift constants, the baseline drop/rise limits, and the minimum gap between the two scripts).
    fn script_shifts(
        c: &MathConstants,
        size: f32,
        e: &MathBox,
        sup: Option<&MathBox>,
        sub: Option<&MathBox>,
    ) -> (f32, f32) {
        let mv = |v: i16| Self::mv(c, v, size);
        let mut up = mv(c.superscript_shift_up).max(e.asc - mv(c.superscript_baseline_drop_max));
        if let Some(s) = sup {
            up = up.max(s.desc + mv(c.superscript_bottom_min));
        }
        let mut down = mv(c.subscript_shift_down).max(e.desc + mv(c.subscript_baseline_drop_min));
        if let Some(s) = sub {
            down = down.max(s.asc - mv(c.subscript_top_max));
        }
        if let (Some(p), Some(b)) = (sup, sub) {
            // Gap between the bottom of the superscript and the top of the subscript.
            let gap = (up - p.desc) - (b.asc - down);
            let min = mv(c.sub_superscript_gap_min);
            if gap < min {
                let extra = min - gap;
                // Raise the superscript up to its limit, lower the subscript for the rest.
                let room = (mv(c.superscript_bottom_max_with_subscript) - (up - p.desc)).max(0.0);
                let r = room.min(extra);
                up += r;
                down += extra - r;
            }
        }
        (up, down)
    }

    /// Children of `el` with letters upright (function names).
    fn upright_children(&mut self, el: &XmlElement, size: f32) -> MathBox {
        let items: Vec<MathBox> = el
            .elements()
            .filter_map(|c| {
                if c.local() == "r" {
                    let text: String = c
                        .elements()
                        .filter(|e| e.local() == "t")
                        .map(text_of)
                        .collect();
                    Some(self.text(&text, size, true))
                } else {
                    self.node(c, size)
                }
            })
            .collect();
        MathBox::row(items)
    }

    /// Rows of cells: columns centred (matrix) or left aligned, rows centred on the axis.
    fn grid(rows: Vec<Vec<MathBox>>, size: f32, axis: f32, centred: bool) -> MathBox {
        let ncol = rows.iter().map(Vec::len).max().unwrap_or(0);
        let mut widths = vec![0.0f32; ncol];
        for r in &rows {
            for (i, c) in r.iter().enumerate() {
                widths[i] = widths[i].max(c.w);
            }
        }
        let col_gap = size * 0.6;
        let row_gap = size * 0.2;
        let heights: Vec<(f32, f32)> = rows
            .iter()
            .map(|r| {
                (
                    r.iter().map(|c| c.asc).fold(size * 0.7, f32::max),
                    r.iter().map(|c| c.desc).fold(size * 0.2, f32::max),
                )
            })
            .collect();
        let total: f32 = heights.iter().map(|(a, d)| a + d).sum::<f32>()
            + row_gap * rows.len().saturating_sub(1) as f32;
        let mut y = -(total / 2.0) - axis;
        let mut out = MathBox::default();
        for (r, (a, d)) in rows.into_iter().zip(heights) {
            let base = y + a;
            let mut x = 0.0;
            for (i, c) in r.into_iter().enumerate() {
                let off = if centred {
                    (widths[i] - c.w) / 2.0
                } else {
                    0.0
                };
                out.put(c, x + off, base);
                x += widths[i] + col_gap;
            }
            y = base + d + row_gap;
        }
        out.w = widths.iter().sum::<f32>() + col_gap * ncol.saturating_sub(1) as f32;
        out
    }
}

/// Child lookup by local name (OMML prefixes vary) and the `m:val` attribute (§22.1.2).
trait Kid {
    fn kid(&self, local: &str) -> Option<&XmlElement>;
    fn val(&self) -> Option<&str>;
}

impl Kid for XmlElement {
    fn kid(&self, local: &str) -> Option<&XmlElement> {
        self.elements().find(|e| e.local() == local)
    }
    fn val(&self) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k.rsplit(':').next() == Some("val"))
            .map(|(_, v)| v.as_str())
    }
}

fn text_of(el: &XmlElement) -> String {
    el.children
        .iter()
        .map(|n| match n {
            XmlNode::Text(t) => t.clone(),
            XmlNode::Element(e) => text_of(e),
        })
        .collect()
}

/// Alignment of a display equation: `Some` for an `m:oMathPara` (ECMA-376 Part 1, §22.1.2.78 oMathPara,
/// §22.1.2.79 oMathParaPr, §22.1.2.51 jc: left / right / center / centerGroup — the default, centred as a
/// group), `None` for an inline `m:oMath`.
pub(crate) fn display_alignment(xml: &str) -> Option<bo_model::Alignment> {
    let el = bo_drawingml::dom::parse(xml.as_bytes(), "/word/document.xml").ok()?;
    if el.local() != "oMathPara" {
        return None;
    }
    let jc = el
        .kid("oMathParaPr")
        .and_then(|p| p.kid("jc"))
        .and_then(|j| j.val());
    Some(match jc {
        Some("left") => bo_model::Alignment::Left,
        Some("right") => bo_model::Alignment::Right,
        _ => bo_model::Alignment::Center,
    })
}

/// A box with only an ascent and a descent (a base for script shifts).
fn opbox_of(asc: f32, desc: f32) -> MathBox {
    MathBox {
        asc,
        desc,
        ..MathBox::default()
    }
}

/// Lays out the OMML `xml` at `size` points with `upright` / `italic` faces and, when given, a math font
/// (`math`: a face with an OpenType MATH table).
pub(crate) fn layout_math(
    xml: &str,
    size: f32,
    fonts: &mut FontManager,
    upright: FaceId,
    italic: FaceId,
    math: Option<FaceId>,
) -> Option<MathBox> {
    let el = bo_drawingml::dom::parse(xml.as_bytes(), "/word/document.xml").ok()?;
    let math = math.and_then(|f| fonts.math_constants(f).map(|c| (f, c)));
    let display = el.local() == "oMathPara";
    let mut cx = MathCtx {
        fonts,
        upright,
        italic,
        math,
        display,
    };
    let mut b = cx.children(&el, size);
    // A little room around the zone.
    let pad = size * 0.08;
    b = b.shifted(pad, 0.0);
    b.w += 2.0 * pad;
    b.asc = b.asc.max(size * 0.7);
    b.desc = b.desc.max(size * 0.2);
    Some(b)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bo_text::ttf;

    const M: &str = r#"xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math""#;

    fn fonts() -> (FontManager, FaceId, FaceId) {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
        let mut fm = FontManager::new();
        fm.add_fonts_from_dir(&dir);
        let text = fm.resolve("times new roman", false, false).unwrap();
        let math = fm.resolve("cambria math", false, false).unwrap();
        assert_eq!(fm.info(math).family, "STIX Two Math");
        (fm, text, math)
    }

    fn lay(fm: &mut FontManager, text: FaceId, math: FaceId, xml: &str) -> MathBox {
        layout_math(xml, 20.0, fm, text, text, Some(math)).unwrap()
    }

    /// The MATH table read straight from the bundled font file (the values the layout must honour).
    fn table(fm: &FontManager, math: FaceId) -> (u16, bo_text::math::MathConstants) {
        let c = fm.math_constants(math).unwrap();
        (c.units_per_em, c)
    }

    fn glyphs_of(b: &MathBox, text: &str) -> Vec<(u16, f32, f32)> {
        b.runs
            .iter()
            .filter(|r| r.text == text)
            .flat_map(|r| r.glyphs.iter().map(move |g| (g.0, r.x + g.1, r.y + g.2)))
            .collect()
    }

    // source: OpenType MATH constants of STIX Two Math (read from the font file) — CORE-MATH-003.
    #[test]
    fn display_fraction_uses_the_math_constants() {
        let (mut fm, text, math) = fonts();
        let (upm, c) = table(&fm, math);
        let k = 20.0 / f32::from(upm);
        let xml = format!(
            r"<m:oMathPara {M}><m:oMath><m:f><m:num><m:r><m:t>a</m:t></m:r></m:num><m:den><m:r><m:t>b</m:t></m:r></m:den></m:f></m:oMath></m:oMathPara>"
        );
        let b = lay(&mut fm, text, math, &xml);
        // The fraction bar: on the axis, with the table's thickness.
        let bar = b.rules[0];
        assert!(
            (bar.1 + f32::from(c.axis_height) * k).abs() < 1e-3,
            "{bar:?}"
        );
        assert!((bar.4 - f32::from(c.fraction_rule_thickness) * k).abs() < 1e-3);
        // Numerator raised by at least the display shift, denominator lowered by at least its shift.
        let a = &b.runs.iter().find(|r| r.text == "\u{1D44E}").unwrap();
        let bb = &b.runs.iter().find(|r| r.text == "\u{1D44F}").unwrap();
        assert!(-a.y >= f32::from(c.fraction_numerator_display_style_shift_up) * k - 1e-3);
        assert!(bb.y >= f32::from(c.fraction_denominator_display_style_shift_down) * k - 1e-3);
        // Letters are Mathematical Italic glyphs of the math font.
        assert_eq!(a.face, math);
        assert_eq!(a.glyphs[0].0, fm.glyph_of(math, '\u{1D44E}'));
    }

    // source: MathVariants of STIX Two Math (vertical constructions of "(" and "∫").
    #[test]
    fn delimiters_and_operators_grow_with_variants_and_assemblies() {
        let (mut fm, text, math) = fonts();
        let (upm, c) = table(&fm, math);
        let k = 20.0 / f32::from(upm);
        let paren = fm.glyph_of(math, '(');
        // A two-level fraction in parentheses: a larger size variant replaces "(".
        let frac = r"<m:f><m:num><m:r><m:t>a</m:t></m:r></m:num><m:den><m:r><m:t>b</m:t></m:r></m:den></m:f>";
        let xml = format!(r"<m:oMath {M}><m:d><m:e>{frac}</m:e></m:d></m:oMath>");
        let b = lay(&mut fm, text, math, &xml);
        let open = glyphs_of(&b, "(");
        assert_eq!(open.len(), 1);
        assert_ne!(open[0].0, paren, "a size variant");
        // Variants come from the table: the chosen glyph is one of the vertical variants of "(".
        let face = fm.face(math).unwrap();
        let variants: Vec<u16> = face
            .tables()
            .math
            .unwrap()
            .variants
            .unwrap()
            .vertical_constructions
            .get(ttf::GlyphId(paren))
            .unwrap()
            .variants
            .into_iter()
            .map(|v| v.variant_glyph.0)
            .collect();
        assert!(variants.contains(&open[0].0), "{variants:?}");
        // A ten-row matrix is taller than every variant: a glyph assembly (several part glyphs).
        let rows: String = (0..10)
            .map(|_| "<m:mr><m:e><m:r><m:t>x</m:t></m:r></m:e></m:mr>")
            .collect();
        let xml = format!(r"<m:oMath {M}><m:d><m:e><m:m>{rows}</m:m></m:e></m:d></m:oMath>");
        let b = lay(&mut fm, text, math, &xml);
        let open = glyphs_of(&b, "(");
        assert!(open.len() >= 3, "assembly parts: {open:?}");
        let content = b.asc + b.desc;
        let (top, bottom) = open
            .iter()
            .fold((f32::MAX, f32::MIN), |(t, bt), &(g, _, y)| {
                let e = fm.glyph_extents(math, g);
                (
                    t.min(y - f32::from(e.y_max) * k),
                    bt.max(y - f32::from(e.y_min) * k),
                )
            });
        assert!(
            bottom - top >= content * 0.9,
            "{} vs {content}",
            bottom - top
        );
        // A display integral is at least DisplayOperatorMinHeight tall.
        let xml = format!(
            r#"<m:oMathPara {M}><m:oMath><m:nary><m:naryPr><m:chr m:val="∫"/></m:naryPr><m:sub/><m:sup/><m:e><m:r><m:t>x</m:t></m:r></m:e></m:nary></m:oMath></m:oMathPara>"#
        );
        let b = lay(&mut fm, text, math, &xml);
        let int = glyphs_of(&b, "∫");
        let e = fm.glyph_extents(math, int[0].0);
        assert!(
            i32::from(e.y_max) - i32::from(e.y_min)
                >= i32::from(c.display_operator_min_height) * 9 / 10
        );
        assert_ne!(int[0].0, fm.glyph_of(math, '∫'));
    }

    // source: radical constants of STIX Two Math.
    #[test]
    fn radical_sign_covers_the_radicand() {
        let (mut fm, text, math) = fonts();
        let (upm, c) = table(&fm, math);
        let k = 20.0 / f32::from(upm);
        let frac = r"<m:f><m:num><m:r><m:t>a</m:t></m:r></m:num><m:den><m:r><m:t>b</m:t></m:r></m:den></m:f>";
        let xml = format!(
            r#"<m:oMath {M}><m:rad><m:radPr><m:degHide m:val="1"/></m:radPr><m:deg/><m:e>{frac}</m:e></m:rad></m:oMath>"#
        );
        let b = lay(&mut fm, text, math, &xml);
        let sign = glyphs_of(&b, "√");
        assert_ne!(sign, [] as [(u16, f32, f32); 0]);
        // The overbar has the radical rule thickness and sits above the radicand by the vertical gap.
        let bar = b
            .rules
            .iter()
            .find(|r| (r.4 - f32::from(c.radical_rule_thickness) * k).abs() < 1e-3);
        assert!(bar.is_some(), "{:?}", b.rules);
        let (_, sign_x, _) = sign[0];
        assert!(bar.unwrap().0 > sign_x);
    }
}
