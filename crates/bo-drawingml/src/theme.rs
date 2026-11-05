//! Themes — ECMA-376 Part 1, §20.1.6 (theme, themeElements) with the colour
//! scheme (§20.1.6.2), font scheme (§20.1.4.1.18) and format scheme (§20.1.4.1.14).

use crate::color::{find_color, write_color};
use crate::dom::{self, walk, AttrBag, El, Kids};
use crate::props::{parse_fill, parse_line, write_fill, write_line};
use crate::text::{parse_font, write_font};
use bo_model::slide::{FontCollection, FormatScheme, TextFont, Theme, XmlElement};

const THEME: &[&str] = &[
    "a:themeElements",
    "a:objectDefaults",
    "a:extraClrSchemeLst",
    "a:custClrLst",
    "a:extLst",
];
const ELEMENTS: &[&str] = &["a:clrScheme", "a:fontScheme", "a:fmtScheme", "a:extLst"];
const FONTS: &[&str] = &["a:majorFont", "a:minorFont", "a:extLst"];
const FONT_COLL: &[&str] = &["a:latin", "a:ea", "a:cs", "a:font", "a:extLst"];
const CLR_SLOTS: &[&str] = &[
    "dk1", "lt1", "dk2", "lt2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6",
    "hlink", "folHlink",
];

fn parse_font_coll(el: &XmlElement) -> FontCollection {
    let mut f = FontCollection::default();
    f.extra = walk(el, FONT_COLL, |c| {
        match c.name.as_str() {
            "a:latin" => f.latin = parse_font(c),
            "a:ea" => f.ea = parse_font(c),
            "a:cs" => f.cs = parse_font(c),
            "a:font" => f.scripts.push((
                c.attr("script").unwrap_or_default().to_owned(),
                c.attr("typeface").unwrap_or_default().to_owned(),
            )),
            _ => return false,
        }
        true
    });
    f
}

fn write_font_coll(f: &FontCollection, name: &str) -> XmlElement {
    let mut k = Kids::new(FONT_COLL);
    k.push(write_font("a:latin", &f.latin));
    k.push(write_font("a:ea", &f.ea));
    k.push(write_font("a:cs", &f.cs));
    for (s, t) in &f.scripts {
        k.push(El::new("a:font").a("script", s).a("typeface", t).done());
    }
    k.extras(&f.extra);
    k.build(El::new(name))
}

/// Parse a theme part (`/ppt/theme/themeN.xml`).
pub fn parse_theme(data: &[u8], part: &str) -> dom::DomResult<Theme> {
    let root = dom::parse(data, part)?;
    let mut bag = AttrBag::of(&root);
    let mut t = Theme {
        name: bag.take("name"),
        ..Theme::default()
    };
    t.info.part_name = part.to_owned();
    t.info.root_attrs = bag.rest();
    t.extra = walk(&root, THEME, |c| {
        if c.name != "a:themeElements" {
            return false;
        }
        t.elements_extra = walk(c, ELEMENTS, |e| {
            match e.name.as_str() {
                "a:clrScheme" => {
                    t.colors.name = e.attr("name").unwrap_or_default().to_owned();
                    t.colors.extra = walk(e, &[], |s| {
                        let local = s.local();
                        if !CLR_SLOTS.contains(&local) {
                            return false;
                        }
                        match find_color(s) {
                            Some(col) => {
                                t.colors.colors.push((local.to_owned(), col));
                                true
                            }
                            None => false,
                        }
                    });
                }
                "a:fontScheme" => {
                    t.fonts.name = e.attr("name").unwrap_or_default().to_owned();
                    t.fonts.extra = walk(e, FONTS, |f| {
                        match f.name.as_str() {
                            "a:majorFont" => t.fonts.major = parse_font_coll(f),
                            "a:minorFont" => t.fonts.minor = parse_font_coll(f),
                            _ => return false,
                        }
                        true
                    });
                }
                "a:fmtScheme" => {
                    let mut fs = FormatScheme {
                        name: e.attr("name").map(str::to_owned),
                        ..FormatScheme::default()
                    };
                    for l in e.elements() {
                        match l.local() {
                            "fillStyleLst" => {
                                fs.fill_styles = l.elements().filter_map(parse_fill).collect();
                            }
                            "lnStyleLst" => fs.line_styles = l.elements().map(parse_line).collect(),
                            "effectStyleLst" => fs.effect_styles = l.elements().cloned().collect(),
                            "bgFillStyleLst" => {
                                fs.bg_fill_styles = l.elements().filter_map(parse_fill).collect();
                            }
                            _ => {}
                        }
                    }
                    t.formats = fs;
                }
                _ => return false,
            }
            true
        });
        true
    });
    Ok(t)
}

/// Serialize a theme part.
pub fn write_theme(t: &Theme) -> Vec<u8> {
    let mut cs = El::new("a:clrScheme").a("name", &t.colors.name);
    for (slot, c) in &t.colors.colors {
        cs = cs.child(El::new(&format!("a:{slot}")).child(write_color(c)).done());
    }
    let mut cs = cs.done();
    let mut k = Kids::new(&[]);
    k.extras(&t.colors.extra);
    k.append_to(&mut cs);

    let mut fk = Kids::new(FONTS);
    fk.push(write_font_coll(&t.fonts.major, "a:majorFont"));
    fk.push(write_font_coll(&t.fonts.minor, "a:minorFont"));
    fk.extras(&t.fonts.extra);
    let fonts = fk.build(El::new("a:fontScheme").a("name", &t.fonts.name));

    let f = &t.formats;
    let mut fill = El::new("a:fillStyleLst");
    for x in &f.fill_styles {
        fill = fill.child(write_fill(x));
    }
    let mut ln = El::new("a:lnStyleLst");
    for x in &f.line_styles {
        ln = ln.child(write_line(x, "a:ln"));
    }
    let mut eff = El::new("a:effectStyleLst");
    for x in &f.effect_styles {
        eff = eff.child(x.clone());
    }
    let mut bg = El::new("a:bgFillStyleLst");
    for x in &f.bg_fill_styles {
        bg = bg.child(write_fill(x));
    }
    let fmt = El::new("a:fmtScheme")
        .opt("name", f.name.as_ref())
        .child(fill.done())
        .child(ln.done())
        .child(eff.done())
        .child(bg.done())
        .done();

    let mut ek = Kids::new(ELEMENTS);
    ek.push(cs);
    ek.push(fonts);
    ek.push(fmt);
    ek.extras(&t.elements_extra);
    let elements = ek.build(El::new("a:themeElements"));

    let mut rk = Kids::new(THEME);
    rk.push(elements);
    rk.extras(&t.extra);
    let mut root = El::new("a:theme");
    if t.info.root_attrs.is_empty() {
        root = root.a(
            "xmlns:a",
            "http://schemas.openxmlformats.org/drawingml/2006/main",
        );
    }
    let root = rk.build(root.rest(&t.info.root_attrs).opt("name", t.name.as_ref()));
    dom::to_part_xml(&root)
}

/// Resolve a theme font reference (`+mj-lt`, `+mn-ea`, …) to a typeface; other
/// names are returned unchanged. §20.1.4.1.18 / §21.1.2.3.7.
pub fn resolve_typeface<'a>(theme: Option<&'a Theme>, name: &'a str) -> &'a str {
    let Some(t) = theme else {
        return name;
    };
    let pick = |c: &'a FontCollection, kind: &str| -> &'a TextFont {
        match kind {
            "ea" => &c.ea,
            "cs" => &c.cs,
            _ => &c.latin,
        }
    };
    match name.strip_prefix('+') {
        Some(rest) => {
            let (which, kind) = rest.split_once('-').unwrap_or((rest, "lt"));
            let coll = if which == "mj" {
                &t.fonts.major
            } else {
                &t.fonts.minor
            };
            &pick(coll, kind).typeface
        }
        None => name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::slide::{ColorKind, Fill};

    pub(crate) const THEME_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Blue"><a:themeElements><a:clrScheme name="Blue"><a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1><a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="44546A"/></a:dk2><a:lt2><a:srgbClr val="E7E6E6"/></a:lt2><a:accent1><a:srgbClr val="4472C4"/></a:accent1><a:accent2><a:srgbClr val="ED7D31"/></a:accent2><a:accent3><a:srgbClr val="A5A5A5"/></a:accent3><a:accent4><a:srgbClr val="FFC000"/></a:accent4><a:accent5><a:srgbClr val="5B9BD5"/></a:accent5><a:accent6><a:srgbClr val="70AD47"/></a:accent6><a:hlink><a:srgbClr val="0563C1"/></a:hlink><a:folHlink><a:srgbClr val="954F72"/></a:folHlink></a:clrScheme><a:fontScheme name="Blue"><a:majorFont><a:latin typeface="Calibri Light"/><a:ea typeface=""/><a:cs typeface=""/><a:font script="Jpan" typeface="Yu Gothic Light"/></a:majorFont><a:minorFont><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme><a:fmtScheme name="Blue"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:gradFill rotWithShape="1"><a:gsLst><a:gs pos="0"><a:schemeClr val="phClr"><a:tint val="50000"/></a:schemeClr></a:gs><a:gs pos="100000"><a:schemeClr val="phClr"/></a:gs></a:gsLst><a:lin ang="5400000" scaled="0"/></a:gradFill></a:fillStyleLst><a:lnStyleLst><a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="12700"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements><a:objectDefaults/><a:extraClrSchemeLst/></a:theme>"#;

    #[test]
    fn parse_and_roundtrip() {
        let t = parse_theme(THEME_XML.as_bytes(), "/ppt/theme/theme1.xml").unwrap();
        assert_eq!(t.colors.colors.len(), 12);
        assert_eq!(
            t.scheme_color("accent1").map(|c| &c.kind),
            Some(&ColorKind::Srgb(0x44_72C4))
        );
        assert_eq!(t.fonts.major.latin.typeface, "Calibri Light");
        assert_eq!(t.fonts.major.scripts.len(), 1);
        assert!(matches!(t.formats.fill_styles[1], Fill::Gradient(_)));
        assert_eq!(t.formats.line_styles[1].w, Some(12_700));
        assert_eq!(resolve_typeface(Some(&t), "+mn-lt"), "Calibri");
        assert_eq!(resolve_typeface(Some(&t), "+mj-lt"), "Calibri Light");
        assert_eq!(resolve_typeface(Some(&t), "Arial"), "Arial");
        let out = write_theme(&t);
        let a = dom::parse(THEME_XML.as_bytes(), "/a").unwrap();
        let b = dom::parse(&out, "/b").unwrap();
        assert_eq!(dom::canonical(&a), dom::canonical(&b));
        assert_eq!(parse_theme(&out, "/ppt/theme/theme1.xml").unwrap(), t);
    }
}
