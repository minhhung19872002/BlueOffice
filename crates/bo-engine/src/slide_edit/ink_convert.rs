//! Ink conversion (CORE-INK-002): ink strokes become DrawingML preset shapes
//! (recognised by [`super::ink_recognize`]), a text box holding recognised
//! handwriting, or an equation (UnicodeMath / LaTeX through
//! [`crate::equation`], written as [MS-ODRAWXML] `a14:m` with a text
//! fallback). Every conversion is one undo step; strokes that are not
//! converted stay ink.

use super::build;
use super::ink::{self, emu, InkStrokeIn, Obj};
use super::ink_recognize::{self as rec, Fit, Pt, Recognized, ShapeKind};
use super::ops::{OpError, Tx};
use super::{next_shape_id, SlideSel, R};
use bo_model::slide::{Geometry, LineEnd, ShapeNode};
use serde::Serialize;

/// One shape made from ink.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertedShape {
    /// `cNvPr@id` of the new (or extended) shape.
    pub id: u32,
    /// `text` / `math` for handwriting, else the recognised shape.
    pub kind: String,
}

/// What a conversion made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InkConverted {
    /// The new shapes (or the line given an arrowhead).
    pub shapes: Vec<ConvertedShape>,
    /// Strokes of the chosen ink that stayed ink.
    pub kept: usize,
}

fn kind_name(k: ShapeKind) -> String {
    serde_json::to_value(k)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Pen of a converted shape.
#[derive(Debug, Clone, PartialEq)]
struct Pen {
    /// `RRGGBB`.
    color: String,
    /// Points.
    width: f64,
    opacity: f64,
}

fn hex6(s: &str) -> String {
    let s = s.trim_start_matches('#');
    if s.len() == 6 && s.chars().all(|c| c.is_ascii_hexdigit()) {
        s.to_ascii_uppercase()
    } else {
        "000000".into()
    }
}

/// §20.1.8.3 solidFill with §20.1.2.3.1 alpha for translucent ink.
fn solid(p: &Pen) -> String {
    let alpha = if p.opacity < 1.0 {
        format!(
            r#"<a:alpha val="{}"/>"#,
            (p.opacity.clamp(0.0, 1.0) * 100_000.0).round() as i64
        )
    } else {
        String::new()
    };
    format!(
        r#"<a:solidFill><a:srgbClr val="{}">{alpha}</a:srgbClr></a:solidFill>"#,
        hex6(&p.color)
    )
}

/// §20.1.2.2.24 ln in the ink's colour and width (§20.1.8.38 headEnd,
/// §20.1.8.57 tailEnd arrows).
fn ln(p: &Pen, head: bool, tail: bool) -> String {
    format!(
        r#"<a:ln w="{}" cap="rnd">{}<a:round/>{}{}</a:ln>"#,
        // Hundredths of a point: InkML stores widths in its own units.
        emu((p.width.clamp(0.25, 200.0) * 100.0).round() / 100.0),
        solid(p),
        if head {
            r#"<a:headEnd type="triangle"/>"#
        } else {
            ""
        },
        if tail {
            r#"<a:tailEnd type="triangle"/>"#
        } else {
            ""
        },
    )
}

/// The shape for a recognised stroke group.
fn shape_xml(id: u32, r: &Recognized, p: &Pen) -> String {
    let preset = r.kind.preset();
    let name = build::esc(&format!("{preset} {id}"));
    match r.fit {
        Fit::Line {
            from,
            to,
            head_from,
            head_to,
        } => {
            // §19.3.1.19 cxnSp with a `line` preset: from the box's top-left to
            // its bottom-right, flipped to run from `from` to `to` (§20.1.7.6).
            let (x0, y0) = (from.0.min(to.0), from.1.min(to.1));
            let flip_h = if to.0 < from.0 { r#" flipH="1""# } else { "" };
            let flip_v = if to.1 < from.1 { r#" flipV="1""# } else { "" };
            format!(
                concat!(
                    r#"<p:cxnSp><p:nvCxnSpPr><p:cNvPr id="{id}" name="{name}"/><p:cNvCxnSpPr/><p:nvPr/></p:nvCxnSpPr>"#,
                    r#"<p:spPr><a:xfrm{fh}{fv}><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm>"#,
                    r#"<a:prstGeom prst="line"><a:avLst/></a:prstGeom>{ln}</p:spPr></p:cxnSp>"#
                ),
                id = id,
                name = name,
                fh = flip_h,
                fv = flip_v,
                x = emu(x0),
                y = emu(y0),
                cx = emu((to.0 - from.0).abs()),
                cy = emu((to.1 - from.1).abs()),
                ln = ln(p, head_from, head_to),
            )
        }
        Fit::Frame {
            center,
            w,
            h,
            rot,
            adj,
        } => {
            // §20.1.7.6 xfrm: the unrotated box, turned about its centre.
            let rot = ((rot.rem_euclid(360.0)) * 60_000.0).round() as i64 % 21_600_000;
            let rot = if rot == 0 {
                String::new()
            } else {
                format!(r#" rot="{rot}""#)
            };
            // §20.1.9.11 gd: the triangle's apex position.
            let av = adj.map_or(String::new(), |a| {
                format!(
                    r#"<a:gd name="adj" fmla="val {}"/>"#,
                    (a.clamp(0.0, 1.0) * 100_000.0).round() as i64
                )
            });
            format!(
                concat!(
                    r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{name}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>"#,
                    r#"<p:spPr><a:xfrm{rot}><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm>"#,
                    r#"<a:prstGeom prst="{preset}"><a:avLst>{av}</a:avLst></a:prstGeom><a:noFill/>{ln}</p:spPr>"#,
                    r#"<p:txBody><a:bodyPr rtlCol="0" anchor="ctr"/><a:lstStyle/>{paras}</p:txBody></p:sp>"#
                ),
                id = id,
                name = name,
                rot = rot,
                x = emu(center.0 - w / 2.0),
                y = emu(center.1 - h / 2.0),
                cx = emu(w.max(1.0)),
                cy = emu(h.max(1.0)),
                preset = preset,
                av = av,
                ln = ln(p, false, false),
                paras = build::paragraphs("", r#"<a:pPr algn="ctr"/>"#, ""),
            )
        }
    }
}

/// One stroke of the chosen ink.
struct StrokeRef {
    obj: usize,
    trace: usize,
    pts: Vec<Pt>,
    pen: Pen,
}

/// The chosen ink objects of `slide` (all when `ids` is empty).
fn chosen(tx: &Tx<'_>, slide: usize, ids: &[u32]) -> Vec<Obj> {
    ink::objects(tx.pres, slide)
        .into_iter()
        .filter(|o| ids.is_empty() || ids.contains(&o.id))
        .collect()
}

/// Every stroke of `objs` in slide points (highlighter strokes too when `all`).
fn strokes(objs: &[Obj], all: bool) -> (Vec<StrokeRef>, usize) {
    let mut out = Vec::new();
    let mut skipped = 0;
    for (oi, o) in objs.iter().enumerate() {
        let scale = o.place.scale.0.midpoint(o.place.scale.1);
        for (ti, t) in o.traces.iter().enumerate() {
            if !all && t.brush.is_highlighter() {
                skipped += 1;
                continue;
            }
            out.push(StrokeRef {
                obj: oi,
                trace: ti,
                pts: o.slide_tips(t).iter().map(|p| (p.x, p.y)).collect(),
                pen: Pen {
                    color: t.brush.hex(),
                    width: t.brush.width * scale,
                    opacity: t.brush.opacity(),
                },
            });
        }
    }
    (out, skipped)
}

/// A new shape: the `(object, trace)` pairs it replaces and its markup (given its id).
type Item<'a> = (Vec<(usize, usize)>, Box<dyn Fn(u32) -> String + 'a>);

/// Replace strokes of `objs` with new shapes, each placed where the lowest of
/// its ink objects was in the z-order. `items`: the `(object, trace)` pairs a
/// shape replaces and its markup builder (given the shape id).
fn replace(tx: &mut Tx<'_>, slide: usize, objs: &[Obj], items: &[Item<'_>]) -> R<Vec<u32>> {
    let base = next_shape_id(tx.pres, slide);
    let mut nodes = Vec::new();
    for (k, (refs, make)) in items.iter().enumerate() {
        let id = base + k as u32;
        let node = build::node(&make(id)).ok_or(super::EditError::Refused("build"))?;
        let z = refs
            .iter()
            .filter_map(|&(o, _)| objs.get(o).map(|o| o.index))
            .min()
            .unwrap_or(usize::MAX);
        nodes.push((z, id, node));
    }
    // Strokes per object, then the objects rewritten from the top down.
    let mut gone: Vec<Vec<usize>> = vec![Vec::new(); objs.len()];
    for (refs, _) in items {
        for &(o, t) in refs {
            if let Some(g) = gone.get_mut(o) {
                g.push(t);
            }
        }
    }
    let mut order: Vec<usize> = (0..objs.len()).filter(|&o| !gone[o].is_empty()).collect();
    order.sort_by_key(|&o| std::cmp::Reverse(objs[o].index));
    let mut removed = Vec::new();
    for o in order {
        let mut doc = objs[o].doc.clone();
        gone[o].sort_unstable();
        gone[o].dedup();
        doc.remove_traces(&gone[o]);
        if ink::put_doc(tx, slide, &objs[o], &doc)? {
            removed.push(objs[o].index);
        }
    }
    // Insert from the bottom up, each at its object's place after removals.
    nodes.sort_by_key(|n| n.0);
    let len = tx.pres.slides.get(slide).map_or(0, |s| s.data.shapes.len());
    let mut ids = Vec::new();
    for (k, (z, id, node)) in nodes.into_iter().enumerate() {
        let below = removed.iter().filter(|&&i| i < z).count();
        let at = (z.saturating_sub(below) + k).min(len + k);
        tx.insert_nodes(slide, &[], at, vec![node])?;
        ids.push(id);
    }
    Ok(ids)
}

/// `inkToShapes`.
pub fn cmd_to_shapes(tx: &mut Tx<'_>, sel: &mut SlideSel, ids: &[u32]) -> R<InkConverted> {
    let slide = sel.slide;
    if slide >= tx.pres.slides.len() {
        return Err(OpError::NoSlide(slide).into());
    }
    let objs = chosen(tx, slide, ids);
    let (refs, skipped) = strokes(&objs, false);
    let pts: Vec<Vec<Pt>> = refs.iter().map(|s| s.pts.clone()).collect();
    let units = rec::recognize_strokes(&pts);
    let used: usize = units.iter().map(|u| u.strokes.len()).sum();
    let kept = refs.len() - used + skipped;
    let kinds: Vec<ShapeKind> = units.iter().map(|u| u.rec.kind).collect();
    let items: Vec<Item<'_>> = units
        .into_iter()
        .map(|u| {
            let members: Vec<(usize, usize)> = u
                .strokes
                .iter()
                .map(|&i| (refs[i].obj, refs[i].trace))
                .collect();
            // The first stroke (the shaft of an arrow) gives the pen.
            let pen = refs[u.strokes[0]].pen.clone();
            let r = u.rec;
            let make: Box<dyn Fn(u32) -> String> = Box::new(move |id| shape_xml(id, &r, &pen));
            (members, make)
        })
        .collect();
    let made = replace(tx, slide, &objs, &items)?;
    sel.ids.clone_from(&made);
    Ok(InkConverted {
        shapes: made
            .into_iter()
            .zip(kinds)
            .map(|(id, k)| ConvertedShape {
                id,
                kind: kind_name(k),
            })
            .collect(),
        kept,
    })
}

/// End points of a top-level line shape (`line` preset, not rotated).
fn line_ends(n: &ShapeNode) -> Option<(Pt, Pt)> {
    let props = match n {
        ShapeNode::Connector(c) => &c.props,
        ShapeNode::Shape(s) => &s.props,
        _ => return None,
    };
    match &props.geometry {
        Some(Geometry::Preset { name, .. }) if name == "line" => {}
        _ => return None,
    }
    let x = props.xfrm.as_ref()?;
    if x.rot.unwrap_or(0) != 0 {
        return None;
    }
    let (ox, oy) = x.off?;
    let (cx, cy) = x.ext?;
    let pt = |v: i64| v as f64 / super::geom::EMU_PT;
    let (l, t, r, b) = (pt(ox), pt(oy), pt(ox + cx), pt(oy + cy));
    let (fx, tx_) = if x.flip_h == Some(true) {
        (r, l)
    } else {
        (l, r)
    };
    let (fy, ty) = if x.flip_v == Some(true) {
        (b, t)
    } else {
        (t, b)
    };
    Some(((fx, fy), (tx_, ty)))
}

/// `addInk` with `autoShape`: the stroke as an arrowhead of `attach_to`, or as
/// a recognised shape; `None` (nothing done) when it is neither.
pub fn cmd_add_auto(
    tx: &mut Tx<'_>,
    sel: &mut SlideSel,
    s: &InkStrokeIn,
    attach_to: Option<u32>,
) -> R<Option<InkConverted>> {
    let slide = sel.slide;
    if slide >= tx.pres.slides.len() || s.highlighter {
        return Ok(None);
    }
    let pts: Vec<Pt> = s.points.iter().map(|p| (p[0], p[1])).collect();
    if let Some(target) = attach_to {
        let found = tx.pres.slides[slide]
            .data
            .shapes
            .iter()
            .enumerate()
            .find(|(_, n)| n.nv().is_some_and(|nv| nv.id == target))
            .and_then(|(i, n)| Some((i, line_ends(n)?, n.clone())));
        if let Some((index, (from, to), mut node)) = found {
            if let Some(at_to) = rec::head_end(from, to, &pts) {
                let props = match &mut node {
                    ShapeNode::Connector(c) => &mut c.props,
                    ShapeNode::Shape(s) => &mut s.props,
                    _ => return Ok(None),
                };
                let line = props.line.get_or_insert_with(Default::default);
                let end = if at_to {
                    &mut line.tail
                } else {
                    &mut line.head
                };
                // The second stroke of a head is taken in by the first.
                if end
                    .as_ref()
                    .and_then(|e| e.kind.as_deref())
                    .is_none_or(|k| k == "none")
                {
                    *end = Some(LineEnd {
                        kind: Some("triangle".into()),
                        w: None,
                        len: None,
                    });
                }
                tx.replace_node(slide, &[index], node)?;
                sel.ids.clear();
                return Ok(Some(InkConverted {
                    shapes: vec![ConvertedShape {
                        id: target,
                        kind: kind_name(ShapeKind::Arrow),
                    }],
                    kept: 0,
                }));
            }
        }
    }
    let Some(r) = rec::recognize_stroke(&pts) else {
        return Ok(None);
    };
    let id = next_shape_id(tx.pres, slide);
    let pen = Pen {
        color: s.color.clone(),
        width: s.width,
        opacity: 1.0,
    };
    let node = build::node(&shape_xml(id, &r, &pen)).ok_or(super::EditError::Refused("build"))?;
    let at = tx.pres.slides[slide].data.shapes.len();
    tx.insert_nodes(slide, &[], at, vec![node])?;
    sel.ids.clear();
    Ok(Some(InkConverted {
        shapes: vec![ConvertedShape {
            id,
            kind: kind_name(r.kind),
        }],
        kept: 0,
    }))
}

/// Font size (points) of text written in a box `h` points tall over `lines` lines.
fn text_size(h: f64, lines: usize) -> f64 {
    (h / lines.max(1) as f64 * 0.75).round().clamp(8.0, 120.0)
}

/// Text box in the ink's place and colour (§19.3.1.43 sp, `txBox`).
fn text_xml(id: u32, text: &str, rect: (f64, f64, f64, f64), pen: &Pen) -> String {
    let sz = (text_size(rect.3, text.lines().count()) * 100.0) as i64;
    let rpr = format!(
        r#"<a:rPr lang="{}" sz="{sz}" dirty="0">{}</a:rPr>"#,
        build::LANG,
        solid(pen)
    );
    let mut paras = String::new();
    for l in text.split('\n') {
        paras.push_str("<a:p>");
        if !l.is_empty() {
            paras.push_str(&format!("<a:r>{rpr}<a:t>{}</a:t></a:r>", build::esc(l)));
        }
        paras.push_str(&format!(
            r#"<a:endParaRPr lang="{}" sz="{sz}" dirty="0"/></a:p>"#,
            build::LANG
        ));
    }
    format!(
        concat!(
            r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="TextBox {id}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr>"#,
            r#"<p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm>"#,
            r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/></p:spPr>"#,
            r#"<p:txBody><a:bodyPr wrap="none" rtlCol="0"><a:spAutoFit/></a:bodyPr><a:lstStyle/>{paras}</p:txBody></p:sp>"#
        ),
        id = id,
        x = emu(rect.0),
        y = emu(rect.1),
        cx = emu(rect.2.max(12.0)),
        cy = emu(rect.3.max(12.0)),
        paras = paras,
    )
}

/// An equation text box: [MS-ODRAWXML] `a14:m` holding the OMML zone
/// (ECMA-376 Part 1, §22.1.2.77 oMathPara) inside mc:AlternateContent
/// (ECMA-376 Part 3, §10.2.1), with the linear text as the fallback.
fn math_xml(id: u32, text: &str, latex: bool, rect: (f64, f64, f64, f64)) -> String {
    let zone = crate::equation::build(text, latex, true);
    let linear = crate::equation::info(&zone).map_or_else(|| text.to_owned(), |i| i.linear);
    let sz = (text_size(rect.3, 1) * 100.0) as i64;
    let sp = |body: &str| {
        format!(
            concat!(
                r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="Equation {id}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr>"#,
                r#"<p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm>"#,
                r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/></p:spPr>"#,
                r#"<p:txBody><a:bodyPr wrap="none" rtlCol="0"><a:spAutoFit/></a:bodyPr><a:lstStyle/><a:p>{body}"#,
                r#"<a:endParaRPr lang="{lang}" sz="{sz}" dirty="0"/></a:p></p:txBody></p:sp>"#
            ),
            id = id,
            x = emu(rect.0),
            y = emu(rect.1),
            cx = emu(rect.2.max(12.0)),
            cy = emu(rect.3.max(12.0)),
            body = body,
            lang = build::LANG,
            sz = sz,
        )
    };
    let choice = sp(&format!("<a14:m>{zone}</a14:m>"));
    let fallback = sp(&format!(
        r#"<a:r><a:rPr lang="{}" sz="{sz}" dirty="0"/><a:t>{}</a:t></a:r>"#,
        build::LANG,
        build::esc(&linear)
    ));
    format!(
        concat!(
            r#"<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">"#,
            r#"<mc:Choice xmlns:a14="http://schemas.microsoft.com/office/drawing/2010/main" Requires="a14">{choice}</mc:Choice>"#,
            r#"<mc:Fallback>{fallback}</mc:Fallback></mc:AlternateContent>"#
        ),
        choice = choice,
        fallback = fallback,
    )
}

/// `inkToText` (`math: None`) and `inkToMath` (`Some(latex)`): every stroke of
/// the chosen ink is replaced by one text box or equation at the ink's box.
pub fn cmd_to_text(
    tx: &mut Tx<'_>,
    sel: &mut SlideSel,
    ids: &[u32],
    text: &str,
    math: Option<bool>,
) -> R<InkConverted> {
    let slide = sel.slide;
    if slide >= tx.pres.slides.len() {
        return Err(OpError::NoSlide(slide).into());
    }
    let text = text.trim();
    if text.is_empty() || text.len() > 10_000 {
        return Err(super::EditError::Refused("badText"));
    }
    let objs = chosen(tx, slide, ids);
    let (refs, _) = strokes(&objs, true);
    let all: Vec<Pt> = refs.iter().flat_map(|s| s.pts.iter().copied()).collect();
    if all.is_empty() {
        return Err(super::EditError::Refused("noInk"));
    }
    let (x0, y0, x1, y1) = all.iter().fold(
        (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ),
        |(a, b, c, d), &(x, y)| (a.min(x), b.min(y), c.max(x), d.max(y)),
    );
    let rect = (x0, y0, x1 - x0, y1 - y0);
    let pen = refs[0].pen.clone();
    let members: Vec<(usize, usize)> = refs.iter().map(|s| (s.obj, s.trace)).collect();
    let text = text.to_owned();
    let make: Box<dyn Fn(u32) -> String> = match math {
        None => Box::new(move |id| text_xml(id, &text, rect, &pen)),
        Some(latex) => Box::new(move |id| math_xml(id, &text, latex, rect)),
    };
    let made = replace(tx, slide, &objs, &[(members, make)])?;
    sel.ids.clone_from(&made);
    Ok(InkConverted {
        shapes: made
            .into_iter()
            .map(|id| ConvertedShape {
                id,
                kind: if math.is_some() { "math" } else { "text" }.into(),
            })
            .collect(),
        kept: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_and_colours() {
        assert_eq!(text_size(40.0, 1), 30.0);
        assert_eq!(text_size(2.0, 1), 8.0);
        assert_eq!(hex6("#1f5fbf"), "1F5FBF");
        assert_eq!(hex6("bad"), "000000");
    }

    #[test]
    fn line_markup_runs_from_start_to_end() {
        let r = Recognized {
            kind: ShapeKind::Arrow,
            fit: Fit::Line {
                from: (100.0, 50.0),
                to: (10.0, 50.0),
                head_from: false,
                head_to: true,
            },
            confidence: 1.0,
        };
        let pen = Pen {
            color: "1F5FBF".into(),
            width: 3.0,
            opacity: 1.0,
        };
        let x = shape_xml(7, &r, &pen);
        assert!(x.contains(r#"<a:xfrm flipH="1">"#), "{x}");
        assert!(x.contains(r#"<a:tailEnd type="triangle"/>"#) && !x.contains("headEnd"));
        assert!(x.contains(r#"w="38100""#), "{x}");
        let n = build::node(&x).unwrap();
        let (from, to) = line_ends(&n).unwrap();
        assert!((from.0 - 100.0).abs() < 1e-3 && (to.0 - 10.0).abs() < 1e-3);
    }
}
