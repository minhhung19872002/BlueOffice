//! Styles part — ECMA-376 Part 1, §18.8 Styles.

use crate::xml::{
    a, a_opt, a_optb, a_optf, a_true, attrs, capture, emit_ordered, for_children, lname, pbool,
    push_unknown, root, take, take_bool, take_f64, take_u32,
};
use crate::Result;
use bo_model::sheet::{
    Border, BorderEdge, CellAlignment, CellProtection, CellStyle, CellXf, Dxf, Fill, Font,
    GradientFill, NumFmt, PatternFill, SheetColor, Stylesheet,
};
use bo_opc::xml::{escape, XmlReader};
use quick_xml::events::BytesStart;
use std::fmt::Write as _;

/// Child sequence of `styleSheet` (§18.8.39 CT_Stylesheet).
const STYLESHEET_SCHEMA: &[&str] = &[
    "numFmts",
    "fonts",
    "fills",
    "borders",
    "cellStyleXfs",
    "cellXfs",
    "cellStyles",
    "dxfs",
    "tableStyles",
    "colors",
    "extLst",
];

pub(crate) const SML_NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";

/// Parse the styles part.
pub(crate) fn read_styles(data: &[u8], part: &str) -> Result<Stylesheet> {
    let mut r = XmlReader::new(data, part);
    let (rootel, empty) = root(&mut r)?;
    let mut ss = Stylesheet {
        root_attrs: crate::read::root_attrs_of(&rootel, false),
        ..Stylesheet::default()
    };
    if empty {
        return Ok(ss);
    }
    let mut prev = String::new();
    for_children(&mut r, |r, e, empty| {
        let name = lname(e);
        match name.as_str() {
            // §18.8.31 numFmts / §18.8.30 numFmt
            "numFmts" if !empty => for_children(r, |_, e, _| {
                if lname(e) == "numFmt" {
                    let mut at = attrs(e);
                    if let (Some(id), Some(code)) =
                        (take_u32(&mut at, "numFmtId"), take(&mut at, "formatCode"))
                    {
                        ss.num_fmts.push(NumFmt { id, code });
                    }
                }
                Ok(())
            })?,
            // §18.8.23 fonts
            "fonts" if !empty => for_children(r, |r, e, empty| {
                if lname(e) == "font" {
                    ss.fonts.push(read_font(r, empty)?);
                }
                Ok(())
            })?,
            // §18.8.21 fills
            "fills" if !empty => for_children(r, |r, e, empty| {
                if lname(e) == "fill" {
                    ss.fills.push(read_fill(r, empty)?);
                }
                Ok(())
            })?,
            // §18.8.5 borders
            "borders" if !empty => for_children(r, |r, e, empty| {
                if lname(e) == "border" {
                    ss.borders.push(read_border(r, e, empty)?);
                }
                Ok(())
            })?,
            // §18.8.9 cellStyleXfs
            "cellStyleXfs" if !empty => for_children(r, |r, e, empty| {
                if lname(e) == "xf" {
                    ss.cell_style_xfs.push(read_xf(r, e, empty)?);
                }
                Ok(())
            })?,
            // §18.8.10 cellXfs
            "cellXfs" if !empty => for_children(r, |r, e, empty| {
                if lname(e) == "xf" {
                    ss.cell_xfs.push(read_xf(r, e, empty)?);
                }
                Ok(())
            })?,
            // §18.8.8 cellStyles / §18.8.7 cellStyle
            "cellStyles" if !empty => for_children(r, |_, e, _| {
                if lname(e) == "cellStyle" {
                    let mut at = attrs(e);
                    ss.cell_styles.push(CellStyle {
                        name: take(&mut at, "name").unwrap_or_default(),
                        xf_id: take_u32(&mut at, "xfId").unwrap_or(0),
                        builtin_id: take_u32(&mut at, "builtinId"),
                        i_level: take_u32(&mut at, "iLevel"),
                        hidden: take_bool(&mut at, "hidden").unwrap_or(false),
                        custom_builtin: take_bool(&mut at, "customBuiltin").unwrap_or(false),
                    });
                }
                Ok(())
            })?,
            // §18.8.15 dxfs
            "dxfs" if !empty => for_children(r, |r, e, empty| {
                if lname(e) == "dxf" && !empty {
                    ss.dxfs.push(read_dxf(r)?);
                } else if lname(e) == "dxf" {
                    ss.dxfs.push(Dxf::default());
                }
                Ok(())
            })?,
            "numFmts" | "fonts" | "fills" | "borders" | "cellStyleXfs" | "cellXfs"
            | "cellStyles" | "dxfs" => {}
            _ => {
                let xml = capture(r, e, empty)?;
                push_unknown(&mut ss.unknown, &mut prev, STYLESHEET_SCHEMA, &name, xml);
                return Ok(());
            }
        }
        prev = name;
        Ok(())
    })?;
    ss.frame_styles = frame_styles(&ss);
    Ok(ss)
}

/// §18.3.1.15 / §18.8.19 color.
pub(crate) fn read_color(e: &BytesStart<'_>) -> SheetColor {
    let mut at = attrs(e);
    SheetColor {
        rgb: take(&mut at, "rgb"),
        theme: take_u32(&mut at, "theme"),
        indexed: take_u32(&mut at, "indexed"),
        tint: take_f64(&mut at, "tint"),
        auto: take_bool(&mut at, "auto").unwrap_or(false),
    }
}

pub(crate) fn write_color(out: &mut String, tag: &str, c: &SheetColor) {
    out.push('<');
    out.push_str(tag);
    a_true(out, "auto", c.auto);
    a_opt(out, "indexed", c.indexed.as_ref());
    a_opt(out, "rgb", c.rgb.as_ref());
    a_opt(out, "theme", c.theme.as_ref());
    a_optf(out, "tint", c.tint);
    out.push_str("/>");
}

fn val(e: &BytesStart<'_>) -> Option<String> {
    let mut at = attrs(e);
    take(&mut at, "val")
}

/// Boolean property element (`CT_BooleanProperty`, §18.17.? — `val` defaults to true).
fn bool_prop(e: &BytesStart<'_>) -> bool {
    val(e).is_none_or(|v| pbool(&v))
}

/// §18.8.22 font, also §18.4.7 rPr (run properties use `rFont` for the name).
pub(crate) fn read_font(r: &mut XmlReader<'_>, empty: bool) -> Result<Font> {
    let mut f = Font::default();
    if empty {
        return Ok(f);
    }
    for_children(r, |_, e, _| {
        match lname(e).as_str() {
            "b" => f.bold = bool_prop(e),
            "i" => f.italic = bool_prop(e),
            "strike" => f.strike = bool_prop(e),
            "outline" => f.outline = bool_prop(e),
            "shadow" => f.shadow = bool_prop(e),
            "condense" => f.condense = bool_prop(e),
            "extend" => f.extend = bool_prop(e),
            // §18.4.13 u — default value "single".
            "u" => f.underline = Some(val(e).unwrap_or_else(|| "single".into())),
            "vertAlign" => f.vert_align = val(e),
            "sz" => {
                f.size = val(e)
                    .and_then(|v| v.parse().ok())
                    .filter(|v: &f64| v.is_finite());
            }
            "color" => f.color = Some(read_color(e)),
            "name" | "rFont" => f.name = val(e),
            "family" => f.family = val(e).and_then(|v| v.parse().ok()),
            "charset" => f.charset = val(e).and_then(|v| v.parse().ok()),
            "scheme" => f.scheme = val(e),
            _ => {}
        }
        Ok(())
    })?;
    Ok(f)
}

/// Write a font as `font` (styles) or `rPr` (rich text, `rich = true`).
pub(crate) fn write_font(out: &mut String, f: &Font, rich: bool) {
    out.push_str(if rich { "<rPr>" } else { "<font>" });
    let flag = |out: &mut String, tag: &str, on: bool| {
        if on {
            let _ = write!(out, "<{tag}/>");
        }
    };
    flag(out, "b", f.bold);
    flag(out, "i", f.italic);
    flag(out, "strike", f.strike);
    flag(out, "condense", f.condense);
    flag(out, "extend", f.extend);
    flag(out, "outline", f.outline);
    flag(out, "shadow", f.shadow);
    if let Some(u) = &f.underline {
        if u == "single" {
            out.push_str("<u/>");
        } else {
            let _ = write!(out, "<u val=\"{}\"/>", escape(u));
        }
    }
    if let Some(v) = &f.vert_align {
        let _ = write!(out, "<vertAlign val=\"{}\"/>", escape(v));
    }
    if let Some(sz) = f.size {
        let _ = write!(out, "<sz val=\"{}\"/>", crate::xml::fmt_f64(sz));
    }
    if let Some(c) = &f.color {
        write_color(out, "color", c);
    }
    if let Some(n) = &f.name {
        let _ = write!(
            out,
            "<{} val=\"{}\"/>",
            if rich { "rFont" } else { "name" },
            escape(n)
        );
    }
    if let Some(v) = f.family {
        let _ = write!(out, "<family val=\"{v}\"/>");
    }
    if let Some(v) = f.charset {
        let _ = write!(out, "<charset val=\"{v}\"/>");
    }
    if let Some(v) = &f.scheme {
        let _ = write!(out, "<scheme val=\"{}\"/>", escape(v));
    }
    out.push_str(if rich { "</rPr>" } else { "</font>" });
}

/// §18.8.20 fill (patternFill §18.8.32, gradientFill §18.8.24).
fn read_fill(r: &mut XmlReader<'_>, empty: bool) -> Result<Fill> {
    let mut fill = Fill::default();
    if empty {
        return Ok(fill);
    }
    for_children(r, |r, e, empty| {
        match lname(e).as_str() {
            "patternFill" => {
                let mut at = attrs(e);
                let mut p = PatternFill {
                    pattern: take(&mut at, "patternType"),
                    ..PatternFill::default()
                };
                if !empty {
                    for_children(r, |_, e, _| {
                        match lname(e).as_str() {
                            "fgColor" => p.fg = Some(read_color(e)),
                            "bgColor" => p.bg = Some(read_color(e)),
                            _ => {}
                        }
                        Ok(())
                    })?;
                }
                fill = Fill::Pattern(p);
            }
            "gradientFill" => {
                let mut at = attrs(e);
                let mut g = GradientFill {
                    kind: take(&mut at, "type"),
                    degree: take_f64(&mut at, "degree"),
                    left: take_f64(&mut at, "left"),
                    right: take_f64(&mut at, "right"),
                    top: take_f64(&mut at, "top"),
                    bottom: take_f64(&mut at, "bottom"),
                    stops: Vec::new(),
                };
                if !empty {
                    for_children(r, |r, e, empty| {
                        if lname(e) == "stop" {
                            let mut at = attrs(e);
                            let pos = take_f64(&mut at, "position").unwrap_or(0.0);
                            let mut color = SheetColor::default();
                            if !empty {
                                for_children(r, |_, e, _| {
                                    if lname(e) == "color" {
                                        color = read_color(e);
                                    }
                                    Ok(())
                                })?;
                            }
                            g.stops.push((pos, color));
                        }
                        Ok(())
                    })?;
                }
                fill = Fill::Gradient(g);
            }
            _ => {}
        }
        Ok(())
    })?;
    Ok(fill)
}

fn write_fill(out: &mut String, f: &Fill) {
    out.push_str("<fill>");
    match f {
        Fill::Pattern(p) => {
            out.push_str("<patternFill");
            a_opt(out, "patternType", p.pattern.as_ref());
            if p.fg.is_none() && p.bg.is_none() {
                out.push_str("/>");
            } else {
                out.push('>');
                if let Some(c) = &p.fg {
                    write_color(out, "fgColor", c);
                }
                if let Some(c) = &p.bg {
                    write_color(out, "bgColor", c);
                }
                out.push_str("</patternFill>");
            }
        }
        Fill::Gradient(g) => {
            out.push_str("<gradientFill");
            a_opt(out, "type", g.kind.as_ref());
            a_optf(out, "degree", g.degree);
            a_optf(out, "left", g.left);
            a_optf(out, "right", g.right);
            a_optf(out, "top", g.top);
            a_optf(out, "bottom", g.bottom);
            out.push('>');
            for (pos, c) in &g.stops {
                let _ = write!(out, "<stop position=\"{}\">", crate::xml::fmt_f64(*pos));
                write_color(out, "color", c);
                out.push_str("</stop>");
            }
            out.push_str("</gradientFill>");
        }
    }
    out.push_str("</fill>");
}

/// §18.8.4 border.
fn read_border(r: &mut XmlReader<'_>, e: &BytesStart<'_>, empty: bool) -> Result<Border> {
    let mut at = attrs(e);
    let mut b = Border {
        diagonal_up: take_bool(&mut at, "diagonalUp").unwrap_or(false),
        diagonal_down: take_bool(&mut at, "diagonalDown").unwrap_or(false),
        outline: take_bool(&mut at, "outline"),
        ..Border::default()
    };
    if empty {
        return Ok(b);
    }
    for_children(r, |r, e, empty| {
        let mut at = attrs(e);
        let mut edge = BorderEdge {
            style: take(&mut at, "style"),
            color: None,
        };
        if !empty {
            for_children(r, |_, e, _| {
                if lname(e) == "color" {
                    edge.color = Some(read_color(e));
                }
                Ok(())
            })?;
        }
        match lname(e).as_str() {
            "left" | "start" => b.left = edge,
            "right" | "end" => b.right = edge,
            "top" => b.top = edge,
            "bottom" => b.bottom = edge,
            "diagonal" => b.diagonal = edge,
            "vertical" => b.vertical = Some(edge),
            "horizontal" => b.horizontal = Some(edge),
            _ => {}
        }
        Ok(())
    })?;
    Ok(b)
}

fn write_edge(out: &mut String, tag: &str, e: &BorderEdge) {
    out.push('<');
    out.push_str(tag);
    a_opt(out, "style", e.style.as_ref());
    match &e.color {
        Some(c) => {
            out.push('>');
            write_color(out, "color", c);
            let _ = write!(out, "</{tag}>");
        }
        None => out.push_str("/>"),
    }
}

fn write_border(out: &mut String, b: &Border) {
    out.push_str("<border");
    a_true(out, "diagonalUp", b.diagonal_up);
    a_true(out, "diagonalDown", b.diagonal_down);
    a_optb(out, "outline", b.outline);
    out.push('>');
    write_edge(out, "left", &b.left);
    write_edge(out, "right", &b.right);
    write_edge(out, "top", &b.top);
    write_edge(out, "bottom", &b.bottom);
    write_edge(out, "diagonal", &b.diagonal);
    if let Some(v) = &b.vertical {
        write_edge(out, "vertical", v);
    }
    if let Some(h) = &b.horizontal {
        write_edge(out, "horizontal", h);
    }
    out.push_str("</border>");
}

/// §18.8.1 alignment.
fn read_alignment(e: &BytesStart<'_>) -> CellAlignment {
    let mut at = attrs(e);
    CellAlignment {
        horizontal: take(&mut at, "horizontal"),
        vertical: take(&mut at, "vertical"),
        wrap_text: take_bool(&mut at, "wrapText").unwrap_or(false),
        shrink_to_fit: take_bool(&mut at, "shrinkToFit").unwrap_or(false),
        indent: take_u32(&mut at, "indent"),
        text_rotation: take_u32(&mut at, "textRotation"),
        reading_order: take_u32(&mut at, "readingOrder"),
        justify_last_line: take_bool(&mut at, "justifyLastLine").unwrap_or(false),
        relative_indent: take(&mut at, "relativeIndent").and_then(|v| v.parse().ok()),
    }
}

fn write_alignment(out: &mut String, al: &CellAlignment) {
    out.push_str("<alignment");
    a_opt(out, "horizontal", al.horizontal.as_ref());
    a_opt(out, "vertical", al.vertical.as_ref());
    a_opt(out, "textRotation", al.text_rotation.as_ref());
    a_true(out, "wrapText", al.wrap_text);
    a_opt(out, "indent", al.indent.as_ref());
    a_opt(out, "relativeIndent", al.relative_indent.as_ref());
    a_true(out, "justifyLastLine", al.justify_last_line);
    a_true(out, "shrinkToFit", al.shrink_to_fit);
    a_opt(out, "readingOrder", al.reading_order.as_ref());
    out.push_str("/>");
}

/// §18.8.33 protection.
fn read_protection(e: &BytesStart<'_>) -> CellProtection {
    let mut at = attrs(e);
    CellProtection {
        locked: take_bool(&mut at, "locked"),
        hidden: take_bool(&mut at, "hidden"),
    }
}

fn write_protection(out: &mut String, p: &CellProtection) {
    out.push_str("<protection");
    a_optb(out, "locked", p.locked);
    a_optb(out, "hidden", p.hidden);
    out.push_str("/>");
}

/// §18.8.45 xf.
fn read_xf(r: &mut XmlReader<'_>, e: &BytesStart<'_>, empty: bool) -> Result<CellXf> {
    let mut at = attrs(e);
    let mut xf = CellXf {
        num_fmt_id: take_u32(&mut at, "numFmtId").unwrap_or(0),
        font_id: take_u32(&mut at, "fontId").unwrap_or(0),
        fill_id: take_u32(&mut at, "fillId").unwrap_or(0),
        border_id: take_u32(&mut at, "borderId").unwrap_or(0),
        xf_id: take_u32(&mut at, "xfId"),
        apply_number_format: take_bool(&mut at, "applyNumberFormat"),
        apply_font: take_bool(&mut at, "applyFont"),
        apply_fill: take_bool(&mut at, "applyFill"),
        apply_border: take_bool(&mut at, "applyBorder"),
        apply_alignment: take_bool(&mut at, "applyAlignment"),
        apply_protection: take_bool(&mut at, "applyProtection"),
        quote_prefix: take_bool(&mut at, "quotePrefix").unwrap_or(false),
        pivot_button: take_bool(&mut at, "pivotButton").unwrap_or(false),
        alignment: None,
        protection: None,
        checkbox: false,
        complement: None,
    };
    if !empty {
        for_children(r, |r, e, empty| {
            match lname(e).as_str() {
                "alignment" => xf.alignment = Some(read_alignment(e)),
                "protection" => xf.protection = Some(read_protection(e)),
                // §18.2.10 extLst → [MS-XLSX] §2.2.4.9 ext {C7286773-…} → xfComplement (§2.6.238).
                "extLst" if !empty => for_children(r, |r, ext, empty| {
                    let uri = bo_opc::xml::attr(ext, b"uri");
                    if lname(ext) == "ext"
                        && !empty
                        && uri.as_deref() == Some(crate::cellctl::EXT_XF_COMPLEMENT)
                    {
                        for_children(r, |_, c, _| {
                            if lname(c) == "xfComplement" {
                                xf.complement =
                                    bo_opc::xml::attr(c, b"i").and_then(|v| v.parse().ok());
                            }
                            Ok(())
                        })?;
                    }
                    Ok(())
                })?,
                _ => {}
            }
            Ok(())
        })?;
    }
    Ok(xf)
}

fn write_xf(out: &mut String, xf: &CellXf) {
    out.push_str("<xf");
    a(out, "numFmtId", &xf.num_fmt_id.to_string());
    a(out, "fontId", &xf.font_id.to_string());
    a(out, "fillId", &xf.fill_id.to_string());
    a(out, "borderId", &xf.border_id.to_string());
    a_opt(out, "xfId", xf.xf_id.as_ref());
    a_true(out, "quotePrefix", xf.quote_prefix);
    a_true(out, "pivotButton", xf.pivot_button);
    a_optb(out, "applyNumberFormat", xf.apply_number_format);
    a_optb(out, "applyFont", xf.apply_font);
    a_optb(out, "applyFill", xf.apply_fill);
    a_optb(out, "applyBorder", xf.apply_border);
    a_optb(out, "applyAlignment", xf.apply_alignment);
    a_optb(out, "applyProtection", xf.apply_protection);
    if xf.alignment.is_none() && xf.protection.is_none() && xf.complement.is_none() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    if let Some(al) = &xf.alignment {
        write_alignment(out, al);
    }
    if let Some(p) = &xf.protection {
        write_protection(out, p);
    }
    if let Some(i) = xf.complement {
        // [MS-XLSX] §2.2.4.9: xf/extLst/ext {C7286773-…} → xfpb:xfComplement (§2.6.238).
        let _ = write!(
            out,
            "<extLst><ext uri=\"{}\" xmlns:xfpb=\"{}\"><xfpb:xfComplement i=\"{i}\"/></ext></extLst>",
            crate::cellctl::EXT_XF_COMPLEMENT,
            crate::cellctl::NS_FPB
        );
    }
    out.push_str("</xf>");
}

/// §18.8.14 dxf.
fn read_dxf(r: &mut XmlReader<'_>) -> Result<Dxf> {
    let mut d = Dxf::default();
    for_children(r, |r, e, empty| {
        match lname(e).as_str() {
            "font" => d.font = Some(read_font(r, empty)?),
            "numFmt" => {
                let mut at = attrs(e);
                d.num_fmt = Some(NumFmt {
                    id: take_u32(&mut at, "numFmtId").unwrap_or(0),
                    code: take(&mut at, "formatCode").unwrap_or_default(),
                });
            }
            "fill" => d.fill = Some(read_fill(r, empty)?),
            "alignment" => d.alignment = Some(read_alignment(e)),
            "border" => d.border = Some(read_border(r, e, empty)?),
            "protection" => d.protection = Some(read_protection(e)),
            _ => {}
        }
        Ok(())
    })?;
    Ok(d)
}

fn write_dxf(out: &mut String, d: &Dxf) {
    out.push_str("<dxf>");
    if let Some(f) = &d.font {
        write_font(out, f, false);
    }
    if let Some(n) = &d.num_fmt {
        write_num_fmt(out, n);
    }
    if let Some(f) = &d.fill {
        write_fill(out, f);
    }
    if let Some(al) = &d.alignment {
        write_alignment(out, al);
    }
    if let Some(b) = &d.border {
        write_border(out, b);
    }
    if let Some(p) = &d.protection {
        write_protection(out, p);
    }
    out.push_str("</dxf>");
}

fn write_num_fmt(out: &mut String, n: &NumFmt) {
    let _ = write!(
        out,
        "<numFmt numFmtId=\"{}\" formatCode=\"{}\"/>",
        n.id,
        escape(&n.code)
    );
}

/// Serialize the styles part.
pub(crate) fn write_styles(ss: &Stylesheet) -> String {
    let mut out =
        String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<styleSheet");
    crate::write::root_attrs(&mut out, &ss.root_attrs, false);
    out.push('>');
    emit_ordered(
        &mut out,
        STYLESHEET_SCHEMA,
        &ss.unknown,
        |slot, out| match slot {
            "numFmts" if !ss.num_fmts.is_empty() => {
                let _ = write!(out, "<numFmts count=\"{}\">", ss.num_fmts.len());
                for n in &ss.num_fmts {
                    write_num_fmt(out, n);
                }
                out.push_str("</numFmts>");
            }
            "fonts" => {
                let _ = write!(out, "<fonts count=\"{}\">", ss.fonts.len());
                for f in &ss.fonts {
                    write_font(out, f, false);
                }
                out.push_str("</fonts>");
            }
            "fills" => {
                let _ = write!(out, "<fills count=\"{}\">", ss.fills.len());
                for f in &ss.fills {
                    write_fill(out, f);
                }
                out.push_str("</fills>");
            }
            "borders" => {
                let _ = write!(out, "<borders count=\"{}\">", ss.borders.len());
                for b in &ss.borders {
                    write_border(out, b);
                }
                out.push_str("</borders>");
            }
            "cellStyleXfs" if !ss.cell_style_xfs.is_empty() => {
                let _ = write!(out, "<cellStyleXfs count=\"{}\">", ss.cell_style_xfs.len());
                for x in &ss.cell_style_xfs {
                    write_xf(out, x);
                }
                out.push_str("</cellStyleXfs>");
            }
            "cellXfs" => {
                let _ = write!(out, "<cellXfs count=\"{}\">", ss.cell_xfs.len());
                for x in &ss.cell_xfs {
                    write_xf(out, x);
                }
                out.push_str("</cellXfs>");
            }
            "cellStyles" if !ss.cell_styles.is_empty() => {
                let _ = write!(out, "<cellStyles count=\"{}\">", ss.cell_styles.len());
                for c in &ss.cell_styles {
                    out.push_str("<cellStyle");
                    a(out, "name", &c.name);
                    a(out, "xfId", &c.xf_id.to_string());
                    a_opt(out, "builtinId", c.builtin_id.as_ref());
                    a_opt(out, "iLevel", c.i_level.as_ref());
                    a_true(out, "hidden", c.hidden);
                    a_true(out, "customBuiltin", c.custom_builtin);
                    out.push_str("/>");
                }
                out.push_str("</cellStyles>");
            }
            "dxfs" if !ss.dxfs.is_empty() => {
                let _ = write!(out, "<dxfs count=\"{}\">", ss.dxfs.len());
                for d in &ss.dxfs {
                    write_dxf(out, d);
                }
                out.push_str("</dxfs>");
            }
            _ => {}
        },
    );
    out.push_str("</styleSheet>");
    out
}

// ------------------------------------------------------------------ custom slicer / timeline styles

/// The dxfs of the [MS-XLSX] `x14:dxfs` extension of `extLst` (in order; referenced by the
/// `dxfId` of `x14:slicerStyleElement` and `x15:timelineStyleElement`).
fn ext_dxfs(xml: &str) -> Vec<Dxf> {
    fn walk(r: &mut XmlReader<'_>, out: &mut Vec<Dxf>) -> Result<()> {
        for_children(r, |r, e, empty| {
            if empty {
                return Ok(());
            }
            if lname(e) == "dxfs" {
                for_children(r, |r, e, empty| {
                    if lname(e) == "dxf" {
                        out.push(if empty { Dxf::default() } else { read_dxf(r)? });
                    }
                    Ok(())
                })
            } else {
                walk(r, out)
            }
        })
    }
    let mut r = XmlReader::new(xml.as_bytes(), "styles extLst");
    let mut out = Vec::new();
    if matches!(root(&mut r), Ok((_, false))) {
        // A broken extension only loses its custom styles.
        let _ = walk(&mut r, &mut out);
    }
    out
}

/// The custom slicer and timeline styles of a stylesheet: each [MS-XLSX] `x14:slicerStyle` /
/// `x15:timelineStyle` of the `extLst` with its `tableStyle` of the same name (§18.8.40;
/// `wholeTable` and `headerRow` formats from §18.8.15 `dxfs`) and its own elements (formats from
/// the `x14:dxfs` extension).
#[must_use]
pub fn frame_styles(ss: &Stylesheet) -> Vec<bo_model::sheet::FrameStyleParts> {
    use crate::chart::{parse_dom, Node};
    let doms: Vec<Node> = ss
        .unknown
        .iter()
        .filter_map(|u| parse_dom(u.xml.as_bytes(), "styles"))
        .collect();
    let find = |name: &str| -> Vec<&Node> {
        let mut out = Vec::new();
        for d in &doms {
            if d.name == name {
                out.push(d);
            }
            d.descendants(name, &mut out);
        }
        out
    };
    let ext = ss
        .unknown
        .iter()
        .find(|u| u.xml.trim_start().starts_with("<extLst"))
        .map(|u| ext_dxfs(&u.xml))
        .unwrap_or_default();
    let dxf_at = |list: &[Dxf], id: Option<&str>| {
        id.and_then(|v| v.parse::<usize>().ok())
            .and_then(|i| list.get(i).cloned())
    };
    // §18.8.40 tableStyle → (wholeTable, headerRow).
    let table = |name: &str| -> (Option<Dxf>, Option<Dxf>) {
        let mut whole = None;
        let mut header = None;
        for t in find("tableStyle") {
            if t.attr("name") != Some(name) {
                continue;
            }
            for el in t.children.iter().filter(|c| c.name == "tableStyleElement") {
                match el.attr("type") {
                    Some("wholeTable") => whole = dxf_at(&ss.dxfs, el.attr("dxfId")),
                    Some("headerRow") => header = dxf_at(&ss.dxfs, el.attr("dxfId")),
                    _ => {}
                }
            }
        }
        (whole, header)
    };
    let mut out = Vec::new();
    for (tag, timeline, selected, unselected) in [
        (
            "slicerStyle",
            false,
            "selectedItemWithData",
            "unselectedItemWithData",
        ),
        (
            "timelineStyle",
            true,
            "selectedTimeBlock",
            "unselectedTimeBlock",
        ),
    ] {
        for s in find(tag) {
            let Some(name) = s.attr("name").filter(|n| !n.is_empty()) else {
                continue;
            };
            let (whole, header) = table(name);
            let mut parts = bo_model::sheet::FrameStyleParts {
                name: name.to_owned(),
                timeline,
                whole,
                header,
                ..Default::default()
            };
            let mut els = Vec::new();
            s.descendants(&format!("{tag}Element"), &mut els);
            for el in els {
                let dxf = dxf_at(&ext, el.attr("dxfId"));
                match el.attr("type") {
                    Some(t) if t == selected => parts.selected = dxf,
                    Some(t) if t == unselected => parts.unselected = dxf,
                    _ => {}
                }
            }
            out.push(parts);
        }
    }
    out
}

#[cfg(test)]
mod frame_style_tests {
    use super::*;

    /// A custom slicer style as [MS-XLSX] lays it out: the `tableStyle` (whole frame, header) in
    /// the main `dxfs`, the item formats in the `x14:dxfs` extension.
    #[test]
    fn custom_slicer_style_formats_are_found() {
        let xml = concat!(
            r#"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#,
            r#"<fonts count="1"><font><sz val="11"/><name val="Calibri"/></font></fonts>"#,
            r#"<fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills>"#,
            r#"<borders count="1"><border/></borders>"#,
            r#"<cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellXfs>"#,
            r#"<dxfs count="2"><dxf><font><b/><color rgb="FF203864"/></font></dxf>"#,
            r#"<dxf><border><left style="thin"><color rgb="FF70AD47"/></left></border></dxf></dxfs>"#,
            r#"<tableStyles count="1"><tableStyle name="Mine" pivot="0" table="0" count="2">"#,
            r#"<tableStyleElement type="wholeTable" dxfId="1"/><tableStyleElement type="headerRow" dxfId="0"/>"#,
            r#"</tableStyle></tableStyles>"#,
            r#"<extLst><ext uri="{EB79DEF2-80B8-43e5-95BD-54CBDDF9020C}" xmlns:x14="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main">"#,
            r#"<x14:slicerStyles defaultSlicerStyle="SlicerStyleLight1"><x14:slicerStyle name="Mine"><x14:slicerStyleElements>"#,
            r#"<x14:slicerStyleElement type="selectedItemWithData" dxfId="0"/>"#,
            r#"<x14:slicerStyleElement type="unselectedItemWithData" dxfId="1"/>"#,
            r#"</x14:slicerStyleElements></x14:slicerStyle></x14:slicerStyles></ext>"#,
            r#"<ext uri="{46F421CA-312F-682f-3DD2-61675219B42D}" xmlns:x14="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main">"#,
            r#"<x14:dxfs count="2"><dxf><fill><patternFill><bgColor rgb="FFC00000"/></patternFill></fill></dxf>"#,
            r#"<dxf><font><color rgb="FF7F7F7F"/></font></dxf></x14:dxfs></ext></extLst></styleSheet>"#
        );
        let ss = read_styles(xml.as_bytes(), "styles.xml").unwrap();
        let styles = frame_styles(&ss);
        assert_eq!(styles.len(), 1);
        let s = &styles[0];
        assert_eq!((s.name.as_str(), s.timeline), ("Mine", false));
        let header = s.header.as_ref().and_then(|d| d.font.as_ref()).unwrap();
        assert!(header.bold);
        assert_eq!(
            header.color.as_ref().and_then(|c| c.rgb.as_deref()),
            Some("FF203864")
        );
        let edge = &s
            .whole
            .as_ref()
            .and_then(|d| d.border.as_ref())
            .unwrap()
            .left;
        assert_eq!(
            edge.color.as_ref().and_then(|c| c.rgb.as_deref()),
            Some("FF70AD47")
        );
        let Some(Fill::Pattern(p)) = s.selected.as_ref().and_then(|d| d.fill.as_ref()) else {
            panic!("selected item fill");
        };
        assert_eq!(
            p.bg.as_ref().and_then(|c| c.rgb.as_deref()),
            Some("FFC00000")
        );
        assert!(s
            .unselected
            .as_ref()
            .and_then(|d| d.font.as_ref())
            .is_some());
        // The styles part is written back with the custom style untouched.
        let back = write_styles(&ss);
        assert!(back.contains(r#"<x14:slicerStyle name="Mine">"#));
        assert!(back.contains(r#"<tableStyle name="Mine""#));
    }
}
