//! Office Math (OMML, ECMA-376 Part 1, §22.1) laid out as an inline object.
//!
//! A `m:oMath` / `m:oMathPara` element (kept verbatim in the model as [`bo_model::RunContent::Markup`]) is
//! laid out into a box with an ascent and a descent around the text baseline, holding glyph runs and
//! rules (fraction bars, radical signs, over/under bars). The layout follows the usual typesetting of
//! mathematics: fractions centred on the math axis, scripts shrunk and shifted, n-ary operators enlarged
//! with limits above/below or beside, delimiters grown to their content. Letters of `m:r` runs are set
//! in italic unless `m:sty` says otherwise (§22.1.2.90). This is an approximation of the OpenType MATH
//! table driven layout of dedicated math fonts: the bundled fonts carry no MATH table.

use bo_model::slide::{XmlElement, XmlNode};
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
    fn shifted(mut self, dx: f32, dy: f32) -> MathBox {
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
}

/// Size factor of scripts and limits (one level; deeper levels are not shrunk further below 60 %).
const SCRIPT: f32 = 0.7;

impl MathCtx<'_> {
    /// Height of the math axis above the baseline (where fraction bars sit).
    fn axis(size: f32) -> f32 {
        size * 0.25
    }

    /// A text run: italic letters unless `upright`.
    fn text(&mut self, s: &str, size: f32, upright: bool) -> MathBox {
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
        let small = (size * SCRIPT).max(size.min(6.0));
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
                Self::grid(rows, size, true)
            }
            // §22.1.2.34 eqArr: equations stacked, left aligned.
            "eqArr" => {
                let rows: Vec<Vec<MathBox>> = el
                    .elements()
                    .filter(|c| c.local() == "e")
                    .map(|c| vec![self.children(c, size)])
                    .collect();
                Self::grid(rows, size, false)
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
    fn grid(rows: Vec<Vec<MathBox>>, size: f32, centred: bool) -> MathBox {
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
        let mut y = -(total / 2.0) - Self::axis(size);
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

/// Lays out the OMML `xml` at `size` points with `upright` / `italic` faces.
pub(crate) fn layout_math(
    xml: &str,
    size: f32,
    fonts: &mut FontManager,
    upright: FaceId,
    italic: FaceId,
) -> Option<MathBox> {
    let el = bo_drawingml::dom::parse(xml.as_bytes(), "/word/document.xml").ok()?;
    let mut cx = MathCtx {
        fonts,
        upright,
        italic,
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
