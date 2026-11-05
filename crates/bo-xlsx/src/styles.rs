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
    };
    if !empty {
        for_children(r, |_, e, _| {
            match lname(e).as_str() {
                "alignment" => xf.alignment = Some(read_alignment(e)),
                "protection" => xf.protection = Some(read_protection(e)),
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
    if xf.alignment.is_none() && xf.protection.is_none() {
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
