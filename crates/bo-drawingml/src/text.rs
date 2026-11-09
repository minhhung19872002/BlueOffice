//! DrawingML text — ECMA-376 Part 1, §21.1.2 (txBody, bodyPr, lstStyle, p, pPr, r, rPr).

use crate::color::{find_color, write_color};
use crate::dom::{attr_int, walk, AttrBag, El, Kids};
use crate::props::{is_fill, parse_fill, parse_line, write_fill, write_line};
use bo_model::slide::{
    Autofit, BodyProps, Bullet, ListStyle, ParaProps, Paragraph, RunProps, Spacing, TabStop,
    TextBody, TextFont, TextRun, XmlElement,
};

const TX_BODY: &[&str] = &["a:bodyPr", "a:lstStyle", "a:p"];
/// Child sequence of bodyPr (CT_TextBodyProperties).
pub const BODY_PR: &[&str] = &[
    "a:prstTxWarp",
    "a:noAutofit",
    "a:normAutofit",
    "a:spAutoFit",
    "a:scene3d",
    "a:sp3d",
    "a:flatTx",
    "a:extLst",
];
/// Child sequence of lstStyle (CT_TextListStyle).
pub const LST_STYLE: &[&str] = &[
    "a:defPPr",
    "a:lvl1pPr",
    "a:lvl2pPr",
    "a:lvl3pPr",
    "a:lvl4pPr",
    "a:lvl5pPr",
    "a:lvl6pPr",
    "a:lvl7pPr",
    "a:lvl8pPr",
    "a:lvl9pPr",
    "a:extLst",
];
const PPR: &[&str] = &[
    "a:lnSpc",
    "a:spcBef",
    "a:spcAft",
    "a:buClrTx",
    "a:buClr",
    "a:buSzTx",
    "a:buSzPct",
    "a:buSzPts",
    "a:buFontTx",
    "a:buFont",
    "a:buNone",
    "a:buAutoNum",
    "a:buChar",
    "a:buBlip",
    "a:tabLst",
    "a:defRPr",
    "a:extLst",
];
const RPR: &[&str] = &[
    "a:ln",
    "a:noFill",
    "a:solidFill",
    "a:gradFill",
    "a:blipFill",
    "a:pattFill",
    "a:grpFill",
    "a:effectLst",
    "a:effectDag",
    "a:highlight",
    "a:uLnTx",
    "a:uLn",
    "a:uFillTx",
    "a:uFill",
    "a:latin",
    "a:ea",
    "a:cs",
    "a:sym",
    "a:hlinkClick",
    "a:hlinkMouseOver",
    "a:rtl",
    "a:extLst",
];

/// §21.1.2.3.7 latin (and ea, cs, sym, buFont).
pub fn parse_font(el: &XmlElement) -> TextFont {
    let mut bag = AttrBag::of(el);
    TextFont {
        typeface: bag.take("typeface").unwrap_or_default(),
        attrs: bag.rest(),
    }
}

/// Write a text font element.
pub fn write_font(name: &str, f: &TextFont) -> XmlElement {
    El::new(name)
        .a("typeface", &f.typeface)
        .rest(&f.attrs)
        .done()
}

/// §21.1.2.3.9 rPr (also defRPr, endParaRPr).
pub fn parse_rpr(el: &XmlElement) -> RunProps {
    let mut bag = AttrBag::of(el);
    let mut p = RunProps {
        lang: bag.take("lang"),
        sz: bag.int("sz"),
        b: bag.bool("b"),
        i: bag.bool("i"),
        u: bag.take("u"),
        strike: bag.take("strike"),
        baseline: bag.int("baseline"),
        spc: bag.int("spc"),
        cap: bag.take("cap"),
        ..RunProps::default()
    };
    p.attrs = bag.rest();
    p.extra = walk(el, RPR, |c| {
        if is_fill(&c.name) {
            p.fill = parse_fill(c);
            return true;
        }
        match c.name.as_str() {
            "a:ln" => p.line = Some(parse_line(c)),
            "a:highlight" => match find_color(c) {
                Some(col) => p.highlight = Some(col),
                None => return false,
            },
            "a:latin" => p.latin = Some(parse_font(c)),
            "a:ea" => p.ea = Some(parse_font(c)),
            "a:cs" => p.cs = Some(parse_font(c)),
            "a:sym" => p.sym = Some(parse_font(c)),
            _ => return false,
        }
        true
    });
    p
}

/// Write run properties with element name `name`.
pub fn write_rpr(p: &RunProps, name: &str) -> XmlElement {
    let mut k = Kids::new(RPR);
    k.push_opt(p.line.as_ref().map(|l| write_line(l, "a:ln")));
    k.push_opt(p.fill.as_ref().map(write_fill));
    k.push_opt(
        p.highlight
            .as_ref()
            .map(|c| El::new("a:highlight").child(write_color(c)).done()),
    );
    k.push_opt(p.latin.as_ref().map(|f| write_font("a:latin", f)));
    k.push_opt(p.ea.as_ref().map(|f| write_font("a:ea", f)));
    k.push_opt(p.cs.as_ref().map(|f| write_font("a:cs", f)));
    k.push_opt(p.sym.as_ref().map(|f| write_font("a:sym", f)));
    k.extras(&p.extra);
    k.build(
        El::new(name)
            .opt("lang", p.lang.as_ref())
            .opt("sz", p.sz)
            .optb("b", p.b)
            .optb("i", p.i)
            .opt("u", p.u.as_ref())
            .opt("strike", p.strike.as_ref())
            .opt("baseline", p.baseline)
            .opt("spc", p.spc)
            .opt("cap", p.cap.as_ref())
            .rest(&p.attrs),
    )
}

fn parse_spacing(el: &XmlElement) -> Option<Spacing> {
    if let Some(p) = el.child("a:spcPct") {
        return attr_int(p, "val").map(Spacing::Percent);
    }
    el.child("a:spcPts")
        .and_then(|p| attr_int(p, "val"))
        .map(Spacing::Points)
}

fn write_spacing(name: &str, s: Spacing) -> XmlElement {
    let inner = match s {
        Spacing::Percent(v) => El::new("a:spcPct").a("val", v),
        Spacing::Points(v) => El::new("a:spcPts").a("val", v),
    };
    El::new(name).child(inner.done()).done()
}

/// §21.1.2.2.7 pPr (and lvlNpPr, defPPr).
pub fn parse_ppr(el: &XmlElement) -> ParaProps {
    let mut bag = AttrBag::of(el);
    let mut p = ParaProps {
        lvl: bag.int("lvl"),
        mar_l: bag.int("marL"),
        mar_r: bag.int("marR"),
        indent: bag.int("indent"),
        algn: bag.take("algn"),
        def_tab_sz: bag.int("defTabSz"),
        rtl: bag.bool("rtl"),
        ..ParaProps::default()
    };
    p.attrs = bag.rest();
    p.extra = walk(el, PPR, |c| {
        match c.name.as_str() {
            // §21.1.2.2.5 lnSpc, §21.1.2.2.10 spcBef, §21.1.2.2.9 spcAft
            "a:lnSpc" | "a:spcBef" | "a:spcAft" => {
                let Some(s) = parse_spacing(c) else {
                    return false;
                };
                match c.local() {
                    "lnSpc" => p.ln_spc = Some(s),
                    "spcBef" => p.spc_bef = Some(s),
                    _ => p.spc_aft = Some(s),
                }
            }
            // §21.1.2.4.5 buClrTx / §21.1.2.4.4 buClr
            "a:buClrTx" => p.bu_clr = Some(None),
            "a:buClr" => match find_color(c) {
                Some(col) => p.bu_clr = Some(Some(col)),
                None => return false,
            },
            // §21.1.2.4.11-13 buSzTx / buSzPct / buSzPts
            "a:buSzTx" => p.bu_sz = Some(None),
            "a:buSzPct" => p.bu_sz = Some(attr_int(c, "val").map(Spacing::Percent)),
            "a:buSzPts" => p.bu_sz = Some(attr_int(c, "val").map(Spacing::Points)),
            // §21.1.2.4.7 buFontTx / §21.1.2.4.6 buFont
            "a:buFontTx" => p.bu_font = Some(None),
            "a:buFont" => p.bu_font = Some(Some(parse_font(c))),
            "a:buNone" => p.bullet = Some(Bullet::None),
            "a:buChar" => p.bullet = Some(Bullet::Char(c.attr("char").unwrap_or("").to_owned())),
            "a:buAutoNum" => {
                p.bullet = Some(Bullet::AutoNum {
                    scheme: c.attr("type").unwrap_or("arabicPeriod").to_owned(),
                    start_at: attr_int(c, "startAt"),
                });
            }
            "a:buBlip" => p.bullet = Some(Bullet::Blip(c.clone())),
            // §21.1.2.2.14 tabLst
            "a:tabLst" => {
                p.tabs = Some(
                    c.elements()
                        .map(|t| TabStop {
                            pos: attr_int(t, "pos"),
                            algn: t.attr("algn").map(str::to_owned),
                        })
                        .collect(),
                );
            }
            "a:defRPr" => p.def_rpr = Some(parse_rpr(c)),
            _ => return false,
        }
        true
    });
    p
}

/// Write paragraph properties with element name `name`.
pub fn write_ppr(p: &ParaProps, name: &str) -> XmlElement {
    let mut k = Kids::new(PPR);
    if let Some(s) = p.ln_spc {
        k.push(write_spacing("a:lnSpc", s));
    }
    if let Some(s) = p.spc_bef {
        k.push(write_spacing("a:spcBef", s));
    }
    if let Some(s) = p.spc_aft {
        k.push(write_spacing("a:spcAft", s));
    }
    match &p.bu_clr {
        Some(None) => k.push(El::new("a:buClrTx").done()),
        Some(Some(c)) => k.push(El::new("a:buClr").child(write_color(c)).done()),
        None => {}
    }
    match p.bu_sz {
        Some(None) => k.push(El::new("a:buSzTx").done()),
        Some(Some(Spacing::Percent(v))) => k.push(El::new("a:buSzPct").a("val", v).done()),
        Some(Some(Spacing::Points(v))) => k.push(El::new("a:buSzPts").a("val", v).done()),
        None => {}
    }
    match &p.bu_font {
        Some(None) => k.push(El::new("a:buFontTx").done()),
        Some(Some(f)) => k.push(write_font("a:buFont", f)),
        None => {}
    }
    match &p.bullet {
        Some(Bullet::None) => k.push(El::new("a:buNone").done()),
        Some(Bullet::Char(c)) => k.push(El::new("a:buChar").a("char", c).done()),
        Some(Bullet::AutoNum { scheme, start_at }) => k.push(
            El::new("a:buAutoNum")
                .a("type", scheme)
                .opt("startAt", *start_at)
                .done(),
        ),
        Some(Bullet::Blip(x)) => k.push(x.clone()),
        None => {}
    }
    if let Some(tabs) = &p.tabs {
        let mut t = El::new("a:tabLst");
        for s in tabs {
            t = t.child(
                El::new("a:tab")
                    .opt("pos", s.pos)
                    .opt("algn", s.algn.as_ref())
                    .done(),
            );
        }
        k.push(t.done());
    }
    k.push_opt(p.def_rpr.as_ref().map(|r| write_rpr(r, "a:defRPr")));
    k.extras(&p.extra);
    k.build(
        El::new(name)
            .opt("marL", p.mar_l)
            .opt("marR", p.mar_r)
            .opt("lvl", p.lvl)
            .opt("indent", p.indent)
            .opt("algn", p.algn.as_ref())
            .opt("defTabSz", p.def_tab_sz)
            .optb("rtl", p.rtl)
            .rest(&p.attrs),
    )
}

/// §21.1.2.4.12 lstStyle (also master `titleStyle`/`bodyStyle`/`otherStyle`, `defaultTextStyle`).
pub fn parse_list_style(el: &XmlElement) -> ListStyle {
    let mut s = ListStyle::default();
    s.extra = walk(el, LST_STYLE, |c| {
        let local = c.local();
        if local == "defPPr" {
            s.def = Some(parse_ppr(c));
            return true;
        }
        if c.name.starts_with("a:lvl") && local.ends_with("pPr") {
            if let Ok(n) = local[3..local.len() - 3].parse::<usize>() {
                if (1..=9).contains(&n) {
                    s.levels[n - 1] = Some(parse_ppr(c));
                    return true;
                }
            }
        }
        false
    });
    s
}

/// Write a list style with element name `name`.
pub fn write_list_style(s: &ListStyle, name: &str) -> XmlElement {
    let mut k = Kids::new(LST_STYLE);
    k.push_opt(s.def.as_ref().map(|p| write_ppr(p, "a:defPPr")));
    for (i, l) in s.levels.iter().enumerate() {
        k.push_opt(
            l.as_ref()
                .map(|p| write_ppr(p, &format!("a:lvl{}pPr", i + 1))),
        );
    }
    k.extras(&s.extra);
    k.build(El::new(name))
}

/// §21.1.2.1.1 bodyPr.
pub fn parse_body_pr(el: &XmlElement) -> BodyProps {
    let mut bag = AttrBag::of(el);
    let mut b = BodyProps {
        l_ins: bag.int("lIns"),
        t_ins: bag.int("tIns"),
        r_ins: bag.int("rIns"),
        b_ins: bag.int("bIns"),
        anchor: bag.take("anchor"),
        anchor_ctr: bag.bool("anchorCtr"),
        wrap: bag.take("wrap"),
        num_col: bag.int("numCol"),
        spc_col: bag.int("spcCol"),
        rot: bag.int("rot"),
        vert: bag.take("vert"),
        ..BodyProps::default()
    };
    b.attrs = bag.rest();
    b.extra = walk(el, BODY_PR, |c| {
        b.autofit = Some(match c.name.as_str() {
            // §21.1.2.1.2 noAutofit
            "a:noAutofit" => Autofit::None,
            // §21.1.2.1.3 normAutofit
            "a:normAutofit" => Autofit::Normal {
                font_scale: attr_int(c, "fontScale"),
                ln_spc_reduction: attr_int(c, "lnSpcReduction"),
            },
            // §21.1.2.1.4 spAutoFit
            "a:spAutoFit" => Autofit::Shape,
            _ => return false,
        });
        true
    });
    b
}

/// Write bodyPr.
pub fn write_body_pr(b: &BodyProps) -> XmlElement {
    let mut k = Kids::new(BODY_PR);
    match &b.autofit {
        Some(Autofit::None) => k.push(El::new("a:noAutofit").done()),
        Some(Autofit::Normal {
            font_scale,
            ln_spc_reduction,
        }) => k.push(
            El::new("a:normAutofit")
                .opt("fontScale", *font_scale)
                .opt("lnSpcReduction", *ln_spc_reduction)
                .done(),
        ),
        Some(Autofit::Shape) => k.push(El::new("a:spAutoFit").done()),
        None => {}
    }
    k.extras(&b.extra);
    k.build(
        El::new("a:bodyPr")
            .opt("rot", b.rot)
            .opt("vert", b.vert.as_ref())
            .opt("wrap", b.wrap.as_ref())
            .opt("lIns", b.l_ins)
            .opt("tIns", b.t_ins)
            .opt("rIns", b.r_ins)
            .opt("bIns", b.b_ins)
            .opt("numCol", b.num_col)
            .opt("spcCol", b.spc_col)
            .opt("anchor", b.anchor.as_ref())
            .optb("anchorCtr", b.anchor_ctr)
            .rest(&b.attrs),
    )
}

fn text_of(el: &XmlElement) -> String {
    el.child("a:t").map(XmlElement::text).unwrap_or_default()
}

fn t_el(s: &str) -> XmlElement {
    El::new("a:t").text(s).done()
}

/// §21.1.2.2.6 p.
pub fn parse_paragraph(el: &XmlElement) -> Paragraph {
    let mut p = Paragraph::default();
    for c in el.elements() {
        match c.name.as_str() {
            "a:pPr" => p.props = Some(parse_ppr(c)),
            "a:endParaRPr" => p.end_props = Some(parse_rpr(c)),
            "a:r" if c.elements().all(|e| e.name == "a:rPr" || e.name == "a:t") => {
                p.runs.push(TextRun::Run {
                    props: c.child("a:rPr").map(parse_rpr).unwrap_or_default(),
                    text: text_of(c),
                });
            }
            "a:br" if c.elements().all(|e| e.name == "a:rPr") => p.runs.push(TextRun::Break {
                props: c.child("a:rPr").map(parse_rpr).unwrap_or_default(),
            }),
            "a:fld"
                if c.elements()
                    .all(|e| matches!(e.name.as_str(), "a:rPr" | "a:pPr" | "a:t"))
                    && c.attrs.iter().all(|(k, _)| k == "id" || k == "type") =>
            {
                p.runs.push(TextRun::Field {
                    id: c.attr("id").unwrap_or_default().to_owned(),
                    kind: c.attr("type").map(str::to_owned),
                    props: c.child("a:rPr").map(parse_rpr).unwrap_or_default(),
                    ppr: c.child("a:pPr").map(|p| Box::new(parse_ppr(p))),
                    text: text_of(c),
                });
            }
            _ => p.runs.push(TextRun::Other(c.clone())),
        }
    }
    p
}

fn rpr_or_none(p: &bo_model::slide::RunProps) -> Option<XmlElement> {
    if *p == RunProps::default() {
        None
    } else {
        Some(write_rpr(p, "a:rPr"))
    }
}

/// Write a paragraph.
pub fn write_paragraph(p: &Paragraph) -> XmlElement {
    let mut e = El::new("a:p");
    if let Some(pp) = &p.props {
        e = e.child(write_ppr(pp, "a:pPr"));
    }
    for r in &p.runs {
        e = e.child(match r {
            TextRun::Run { props, text } => {
                let mut r = El::new("a:r");
                if let Some(x) = rpr_or_none(props) {
                    r = r.child(x);
                }
                r.child(t_el(text)).done()
            }
            TextRun::Break { props } => {
                let mut r = El::new("a:br");
                if let Some(x) = rpr_or_none(props) {
                    r = r.child(x);
                }
                r.done()
            }
            TextRun::Field {
                id,
                kind,
                props,
                ppr,
                text,
            } => {
                let mut r = El::new("a:fld").a("id", id).opt("type", kind.as_ref());
                if let Some(x) = rpr_or_none(props) {
                    r = r.child(x);
                }
                if let Some(pp) = ppr {
                    r = r.child(write_ppr(pp, "a:pPr"));
                }
                r.child(t_el(text)).done()
            }
            TextRun::Other(x) => x.clone(),
        });
    }
    if let Some(ep) = &p.end_props {
        e = e.child(write_rpr(ep, "a:endParaRPr"));
    }
    e.done()
}

/// §21.1.2.1.2 txBody (`p:txBody`, `a:txBody` in table cells).
pub fn parse_text_body(el: &XmlElement) -> TextBody {
    let mut t = TextBody::default();
    let _ = walk(el, TX_BODY, |c| {
        match c.name.as_str() {
            "a:bodyPr" => t.body_pr = parse_body_pr(c),
            "a:lstStyle" => t.list_style = Some(parse_list_style(c)),
            "a:p" => t.paragraphs.push(parse_paragraph(c)),
            _ => return false,
        }
        true
    });
    t
}

/// Write a text body with element name `name`.
pub fn write_text_body(t: &TextBody, name: &str) -> XmlElement {
    let mut e = El::new(name).child(write_body_pr(&t.body_pr));
    if let Some(ls) = &t.list_style {
        e = e.child(write_list_style(ls, "a:lstStyle"));
    }
    for p in &t.paragraphs {
        e = e.child(write_paragraph(p));
    }
    e.done()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::{canonical, parse};

    #[test]
    fn text_body_roundtrip() {
        let xml = r#"<p:txBody><a:bodyPr wrap="square" lIns="91440" numCol="2" spcCol="182880" anchor="ctr" rtlCol="0"><a:normAutofit fontScale="62500" lnSpcReduction="20000"/></a:bodyPr><a:lstStyle><a:lvl1pPr marL="0" indent="0"><a:buNone/><a:defRPr sz="2400"/></a:lvl1pPr></a:lstStyle><a:p><a:pPr lvl="1" algn="ctr"><a:lnSpc><a:spcPct val="90000"/></a:lnSpc><a:spcBef><a:spcPts val="600"/></a:spcBef><a:buClr><a:srgbClr val="FF0000"/></a:buClr><a:buSzPct val="80000"/><a:buFont typeface="Arial"/><a:buChar char="•"/></a:pPr><a:r><a:rPr lang="vi-VN" sz="2000" b="1" dirty="0"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mn-lt"/><a:hlinkClick r:id="rId2"/></a:rPr><a:t>Xin chào </a:t></a:r><a:br><a:rPr lang="en-US"/></a:br><a:fld id="{1}" type="slidenum"><a:rPr lang="en-US"/><a:t>3</a:t></a:fld><a:endParaRPr lang="en-US" dirty="0"/></a:p></p:txBody>"#;
        let el = parse(xml.as_bytes(), "/t").unwrap();
        let t = parse_text_body(&el);
        assert_eq!(
            t.body_pr.autofit,
            Some(Autofit::Normal {
                font_scale: Some(62_500),
                ln_spc_reduction: Some(20_000)
            })
        );
        assert_eq!(t.body_pr.num_col, Some(2));
        let p = &t.paragraphs[0];
        assert_eq!(p.text(), "Xin chào \n3");
        let pp = p.props.as_ref().unwrap();
        assert_eq!(pp.bullet, Some(Bullet::Char("•".into())));
        assert_eq!(pp.bu_sz, Some(Some(Spacing::Percent(80_000))));
        let TextRun::Run { props, .. } = &p.runs[0] else {
            panic!()
        };
        assert_eq!(props.sz, Some(2000));
        assert_eq!(props.extra.len(), 1); // hlinkClick kept opaque
        assert_eq!(canonical(&write_text_body(&t, "p:txBody")), canonical(&el));
    }
}
