//! Worksheet header / footer text: the `&` format codes of `oddHeader`,
//! `oddFooter`, `evenHeader`, `evenFooter`, `firstHeader` and `firstFooter`
//! (ECMA-376 Part 1, §18.3.1.39–42 and §18.3.1.48–49; the code table is given
//! with `evenFooter`, §18.3.1.39) — parsing, field expansion and drawing.
//!
//! Supported codes: `&L` `&C` `&R` (sections), `&P` page number, `&N` page
//! count, `&D` date, `&T` time, `&Z` file path, `&F` file name, `&A` sheet
//! name, `&G` picture (kept as a field; nothing is drawn for it), `&&`, font
//! `&"name,style"`, size `&nn`, `&B` `&I` `&U` `&E` (double underline) `&S`
//! `&X` (superscript) `&Y` (subscript) `&O` (outline: the letters' edge in the
//! text colour, hollow) `&H` (shadow: a grey copy below right) and colour `&KRRGGBB` / `&KTTSNNN` (theme colour TT with tint
//! sign S and percent NNN).
//!
//! Text before any section code belongs to the centre section (our reading).
//! Formatting toggles stay on until toggled again or a new section starts
//! (each section starts from the default font — our choice).

use crate::color::resolve_color;
use bo_model::sheet::{SheetColor, ThemeColors};
use bo_render::{Command, Glyph, Rgba};
use bo_text::{FaceId, FontManager};

/// Text colour from `&K`.
#[derive(Debug, Clone, PartialEq)]
pub enum HfColor {
    /// `&KRRGGBB`.
    Rgb(u8, u8, u8),
    /// `&KTTSNNN`: theme colour index and tint (−1.0..=1.0).
    Theme(u32, f64),
}

/// Character formatting of a header / footer run.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HfFormat {
    /// Font family (`&"name,style"`; `None` = the workbook's default font).
    pub font: Option<String>,
    /// Bold (`&B` or a "Bold" style).
    pub bold: bool,
    /// Italic (`&I` or an "Italic" style).
    pub italic: bool,
    /// 0 = none, 1 = single (`&U`), 2 = double (`&E`).
    pub underline: u8,
    /// Strikethrough (`&S`).
    pub strike: bool,
    /// Superscript (`&X`).
    pub superscript: bool,
    /// Subscript (`&Y`).
    pub subscript: bool,
    /// Outline (`&O`).
    pub outline: bool,
    /// Shadow (`&H`).
    pub shadow: bool,
    /// Size in points (`&nn`).
    pub size: Option<f64>,
    /// Colour (`&K`).
    pub color: Option<HfColor>,
}

/// One piece of a section.
#[derive(Debug, Clone, PartialEq)]
pub enum HfField {
    /// Literal text (may contain `\n` line breaks).
    Text(String),
    /// `&P`: page number.
    Page,
    /// `&N`: number of pages.
    Pages,
    /// `&D`: current date.
    Date,
    /// `&T`: current time.
    Time,
    /// `&Z`: path of the file.
    Path,
    /// `&F`: file name.
    File,
    /// `&A`: sheet name.
    Sheet,
    /// `&G`: picture (from the sheet's `legacyDrawingHF`).
    Picture,
}

/// A field with its formatting.
#[derive(Debug, Clone, PartialEq)]
pub struct HfPiece {
    /// Formatting in effect.
    pub fmt: HfFormat,
    /// Content.
    pub field: HfField,
}

/// Parsed header or footer: three sections.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HeaderFooterText {
    /// `&L` section.
    pub left: Vec<HfPiece>,
    /// `&C` section (also text before any section code).
    pub center: Vec<HfPiece>,
    /// `&R` section.
    pub right: Vec<HfPiece>,
}

/// Values substituted for the fields.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HfContext {
    /// Page number (`&P`).
    pub page: u32,
    /// Page count (`&N`).
    pub pages: u32,
    /// Formatted date (`&D`).
    pub date: String,
    /// Formatted time (`&T`).
    pub time: String,
    /// File path without the name (`&Z`).
    pub path: String,
    /// File name (`&F`).
    pub file: String,
    /// Sheet name (`&A`).
    pub sheet: String,
}

fn push_text(out: &mut Vec<HfPiece>, fmt: &HfFormat, s: &str) {
    if s.is_empty() {
        return;
    }
    if let Some(HfPiece {
        fmt: f,
        field: HfField::Text(t),
    }) = out.last_mut()
    {
        if f == fmt {
            t.push_str(s);
            return;
        }
    }
    out.push(HfPiece {
        fmt: fmt.clone(),
        field: HfField::Text(s.to_owned()),
    });
}

/// `&K` argument: 6 hex digits, or `TTSNNN` (theme, tint sign, tint percent).
fn parse_color(arg: &str) -> Option<HfColor> {
    if arg.len() != 6 {
        return None;
    }
    let b = arg.as_bytes();
    if b.iter().all(u8::is_ascii_hexdigit) {
        let h = |i: usize| u8::from_str_radix(&arg[i..i + 2], 16).ok();
        return Some(HfColor::Rgb(h(0)?, h(2)?, h(4)?));
    }
    if b[0].is_ascii_digit()
        && b[1].is_ascii_digit()
        && (b[2] == b'+' || b[2] == b'-')
        && b[3..].iter().all(u8::is_ascii_digit)
    {
        let theme: u32 = arg[..2].parse().ok()?;
        let pct: f64 = arg[3..].parse().ok()?;
        let tint = (pct / 100.0).min(1.0);
        return Some(HfColor::Theme(
            theme,
            if b[2] == b'-' { -tint } else { tint },
        ));
    }
    None
}

/// Parses header / footer text with its `&` codes.
#[must_use]
pub fn parse(text: &str) -> HeaderFooterText {
    let mut out = HeaderFooterText::default();
    // 0 = left, 1 = centre, 2 = right.
    let mut sec = 1usize;
    let mut fmt = HfFormat::default();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut buf = String::new();
    macro_rules! section {
        () => {
            match sec {
                0 => &mut out.left,
                2 => &mut out.right,
                _ => &mut out.center,
            }
        };
    }
    while i < chars.len() {
        let c = chars[i];
        if c != '&' || i + 1 >= chars.len() {
            buf.push(c);
            i += 1;
            continue;
        }
        let code = chars[i + 1];
        i += 2;
        if code == '&' {
            buf.push('&');
            continue;
        }
        push_text(section!(), &fmt, &buf);
        buf.clear();
        let field = match code {
            'L' | 'C' | 'R' => {
                sec = match code {
                    'L' => 0,
                    'C' => 1,
                    _ => 2,
                };
                fmt = HfFormat::default();
                None
            }
            'P' => Some(HfField::Page),
            'N' => Some(HfField::Pages),
            'D' => Some(HfField::Date),
            'T' => Some(HfField::Time),
            'Z' => Some(HfField::Path),
            'F' => Some(HfField::File),
            'A' => Some(HfField::Sheet),
            'G' => Some(HfField::Picture),
            'B' => {
                fmt.bold = !fmt.bold;
                None
            }
            'I' => {
                fmt.italic = !fmt.italic;
                None
            }
            'U' => {
                fmt.underline = u8::from(fmt.underline != 1);
                None
            }
            'E' => {
                fmt.underline = if fmt.underline == 2 { 0 } else { 2 };
                None
            }
            'S' => {
                fmt.strike = !fmt.strike;
                None
            }
            'X' => {
                fmt.superscript = !fmt.superscript;
                fmt.subscript = false;
                None
            }
            'Y' => {
                fmt.subscript = !fmt.subscript;
                fmt.superscript = false;
                None
            }
            'O' => {
                fmt.outline = !fmt.outline;
                None
            }
            'H' => {
                fmt.shadow = !fmt.shadow;
                None
            }
            'K' => {
                let arg: String = chars[i..chars.len().min(i + 6)].iter().collect();
                if let Some(col) = parse_color(&arg) {
                    fmt.color = Some(col);
                    i += 6;
                }
                None
            }
            '"' => {
                // &"font name,font style"
                let end = chars[i..].iter().position(|&ch| ch == '"').map(|p| i + p);
                let spec: String = chars[i..end.unwrap_or(chars.len())].iter().collect();
                i = end.map_or(chars.len(), |e| e + 1);
                let (name, style) = spec.split_once(',').unwrap_or((spec.as_str(), ""));
                let name = name.trim();
                fmt.font = (!name.is_empty() && name != "-").then(|| name.to_owned());
                let st = style.to_ascii_lowercase();
                if !st.is_empty() {
                    fmt.bold = st.contains("bold");
                    fmt.italic = st.contains("italic") || st.contains("oblique");
                }
                None
            }
            d if d.is_ascii_digit() => {
                let start = i - 1;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                let n: String = chars[start..i].iter().collect();
                if let Ok(v) = n.parse::<f64>() {
                    if v >= 1.0 {
                        fmt.size = Some(v);
                    }
                }
                None
            }
            // Unknown code: dropped.
            _ => None,
        };
        if let Some(field) = field {
            section!().push(HfPiece {
                fmt: fmt.clone(),
                field,
            });
        }
    }
    push_text(section!(), &fmt, &buf);
    out
}

/// Expands the fields of a section into formatted text runs (equal
/// neighbouring formats merged; `&G` expands to nothing).
#[must_use]
pub fn expand(pieces: &[HfPiece], ctx: &HfContext) -> Vec<(HfFormat, String)> {
    let mut out: Vec<(HfFormat, String)> = Vec::new();
    for p in pieces {
        let s = match &p.field {
            HfField::Text(t) => t.clone(),
            HfField::Page => ctx.page.to_string(),
            HfField::Pages => ctx.pages.to_string(),
            HfField::Date => ctx.date.clone(),
            HfField::Time => ctx.time.clone(),
            HfField::Path => ctx.path.clone(),
            HfField::File => ctx.file.clone(),
            HfField::Sheet => ctx.sheet.clone(),
            HfField::Picture => String::new(),
        };
        if s.is_empty() {
            continue;
        }
        match out.last_mut() {
            Some((f, t)) if *f == p.fmt => t.push_str(&s),
            _ => out.push((p.fmt.clone(), s)),
        }
    }
    out
}

/// Plain expanded text of a section.
#[must_use]
pub fn plain(pieces: &[HfPiece], ctx: &HfContext) -> String {
    expand(pieces, ctx).into_iter().map(|(_, t)| t).collect()
}

/// Horizontal anchoring of a section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HfAlign {
    /// Starts at `x`.
    Left,
    /// Centred on `x`.
    Center,
    /// Ends at `x`.
    Right,
}

/// Default font of header / footer text.
#[derive(Debug, Clone, PartialEq)]
pub struct HfBase<'a> {
    /// Font family.
    pub font: &'a str,
    /// Size in points.
    pub size: f64,
    /// Theme colours for `&KTTSNNN`.
    pub theme: Option<&'a ThemeColors>,
    /// Multiplier for every size (`scaleWithDoc`).
    pub scale: f64,
}

#[allow(clippy::cast_possible_truncation)]
fn f(v: f64) -> f32 {
    v as f32
}

struct Shaped {
    runs: Vec<(FaceId, String, Vec<Glyph>, f64)>,
    width: f64,
}

fn shape(fonts: &mut FontManager, face: FaceId, text: &str, size: f64) -> Shaped {
    let mut x = 0f64;
    let mut runs = Vec::new();
    for (range, fid) in fonts.itemize(face, text) {
        let part = &text[range];
        let scale = size / f64::from(fonts.info(fid).units_per_em.max(1));
        let shaped = fonts.shape(fid, part);
        let start = x;
        let mut glyphs = Vec::with_capacity(shaped.len());
        for g in shaped.iter() {
            glyphs.push(Glyph(
                g.glyph_id,
                f(x - start + f64::from(g.x_offset) * scale),
                f(-f64::from(g.y_offset) * scale),
                g.cluster,
            ));
            x += f64::from(g.x_advance) * scale;
        }
        runs.push((fid, part.to_owned(), glyphs, x - start));
    }
    Shaped { runs, width: x }
}

/// One measured run of a line.
struct Piece {
    face: FaceId,
    text: String,
    size: f64,
    rise: f64,
    color: Rgba,
    fmt: HfFormat,
    width: f64,
}

/// Total height of the drawn section (points), for footers anchored at their bottom.
#[must_use]
pub fn height(runs: &[(HfFormat, String)], base: &HfBase<'_>) -> f64 {
    lines(runs).iter().map(|l| line_size(l, base) * 1.2).sum()
}

fn lines(runs: &[(HfFormat, String)]) -> Vec<Vec<(HfFormat, String)>> {
    let mut out = vec![Vec::new()];
    for (fmt, text) in runs {
        for (i, part) in text.split('\n').enumerate() {
            if i > 0 {
                out.push(Vec::new());
            }
            if !part.is_empty() {
                if let Some(l) = out.last_mut() {
                    l.push((fmt.clone(), part.to_owned()));
                }
            }
        }
    }
    out
}

fn line_size(line: &[(HfFormat, String)], base: &HfBase<'_>) -> f64 {
    line.iter()
        .map(|(fm, _)| fm.size.unwrap_or(base.size))
        .fold(0.0, f64::max)
        .max(if line.is_empty() { base.size } else { 0.0 })
        * base.scale
}

/// Draws a section with its first line's top at `top`; returns the commands.
/// Lines are 1.2 × the largest size; the baseline sits 0.8 × size below the
/// line top (our approximation of the ascent).
#[must_use]
pub fn draw(
    fonts: &mut FontManager,
    runs: &[(HfFormat, String)],
    base: &HfBase<'_>,
    align: HfAlign,
    x: f64,
    top: f64,
) -> Vec<Command> {
    let mut out = Vec::new();
    let mut y = top;
    for line in lines(runs) {
        let lsize = line_size(&line, base);
        let baseline = y + lsize * 0.95;
        let mut pieces = Vec::new();
        for (fm, text) in &line {
            let mut size = fm.size.unwrap_or(base.size) * base.scale;
            let mut rise = 0.0;
            if fm.superscript || fm.subscript {
                rise = if fm.superscript {
                    -size * 0.35
                } else {
                    size * 0.15
                };
                size *= 0.6;
            }
            let family = fm.font.as_deref().unwrap_or(base.font);
            let Some(face) = fonts
                .resolve(family, fm.bold, fm.italic)
                .or_else(|| fonts.resolve(base.font, fm.bold, fm.italic))
            else {
                continue;
            };
            let color = match &fm.color {
                Some(HfColor::Rgb(r, g, b)) => Rgba(*r, *g, *b, 255),
                Some(HfColor::Theme(t, tint)) => resolve_color(
                    &SheetColor {
                        theme: Some(*t),
                        tint: (*tint != 0.0).then_some(*tint),
                        ..SheetColor::default()
                    },
                    base.theme,
                )
                .unwrap_or(Rgba(0, 0, 0, 255)),
                None => Rgba(0, 0, 0, 255),
            };
            let width = shape(fonts, face, text, size).width;
            pieces.push(Piece {
                face,
                text: text.clone(),
                size,
                rise,
                color,
                fmt: fm.clone(),
                width,
            });
        }
        let total: f64 = pieces.iter().map(|p| p.width).sum();
        let mut px = match align {
            HfAlign::Left => x,
            HfAlign::Center => x - total / 2.0,
            HfAlign::Right => x - total,
        };
        for p in pieces {
            let sh = shape(fonts, p.face, &p.text, p.size);
            let by = baseline + p.rise;
            let mut rx = px;
            for (fid, part, glyphs, w) in sh.runs {
                if !part.trim().is_empty() {
                    let copy = |dx: f64, dy: f64, color: Rgba| Command::Glyphs {
                        face: fid.0,
                        size: f(p.size),
                        x: f(rx + dx),
                        y: f(by + dy),
                        color,
                        text: part.clone(),
                        glyphs: glyphs.clone(),
                    };
                    // `&H` shadow: a grey copy below and to the right.
                    if p.fmt.shadow {
                        let d = (p.size * 0.06).max(0.6);
                        out.push(copy(d, d, SHADOW));
                    }
                    // `&O` outline: the letters drawn as their edge — copies shifted all
                    // round in the text colour, the letters themselves in white on top.
                    let fill = if p.fmt.outline {
                        let o = (p.size * 0.035).max(0.35);
                        for (dx, dy) in [
                            (-o, 0.0),
                            (o, 0.0),
                            (0.0, -o),
                            (0.0, o),
                            (-o, -o),
                            (o, -o),
                            (-o, o),
                            (o, o),
                        ] {
                            out.push(copy(dx, dy, p.color));
                        }
                        Rgba(255, 255, 255, 255)
                    } else {
                        p.color
                    };
                    out.push(copy(0.0, 0.0, fill));
                }
                rx += w;
            }
            let lw = (p.size * 0.06).max(0.5);
            if p.fmt.underline > 0 {
                let uy = by + p.size * 0.12;
                out.push(line_cmd(px, px + p.width, uy, lw, p.color));
                if p.fmt.underline == 2 {
                    out.push(line_cmd(px, px + p.width, uy + lw * 2.0, lw, p.color));
                }
            }
            if p.fmt.strike {
                let sy = by - p.size * 0.3;
                out.push(line_cmd(px, px + p.width, sy, lw, p.color));
            }
            px += p.width;
        }
        y += lsize * 1.2;
    }
    out
}

/// Colour of the `&H` text shadow.
const SHADOW: Rgba = Rgba(0x80, 0x80, 0x80, 255);

fn line_cmd(x1: f64, x2: f64, y: f64, width: f64, color: Rgba) -> Command {
    Command::Line {
        x1: f(x1),
        y1: f(y),
        x2: f(x2),
        y2: f(y),
        width: f(width),
        color,
        dash: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> HfContext {
        HfContext {
            page: 3,
            pages: 7,
            date: "10/11/2026".into(),
            time: "14:05".into(),
            path: "D:\\Reports\\".into(),
            file: "Budget.xlsx".into(),
            sheet: "Sales".into(),
        }
    }

    #[test]
    fn sections_and_fields() {
        let h = parse("&LPage &P of &N&CTitle&R&D &T");
        let c = ctx();
        assert_eq!(plain(&h.left, &c), "Page 3 of 7");
        assert_eq!(plain(&h.center, &c), "Title");
        assert_eq!(plain(&h.right, &c), "10/11/2026 14:05");
        let h = parse("&Z&F - &A&&co");
        assert_eq!(plain(&h.center, &c), "D:\\Reports\\Budget.xlsx - Sales&co");
        assert!(h.left.is_empty() && h.right.is_empty());
        // Picture field is kept but expands to nothing.
        let h = parse("&R&G");
        assert_eq!(h.right[0].field, HfField::Picture);
        assert_eq!(plain(&h.right, &c), "");
    }

    #[test]
    fn font_codes() {
        let h = parse("&C&\"Arial,Bold Italic\"&14Big&B &Unormal&K00FF00g&K04+050t");
        let runs = expand(&h.center, &ctx());
        assert_eq!(runs[0].1, "Big");
        assert_eq!(runs[0].0.font.as_deref(), Some("Arial"));
        assert!(runs[0].0.bold && runs[0].0.italic);
        assert_eq!(runs[0].0.size, Some(14.0));
        // &B toggles bold off.
        assert_eq!(runs[1].1, " ");
        assert!(!runs[1].0.bold && runs[1].0.italic);
        assert_eq!(runs[2].1, "normal");
        assert_eq!(runs[2].0.underline, 1);
        assert_eq!(runs[3].0.color, Some(HfColor::Rgb(0, 255, 0)));
        assert_eq!(runs[4].0.color, Some(HfColor::Theme(4, 0.5)));
        let h = parse("&L&ES&X2&Y3&S-");
        let r = expand(&h.left, &ctx());
        assert_eq!(r[0].0.underline, 2);
        assert!(r[1].0.superscript);
        assert!(r[2].0.subscript && !r[2].0.superscript);
        assert!(r[3].0.strike);
        // A new section starts with default formatting; `-` font = default.
        let h = parse("&L&B&\"-,Regular\"x&Ry");
        let r = expand(&h.left, &ctx());
        assert!(!r[0].0.bold && r[0].0.font.is_none());
        assert_eq!(expand(&h.right, &ctx())[0].0, HfFormat::default());
    }

    #[test]
    fn outline_and_shadow_are_drawn() {
        let mut fm = FontManager::new();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
        fm.add_fonts_from_dir(&dir);
        let base = HfBase {
            font: "Liberation Sans",
            size: 10.0,
            theme: None,
            scale: 1.0,
        };
        let glyphs = |code: &str, fm: &mut FontManager| -> Vec<(f32, f32, Rgba)> {
            let runs = expand(&parse(code).center, &ctx());
            draw(fm, &runs, &base, HfAlign::Left, 0.0, 0.0)
                .into_iter()
                .filter_map(|c| match c {
                    Command::Glyphs { x, y, color, .. } => Some((x, y, color)),
                    _ => None,
                })
                .collect()
        };
        let plain = glyphs("&CAb", &mut fm);
        assert_eq!(plain.len(), 1);
        let (x, y, black) = plain[0];
        assert_eq!(black, Rgba(0, 0, 0, 255));
        // Shadow: a grey copy 0.6 pt down and right (6 % of 10 pt), then the text.
        let shadow = glyphs("&C&HAb", &mut fm);
        assert_eq!(shadow.len(), 2);
        assert_eq!(shadow[0].2, SHADOW);
        assert!((shadow[0].0 - x - 0.6).abs() < 1e-4 && (shadow[0].1 - y - 0.6).abs() < 1e-4);
        assert_eq!(shadow[1], plain[0]);
        // Outline: eight copies around in the text colour, the letters in white.
        let outline = glyphs("&C&OAb", &mut fm);
        assert_eq!(outline.len(), 9);
        assert!(outline[..8].iter().all(|g| g.2 == black));
        assert_eq!(outline[8], (x, y, Rgba(255, 255, 255, 255)));
    }

    #[test]
    fn line_breaks_count_in_height() {
        let base = HfBase {
            font: "Calibri",
            size: 10.0,
            theme: None,
            scale: 1.0,
        };
        let runs = expand(&parse("a\nb").center, &ctx());
        // Two lines of 10 pt at 1.2 line spacing.
        assert!((height(&runs, &base) - 24.0).abs() < 1e-9);
    }
}
