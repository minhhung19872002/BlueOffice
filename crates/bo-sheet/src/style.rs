//! Cell formatting edits (ECMA-376 Part 1, §18.8 Styles).
//!
//! Every edit goes through the `cellXfs` table (§18.8.10): the current `xf` of a
//! cell is copied, the requested parts are changed (font §18.8.22, fill §18.8.20,
//! border §18.8.4, alignment §18.8.1, number format §18.8.30) and the result is
//! added through the deduplicating `Stylesheet::add_*` helpers, so applying the
//! same formatting to many cells produces one record.
//!
//! The named cell styles of the gallery (§18.8.7 `cellStyle`, §18.8.9
//! `cellStyleXfs`) use BlueSheet's own palette.

use crate::numfmt::{builtin_format, format_code, Locale};
use bo_model::sheet::{
    Border, BorderEdge, CellAlignment, CellProtection, CellRange, CellStyle, CellXf, Fill, Font,
    SheetColor, Stylesheet, Workbook, Worksheet,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Maximum number of cells a single formatting edit may touch.
pub const MAX_FORMAT_CELLS: u64 = 2_000_000;

/// Partial formatting change: `None` fields stay as they are.
///
/// Colours are `RRGGBB`; an empty string means "automatic" (font colour) or
/// "no fill". Alignment strings use the spec values (§18.18.40 / §18.18.88);
/// an empty string resets to `general` / bottom.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StylePatch {
    /// Number format code (§18.8.31), e.g. `#,##0 [$₫-42A]` or `dd/mm/yyyy`.
    pub num_fmt: Option<String>,
    /// Font typeface (§18.8.29 `name`).
    pub font_name: Option<String>,
    /// Font size in points (§18.4.11 `sz`).
    pub font_size: Option<f64>,
    /// Bold (`b`).
    pub bold: Option<bool>,
    /// Italic (`i`).
    pub italic: Option<bool>,
    /// `none` / `single` / `double` (§18.18.82 `ST_UnderlineValues`).
    pub underline: Option<String>,
    /// Strikethrough (`strike`).
    pub strike: Option<bool>,
    /// Font colour `RRGGBB` (empty = automatic).
    pub font_color: Option<String>,
    /// Solid fill colour `RRGGBB` (empty = no fill).
    pub fill_color: Option<String>,
    /// Horizontal alignment (§18.18.40 `ST_HorizontalAlignment`).
    pub h_align: Option<String>,
    /// Vertical alignment (§18.18.88 `ST_VerticalAlignment`).
    pub v_align: Option<String>,
    /// Wrap text (`wrapText`).
    pub wrap: Option<bool>,
    /// Indent level (`indent`, steps of three spaces).
    pub indent: Option<u32>,
    /// `textRotation` (0–180, 255 = vertical text).
    pub rotation: Option<u32>,
    /// `shrinkToFit` (§18.8.1): scale the font so the text fits the cell width.
    pub shrink: Option<bool>,
    /// `protection/@locked` (§18.8.33): edits are refused while the sheet is protected.
    pub locked: Option<bool>,
    /// `protection/@hidden` (§18.8.33): the formula is not shown while protected.
    pub formula_hidden: Option<bool>,
}

/// Which edges a border command draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BorderPreset {
    /// Outer edges of the range.
    Outline,
    /// Every cell edge inside and around the range.
    All,
    /// Only the edges between cells.
    Inside,
    /// Remove all borders.
    None,
    /// Top edge of the range.
    Top,
    /// Bottom edge of the range.
    Bottom,
    /// Left edge of the range.
    Left,
    /// Right edge of the range.
    Right,
}

/// A border command: preset, line style (§18.18.3 `ST_BorderStyle`) and colour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BorderSpec {
    /// Which edges are drawn.
    pub preset: BorderPreset,
    /// Line style, e.g. `thin`, `medium`, `dashed` (default `thin`).
    #[serde(default = "thin")]
    pub style: String,
    /// `RRGGBB`; `None` = automatic (black).
    #[serde(default)]
    pub color: Option<String>,
}

fn thin() -> String {
    "thin".into()
}

/// The formatting of one cell, as shown by the ribbon and the Format Cells dialog.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatState {
    /// Number format code of the cell (§18.8.31).
    pub num_fmt: String,
    /// Font typeface.
    pub font_name: String,
    /// Font size in points.
    pub font_size: f64,
    /// Bold.
    pub bold: bool,
    /// Italic.
    pub italic: bool,
    /// Underline (`none` / `single` / `double`).
    pub underline: String,
    /// Strikethrough.
    pub strike: bool,
    /// Font colour `RRGGBB` (empty = automatic).
    pub font_color: String,
    /// Fill colour `RRGGBB` (empty = no fill).
    pub fill_color: String,
    /// Horizontal alignment (empty = `general`).
    pub h_align: String,
    /// Vertical alignment (empty = bottom).
    pub v_align: String,
    /// Wrap text.
    pub wrap: bool,
    /// Indent level.
    pub indent: u32,
    /// Text rotation (0–180, 255 = vertical).
    pub rotation: u32,
    /// Shrink to fit (`shrinkToFit`).
    pub shrink: bool,
    /// Line style of each edge (`""` = none): top, right, bottom, left.
    pub borders: [String; 4],
    /// Name of the cell style (`cellStyles`) the cell is based on.
    pub style_name: String,
    /// The cell is part of a merged range.
    pub merged: bool,
    /// `protection/@locked` (§18.8.33, default true).
    pub locked: bool,
    /// `protection/@hidden` (§18.8.33, default false).
    pub formula_hidden: bool,
}

/// `RRGGBB` → colour, `""` → `None`.
fn color_of(hex: &str) -> Option<SheetColor> {
    let h = hex.trim().trim_start_matches('#');
    (h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| SheetColor::rgb(&h.to_ascii_uppercase()))
}

fn hex_of(c: Option<&SheetColor>) -> String {
    c.and_then(|c| c.rgb.as_deref())
        .map(|s| s.get(s.len().saturating_sub(6)..).unwrap_or("").to_owned())
        .unwrap_or_default()
}

/// Number format id of a code: a built-in id when the code is one (§18.8.30
/// predefined ids are locale-independent here), otherwise a custom `numFmt`.
pub fn num_fmt_id(styles: &mut Stylesheet, code: &str) -> u32 {
    let code = code.trim();
    if code.is_empty() || code.eq_ignore_ascii_case("general") {
        return 0;
    }
    // Locale-independent built-ins only (ids 1–4, 9–13, 18–21, 37–49).
    for id in (1..=4).chain(9..=13).chain(18..=21).chain(37..=49) {
        if builtin_format(id, &Locale::EN_US).as_deref() == Some(code) {
            return id;
        }
    }
    styles.add_num_fmt(code)
}

fn patch_font(base: &Font, p: &StylePatch) -> Font {
    let mut f = base.clone();
    if let Some(n) = &p.font_name {
        if !n.trim().is_empty() {
            f.name = Some(n.trim().to_owned());
            // A named font is no longer the theme font (§18.8.35 scheme).
            f.scheme = None;
        }
    }
    if let Some(s) = p.font_size {
        if s.is_finite() {
            f.size = Some(s.clamp(1.0, 409.0));
        }
    }
    if let Some(b) = p.bold {
        f.bold = b;
    }
    if let Some(i) = p.italic {
        f.italic = i;
    }
    if let Some(u) = &p.underline {
        f.underline = match u.as_str() {
            "" | "none" => None,
            other => Some(other.to_owned()),
        };
    }
    if let Some(s) = p.strike {
        f.strike = s;
    }
    if let Some(c) = &p.font_color {
        f.color = color_of(c);
    }
    f
}

fn patch_alignment(base: Option<&CellAlignment>, p: &StylePatch) -> Option<CellAlignment> {
    let mut a = base.cloned().unwrap_or_default();
    let opt = |s: &str| (!s.is_empty() && s != "general").then(|| s.to_owned());
    if let Some(h) = &p.h_align {
        a.horizontal = opt(h);
    }
    if let Some(v) = &p.v_align {
        a.vertical = (!v.is_empty() && v != "bottom").then(|| v.clone());
    }
    if let Some(w) = p.wrap {
        a.wrap_text = w;
    }
    if let Some(i) = p.indent {
        a.indent = (i > 0).then_some(i.min(250));
    }
    if let Some(r) = p.rotation {
        a.text_rotation = (r > 0 && (r <= 180 || r == 255)).then_some(r);
    }
    // ECMA-376 Part 1, §18.8.1 alignment/@shrinkToFit.
    if let Some(sh) = p.shrink {
        a.shrink_to_fit = sh;
    }
    (a != CellAlignment::default()).then_some(a)
}

fn touches_font(p: &StylePatch) -> bool {
    p.font_name.is_some()
        || p.font_size.is_some()
        || p.bold.is_some()
        || p.italic.is_some()
        || p.underline.is_some()
        || p.strike.is_some()
        || p.font_color.is_some()
}

fn touches_alignment(p: &StylePatch) -> bool {
    p.h_align.is_some()
        || p.v_align.is_some()
        || p.wrap.is_some()
        || p.indent.is_some()
        || p.rotation.is_some()
        || p.shrink.is_some()
}

/// New `cellXfs` index for `xf` with `patch` applied.
pub fn patched_xf(styles: &mut Stylesheet, xf: u32, patch: &StylePatch) -> u32 {
    let mut x = styles.xf(xf).cloned().unwrap_or_default();
    if let Some(code) = &patch.num_fmt {
        x.num_fmt_id = num_fmt_id(styles, code);
        x.apply_number_format = Some(true);
    }
    if touches_font(patch) {
        let base = styles
            .fonts
            .get(x.font_id as usize)
            .or_else(|| styles.fonts.first())
            .cloned()
            .unwrap_or_default();
        x.font_id = styles.add_font(patch_font(&base, patch));
        x.apply_font = Some(true);
    }
    if let Some(c) = &patch.fill_color {
        // §18.8.20: fills 0 (none) and 1 (gray125) are reserved.
        x.fill_id = match color_of(c) {
            Some(_) => styles.add_fill(Fill::solid(c.trim().trim_start_matches('#'))),
            None => 0,
        };
        x.apply_fill = Some(true);
    }
    if touches_alignment(patch) {
        x.alignment = patch_alignment(x.alignment.as_ref(), patch);
        x.apply_alignment = Some(true);
    }
    if patch.locked.is_some() || patch.formula_hidden.is_some() {
        // §18.8.33 protection: `locked` defaults to true, `hidden` to false.
        let mut p = x.protection.clone().unwrap_or_default();
        if let Some(l) = patch.locked {
            p.locked = (!l).then_some(false);
        }
        if let Some(h) = patch.formula_hidden {
            p.hidden = h.then_some(true);
        }
        x.protection = (p != CellProtection::default()).then_some(p);
        x.apply_protection = Some(true);
    }
    if x.xf_id.is_none() {
        x.xf_id = Some(0);
    }
    styles.add_xf(x)
}

fn cell_count(r: &CellRange) -> u64 {
    u64::from(r.rows()) * u64::from(r.cols())
}

/// Applies `f(old_style) -> new_style` to every cell of `range`, creating
/// empty styled cells where needed. Results are memoised per old style.
fn restyle(ws: &mut Worksheet, range: CellRange, mut f: impl FnMut(u32, u32, u32) -> u32) -> bool {
    if cell_count(&range) > MAX_FORMAT_CELLS {
        return false;
    }
    for r in range.first.row..=range.last.row {
        for c in range.first.col..=range.last.col {
            let old = ws.cell(r, c).map_or(0, |c| c.style);
            let new = f(old, r, c);
            if new != old {
                ws.cells.entry((r, c)).or_default().style = new;
            }
        }
    }
    true
}

/// Applies a [`StylePatch`] to `range` of sheet `sheet`. Returns `false` when the
/// range is too large or the sheet does not exist.
pub fn apply_patch(wb: &mut Workbook, sheet: usize, range: CellRange, patch: &StylePatch) -> bool {
    let Workbook { sheets, styles, .. } = wb;
    let Some(ws) = sheets.get_mut(sheet) else {
        return false;
    };
    let mut memo = HashMap::new();
    restyle(ws, range, |old, _, _| {
        *memo
            .entry(old)
            .or_insert_with(|| patched_xf(styles, old, patch))
    })
}

fn edge(style: &str, color: Option<&str>) -> BorderEdge {
    if style.is_empty() || style == "none" {
        return BorderEdge::default();
    }
    BorderEdge {
        style: Some(style.to_owned()),
        color: color.and_then(color_of),
    }
}

/// Applies a border preset to `range` (§18.8.4 `border`).
pub fn apply_borders(wb: &mut Workbook, sheet: usize, range: CellRange, spec: &BorderSpec) -> bool {
    let Workbook { sheets, styles, .. } = wb;
    let Some(ws) = sheets.get_mut(sheet) else {
        return false;
    };
    let e = edge(&spec.style, spec.color.as_deref());
    let (r0, r1, c0, c1) = (
        range.first.row,
        range.last.row,
        range.first.col,
        range.last.col,
    );
    let mut memo: HashMap<(u32, [bool; 4]), u32> = HashMap::new();
    restyle(ws, range, |old, r, c| {
        // Which edges (top, right, bottom, left) this cell gets.
        let sides = match spec.preset {
            BorderPreset::All | BorderPreset::None => [true; 4],
            BorderPreset::Outline => [r == r0, c == c1, r == r1, c == c0],
            BorderPreset::Inside => [r > r0, c < c1, r < r1, c > c0],
            BorderPreset::Top => [r == r0, false, false, false],
            BorderPreset::Bottom => [false, false, r == r1, false],
            BorderPreset::Left => [false, false, false, c == c0],
            BorderPreset::Right => [false, c == c1, false, false],
        };
        if sides == [false; 4] {
            return old;
        }
        *memo.entry((old, sides)).or_insert_with(|| {
            let mut x = styles.xf(old).cloned().unwrap_or_default();
            let mut b = styles
                .borders
                .get(x.border_id as usize)
                .cloned()
                .unwrap_or_default();
            if spec.preset == BorderPreset::None {
                b = Border::default();
            } else {
                if sides[0] {
                    b.top = e.clone();
                }
                if sides[1] {
                    b.right = e.clone();
                }
                if sides[2] {
                    b.bottom = e.clone();
                }
                if sides[3] {
                    b.left = e.clone();
                }
            }
            x.border_id = styles.add_border(b);
            x.apply_border = Some(true);
            if x.xf_id.is_none() {
                x.xf_id = Some(0);
            }
            styles.add_xf(x)
        })
    })
}

/// Resets `range` to the default format (`xf` 0), keeping values.
pub fn clear_formats(wb: &mut Workbook, sheet: usize, range: CellRange) -> bool {
    let Some(ws) = wb.sheets.get_mut(sheet) else {
        return false;
    };
    if cell_count(&range) > MAX_FORMAT_CELLS {
        return false;
    }
    let keys: Vec<(u32, u32)> = ws
        .cells
        .range((range.first.row, 0)..=(range.last.row, u32::MAX))
        .filter(|(&(_, c), _)| c >= range.first.col && c <= range.last.col)
        .map(|(&k, _)| k)
        .collect();
    for k in keys {
        if let Some(cell) = ws.cells.get_mut(&k) {
            cell.style = 0;
            if matches!(cell.value, bo_model::sheet::CellValue::Empty) && cell.formula.is_none() {
                ws.cells.remove(&k);
            }
        }
    }
    true
}

/// Format Painter: tiles the styles of `source` over `target`.
pub fn copy_formats(wb: &mut Workbook, sheet: usize, source: CellRange, target: CellRange) -> bool {
    let Some(ws) = wb.sheets.get_mut(sheet) else {
        return false;
    };
    let (h, w) = (source.rows(), source.cols());
    let styles: Vec<u32> = (0..h)
        .flat_map(|dr| (0..w).map(move |dc| (dr, dc)))
        .map(|(dr, dc)| {
            ws.cell(source.first.row + dr, source.first.col + dc)
                .map_or(0, |c| c.style)
        })
        .collect();
    restyle(ws, target, |_, r, c| {
        let dr = (r - target.first.row) % h;
        let dc = (c - target.first.col) % w;
        styles[(dr * w + dc) as usize]
    })
}

/// A built-in named style of the gallery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NamedStyle {
    /// Display name of the style (`cellStyle/@name`).
    pub name: &'static str,
    /// `builtinId` (§18.8.7).
    pub builtin_id: u32,
}

/// The cell style gallery (names and built-in ids from §18.8.7; colours are
/// BlueSheet's own).
pub const NAMED_STYLES: &[NamedStyle] = &[
    NamedStyle {
        name: "Normal",
        builtin_id: 0,
    },
    NamedStyle {
        name: "Good",
        builtin_id: 26,
    },
    NamedStyle {
        name: "Bad",
        builtin_id: 27,
    },
    NamedStyle {
        name: "Neutral",
        builtin_id: 28,
    },
    NamedStyle {
        name: "Title",
        builtin_id: 15,
    },
    NamedStyle {
        name: "Heading 1",
        builtin_id: 16,
    },
    NamedStyle {
        name: "Heading 2",
        builtin_id: 17,
    },
    NamedStyle {
        name: "Total",
        builtin_id: 25,
    },
    NamedStyle {
        name: "Note",
        builtin_id: 10,
    },
];

/// Formatting record of a gallery style.
fn named_xf(styles: &mut Stylesheet, name: &str) -> Option<CellXf> {
    let base = styles.fonts.first().cloned().unwrap_or_default();
    let font = |color: &str, bold: bool, size: Option<f64>| Font {
        color: color_of(color),
        bold,
        size: size.or(base.size),
        ..base.clone()
    };
    let line = |style: &str, color: &str| BorderEdge {
        style: Some(style.into()),
        color: color_of(color),
    };
    let (font, fill, border) = match name {
        "Normal" => (base.clone(), None, Border::default()),
        "Good" => (
            font("1E6B3C", false, None),
            Some("D8F3E3"),
            Border::default(),
        ),
        "Bad" => (
            font("9B1C1C", false, None),
            Some("FADBD8"),
            Border::default(),
        ),
        "Neutral" => (
            font("7A5A00", false, None),
            Some("FDF0C4"),
            Border::default(),
        ),
        "Title" => (font("1F3A5F", true, Some(18.0)), None, Border::default()),
        "Heading 1" => (
            font("1F3A5F", true, Some(15.0)),
            None,
            Border {
                bottom: line("thick", "4A7BB7"),
                ..Border::default()
            },
        ),
        "Heading 2" => (
            font("1F3A5F", true, Some(13.0)),
            None,
            Border {
                bottom: line("medium", "9DB7D9"),
                ..Border::default()
            },
        ),
        "Total" => (
            font("1B1F24", true, None),
            None,
            Border {
                top: line("thin", "4A7BB7"),
                bottom: line("double", "4A7BB7"),
                ..Border::default()
            },
        ),
        "Note" => {
            let edge = line("thin", "B5A55A");
            (
                base.clone(),
                Some("FFF8D6"),
                Border {
                    top: edge.clone(),
                    bottom: edge.clone(),
                    left: edge.clone(),
                    right: edge,
                    ..Border::default()
                },
            )
        }
        _ => return None,
    };
    Some(CellXf {
        font_id: styles.add_font(font),
        fill_id: fill.map_or(0, |c| styles.add_fill(Fill::solid(c))),
        border_id: styles.add_border(border),
        ..CellXf::default()
    })
}

/// Index into `cellStyleXfs` of the named style, created on first use.
fn style_xf_id(styles: &mut Stylesheet, name: &str) -> Option<u32> {
    if let Some(s) = styles
        .cell_styles
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(name))
    {
        return Some(s.xf_id);
    }
    let def = NAMED_STYLES
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(name))?;
    let xf = named_xf(styles, def.name)?;
    styles.cell_style_xfs.push(xf);
    let id = u32::try_from(styles.cell_style_xfs.len() - 1).ok()?;
    styles.cell_styles.push(CellStyle {
        name: def.name.to_owned(),
        xf_id: id,
        builtin_id: Some(def.builtin_id),
        ..CellStyle::default()
    });
    Some(id)
}

/// Applies a gallery style to `range`: the cell format becomes the style's
/// format (number format kept), linked through `xfId` (§18.8.45).
pub fn apply_named_style(wb: &mut Workbook, sheet: usize, range: CellRange, name: &str) -> bool {
    let Workbook { sheets, styles, .. } = wb;
    let Some(ws) = sheets.get_mut(sheet) else {
        return false;
    };
    let Some(xf_id) = style_xf_id(styles, name) else {
        return false;
    };
    let Some(style) = styles.cell_style_xfs.get(xf_id as usize).cloned() else {
        return false;
    };
    let mut memo = HashMap::new();
    restyle(ws, range, |old, _, _| {
        *memo.entry(old).or_insert_with(|| {
            let num = styles.xf(old).map_or(0, |x| x.num_fmt_id);
            styles.add_xf(CellXf {
                num_fmt_id: num,
                xf_id: Some(xf_id),
                apply_number_format: (num != 0).then_some(true),
                ..style.clone()
            })
        })
    })
}

/// Formatting summary of the cell at (`row`, `col`).
pub fn format_state(
    wb: &Workbook,
    sheet: usize,
    row: u32,
    col: u32,
    locale: &Locale,
) -> FormatState {
    let styles = &wb.styles;
    let ws = wb.sheets.get(sheet);
    let style = ws.and_then(|ws| ws.cell(row, col)).map_or(0, |c| c.style);
    let xf = styles.xf(style).cloned().unwrap_or_default();
    let font = styles
        .fonts
        .get(xf.font_id as usize)
        .or_else(|| styles.fonts.first())
        .cloned()
        .unwrap_or_default();
    let fill = match styles.fills.get(xf.fill_id as usize) {
        Some(Fill::Pattern(p)) if p.pattern.as_deref() == Some("solid") => hex_of(p.fg.as_ref()),
        _ => String::new(),
    };
    let b = styles
        .borders
        .get(xf.border_id as usize)
        .cloned()
        .unwrap_or_default();
    let es = |e: &BorderEdge| e.style.clone().unwrap_or_default();
    let al = xf.alignment.clone().unwrap_or_default();
    let code = format_code(styles, xf.num_fmt_id, locale);
    FormatState {
        num_fmt: code,
        font_name: font.name.unwrap_or_else(|| "Calibri".into()),
        font_size: font.size.unwrap_or(11.0),
        bold: font.bold,
        italic: font.italic,
        underline: font.underline.unwrap_or_default(),
        strike: font.strike,
        font_color: hex_of(font.color.as_ref()),
        fill_color: fill,
        h_align: al.horizontal.unwrap_or_default(),
        v_align: al.vertical.unwrap_or_default(),
        wrap: al.wrap_text,
        indent: al.indent.unwrap_or(0),
        rotation: al.text_rotation.unwrap_or(0),
        shrink: al.shrink_to_fit,
        borders: [es(&b.top), es(&b.right), es(&b.bottom), es(&b.left)],
        style_name: styles
            .cell_styles
            .iter()
            .find(|s| Some(s.xf_id) == xf.xf_id)
            .map_or_else(|| "Normal".into(), |s| s.name.clone()),
        merged: ws.is_some_and(|ws| {
            ws.merge_at(bo_model::sheet::CellRef::new(row, col))
                .is_some()
        }),
        locked: xf
            .protection
            .as_ref()
            .and_then(|p| p.locked)
            .unwrap_or(true),
        formula_hidden: xf
            .protection
            .as_ref()
            .and_then(|p| p.hidden)
            .unwrap_or(false),
    }
}

/// Number format presets offered by the ribbon / dialog (vi-VN first).
pub const NUMBER_PRESETS: &[(&str, &str)] = &[
    ("general", "General"),
    ("number", "#,##0.00"),
    ("integer", "#,##0"),
    // ₫ with the Vietnamese locale tag (§18.8.31 `[$₫-42A]`).
    ("currencyVnd", "#,##0 [$₫-42A]"),
    ("currencyUsd", "[$$-409]#,##0.00"),
    (
        "accounting",
        r#"_-* #,##0 [$₫-42A]_-;-* #,##0 [$₫-42A]_-;_-* "-" [$₫-42A]_-;_-@_-"#,
    ),
    ("percent", "0%"),
    ("percent2", "0.00%"),
    ("date", "dd/mm/yyyy"),
    ("dateLong", "dddd, dd mmmm yyyy"),
    ("time", "hh:mm:ss"),
    ("dateTime", "dd/mm/yyyy hh:mm"),
    ("scientific", "0.00E+00"),
    ("fraction", "# ?/?"),
    ("text", "@"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::sheet::{Cell, CellRef, CellValue};

    fn rng(s: &str) -> CellRange {
        CellRange::parse(s).unwrap()
    }

    #[test]
    fn patches_are_deduplicated() {
        let mut wb = Workbook::new();
        wb.sheets[0].set_a1("A1", CellValue::Number(1.0));
        let before = wb.styles.cell_xfs.len();
        let bold = StylePatch {
            bold: Some(true),
            ..StylePatch::default()
        };
        assert!(apply_patch(&mut wb, 0, rng("A1:C3"), &bold));
        assert_eq!(wb.styles.cell_xfs.len(), before + 1);
        assert!(apply_patch(&mut wb, 0, rng("D1:D9"), &bold));
        assert_eq!(wb.styles.cell_xfs.len(), before + 1, "same xf reused");
        let s = wb.sheets[0].cell(2, 2).unwrap().style;
        assert_eq!(s, wb.sheets[0].cell(0, 0).unwrap().style);
        let f = &wb.styles.fonts[wb.styles.cell_xfs[s as usize].font_id as usize];
        assert!(f.bold);
        // Value kept.
        assert_eq!(
            wb.sheets[0].cell(0, 0).unwrap().value,
            CellValue::Number(1.0)
        );
        // Unbolding returns to the original font record.
        let unbold = StylePatch {
            bold: Some(false),
            ..StylePatch::default()
        };
        apply_patch(&mut wb, 0, rng("A1"), &unbold);
        let s = wb.sheets[0].cell(0, 0).unwrap().style;
        assert_eq!(wb.styles.cell_xfs[s as usize].font_id, 0);
    }

    #[test]
    fn rotation_and_shrink_patches() {
        let mut wb = Workbook::new();
        let p = StylePatch {
            rotation: Some(135),
            shrink: Some(true),
            ..StylePatch::default()
        };
        apply_patch(&mut wb, 0, rng("C3"), &p);
        let st = format_state(&wb, 0, 2, 2, &Locale::VI_VN);
        assert_eq!(st.rotation, 135);
        assert!(st.shrink);
        let off = StylePatch {
            rotation: Some(255),
            shrink: Some(false),
            ..StylePatch::default()
        };
        apply_patch(&mut wb, 0, rng("C3"), &off);
        let st = format_state(&wb, 0, 2, 2, &Locale::VI_VN);
        assert_eq!(st.rotation, 255);
        assert!(!st.shrink);
    }

    #[test]
    fn number_formats_fill_alignment() {
        let mut wb = Workbook::new();
        let p = StylePatch {
            num_fmt: Some("#,##0 [$₫-42A]".into()),
            fill_color: Some("FFEE00".into()),
            h_align: Some("center".into()),
            wrap: Some(true),
            indent: Some(2),
            ..StylePatch::default()
        };
        apply_patch(&mut wb, 0, rng("B2"), &p);
        let st = format_state(&wb, 0, 1, 1, &Locale::VI_VN);
        assert_eq!(st.num_fmt, "#,##0 [$₫-42A]");
        assert_eq!(st.fill_color, "FFEE00");
        assert_eq!(st.h_align, "center");
        assert!(st.wrap);
        assert_eq!(st.indent, 2);
        wb.sheets[0].cells.get_mut(&(1, 1)).unwrap().value = CellValue::Number(1_234_567.0);
        let cell = wb.sheets[0].cell(1, 1).unwrap();
        assert_eq!(
            crate::render::format_cell(&wb, cell, &Locale::VI_VN).text,
            "1.234.567 ₫"
        );
        // Built-in codes map to built-in ids.
        assert_eq!(num_fmt_id(&mut wb.styles, "0.00%"), 10);
        assert_eq!(num_fmt_id(&mut wb.styles, "General"), 0);
        let id = num_fmt_id(&mut wb.styles, "dd/mm/yyyy");
        assert!(id >= 164);
        wb.sheets[0].set_cell(0, 0, Cell::new(CellValue::Number(45_658.0)));
        apply_patch(
            &mut wb,
            0,
            rng("A1"),
            &StylePatch {
                num_fmt: Some("dd/mm/yyyy".into()),
                ..StylePatch::default()
            },
        );
        let c = wb.sheets[0].cell(0, 0).unwrap();
        assert_eq!(
            crate::render::format_cell(&wb, c, &Locale::VI_VN).text,
            "01/01/2025"
        );
        // Removing the fill.
        apply_patch(
            &mut wb,
            0,
            rng("B2"),
            &StylePatch {
                fill_color: Some(String::new()),
                ..StylePatch::default()
            },
        );
        assert_eq!(format_state(&wb, 0, 1, 1, &Locale::VI_VN).fill_color, "");
    }

    #[test]
    fn border_presets() {
        let mut wb = Workbook::new();
        let spec = |preset| BorderSpec {
            preset,
            style: "thin".into(),
            color: Some("FF0000".into()),
        };
        apply_borders(&mut wb, 0, rng("A1:C3"), &spec(BorderPreset::Outline));
        let st = |wb: &Workbook, r, c| format_state(wb, 0, r, c, &Locale::VI_VN).borders;
        assert_eq!(st(&wb, 0, 0), ["thin", "", "", "thin"].map(String::from));
        assert_eq!(st(&wb, 1, 1), ["", "", "", ""].map(String::from));
        assert_eq!(st(&wb, 2, 2), ["", "thin", "thin", ""].map(String::from));
        apply_borders(&mut wb, 0, rng("A1:C3"), &spec(BorderPreset::Inside));
        assert_eq!(st(&wb, 1, 1), ["thin"; 4].map(String::from));
        apply_borders(&mut wb, 0, rng("A1:C3"), &spec(BorderPreset::None));
        assert_eq!(st(&wb, 0, 0), ["", "", "", ""].map(String::from));
        apply_borders(&mut wb, 0, rng("B2"), &spec(BorderPreset::All));
        assert_eq!(st(&wb, 1, 1), ["thin"; 4].map(String::from));
    }

    #[test]
    fn named_styles_painter_and_clear() {
        let mut wb = Workbook::new();
        assert!(apply_named_style(&mut wb, 0, rng("A1:B1"), "Good"));
        let st = format_state(&wb, 0, 0, 1, &Locale::VI_VN);
        assert_eq!(st.style_name, "Good");
        assert_eq!(st.fill_color, "D8F3E3");
        assert!(wb
            .styles
            .cell_styles
            .iter()
            .any(|s| s.name == "Good" && s.builtin_id == Some(26)));
        let n = wb.styles.cell_style_xfs.len();
        apply_named_style(&mut wb, 0, rng("C1"), "good");
        assert_eq!(wb.styles.cell_style_xfs.len(), n, "style created once");
        assert!(!apply_named_style(&mut wb, 0, rng("C1"), "Unknown"));
        // Painter tiles A1:B1 over D1:G2.
        copy_formats(&mut wb, 0, rng("A1:B1"), rng("D1:G2"));
        let a1 = wb.sheets[0].cell(0, 0).unwrap().style;
        assert_eq!(wb.sheets[0].cell(1, 6).unwrap().style, a1);
        clear_formats(&mut wb, 0, rng("A1:G2"));
        assert!(wb.sheets[0].cells.is_empty());
        assert!(!format_state(&wb, 0, 5, 5, &Locale::VI_VN).merged);
        wb.sheets[0].merges.push(rng("E5:F6"));
        assert!(format_state(&wb, 0, 5, 5, &Locale::VI_VN).merged);
        let _ = CellRef::new(0, 0);
    }

    #[test]
    fn huge_ranges_are_refused() {
        let mut wb = Workbook::new();
        let all = CellRange::new(CellRef::new(0, 0), CellRef::new(1_048_575, 16_383));
        assert!(!apply_patch(
            &mut wb,
            0,
            all,
            &StylePatch {
                bold: Some(true),
                ..StylePatch::default()
            }
        ));
        assert!(!apply_patch(&mut wb, 5, rng("A1"), &StylePatch::default()));
    }
}
