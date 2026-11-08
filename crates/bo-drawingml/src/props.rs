//! Shape properties: §20.1.2.2.35 spPr, §20.1.7.6 xfrm, §20.1.8 fills/lines/effects,
//! §20.1.9 geometry, §20.1.4.2 style matrix references.

use crate::color::{find_color, is_color_element, parse_color, write_color};
use crate::dom::{attr_int, parse_bool, walk, AttrBag, El, Kids};
use bo_model::slide::{
    BlipFill, CustomGeometry, Effect, Fill, FontRef, GeomPath, Geometry, Glow, GradientFill,
    GradientShade, GradientStop, Guide, Line, LineEnd, OuterShadow, PathCommand, PatternFill,
    ShapeProps, ShapeStyle, StyleRef, Xfrm, XmlElement,
};

/// Child sequence of spPr / grpSpPr (CT_ShapeProperties, CT_GroupShapeProperties).
pub const SP_PR: &[&str] = &[
    "a:xfrm",
    "a:custGeom",
    "a:prstGeom",
    "a:noFill",
    "a:solidFill",
    "a:gradFill",
    "a:blipFill",
    "a:pattFill",
    "a:grpFill",
    "a:ln",
    "a:effectLst",
    "a:effectDag",
    "a:scene3d",
    "a:sp3d",
    "a:extLst",
];

/// Child sequence of ln (CT_LineProperties).
pub const LN: &[&str] = &[
    "a:noFill",
    "a:solidFill",
    "a:gradFill",
    "a:pattFill",
    "a:prstDash",
    "a:custDash",
    "a:round",
    "a:bevel",
    "a:miter",
    "a:headEnd",
    "a:tailEnd",
    "a:extLst",
];

const GRAD: &[&str] = &["a:gsLst", "a:lin", "a:path", "a:tileRect"];
const CUST_GEOM: &[&str] = &[
    "a:avLst",
    "a:gdLst",
    "a:ahLst",
    "a:cxnLst",
    "a:rect",
    "a:pathLst",
];

/// Is `name` a fill element (EG_FillProperties)?
pub fn is_fill(name: &str) -> bool {
    matches!(
        name,
        "a:noFill" | "a:solidFill" | "a:gradFill" | "a:blipFill" | "a:pattFill" | "a:grpFill"
    )
}

fn pair(el: &XmlElement, a: &str, b: &str) -> (i64, i64) {
    (attr_int(el, a).unwrap_or(0), attr_int(el, b).unwrap_or(0))
}

/// §20.1.7.6 xfrm.
pub fn parse_xfrm(el: &XmlElement) -> Xfrm {
    let mut x = Xfrm {
        rot: attr_int(el, "rot"),
        flip_h: el.attr("flipH").and_then(parse_bool),
        flip_v: el.attr("flipV").and_then(parse_bool),
        ..Xfrm::default()
    };
    for c in el.elements() {
        match c.local() {
            "off" => x.off = Some(pair(c, "x", "y")),
            "ext" => x.ext = Some(pair(c, "cx", "cy")),
            "chOff" => x.ch_off = Some(pair(c, "x", "y")),
            "chExt" => x.ch_ext = Some(pair(c, "cx", "cy")),
            _ => {}
        }
    }
    x
}

/// Write an xfrm with element name `name` (`a:xfrm` or `p:xfrm`).
pub fn write_xfrm(x: &Xfrm, name: &str) -> XmlElement {
    let mut e = El::new(name)
        .opt("rot", x.rot)
        .optb("flipH", x.flip_h)
        .optb("flipV", x.flip_v);
    if let Some((a, b)) = x.off {
        e = e.child(El::new("a:off").a("x", a).a("y", b).done());
    }
    if let Some((a, b)) = x.ext {
        e = e.child(El::new("a:ext").a("cx", a).a("cy", b).done());
    }
    if let Some((a, b)) = x.ch_off {
        e = e.child(El::new("a:chOff").a("x", a).a("y", b).done());
    }
    if let Some((a, b)) = x.ch_ext {
        e = e.child(El::new("a:chExt").a("cx", a).a("cy", b).done());
    }
    e.done()
}

fn parse_guides(el: Option<&XmlElement>) -> Vec<Guide> {
    el.map(|l| {
        l.elements()
            .filter(|g| g.local() == "gd")
            .map(|g| Guide {
                name: g.attr("name").unwrap_or_default().to_owned(),
                fmla: g.attr("fmla").unwrap_or_default().to_owned(),
            })
            .collect()
    })
    .unwrap_or_default()
}

fn write_guides(name: &str, gs: &[Guide]) -> XmlElement {
    let mut e = El::new(name);
    for g in gs {
        e = e.child(El::new("a:gd").a("name", &g.name).a("fmla", &g.fmla).done());
    }
    e.done()
}

fn pt(el: &XmlElement) -> (String, String) {
    (
        el.attr("x").unwrap_or("0").to_owned(),
        el.attr("y").unwrap_or("0").to_owned(),
    )
}

/// §20.1.9.8 custGeom.
pub fn parse_cust_geom(el: &XmlElement) -> CustomGeometry {
    let mut g = CustomGeometry::default();
    g.extra = walk(el, CUST_GEOM, |c| match c.name.as_str() {
        "a:avLst" => {
            g.av = parse_guides(Some(c));
            true
        }
        "a:gdLst" => {
            g.gd = parse_guides(Some(c));
            true
        }
        "a:rect" => {
            let a = |n: &str| c.attr(n).unwrap_or("0").to_owned();
            g.rect = Some([a("l"), a("t"), a("r"), a("b")]);
            true
        }
        "a:pathLst" => {
            for p in c.elements().filter(|p| p.local() == "path") {
                let mut gp = GeomPath {
                    w: attr_int(p, "w"),
                    h: attr_int(p, "h"),
                    fill: p.attr("fill").map(str::to_owned),
                    stroke: p.attr("stroke").and_then(parse_bool),
                    extrusion_ok: p.attr("extrusionOk").and_then(parse_bool),
                    commands: Vec::new(),
                };
                for cmd in p.elements() {
                    let pts: Vec<(String, String)> = cmd
                        .elements()
                        .filter(|e| e.local() == "pt")
                        .map(pt)
                        .collect();
                    let c = match cmd.local() {
                        "moveTo" if !pts.is_empty() => {
                            PathCommand::MoveTo(pts[0].0.clone(), pts[0].1.clone())
                        }
                        "lnTo" if !pts.is_empty() => {
                            PathCommand::LineTo(pts[0].0.clone(), pts[0].1.clone())
                        }
                        "arcTo" => PathCommand::ArcTo {
                            w_r: cmd.attr("wR").unwrap_or("0").to_owned(),
                            h_r: cmd.attr("hR").unwrap_or("0").to_owned(),
                            st_ang: cmd.attr("stAng").unwrap_or("0").to_owned(),
                            sw_ang: cmd.attr("swAng").unwrap_or("0").to_owned(),
                        },
                        "quadBezTo" if pts.len() >= 2 => PathCommand::QuadTo([
                            pts[0].0.clone(),
                            pts[0].1.clone(),
                            pts[1].0.clone(),
                            pts[1].1.clone(),
                        ]),
                        "cubicBezTo" if pts.len() >= 3 => PathCommand::CubicTo([
                            pts[0].0.clone(),
                            pts[0].1.clone(),
                            pts[1].0.clone(),
                            pts[1].1.clone(),
                            pts[2].0.clone(),
                            pts[2].1.clone(),
                        ]),
                        "close" => PathCommand::Close,
                        _ => continue,
                    };
                    gp.commands.push(c);
                }
                g.paths.push(gp);
            }
            true
        }
        _ => false,
    });
    g
}

fn pt_el(x: &str, y: &str) -> XmlElement {
    El::new("a:pt").a("x", x).a("y", y).done()
}

/// Write custGeom.
pub fn write_cust_geom(g: &CustomGeometry) -> XmlElement {
    let mut k = Kids::new(CUST_GEOM);
    k.push(write_guides("a:avLst", &g.av));
    k.push(write_guides("a:gdLst", &g.gd));
    if let Some([l, t, r, b]) = &g.rect {
        k.push(
            El::new("a:rect")
                .a("l", l)
                .a("t", t)
                .a("r", r)
                .a("b", b)
                .done(),
        );
    }
    let mut pl = El::new("a:pathLst");
    for p in &g.paths {
        let mut pe = El::new("a:path")
            .opt("w", p.w)
            .opt("h", p.h)
            .opt("fill", p.fill.as_ref())
            .optb("stroke", p.stroke)
            .optb("extrusionOk", p.extrusion_ok);
        for c in &p.commands {
            let ce = match c {
                PathCommand::MoveTo(x, y) => El::new("a:moveTo").child(pt_el(x, y)),
                PathCommand::LineTo(x, y) => El::new("a:lnTo").child(pt_el(x, y)),
                PathCommand::ArcTo {
                    w_r,
                    h_r,
                    st_ang,
                    sw_ang,
                } => El::new("a:arcTo")
                    .a("wR", w_r)
                    .a("hR", h_r)
                    .a("stAng", st_ang)
                    .a("swAng", sw_ang),
                PathCommand::QuadTo(p) => El::new("a:quadBezTo")
                    .child(pt_el(&p[0], &p[1]))
                    .child(pt_el(&p[2], &p[3])),
                PathCommand::CubicTo(p) => El::new("a:cubicBezTo")
                    .child(pt_el(&p[0], &p[1]))
                    .child(pt_el(&p[2], &p[3]))
                    .child(pt_el(&p[4], &p[5])),
                PathCommand::Close => El::new("a:close"),
            };
            pe = pe.child(ce.done());
        }
        pl = pl.child(pe.done());
    }
    k.push(pl.done());
    k.extras(&g.extra);
    k.build(El::new("a:custGeom"))
}

/// Parse a geometry element (prstGeom / custGeom).
pub fn parse_geometry(el: &XmlElement) -> Option<Geometry> {
    match el.name.as_str() {
        // §20.1.9.18 prstGeom
        "a:prstGeom" => Some(Geometry::Preset {
            name: el.attr("prst").unwrap_or("rect").to_owned(),
            av: parse_guides(el.child("a:avLst")),
        }),
        "a:custGeom" => Some(Geometry::Custom(parse_cust_geom(el))),
        _ => None,
    }
}

/// Write a geometry element.
pub fn write_geometry(g: &Geometry) -> XmlElement {
    match g {
        Geometry::Preset { name, av } => El::new("a:prstGeom")
            .a("prst", name)
            .child(write_guides("a:avLst", av))
            .done(),
        Geometry::Custom(c) => write_cust_geom(c),
    }
}

/// §20.1.8.14 blipFill (also `p:blipFill` in pictures).
pub fn parse_blip_fill(el: &XmlElement) -> BlipFill {
    let mut b = BlipFill {
        dpi: el.attr("dpi").map(str::to_owned),
        rot_with_shape: el.attr("rotWithShape").and_then(parse_bool),
        ..BlipFill::default()
    };
    for c in el.elements() {
        match c.local() {
            "blip" => {
                let mut bag = AttrBag::of(c);
                b.embed = bag.take("r:embed");
                b.link = bag.take("r:link");
                b.blip_attrs = bag.rest();
                b.blip_children = c.elements().cloned().collect();
            }
            "srcRect" => {
                let v = |n| attr_int(c, n).unwrap_or(0);
                b.src_rect = Some([v("l"), v("t"), v("r"), v("b")]);
            }
            "stretch" => b.stretch = true,
            "tile" => b.tile = Some(c.clone()),
            _ => {}
        }
    }
    b
}

/// Write blipFill with element name `name` and namespace prefix `ns` for children.
pub fn write_blip_fill(b: &BlipFill, name: &str) -> XmlElement {
    let mut blip = El::new("a:blip")
        .opt("r:embed", b.embed.as_ref())
        .opt("r:link", b.link.as_ref())
        .rest(&b.blip_attrs);
    for c in &b.blip_children {
        blip = blip.child(c.clone());
    }
    let mut e = El::new(name)
        .opt("dpi", b.dpi.as_ref())
        .optb("rotWithShape", b.rot_with_shape)
        .child(blip.done());
    if let Some([l, t, r, bb]) = b.src_rect {
        let mut s = El::new("a:srcRect");
        for (k, v) in [("l", l), ("t", t), ("r", r), ("b", bb)] {
            if v != 0 {
                s = s.a(k, v);
            }
        }
        e = e.child(s.done());
    }
    if let Some(t) = &b.tile {
        e = e.child(t.clone());
    }
    if b.stretch {
        e = e.child(
            El::new("a:stretch")
                .child(El::new("a:fillRect").done())
                .done(),
        );
    }
    e.done()
}

/// Parse a fill element (EG_FillProperties, §20.1.8).
pub fn parse_fill(el: &XmlElement) -> Option<Fill> {
    Some(match el.name.as_str() {
        // §20.1.8.44 noFill
        "a:noFill" => Fill::None,
        // §20.1.8.54 solidFill
        "a:solidFill" => match find_color(el) {
            Some(c) => Fill::Solid(c),
            None => Fill::None,
        },
        // §20.1.8.33 gradFill
        "a:gradFill" => {
            let mut g = GradientFill {
                rot_with_shape: el.attr("rotWithShape").and_then(parse_bool),
                flip: el.attr("flip").map(str::to_owned),
                ..GradientFill::default()
            };
            g.extra = walk(el, GRAD, |c| match c.name.as_str() {
                "a:gsLst" => {
                    for gs in c.elements() {
                        if let Some(color) = find_color(gs) {
                            g.stops.push(GradientStop {
                                pos: attr_int(gs, "pos").unwrap_or(0),
                                color,
                            });
                        }
                    }
                    true
                }
                // §20.1.8.41 lin
                "a:lin" => {
                    g.shade = Some(GradientShade::Linear {
                        ang: attr_int(c, "ang").unwrap_or(0),
                        scaled: c.attr("scaled").and_then(parse_bool),
                    });
                    true
                }
                // §20.1.8.46 path
                "a:path" => {
                    g.shade = Some(GradientShade::Path {
                        path: c.attr("path").unwrap_or("circle").to_owned(),
                        fill_to: c.child("a:fillToRect").map(|r| {
                            let v = |n| attr_int(r, n).unwrap_or(0);
                            [v("l"), v("t"), v("r"), v("b")]
                        }),
                    });
                    true
                }
                _ => false,
            });
            Fill::Gradient(g)
        }
        // §20.1.8.47 pattFill
        "a:pattFill" => Fill::Pattern(PatternFill {
            preset: el.attr("prst").map(str::to_owned),
            fg: el.child("a:fgClr").and_then(find_color),
            bg: el.child("a:bgClr").and_then(find_color),
        }),
        "a:blipFill" => Fill::Picture(parse_blip_fill(el)),
        // §20.1.8.35 grpFill
        "a:grpFill" => Fill::Group,
        _ => return None,
    })
}

/// Write a fill element.
pub fn write_fill(f: &Fill) -> XmlElement {
    match f {
        Fill::None => El::new("a:noFill").done(),
        Fill::Solid(c) => El::new("a:solidFill").child(write_color(c)).done(),
        Fill::Gradient(g) => {
            let mut k = Kids::new(GRAD);
            let mut gs = El::new("a:gsLst");
            for s in &g.stops {
                gs = gs.child(
                    El::new("a:gs")
                        .a("pos", s.pos)
                        .child(write_color(&s.color))
                        .done(),
                );
            }
            k.push(gs.done());
            match &g.shade {
                Some(GradientShade::Linear { ang, scaled }) => {
                    k.push(
                        El::new("a:lin")
                            .a("ang", ang)
                            .optb("scaled", *scaled)
                            .done(),
                    );
                }
                Some(GradientShade::Path { path, fill_to }) => {
                    let mut p = El::new("a:path").a("path", path);
                    if let Some([l, t, r, b]) = fill_to {
                        let mut ft = El::new("a:fillToRect");
                        for (k, v) in [("l", l), ("t", t), ("r", r), ("b", b)] {
                            if *v != 0 {
                                ft = ft.a(k, v);
                            }
                        }
                        p = p.child(ft.done());
                    }
                    k.push(p.done());
                }
                None => {}
            }
            k.extras(&g.extra);
            k.build(
                El::new("a:gradFill")
                    .opt("flip", g.flip.as_ref())
                    .optb("rotWithShape", g.rot_with_shape),
            )
        }
        Fill::Pattern(p) => {
            let mut e = El::new("a:pattFill").opt("prst", p.preset.as_ref());
            if let Some(c) = &p.fg {
                e = e.child(El::new("a:fgClr").child(write_color(c)).done());
            }
            if let Some(c) = &p.bg {
                e = e.child(El::new("a:bgClr").child(write_color(c)).done());
            }
            e.done()
        }
        Fill::Picture(b) => write_blip_fill(b, "a:blipFill"),
        Fill::Group => El::new("a:grpFill").done(),
    }
}

fn parse_line_end(el: &XmlElement) -> LineEnd {
    LineEnd {
        kind: el.attr("type").map(str::to_owned),
        w: el.attr("w").map(str::to_owned),
        len: el.attr("len").map(str::to_owned),
    }
}

fn write_line_end(name: &str, e: &LineEnd) -> XmlElement {
    El::new(name)
        .opt("type", e.kind.as_ref())
        .opt("w", e.w.as_ref())
        .opt("len", e.len.as_ref())
        .done()
}

/// §20.1.2.2.24 ln (also lnL/lnR/lnT/lnB in tables).
pub fn parse_line(el: &XmlElement) -> Line {
    let mut l = Line {
        w: attr_int(el, "w"),
        cap: el.attr("cap").map(str::to_owned),
        cmpd: el.attr("cmpd").map(str::to_owned),
        algn: el.attr("algn").map(str::to_owned),
        ..Line::default()
    };
    l.extra = walk(el, LN, |c| {
        if is_fill(&c.name) {
            l.fill = parse_fill(c);
            return true;
        }
        match c.name.as_str() {
            // §20.1.8.48 prstDash
            "a:prstDash" => {
                l.dash = c.attr("val").map(str::to_owned);
                true
            }
            "a:round" | "a:bevel" if c.attrs.is_empty() => {
                l.join = Some(c.local().to_owned());
                true
            }
            "a:miter" if c.attrs.is_empty() => {
                l.join = Some("miter".into());
                true
            }
            "a:headEnd" => {
                l.head = Some(parse_line_end(c));
                true
            }
            "a:tailEnd" => {
                l.tail = Some(parse_line_end(c));
                true
            }
            _ => false,
        }
    });
    l
}

/// Write a line with element name `name`.
pub fn write_line(l: &Line, name: &str) -> XmlElement {
    let mut k = Kids::new(LN);
    k.push_opt(l.fill.as_ref().map(write_fill));
    k.push_opt(
        l.dash
            .as_ref()
            .map(|d| El::new("a:prstDash").a("val", d).done()),
    );
    k.push_opt(l.join.as_ref().map(|j| El::new(&format!("a:{j}")).done()));
    k.push_opt(l.head.as_ref().map(|e| write_line_end("a:headEnd", e)));
    k.push_opt(l.tail.as_ref().map(|e| write_line_end("a:tailEnd", e)));
    k.extras(&l.extra);
    k.build(
        El::new(name)
            .opt("w", l.w)
            .opt("cap", l.cap.as_ref())
            .opt("cmpd", l.cmpd.as_ref())
            .opt("algn", l.algn.as_ref()),
    )
}

/// §20.1.8.26 effectLst.
pub fn parse_effects(el: &XmlElement) -> Vec<Effect> {
    el.elements()
        .map(|c| {
            if c.name == "a:outerShdw" && c.elements().all(|e| is_color_element(&e.name)) {
                let mut bag = AttrBag::of(c);
                Effect::OuterShadow(OuterShadow {
                    blur_rad: bag.int("blurRad"),
                    dist: bag.int("dist"),
                    dir: bag.int("dir"),
                    color: c.elements().next().and_then(parse_color),
                    attrs: bag.rest(),
                })
            } else if c.name == "a:glow" && c.elements().all(|e| is_color_element(&e.name)) {
                // §20.1.8.32 glow
                let mut bag = AttrBag::of(c);
                Effect::Glow(Glow {
                    rad: bag.int("rad"),
                    color: c.elements().next().and_then(parse_color),
                    attrs: bag.rest(),
                })
            } else {
                Effect::Other(c.clone())
            }
        })
        .collect()
}

/// Write effectLst.
pub fn write_effects(effects: &[Effect]) -> XmlElement {
    let mut e = El::new("a:effectLst");
    for x in effects {
        e = e.child(match x {
            Effect::OuterShadow(s) => {
                let mut o = El::new("a:outerShdw")
                    .opt("blurRad", s.blur_rad)
                    .opt("dist", s.dist)
                    .opt("dir", s.dir)
                    .rest(&s.attrs);
                if let Some(c) = &s.color {
                    o = o.child(write_color(c));
                }
                o.done()
            }
            Effect::Glow(g) => {
                let mut o = El::new("a:glow").opt("rad", g.rad).rest(&g.attrs);
                if let Some(c) = &g.color {
                    o = o.child(write_color(c));
                }
                o.done()
            }
            Effect::Other(x) => x.clone(),
        });
    }
    e.done()
}

/// §20.1.2.2.35 spPr / §20.1.2.2.22 grpSpPr.
pub fn parse_sp_pr(el: &XmlElement) -> ShapeProps {
    let mut p = ShapeProps {
        bw_mode: el.attr("bwMode").map(str::to_owned),
        ..ShapeProps::default()
    };
    p.extra = walk(el, SP_PR, |c| {
        if is_fill(&c.name) {
            p.fill = parse_fill(c);
            return true;
        }
        match c.name.as_str() {
            "a:xfrm" => {
                p.xfrm = Some(parse_xfrm(c));
                true
            }
            "a:prstGeom" | "a:custGeom" => {
                p.geometry = parse_geometry(c);
                true
            }
            "a:ln" => {
                p.line = Some(parse_line(c));
                true
            }
            "a:effectLst" => {
                p.effects = Some(parse_effects(c));
                true
            }
            _ => false,
        }
    });
    p
}

/// Write spPr / grpSpPr with element name `name`.
pub fn write_sp_pr(p: &ShapeProps, name: &str) -> XmlElement {
    let mut k = Kids::new(SP_PR);
    k.push_opt(p.xfrm.as_ref().map(|x| write_xfrm(x, "a:xfrm")));
    k.push_opt(p.geometry.as_ref().map(write_geometry));
    k.push_opt(p.fill.as_ref().map(write_fill));
    k.push_opt(p.line.as_ref().map(|l| write_line(l, "a:ln")));
    k.push_opt(p.effects.as_deref().map(write_effects));
    k.extras(&p.extra);
    k.build(El::new(name).opt("bwMode", p.bw_mode.as_ref()))
}

fn style_ref(el: Option<&XmlElement>) -> StyleRef {
    StyleRef {
        idx: el.and_then(|e| attr_int(e, "idx")).unwrap_or(0),
        color: el.and_then(find_color),
    }
}

/// §19.3.1.46 / §20.1.2.2.37 style.
pub fn parse_style(el: &XmlElement) -> ShapeStyle {
    let fr = el.child("a:fontRef");
    ShapeStyle {
        ln_ref: style_ref(el.child("a:lnRef")),
        fill_ref: style_ref(el.child("a:fillRef")),
        effect_ref: style_ref(el.child("a:effectRef")),
        font_ref: FontRef {
            idx: fr.and_then(|f| f.attr("idx")).unwrap_or("minor").to_owned(),
            color: fr.and_then(find_color),
        },
    }
}

/// Write style with element name `name` (`p:style`).
pub fn write_style(s: &ShapeStyle, name: &str) -> XmlElement {
    let r = |n: &str, r: &StyleRef| {
        let mut e = El::new(n).a("idx", r.idx);
        if let Some(c) = &r.color {
            e = e.child(write_color(c));
        }
        e.done()
    };
    let mut f = El::new("a:fontRef").a("idx", &s.font_ref.idx);
    if let Some(c) = &s.font_ref.color {
        f = f.child(write_color(c));
    }
    El::new(name)
        .child(r("a:lnRef", &s.ln_ref))
        .child(r("a:fillRef", &s.fill_ref))
        .child(r("a:effectRef", &s.effect_ref))
        .child(f.done())
        .done()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::parse;

    #[test]
    fn sp_pr_roundtrip_keeps_unknown_in_place() {
        let xml = br#"<p:spPr><a:xfrm rot="5400000" flipH="1"><a:off x="1" y="2"/><a:ext cx="3" cy="4"/></a:xfrm><a:prstGeom prst="roundRect"><a:avLst><a:gd name="adj" fmla="val 10000"/></a:avLst></a:prstGeom><a:gradFill rotWithShape="1"><a:gsLst><a:gs pos="0"><a:schemeClr val="accent1"/></a:gs><a:gs pos="100000"><a:srgbClr val="FF0000"/></a:gs></a:gsLst><a:lin ang="5400000" scaled="0"/></a:gradFill><a:ln w="12700"><a:solidFill><a:srgbClr val="000000"/></a:solidFill><a:prstDash val="dash"/><a:round/><a:tailEnd type="triangle"/></a:ln><a:effectLst><a:outerShdw blurRad="40000" dist="20000" dir="5400000" rotWithShape="0"><a:srgbClr val="000000"><a:alpha val="38000"/></a:srgbClr></a:outerShdw></a:effectLst><a:scene3d><a:camera prst="orthographicFront"/></a:scene3d><a:extLst><a:ext uri="x"/></a:extLst></p:spPr>"#;
        let el = parse(xml, "/t").unwrap();
        let p = parse_sp_pr(&el);
        assert_eq!(p.xfrm.as_ref().unwrap().rot, Some(5_400_000));
        assert!(matches!(p.fill, Some(Fill::Gradient(_))));
        assert_eq!(p.line.as_ref().unwrap().dash.as_deref(), Some("dash"));
        assert!(matches!(
            p.effects.as_deref(),
            Some([Effect::OuterShadow(_)])
        ));
        assert_eq!(p.extra.len(), 2);
        let back = write_sp_pr(&p, "p:spPr");
        assert_eq!(crate::dom::canonical(&back), crate::dom::canonical(&el));
    }

    #[test]
    fn cust_geom_roundtrip() {
        let xml = br#"<a:custGeom><a:avLst/><a:gdLst><a:gd name="g" fmla="*/ w 1 2"/></a:gdLst><a:ahLst/><a:cxnLst/><a:rect l="l" t="t" r="r" b="b"/><a:pathLst><a:path w="100" h="100"><a:moveTo><a:pt x="0" y="0"/></a:moveTo><a:lnTo><a:pt x="100" y="0"/></a:lnTo><a:arcTo wR="50" hR="50" stAng="0" swAng="5400000"/><a:quadBezTo><a:pt x="1" y="2"/><a:pt x="3" y="4"/></a:quadBezTo><a:cubicBezTo><a:pt x="1" y="2"/><a:pt x="3" y="4"/><a:pt x="5" y="6"/></a:cubicBezTo><a:close/></a:path></a:pathLst></a:custGeom>"#;
        let el = parse(xml, "/t").unwrap();
        let g = parse_cust_geom(&el);
        assert_eq!(g.paths[0].commands.len(), 6);
        assert_eq!(g.extra.len(), 2);
        assert_eq!(
            crate::dom::canonical(&write_cust_geom(&g)),
            crate::dom::canonical(&el)
        );
    }
}
