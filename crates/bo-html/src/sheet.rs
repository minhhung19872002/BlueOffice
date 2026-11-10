//! Worksheets ↔ HTML tables.
//!
//! Saving writes one `<table>` per sheet (HTML Living Standard §4.9 "Tabular
//! data"): the displayed text of every cell with its font, fill, alignment and
//! borders as CSS, merged cells as `colspan` / `rowspan`. The value behind the text
//! travels in `data-` attributes (`data-num`, `data-bool`, `data-formula`,
//! `data-fmt`), so a page saved here opens again with numbers, formulas and number
//! formats intact; any other page opens with numbers recognised from the text.

use crate::css::{self, Decls, StyleSheet};
use crate::dom::{self, Element, Node};
use crate::entities::{escape, escape_attr};
use bo_model::sheet::{
    Border, BorderEdge, Cell, CellAlignment, CellRange, CellRef, CellValue, CellXf, ColInfo, Fill,
    Font, RowInfo, SheetColor, Workbook, Worksheet,
};
use bo_sheet::numfmt::Locale;
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

/// Most cells written for one sheet (the used range is cut to fit).
const MAX_WRITE_CELLS: usize = 2_000_000;
/// Limits of a worksheet (ECMA-376 Part 1, §18.3.1.73 row, §18.3.1.4 c).
const MAX_ROWS: u32 = 1_048_576;
const MAX_COLS: u32 = 16_384;
/// Most cells read from one page.
const MAX_READ_CELLS: usize = 4_000_000;

fn rgb(c: &SheetColor, wb: &Workbook) -> Option<String> {
    bo_sheet::color::resolve_color(c, wb.theme.as_ref())
        .map(|c| format!("#{:02X}{:02X}{:02X}", c.0, c.1, c.2))
}

fn edge_css(e: &BorderEdge, wb: &Workbook) -> Option<String> {
    let style = e.style.as_deref()?;
    let (width, line) = match style {
        "none" => return None,
        "medium" => ("2px", "solid"),
        "thick" => ("3px", "solid"),
        "double" => ("3px", "double"),
        "hair" | "dotted" => ("1px", "dotted"),
        "dashed" | "dashDot" | "dashDotDot" | "slantDashDot" => ("1px", "dashed"),
        "mediumDashed" | "mediumDashDot" | "mediumDashDotDot" => ("2px", "dashed"),
        _ => ("1px", "solid"),
    };
    let color = e
        .color
        .as_ref()
        .and_then(|c| rgb(c, wb))
        .unwrap_or_else(|| "#000000".into());
    Some(format!("{width} {line} {color}"))
}

/// CSS of a cell format, relative to the workbook's default font.
fn xf_css(wb: &Workbook, xf: &CellXf, value: &CellValue) -> String {
    let mut css = String::new();
    let st = &wb.styles;
    let base = st.fonts.first();
    if let Some(f) = st.fonts.get(xf.font_id as usize) {
        if f.name.is_some() && f.name != base.and_then(|b| b.name.clone()) {
            let name: String = f
                .name
                .as_deref()
                .unwrap_or("")
                .chars()
                .filter(|c| !matches!(c, '\'' | '"' | '<' | '>' | ';' | '\\'))
                .collect();
            let _ = write!(css, "font-family:'{name}';");
        }
        if let Some(s) = f.size.filter(|s| Some(*s) != base.and_then(|b| b.size)) {
            let _ = write!(css, "font-size:{s}pt;");
        }
        if f.bold {
            css.push_str("font-weight:bold;");
        }
        if f.italic {
            css.push_str("font-style:italic;");
        }
        let under = f.underline.as_deref().is_some_and(|u| u != "none");
        if under || f.strike {
            let _ = write!(
                css,
                "text-decoration:{}{}{};",
                if under { "underline" } else { "" },
                if under && f.strike { " " } else { "" },
                if f.strike { "line-through" } else { "" }
            );
        }
        if let Some(c) = f.color.as_ref().and_then(|c| rgb(c, wb)) {
            if c != "#000000" {
                let _ = write!(css, "color:{c};");
            }
        }
    }
    if let Some(Fill::Pattern(p)) = st.fills.get(xf.fill_id as usize) {
        if p.pattern.as_deref() == Some("solid") {
            if let Some(c) = p.fg.as_ref().and_then(|c| rgb(c, wb)) {
                let _ = write!(css, "background-color:{c};");
            }
        }
    }
    let align = xf.alignment.as_ref();
    let h = align
        .and_then(|a| a.horizontal.as_deref())
        .and_then(|h| match h {
            "left" => Some("left"),
            "center" | "centerContinuous" => Some("center"),
            "right" => Some("right"),
            "justify" | "distributed" => Some("justify"),
            _ => None,
        });
    // General alignment: numbers right, booleans and errors centred (§18.18.40).
    let h = h.or(match value {
        CellValue::Number(_) | CellValue::Date(_) => Some("right"),
        CellValue::Bool(_) | CellValue::Error(_) => Some("center"),
        _ => None,
    });
    if let Some(h) = h {
        let _ = write!(css, "text-align:{h};");
    }
    match align.and_then(|a| a.vertical.as_deref()) {
        Some("top") => css.push_str("vertical-align:top;"),
        Some("center") => css.push_str("vertical-align:middle;"),
        _ => {}
    }
    if align.is_some_and(|a| a.wrap_text) {
        css.push_str("white-space:normal;");
    }
    if let Some(b) = st.borders.get(xf.border_id as usize) {
        for (side, e) in [
            ("top", &b.top),
            ("left", &b.left),
            ("bottom", &b.bottom),
            ("right", &b.right),
        ] {
            if let Some(v) = edge_css(e, wb) {
                let _ = write!(css, "border-{side}:{v};");
            }
        }
    }
    css
}

/// The shortest text that parses back to `n`.
fn num_text(n: f64) -> String {
    format!("{n}")
}

fn write_sheet(wb: &Workbook, ws: &Worksheet, locale: &Locale, out: &mut String) {
    let _ = writeln!(
        out,
        "<table class=\"bo-sheet\" data-sheet=\"{}\">\n<caption>{}</caption>",
        escape_attr(&ws.name),
        escape(&ws.name)
    );
    let Some(used) = ws.used_range() else {
        out.push_str("<tr><td></td></tr>\n</table>\n");
        return;
    };
    // The table starts at A1 so that cell addresses survive a round trip.
    let mut last_row = used.last.row;
    let mut last_col = used.last.col;
    for m in &ws.merges {
        last_row = last_row.max(m.last.row.min(MAX_ROWS - 1));
        last_col = last_col.max(m.last.col.min(MAX_COLS - 1));
    }
    let n_cols = (last_col as usize + 1).min(MAX_COLS as usize);
    let n_rows = (last_row as usize + 1).min((MAX_WRITE_CELLS / n_cols).max(1));
    out.push_str("<colgroup>");
    for c in 0..n_cols {
        let width = ws
            .col_info(c as u32)
            .and_then(|i| i.width)
            .or(ws.format.default_col_width)
            .unwrap_or(8.43);
        // Column width in characters → pixels (§18.3.1.13: 7 px digits + 5 px padding).
        let px = (width * 7.0 + 5.0).round().clamp(0.0, 2000.0);
        let _ = write!(out, "<col style=\"width:{px}px\">");
    }
    out.push_str("</colgroup>\n");
    // Merged ranges: spans at the top-left cell, the other cells are not written.
    let mut span_at: HashMap<(u32, u32), (u32, u32)> = HashMap::new();
    let mut covered: HashSet<(u32, u32)> = HashSet::new();
    for m in &ws.merges {
        let (r0, c0) = (m.first.row, m.first.col);
        let r1 = m.last.row.min(n_rows as u32 - 1);
        let c1 = m.last.col.min(n_cols as u32 - 1);
        if r0 > r1 || c0 > c1 || (r1 - r0 + 1) as usize * (c1 - c0 + 1) as usize > MAX_WRITE_CELLS {
            continue;
        }
        span_at.insert((r0, c0), (r1 - r0 + 1, c1 - c0 + 1));
        for r in r0..=r1 {
            for c in c0..=c1 {
                if (r, c) != (r0, c0) && covered.len() < MAX_WRITE_CELLS {
                    covered.insert((r, c));
                }
            }
        }
    }
    let empty = Cell::default();
    for r in 0..n_rows as u32 {
        match ws
            .rows
            .get(&r)
            .and_then(|i| i.height.filter(|_| i.custom_height))
        {
            Some(h) => {
                let _ = write!(out, "<tr style=\"height:{h}pt\">");
            }
            None => out.push_str("<tr>"),
        }
        for c in 0..n_cols as u32 {
            if covered.contains(&(r, c)) {
                continue;
            }
            let cell = ws.cell(r, c).unwrap_or(&empty);
            out.push_str("<td");
            if let Some((rs, cs)) = span_at.get(&(r, c)) {
                if *rs > 1 {
                    let _ = write!(out, " rowspan=\"{rs}\"");
                }
                if *cs > 1 {
                    let _ = write!(out, " colspan=\"{cs}\"");
                }
            }
            match &cell.value {
                CellValue::Number(n) if n.is_finite() => {
                    let _ = write!(out, " data-num=\"{}\"", num_text(*n));
                }
                CellValue::Bool(b) => {
                    let _ = write!(out, " data-bool=\"{}\"", u8::from(*b));
                }
                _ => {}
            }
            if let Some(f) = &cell.formula {
                if !f.text.is_empty() {
                    let _ = write!(out, " data-formula=\"={}\"", escape_attr(&f.text));
                }
            }
            if let Some(xf) = wb.styles.cell_xfs.get(cell.style as usize) {
                if let Some(code) = wb.styles.custom_num_fmt(xf.num_fmt_id) {
                    let _ = write!(out, " data-fmt=\"{}\"", escape_attr(code));
                } else if xf.num_fmt_id != 0 {
                    let _ = write!(out, " data-fmt-id=\"{}\"", xf.num_fmt_id);
                }
                let css = xf_css(wb, xf, &cell.value);
                if !css.is_empty() {
                    let _ = write!(out, " style=\"{}\"", escape_attr(&css));
                }
            }
            out.push('>');
            let text = bo_sheet::render::format_cell(wb, cell, locale).text;
            out.push_str(&escape(&text).replace('\n', "<br>"));
            out.push_str("</td>");
        }
        out.push_str("</tr>\n");
    }
    out.push_str("</table>\n");
}

/// Save a workbook (`sheet`: only that sheet) as an HTML page of tables.
pub fn write_workbook_html(
    wb: &Workbook,
    sheet: Option<usize>,
    locale: &Locale,
    title: &str,
) -> String {
    let mut out = String::from("<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n");
    out.push_str("<meta name=\"generator\" content=\"BlueOffice\">\n");
    let _ = writeln!(out, "<title>{}</title>", escape(title));
    let base = wb.styles.fonts.first();
    let _ = writeln!(
        out,
        "<style>\nbody{{font-family:'{}';font-size:{}pt}}\ntable.bo-sheet{{border-collapse:collapse;table-layout:fixed;margin-bottom:12pt}}\ncaption{{text-align:left;font-weight:bold;padding:3pt 0}}\ntable.bo-sheet td{{border:1px solid #D0D0D0;padding:1px 3px;white-space:nowrap;overflow:hidden;vertical-align:bottom}}\n</style>\n</head>\n<body>",
        base.and_then(|f| f.name.as_deref())
            .unwrap_or("Arial")
            .replace(['\'', '"', '<', '>', ';', '\\'], ""),
        base.and_then(|f| f.size).unwrap_or(11.0)
    );
    for (i, ws) in wb.sheets.iter().enumerate() {
        if sheet.is_none_or(|s| s == i) {
            write_sheet(wb, ws, locale, &mut out);
        }
    }
    out.push_str("</body>\n</html>\n");
    out
}

// ------------------------------------------------------------------ reading

/// Text of a cell: white space collapsed, `<br>` and block boundaries as line breaks.
fn cell_text(e: &Element) -> String {
    fn walk(e: &Element, out: &mut String, depth: usize) {
        for n in &e.children {
            match n {
                Node::Text(t) => {
                    for c in t.chars() {
                        if c.is_whitespace() && c != '\u{a0}' {
                            if !out.ends_with([' ', '\n']) && !out.is_empty() {
                                out.push(' ');
                            }
                        } else {
                            out.push(c);
                        }
                    }
                }
                Node::Element(c) => {
                    let block = matches!(
                        c.name.as_str(),
                        "p" | "div" | "li" | "tr" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
                    );
                    if c.name == "br" || (block && !out.is_empty()) {
                        while out.ends_with(' ') {
                            out.pop();
                        }
                        if c.name == "br" || !out.ends_with('\n') {
                            out.push('\n');
                        }
                    }
                    if !matches!(c.name.as_str(), "style" | "title") && depth < dom::MAX_DEPTH + 8 {
                        walk(c, out, depth + 1);
                    }
                }
            }
        }
    }
    let mut out = String::new();
    walk(e, &mut out, 0);
    out.trim_matches(|c: char| c == ' ' || c == '\n')
        .replace('\u{a0}', " ")
        .trim()
        .to_owned()
}

/// A plain decimal number as a spreadsheet would read it (no leading zeros, at
/// most 15 significant digits — longer digit strings are identifiers).
fn parse_number(s: &str) -> Option<f64> {
    let body = s.strip_prefix(['-', '+']).unwrap_or(s);
    if body.is_empty() || body.len() > 24 {
        return None;
    }
    let (mantissa, exp) = match body.find(['e', 'E']) {
        Some(i) => (&body[..i], Some(&body[i + 1..])),
        None => (body, None),
    };
    let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = |x: &str| !x.is_empty() && x.bytes().all(|b| b.is_ascii_digit());
    if !digits(int) || (mantissa.contains('.') && !digits(frac)) {
        return None;
    }
    if let Some(e) = exp {
        if !digits(e.strip_prefix(['-', '+']).unwrap_or(e)) {
            return None;
        }
    }
    if (int.len() > 1 && int.starts_with('0')) || int.len() + frac.len() > 15 {
        return None;
    }
    s.parse::<f64>().ok().filter(|n| n.is_finite())
}

fn sheet_color(v: &str) -> Option<SheetColor> {
    css::color(v).map(|c| SheetColor::rgb(&c.to_hex()))
}

fn edge_from_css(d: &Decls, side: &str) -> BorderEdge {
    let v =
        d.0.iter()
            .rev()
            .find(|(k, _)| *k == format!("border-{side}") || k == "border")
            .map(|(_, v)| v.to_ascii_lowercase());
    let Some(v) = v else {
        return BorderEdge::default();
    };
    let mut width = 1.0;
    let mut line = "";
    let mut color = None;
    for tok in v.split_ascii_whitespace() {
        match tok {
            "none" | "hidden" => return BorderEdge::default(),
            "solid" | "double" | "dashed" | "dotted" => line = tok,
            "thin" => width = 1.0,
            "medium" => width = 2.0,
            "thick" => width = 3.0,
            _ => {
                if let Some(c) = sheet_color(tok) {
                    color = Some(c);
                } else if let Some(pt) = css::length_pt(tok, 11.0) {
                    width = pt / 0.75;
                }
            }
        }
    }
    if line.is_empty() || width <= 0.0 {
        return BorderEdge::default();
    }
    let style = match line {
        "double" => "double",
        "dashed" if width >= 1.5 => "mediumDashed",
        "dashed" => "dashed",
        "dotted" => "dotted",
        _ if width >= 2.5 => "thick",
        _ if width >= 1.5 => "medium",
        _ => "thin",
    };
    BorderEdge {
        style: Some(style.into()),
        color,
    }
}

struct Reader {
    css: StyleSheet,
    wb: Workbook,
    cells: usize,
    /// Cell formats already built, by what determined them.
    formats: HashMap<String, u32>,
}

impl Reader {
    /// Cell format of a `td` / `th` from its CSS and presentational markup.
    fn style_of(&mut self, td: &Element, tr: &Decls, table: &Decls) -> u32 {
        let own = self.css.computed(td);
        let mut d = table.clone();
        d.extend(tr);
        d.extend(&own);
        let inner = td.descendants();
        let has = |tag: &[&str]| inner.iter().skip(1).any(|e| tag.contains(&e.name.as_str()));
        let key = format!(
            "{}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{}{}{}{}",
            td.name,
            d.0,
            td.attr("bgcolor"),
            td.attr("align"),
            td.attr("valign"),
            td.attr("data-fmt"),
            td.attr("data-fmt-id"),
            u8::from(has(&["b", "strong"])),
            u8::from(has(&["i", "em"])),
            u8::from(has(&["u"])),
            u8::from(has(&["s", "strike", "del"])),
        );
        if let Some(&xf) = self.formats.get(&key) {
            return xf;
        }
        let xf = self.build_format(td, &own, &d, &has);
        if self.formats.len() < 100_000 {
            self.formats.insert(key, xf);
        }
        xf
    }

    fn build_format(
        &mut self,
        td: &Element,
        own: &Decls,
        d: &Decls,
        has: &dyn Fn(&[&str]) -> bool,
    ) -> u32 {
        let base = self.wb.styles.fonts.first().cloned().unwrap_or_default();
        let mut font = Font {
            scheme: None,
            ..base.clone()
        };
        let weight = d.get("font-weight").map(str::to_ascii_lowercase);
        font.bold = match weight.as_deref() {
            Some("bold" | "bolder") => true,
            Some("normal" | "lighter") => false,
            Some(n) => n.parse::<f64>().is_ok_and(|w| w >= 600.0),
            None => td.name == "th",
        } || has(&["b", "strong"]);
        font.italic = d
            .get("font-style")
            .is_some_and(|v| v.eq_ignore_ascii_case("italic") || v.eq_ignore_ascii_case("oblique"))
            || has(&["i", "em"]);
        let deco = d
            .get("text-decoration")
            .or_else(|| d.get("text-decoration-line"))
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        if deco.contains("underline") || has(&["u"]) {
            font.underline = Some("single".into());
        }
        font.strike = deco.contains("line-through") || has(&["s", "strike", "del"]);
        if let Some(c) = d.get("color").and_then(sheet_color) {
            font.color = Some(c);
        }
        if let Some(pt) = d.get("font-size").and_then(|v| css::font_size_pt(v, 11.0)) {
            font.size = Some((pt * 2.0).round() / 2.0);
        }
        if let Some(f) = d.get("font-family").and_then(css::font_family) {
            font.name = Some(f);
        }
        let font = if (Font {
            scheme: base.scheme.clone(),
            ..font.clone()
        }) == base
        {
            base
        } else {
            font
        };
        let fill = d
            .get("background-color")
            .or_else(|| d.get("background"))
            .and_then(|v| v.split_ascii_whitespace().find_map(css::color))
            .or_else(|| td.attr("bgcolor").and_then(css::color));
        let mut align = CellAlignment::default();
        let h = d
            .get("text-align")
            .or_else(|| td.attr("align"))
            .map(str::to_ascii_lowercase);
        align.horizontal = match h.as_deref() {
            Some("left") => Some("left".into()),
            Some("center") => Some("center".into()),
            Some("right") => Some("right".into()),
            Some("justify") => Some("justify".into()),
            None if td.name == "th" => Some("center".into()),
            _ => None,
        };
        let v = d
            .get("vertical-align")
            .or_else(|| td.attr("valign"))
            .map(str::to_ascii_lowercase);
        align.vertical = match v.as_deref() {
            Some("top") => Some("top".into()),
            Some("middle" | "center") => Some("center".into()),
            _ => None,
        };
        align.wrap_text = own.get("white-space").is_some_and(|v| {
            v.eq_ignore_ascii_case("normal") || v.eq_ignore_ascii_case("pre-wrap")
        });
        let border = Border {
            top: edge_from_css(own, "top"),
            left: edge_from_css(own, "left"),
            bottom: edge_from_css(own, "bottom"),
            right: edge_from_css(own, "right"),
            ..Border::default()
        };
        let num_fmt_id = match (td.attr("data-fmt"), td.attr("data-fmt-id")) {
            (Some(code), _) if !code.is_empty() && code.len() <= 255 => {
                self.wb.styles.add_num_fmt(code)
            }
            (_, Some(id)) => id.parse::<u32>().ok().filter(|i| *i < 164).unwrap_or(0),
            _ => 0,
        };
        let st = &mut self.wb.styles;
        // The style tables stay bounded whatever the page declares.
        if st.cell_xfs.len() >= 4000 {
            return 0;
        }
        let font_id = st.add_font(font);
        let fill_id = fill.map_or(0, |c| st.add_fill(Fill::solid(&c.to_hex())));
        let border_id = st.add_border(border);
        let has_align = align != CellAlignment::default();
        st.add_xf(CellXf {
            num_fmt_id,
            font_id,
            fill_id,
            border_id,
            xf_id: Some(0),
            apply_number_format: (num_fmt_id != 0).then_some(true),
            apply_font: (font_id != 0).then_some(true),
            apply_fill: (fill_id != 0).then_some(true),
            apply_border: (border_id != 0).then_some(true),
            apply_alignment: has_align.then_some(true),
            alignment: has_align.then_some(align),
            ..CellXf::default()
        })
    }

    fn table(&mut self, t: &Element, name: &str) -> Worksheet {
        let mut ws = Worksheet::new(name);
        let table_decls = {
            // Only inherited text properties of the table reach its cells.
            let all = self.css.computed(t);
            Decls(
                all.0
                    .into_iter()
                    .filter(|(k, _)| k.starts_with("font") || k == "color" || k == "text-align")
                    .collect(),
            )
        };
        // Column widths.
        let mut col = 0u32;
        for c in t.descendants().into_iter().filter(|e| e.name == "col") {
            let span = c
                .attr("span")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(1)
                .clamp(1, 1000);
            let d = self.css.computed(c);
            let px = d
                .get("width")
                .or_else(|| c.attr("width"))
                .filter(|v| !v.contains('%'))
                .and_then(|v| css::length_pt(v, 11.0))
                .map(|pt| pt / 0.75);
            if let Some(px) = px.filter(|p| *p > 5.0) {
                if col + span <= MAX_COLS && ws.cols.len() < 1024 {
                    ws.cols.push(ColInfo {
                        min: col,
                        max: col + span - 1,
                        width: Some(((px - 5.0) / 7.0 * 100.0).round() / 100.0),
                        custom_width: true,
                        ..ColInfo::default()
                    });
                }
            }
            col = col.saturating_add(span);
        }
        // Rows of this table only (not of tables nested in its cells).
        let mut rows: Vec<&Element> = Vec::new();
        for c in t.elements() {
            match c.name.as_str() {
                "tr" => rows.push(c),
                "thead" | "tbody" | "tfoot" => rows.extend(c.elements().filter(|r| r.name == "tr")),
                _ => {}
            }
        }
        let mut occupied: HashSet<(u32, u32)> = HashSet::new();
        for (r, tr) in rows.iter().enumerate() {
            let Ok(r) = u32::try_from(r) else { break };
            if r >= MAX_ROWS {
                break;
            }
            let tr_all = self.css.computed(tr);
            let tr_decls = Decls(
                tr_all
                    .0
                    .iter()
                    .filter(|(k, _)| {
                        k.starts_with("font")
                            || k.starts_with("background")
                            || k == "color"
                            || k == "text-align"
                            || k == "vertical-align"
                    })
                    .cloned()
                    .collect(),
            );
            if let Some(h) = tr_all.get("height").and_then(|v| css::length_pt(v, 11.0)) {
                if h > 0.0 && h <= 409.0 {
                    ws.rows.insert(
                        r,
                        RowInfo {
                            height: Some(h),
                            custom_height: true,
                            ..RowInfo::default()
                        },
                    );
                }
            }
            let mut c = 0u32;
            for td in tr
                .elements()
                .filter(|e| matches!(e.name.as_str(), "td" | "th"))
            {
                while occupied.contains(&(r, c)) {
                    c += 1;
                }
                if c >= MAX_COLS || self.cells >= MAX_READ_CELLS {
                    break;
                }
                self.cells += 1;
                let span = |name: &str, max: u32| {
                    td.attr(name)
                        .and_then(|v| v.trim().parse::<u32>().ok())
                        .unwrap_or(1)
                        .clamp(1, max)
                };
                let cs = span("colspan", (MAX_COLS - c).min(1024));
                let rs = span("rowspan", (rows.len() as u32 - r).clamp(1, 4096));
                if cs > 1 || rs > 1 {
                    if occupied.len() + (cs * rs) as usize <= MAX_READ_CELLS {
                        for rr in r..r + rs {
                            for cc in c..c + cs {
                                occupied.insert((rr, cc));
                            }
                        }
                    }
                    ws.merges.push(CellRange::new(
                        CellRef { row: r, col: c },
                        CellRef {
                            row: r + rs - 1,
                            col: c + cs - 1,
                        },
                    ));
                }
                let text = cell_text(td);
                let value = if let Some(n) = td
                    .attr("data-num")
                    .or_else(|| td.attr("x:num").filter(|v| !v.is_empty()))
                    .and_then(|v| v.trim().parse::<f64>().ok())
                    .filter(|n| n.is_finite())
                {
                    CellValue::Number(n)
                } else if let Some(b) = td.attr("data-bool") {
                    CellValue::Bool(b.trim() == "1")
                } else if text.is_empty() {
                    CellValue::Empty
                } else if let Some(n) = parse_number(&text) {
                    CellValue::Number(n)
                } else {
                    CellValue::Text(text.chars().take(32_767).collect())
                };
                let style = self.style_of(td, &tr_decls, &table_decls);
                let formula = td
                    .attr("data-formula")
                    .and_then(|f| f.strip_prefix('='))
                    .filter(|f| !f.is_empty() && f.len() <= 8192 && bo_formula::parse(f).is_ok());
                if value != CellValue::Empty || style != 0 || formula.is_some() {
                    let mut cell = Cell::new(value).with_style(style);
                    if let Some(f) = formula {
                        cell = cell.with_formula(f);
                    }
                    ws.set_cell(r, c, cell);
                }
                c += cs;
            }
        }
        ws
    }
}

/// A valid, unique sheet name (§18.2.19: at most 31 characters, none of `[]:*?/\`).
fn sheet_name(raw: &str, index: usize, taken: &[String]) -> String {
    let clean: String = raw
        .trim()
        .chars()
        .filter(|c| !matches!(c, '[' | ']' | ':' | '*' | '?' | '/' | '\\') && !c.is_control())
        .take(31)
        .collect();
    let clean = clean.trim_matches('\'').trim().to_owned();
    let mut name = if clean.is_empty() {
        format!("Sheet{}", index + 1)
    } else {
        clean
    };
    let mut n = 2;
    while taken.iter().any(|t| t.eq_ignore_ascii_case(&name)) {
        let suffix = format!(" ({n})");
        let stem = name.rsplit_once(" (").map_or(name.as_str(), |(a, _)| a);
        let keep: String = stem.chars().take(31 - suffix.chars().count()).collect();
        name = format!("{keep}{suffix}");
        n += 1;
        if n > 10_000 {
            break;
        }
    }
    name
}

/// Open an HTML page as a workbook: one sheet per top-level table (a page without
/// tables becomes one sheet with a line of text per row).
pub fn read_workbook_html(bytes: &[u8]) -> Workbook {
    read_workbook_html_str(&bo_encoding::web::decode_html(bytes, None))
}

/// [`read_workbook_html`] for decoded text.
pub fn read_workbook_html_str(text: &str) -> Workbook {
    let root = dom::parse(text);
    let mut css = StyleSheet::default();
    for e in root.descendants() {
        if e.name == "style" {
            css.add(&e.text());
        }
    }
    let mut reader = Reader {
        css,
        wb: Workbook::new(),
        cells: 0,
        formats: HashMap::new(),
    };
    reader.wb.sheets.clear();
    // Top-level tables: depth-first, not descending into a table.
    let mut tables: Vec<&Element> = Vec::new();
    let mut stack: Vec<&Element> = vec![&root];
    while let Some(e) = stack.pop() {
        if e.name == "table" {
            tables.push(e);
            continue;
        }
        let kids: Vec<&Element> = e.elements().collect();
        stack.extend(kids.into_iter().rev());
    }
    let mut names: Vec<String> = Vec::new();
    for (i, t) in tables.iter().take(255).enumerate() {
        let raw = t
            .attr("data-sheet")
            .map(str::to_owned)
            .or_else(|| t.elements().find(|c| c.name == "caption").map(cell_text))
            .unwrap_or_default();
        let name = sheet_name(&raw, i, &names);
        names.push(name.clone());
        let mut ws = reader.table(t, &name);
        ws.sheet_id = i as u32 + 1;
        reader.wb.sheets.push(ws);
    }
    if reader.wb.sheets.is_empty() {
        let mut ws = Worksheet::new("Sheet1");
        ws.sheet_id = 1;
        let body = root.find("body").unwrap_or(&root);
        for (r, line) in cell_text(body)
            .lines()
            .filter(|l| !l.trim().is_empty())
            .take(MAX_ROWS as usize)
            .enumerate()
        {
            let line = line.trim();
            let value = parse_number(line).map_or_else(
                || CellValue::Text(line.chars().take(32_767).collect()),
                CellValue::Number,
            );
            ws.set_cell(r as u32, 0, Cell::new(value));
        }
        reader.wb.sheets.push(ws);
    }
    reader.wb
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::sheet::Formula;

    #[test]
    fn numbers_are_recognised_conservatively() {
        assert_eq!(parse_number("42"), Some(42.0));
        assert_eq!(parse_number("-3.50"), Some(-3.5));
        assert_eq!(parse_number("1e3"), Some(1000.0));
        for s in [
            "007",
            "1,234",
            "12%",
            "1.",
            ".5",
            "0123456789012345678",
            "NaN",
            "inf",
            "1e",
            "+",
            "",
        ] {
            assert_eq!(parse_number(s), None, "{s}");
        }
    }

    #[test]
    fn reads_tables_as_sheets() {
        let wb = read_workbook_html_str(
            "<style>.h{font-weight:bold;background:#FFCC00}</style>
             <table><caption>Doanh thu [Q1]</caption>
              <tr class=h><th>Tháng</th><th colspan=2>Số liệu</th></tr>
              <tr><td rowspan=2>Một<br>Hai</td><td align=right>1500</td><td>2.5</td></tr>
              <tr><td style='color:red'><i>x</i></td><td>007</td></tr>
              <tr><td><table><tr><td>lồng</td></tr></table></td></tr>
             </table>
             <p>giữa</p>
             <table><tr><td>B1</td></table>",
        );
        assert_eq!(wb.sheets.len(), 2);
        let s = &wb.sheets[0];
        assert_eq!(s.name, "Doanh thu Q1");
        assert_eq!(wb.sheets[1].name, "Sheet2");
        assert_eq!(
            s.cell_a1("A1").unwrap().value,
            CellValue::Text("Tháng".into())
        );
        assert_eq!(
            s.cell_a1("A2").unwrap().value,
            CellValue::Text("Một\nHai".into())
        );
        assert_eq!(s.cell_a1("B2").unwrap().value, CellValue::Number(1500.0));
        assert_eq!(s.cell_a1("C2").unwrap().value, CellValue::Number(2.5));
        // The row-spanned column is skipped in the next row.
        assert_eq!(s.cell_a1("B3").unwrap().value, CellValue::Text("x".into()));
        assert_eq!(
            s.cell_a1("C3").unwrap().value,
            CellValue::Text("007".into())
        );
        assert_eq!(
            s.cell_a1("A4").unwrap().value,
            CellValue::Text("lồng".into())
        );
        assert_eq!(s.merges.len(), 2);
        assert_eq!(s.merges[0].to_string(), "B1:C1");
        assert_eq!(s.merges[1].to_string(), "A2:A3");
        let xf = |a1: &str| wb.styles.xf(s.cell_a1(a1).unwrap().style).unwrap().clone();
        let head = xf("A1");
        assert!(wb.styles.fonts[head.font_id as usize].bold);
        assert_eq!(
            wb.styles.fills[head.fill_id as usize],
            Fill::solid("FFCC00")
        );
        let red = xf("B3");
        let f = &wb.styles.fonts[red.font_id as usize];
        assert!(f.italic);
        assert_eq!(f.color, Some(SheetColor::rgb("FF0000")));
        assert_eq!(
            xf("B2").alignment.unwrap().horizontal.as_deref(),
            Some("right")
        );
    }

    #[test]
    fn workbook_round_trips_through_html() {
        let mut wb = Workbook::new();
        let pct = wb.styles.add_num_fmt("0.00%");
        let bold = wb.styles.add_font(Font {
            name: Some("Calibri".into()),
            size: Some(11.0),
            bold: true,
            ..Font::default()
        });
        let fill = wb.styles.add_fill(Fill::solid("DDEBF7"));
        let style = wb.styles.add_xf(CellXf {
            num_fmt_id: pct,
            font_id: bold,
            fill_id: fill,
            xf_id: Some(0),
            ..CellXf::default()
        });
        let ws = &mut wb.sheets[0];
        ws.name = "Số liệu".into();
        ws.set_a1("A1", CellValue::Text("Tên & <họ>".into()));
        ws.set_a1("B1", CellValue::Number(0.125));
        ws.cells.get_mut(&(0, 1)).unwrap().style = style;
        ws.set_a1("A2", CellValue::Number(1234.5678));
        ws.set_a1("B2", CellValue::Bool(true));
        ws.set_cell(
            2,
            0,
            Cell {
                value: CellValue::Number(1234.6928),
                formula: Some(Formula::normal("SUM(A2:B1)+\"x<y\"")),
                ..Cell::default()
            },
        );
        ws.merges.push(CellRange::parse("A4:C5").unwrap());
        ws.set_a1("A4", CellValue::Text("gộp".into()));
        let mut second = Worksheet::new("Hai");
        second.set_a1("C3", CellValue::Text("xa".into()));
        wb.sheets.push(second);

        let html = write_workbook_html(&wb, None, &Locale::EN_US, "t");
        assert!(html.contains(">12.50%</td>"), "{html}");
        assert!(html.contains("rowspan=\"2\" colspan=\"3\""));
        let back = read_workbook_html_str(&html);
        assert_eq!(back.sheets.len(), 2);
        let s = &back.sheets[0];
        assert_eq!(s.name, "Số liệu");
        assert_eq!(
            s.cell_a1("A1").unwrap().value,
            CellValue::Text("Tên & <họ>".into())
        );
        assert_eq!(s.cell_a1("B1").unwrap().value, CellValue::Number(0.125));
        assert_eq!(s.cell_a1("A2").unwrap().value, CellValue::Number(1234.5678));
        assert_eq!(s.cell_a1("B2").unwrap().value, CellValue::Bool(true));
        let f = s.cell_a1("A3").unwrap();
        assert_eq!(f.formula.as_ref().unwrap().text, "SUM(A2:B1)+\"x<y\"");
        assert_eq!(f.value, CellValue::Number(1234.6928));
        assert_eq!(s.merges, vec![CellRange::parse("A4:C5").unwrap()]);
        let xf = back.styles.xf(s.cell_a1("B1").unwrap().style).unwrap();
        assert_eq!(back.styles.custom_num_fmt(xf.num_fmt_id), Some("0.00%"));
        assert!(back.styles.fonts[xf.font_id as usize].bold);
        assert_eq!(
            back.styles.fills[xf.fill_id as usize],
            Fill::solid("DDEBF7")
        );
        assert_eq!(
            back.sheets[1].cell_a1("C3").unwrap().value,
            CellValue::Text("xa".into())
        );
        // One sheet only.
        let one = read_workbook_html_str(&write_workbook_html(&wb, Some(1), &Locale::EN_US, "t"));
        assert_eq!(one.sheets.len(), 1);
        assert_eq!(one.sheets[0].name, "Hai");
    }

    #[test]
    fn pages_without_tables_and_hostile_spans() {
        let wb = read_workbook_html_str("<h1>Tiêu đề</h1><p>12</p><p>dòng</p>");
        let s = &wb.sheets[0];
        assert_eq!(
            s.cell_a1("A1").unwrap().value,
            CellValue::Text("Tiêu đề".into())
        );
        assert_eq!(s.cell_a1("A2").unwrap().value, CellValue::Number(12.0));
        let wb = read_workbook_html_str(
            "<table><tr><td colspan=999999999 rowspan=999999999>a<td>b</table>",
        );
        assert!(wb.sheets[0].cells.len() <= 2);
        let _ = read_workbook_html_str("");
    }
}
