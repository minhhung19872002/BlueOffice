//! Format Cells dialog of BlueSheet on the self-drawn form (`sheet_form.rs`): the six pages of
//! the reference spreadsheet's dialog with a category list, type lists and a live sample
//! (Number), alignment boxes and the orientation dial (Alignment), font / style / size lists
//! with a preview (Font), line styles, presets and the border box (Border), the colour
//! palette and patterns (Fill), and Protection. Positions are the reference's, measured
//! black-box at 125 % (docs/ui/REPORT-sheet.md round 13).
use super::form_impl::{Event, Form, Kind};
use super::{SheetWindow, FONTS};
use crate::Painter;
use bo_model::sheet::{CellValue, SheetColor};
use bo_render::{PathSeg, Rgba};
use serde_json::json;

/// Number categories (reference order).
const CATEGORIES: [(&str, &str); 12] = [
    ("sheetFc.catGeneral", "General"),
    ("sheetFc.catNumber", "Number"),
    ("sheetFc.catCurrency", "Currency"),
    ("sheetFc.catAccounting", "Accounting"),
    ("sheetFc.catDate", "Date"),
    ("sheetFc.catTime", "Time"),
    ("sheetFc.catPercentage", "Percentage"),
    ("sheetFc.catFraction", "Fraction"),
    ("sheetFc.catScientific", "Scientific"),
    ("sheetFc.catText", "Text"),
    ("sheetFc.catSpecial", "Special"),
    ("sheetFc.catCustom", "Custom"),
];

const DATE_TYPES: [&str; 10] = [
    "m/d/yyyy",
    "[$-F800]dddd, mmmm dd, yyyy",
    "d-mmm",
    "d-mmm-yy",
    "dd-mmm-yy",
    "mmm-yy",
    "mmmm-yy",
    "mmmm d, yyyy",
    "m/d/yy h:mm",
    "yyyy-mm-dd",
];
const TIME_TYPES: [&str; 7] = [
    "[$-F400]h:mm:ss AM/PM",
    "h:mm;@",
    "h:mm AM/PM",
    "h:mm:ss",
    "mm:ss.0",
    "[h]:mm:ss",
    "m/d/yy h:mm",
];
const FRACTION_TYPES: [&str; 9] = [
    "# ?/?",
    "# ??/??",
    "# ???/???",
    "# ?/2",
    "# ?/4",
    "# ?/8",
    "# ??/16",
    "# ?/10",
    "# ??/100",
];
const SPECIAL_TYPES: [&str; 4] = [
    "00000",
    "00000-0000",
    "[<=9999999]###-####;(###) ###-####",
    "000-00-0000",
];
const CUSTOM_TYPES: [&str; 22] = [
    "General",
    "0",
    "0.00",
    "#,##0",
    "#,##0.00",
    "#,##0_);(#,##0)",
    "#,##0_);[Red](#,##0)",
    "#,##0.00_);(#,##0.00)",
    "#,##0.00_);[Red](#,##0.00)",
    "0%",
    "0.00%",
    "0.00E+00",
    "##0.0E+0",
    "# ?/?",
    "# ??/??",
    "m/d/yyyy",
    "d-mmm-yy",
    "d-mmm",
    "mmm-yy",
    "h:mm AM/PM",
    "h:mm:ss",
    "@",
];
const SYMBOLS: [&str; 6] = ["None", "$", "€", "£", "¥", "₫"];

/// Line styles of the Border page (two columns of seven, reference order).
pub(super) const LINE_STYLES: [&str; 14] = [
    "none",
    "hair",
    "dotted",
    "dashDotDot",
    "dashDot",
    "dashed",
    "thin",
    "mediumDashDotDot",
    "slantDashDot",
    "mediumDashDot",
    "mediumDashed",
    "medium",
    "thick",
    "double",
];

/// Pattern styles (§18.18.55), the first meaning none.
const PATTERNS: [&str; 19] = [
    "none",
    "solid",
    "darkGray",
    "mediumGray",
    "lightGray",
    "gray125",
    "gray0625",
    "darkHorizontal",
    "darkVertical",
    "darkDown",
    "darkUp",
    "darkGrid",
    "darkTrellis",
    "lightHorizontal",
    "lightVertical",
    "lightDown",
    "lightUp",
    "lightGrid",
    "lightTrellis",
];

/// Named colours of the colour drop-down boxes (`""` = automatic).
const NAMED_COLORS: [(&str, &str); 13] = [
    ("sheetFc.colAuto", ""),
    ("sheetFc.colBlack", "000000"),
    ("sheetFc.colWhite", "FFFFFF"),
    ("sheetFc.colDarkRed", "C00000"),
    ("sheetFc.colRed", "FF0000"),
    ("sheetFc.colOrange", "FFC000"),
    ("sheetFc.colYellow", "FFFF00"),
    ("sheetFc.colLightGreen", "92D050"),
    ("sheetFc.colGreen", "00B050"),
    ("sheetFc.colLightBlue", "00B0F0"),
    ("sheetFc.colBlue", "0070C0"),
    ("sheetFc.colDarkBlue", "002060"),
    ("sheetFc.colPurple", "7030A0"),
];

/// Border edges of the Border page: top, inside horizontal, bottom, left, inside vertical,
/// right, diagonal up, diagonal down.
const EDGES: usize = 8;

/// State of the Format Cells dialog that is not a plain control value.
#[derive(Debug, Clone, Default)]
pub(super) struct FcState {
    /// Border edge styles (index into [`LINE_STYLES`], 0 = none) and colours.
    pub edges: [(usize, String); EDGES],
    /// The border was changed (else it is left as it is).
    pub border_changed: bool,
    /// Chosen line style and colour.
    pub line: usize,
    /// Text orientation in degrees (-90…90) and vertical stacked text.
    pub degrees: i32,
    pub vertical: bool,
    /// Background colour chosen on the Fill page (`None`: no change, `Some("")`: no colour).
    pub fill: Option<String>,
    /// Theme / standard colours of the palette (RRGGBB).
    pub palette: Vec<String>,
    /// Category shown when the dialog opened, and the code it had.
    pub code: String,
}

fn hex(c: Rgba) -> String {
    format!("{:02X}{:02X}{:02X}", c.0, c.1, c.2)
}

fn rgba(h: &str) -> Option<Rgba> {
    let h = h.trim().trim_start_matches('#');
    let h = &h[h.len().saturating_sub(6)..];
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some(Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255))
}

/// Category of a number format code, its decimals, thousands separator and negative style.
fn classify(code: &str) -> (usize, i32, bool, usize) {
    let c = code.trim();
    let first = c.split(';').next().unwrap_or("");
    let dec = first.split_once('.').map_or(0, |(_, b)| {
        b.chars().take_while(|ch| *ch == '0').count() as i32
    });
    let sep = first.contains("#,##");
    let neg = if c.contains("[Red](") {
        3
    } else if c.contains(";(") || c.contains("_);") {
        2
    } else {
        usize::from(c.contains("[Red]"))
    };
    let lower = first.to_ascii_lowercase();
    let bare: String = lower
        .split('"')
        .step_by(2)
        .collect::<String>()
        .replace("[red]", "");
    let cat = if c.is_empty() || c.eq_ignore_ascii_case("general") {
        0
    } else if c == "@" {
        9
    } else if c.starts_with("_(") || c.starts_with("_-") || c.contains("* #") {
        3
    } else if bare.contains('%') {
        6
    } else if bare.contains("e+") {
        8
    } else if bare.contains("?/") {
        7
    } else if bare.contains('h') || bare.contains('s') && !bare.contains('y') {
        5
    } else if bare.contains('y') || bare.contains('d') || bare.contains("mmm") {
        4
    } else if ["$", "€", "£", "¥", "₫"].iter().any(|s| c.contains(s)) {
        2
    } else if bare.chars().all(|ch| "0#,._();-[]red ".contains(ch)) {
        1
    } else {
        11
    };
    (cat, dec, sep, neg)
}

/// The format code of a category with the Number page's settings.
fn code_for(cat: usize, dec: i32, sep: bool, neg: usize, sym: usize, ty: &str) -> String {
    let zeros = if dec > 0 {
        format!(".{}", "0".repeat(dec as usize))
    } else {
        String::new()
    };
    let num = |sep: bool| format!("{}0{zeros}", if sep { "#,##" } else { "" });
    let with_neg = |x: String| match neg {
        1 => format!("{x};[Red]{x}"),
        2 => format!("{x}_);({x})"),
        3 => format!("{x}_);[Red]({x})"),
        _ => x,
    };
    let symbol = SYMBOLS.get(sym).copied().unwrap_or("$");
    match cat {
        1 => with_neg(num(sep)),
        2 => {
            let x = match symbol {
                "None" => num(true),
                "₫" => format!("{} \"₫\"", num(true)),
                s => format!("\"{s}\"{}", num(true)),
            };
            with_neg(x)
        }
        3 => {
            let s = if symbol == "None" {
                String::new()
            } else {
                format!("\"{symbol}\"")
            };
            let n = num(true);
            let dash = if dec > 0 {
                format!("\"-\"{}", "?".repeat(dec as usize))
            } else {
                "\"-\"".to_owned()
            };
            format!("_({s}* {n}_);_({s}* ({n});_({s}* {dash}_);_(@_)")
        }
        6 => format!("0{zeros}%"),
        8 => format!("0{zeros}E+00"),
        9 => "@".to_owned(),
        4 | 5 | 7 | 10 | 11 => ty.to_owned(),
        _ => "General".to_owned(),
    }
}

impl SheetWindow {
    /// Open the Format Cells dialog on page `page` (0 Number … 5 Protection).
    pub(super) fn open_format_cells(&mut self, page: usize) -> bool {
        if self.view.editing() {
            self.view.commit();
        }
        let t = |k: &str, f: &str| self.t(k, f);
        let tabs = [
            ("sheetMenu.fcTabNumber", "Number"),
            ("sheetMenu.fcTabAlignment", "Alignment"),
            ("sheetMenu.fcTabFont", "Font"),
            ("sheetMenu.fcTabBorder", "Border"),
            ("sheetMenu.fcTabFill", "Fill"),
            ("sheetMenu.fcTabProtection", "Protection"),
        ]
        .iter()
        .map(|(k, f)| t(k, f))
        .collect();
        let title = t("sheetMenu.cells.formatCells", "Format Cells").replace('…', "");
        let mut f = Form::new(&title, (692.0, 636.0), tabs);
        let fmt = self.view.format().clone();
        let x = self.fc_cell();
        // ------------------------------------------------------------ Number
        let p = Some(0);
        f.add(
            "catLabel",
            p,
            [32.0, 88.0, 0.0, 0.0],
            Kind::Label(t("sheetFc.category", "Category:")),
        );
        let cats: Vec<String> = CATEGORIES.iter().map(|(k, d)| t(k, d)).collect();
        let (cat, dec, sep, neg) = classify(&fmt.num_fmt);
        f.add(
            "cat",
            p,
            [32.0, 108.0, 179.0, 344.0],
            Kind::List {
                items: cats,
                sel: Some(cat),
                top: 0,
            },
        );
        f.add(
            "sampleBox",
            p,
            [225.0, 108.0, 434.0, 50.0],
            Kind::Group(t("sheetFc.sample", "Sample")),
        );
        f.add("sample", p, [232.0, 128.0, 420.0, 24.0], Kind::Canvas);
        f.add(
            "decLabel",
            p,
            [225.0, 168.0, 0.0, 0.0],
            Kind::Label(t("sheetFc.decimals", "Decimal places:")),
        );
        f.add(
            "dec",
            p,
            [329.0, 164.0, 83.0, 25.0],
            Kind::Spin {
                value: if cat == 0 { 2 } else { dec },
                min: 0,
                max: 30,
            },
        );
        f.add(
            "sep",
            p,
            [225.0, 198.0, 260.0, 20.0],
            Kind::Check {
                label: t("sheetFc.separator", "Use 1000 Separator (,)"),
                on: sep,
            },
        );
        f.add(
            "symLabel",
            p,
            [225.0, 204.0, 0.0, 0.0],
            Kind::Label(t("sheetFc.symbol", "Symbol:")),
        );
        let sym = SYMBOLS
            .iter()
            .position(|s| *s != "None" && fmt.num_fmt.contains(s))
            .unwrap_or(1);
        f.add(
            "sym",
            p,
            [329.0, 198.0, 160.0, 25.0],
            Kind::Combo {
                items: SYMBOLS.iter().map(|s| (*s).to_owned()).collect(),
                sel: sym,
            },
        );
        f.add(
            "negLabel",
            p,
            [225.0, 232.0, 0.0, 0.0],
            Kind::Label(t("sheetFc.negative", "Negative numbers:")),
        );
        f.add(
            "neg",
            p,
            [225.0, 252.0, 434.0, 90.0],
            Kind::List {
                items: Vec::new(),
                sel: Some(neg),
                top: 0,
            },
        );
        f.add(
            "typeLabel",
            p,
            [225.0, 168.0, 0.0, 0.0],
            Kind::Label(t("sheetFc.type", "Type:")),
        );
        f.add(
            "code",
            p,
            [225.0, 188.0, 434.0, 24.0],
            Kind::Edit {
                value: fmt.num_fmt.chars().collect(),
                caret: fmt.num_fmt.chars().count(),
            },
        );
        f.add(
            "type",
            p,
            [225.0, 188.0, 434.0, 208.0],
            Kind::List {
                items: Vec::new(),
                sel: None,
                top: 0,
            },
        );
        f.add(
            "desc",
            p,
            [32.0, 480.0, 0.0, 0.0],
            Kind::Note(String::new()),
        );
        // ------------------------------------------------------------ Alignment
        let p = Some(1);
        f.add(
            "taGroup",
            p,
            [32.0, 88.0, 448.0, 0.0],
            Kind::Group(t("sheetFc.textAlignment", "Text alignment")),
        );
        f.add(
            "hLabel",
            p,
            [46.0, 114.0, 0.0, 0.0],
            Kind::Label(t("sheetFc.horizontal", "Horizontal:")),
        );
        let h_items = [
            ("sheetFc.hGeneral", "General", ""),
            ("sheetFc.hLeft", "Left (Indent)", "left"),
            ("sheetFc.hCenter", "Center", "center"),
            ("sheetFc.hRight", "Right (Indent)", "right"),
            ("sheetFc.hFill", "Fill", "fill"),
            ("sheetFc.hJustify", "Justify", "justify"),
            (
                "sheetFc.hCenterAcross",
                "Center Across Selection",
                "centerContinuous",
            ),
            (
                "sheetFc.hDistributed",
                "Distributed (Indent)",
                "distributed",
            ),
        ];
        let h_sel = h_items
            .iter()
            .position(|(_, _, v)| *v == fmt.h_align)
            .unwrap_or(0);
        f.add(
            "h",
            p,
            [46.0, 134.0, 175.0, 26.0],
            Kind::Combo {
                items: h_items.iter().map(|(k, d, _)| t(k, d)).collect(),
                sel: h_sel,
            },
        );
        f.add(
            "indentLabel",
            p,
            [236.0, 142.0, 0.0, 0.0],
            Kind::Label(t("sheetFc.indent", "Indent:")),
        );
        f.add(
            "indent",
            p,
            [236.0, 161.0, 83.0, 25.0],
            Kind::Spin {
                value: x.0 as i32,
                min: 0,
                max: 250,
            },
        );
        f.add(
            "vLabel",
            p,
            [46.0, 167.0, 0.0, 0.0],
            Kind::Label(t("sheetFc.vertical", "Vertical:")),
        );
        let v_items = [
            ("sheetFc.vTop", "Top", "top"),
            ("sheetFc.vCenter", "Center", "center"),
            ("sheetFc.vBottom", "Bottom", "bottom"),
            ("sheetFc.vJustify", "Justify", "justify"),
            ("sheetFc.vDistributed", "Distributed", "distributed"),
        ];
        let v_now = if fmt.v_align.is_empty() {
            "bottom"
        } else {
            fmt.v_align.as_str()
        };
        f.add(
            "v",
            p,
            [46.0, 187.0, 175.0, 26.0],
            Kind::Combo {
                items: v_items.iter().map(|(k, d, _)| t(k, d)).collect(),
                sel: v_items
                    .iter()
                    .position(|(_, _, v)| *v == v_now)
                    .unwrap_or(2),
            },
        );
        f.add(
            "justDist",
            p,
            [46.0, 221.0, 200.0, 20.0],
            Kind::Check {
                label: t("sheetFc.justifyDistributed", "Justify distributed"),
                on: false,
            },
        );
        f.enable("justDist", false);
        f.add(
            "tcGroup",
            p,
            [32.0, 250.0, 448.0, 0.0],
            Kind::Group(t("sheetFc.textControl", "Text control")),
        );
        for (id, y, k, d, on) in [
            ("wrap", 277.0, "sheetFc.wrap", "Wrap text", fmt.wrap),
            ("shrink", 302.0, "sheetMenu.fcShrink", "Shrink to fit", x.2),
            (
                "merge",
                327.0,
                "sheetMenu.fcMerge",
                "Merge cells",
                self.fc_merged(),
            ),
        ] {
            f.add(
                id,
                p,
                [46.0, y, 200.0, 20.0],
                Kind::Check { label: t(k, d), on },
            );
        }
        f.add(
            "rtlGroup",
            p,
            [32.0, 357.0, 448.0, 0.0],
            Kind::Group(t("sheetFc.rtl", "Right-to-left")),
        );
        f.add(
            "dirLabel",
            p,
            [46.0, 384.0, 0.0, 0.0],
            Kind::Label(t("sheetFc.direction", "Text direction:")),
        );
        f.add(
            "dir",
            p,
            [46.0, 404.0, 116.0, 26.0],
            Kind::Combo {
                items: vec![
                    t("sheetFc.dirContext", "Context"),
                    t("sheetFc.dirLtr", "Left-to-Right"),
                    t("sheetFc.dirRtl", "Right-to-Left"),
                ],
                sel: 0,
            },
        );
        f.add(
            "orGroup",
            p,
            [494.0, 88.0, 165.0, 212.0],
            Kind::Group(t("sheetFc.orientation", "Orientation")),
        );
        f.add("vtext", p, [508.0, 113.0, 36.0, 150.0], Kind::Canvas);
        f.add("dial", p, [549.0, 113.0, 79.0, 150.0], Kind::Canvas);
        let rot = x.1;
        let degrees = match rot {
            0..=90 => rot as i32,
            91..=180 => 90 - rot as i32,
            _ => 0,
        };
        f.add(
            "deg",
            p,
            [508.0, 268.0, 83.0, 25.0],
            Kind::Spin {
                value: degrees,
                min: -90,
                max: 90,
            },
        );
        f.add(
            "degLabel",
            p,
            [597.0, 273.0, 0.0, 0.0],
            Kind::Label(t("sheetFc.degrees", "Degrees")),
        );
        // ------------------------------------------------------------ Font
        let p = Some(2);
        f.add(
            "fontLabel",
            p,
            [32.0, 88.0, 0.0, 0.0],
            Kind::Label(t("sheetFc.font", "Font:")),
        );
        f.add(
            "font",
            p,
            [32.0, 108.0, 275.0, 24.0],
            Kind::Edit {
                value: fmt.font.chars().collect(),
                caret: fmt.font.chars().count(),
            },
        );
        let mut fonts: Vec<String> = FONTS.iter().map(|s| (*s).to_owned()).collect();
        if !fmt.font.is_empty() && !fonts.contains(&fmt.font) {
            fonts.push(fmt.font.clone());
        }
        fonts.sort_by_key(|a| a.to_lowercase());
        let font_sel = fonts.iter().position(|n| *n == fmt.font);
        f.add(
            "fonts",
            p,
            [32.0, 133.0, 275.0, 107.0],
            Kind::List {
                items: fonts,
                sel: None,
                top: 0,
            },
        );
        f.set_sel("fonts", font_sel);
        f.add(
            "styleLabel",
            p,
            [321.0, 88.0, 0.0, 0.0],
            Kind::Label(t("sheetFc.fontStyle", "Font style:")),
        );
        let styles = vec![
            t("sheetFc.regular", "Regular"),
            t("sheetFc.italic", "Italic"),
            t("sheetFc.bold", "Bold"),
            t("sheetFc.boldItalic", "Bold Italic"),
        ];
        let style_sel = usize::from(fmt.italic) + 2 * usize::from(fmt.bold);
        f.add(
            "styleName",
            p,
            [321.0, 108.0, 166.0, 24.0],
            Kind::Edit {
                value: styles[style_sel].chars().collect(),
                caret: 0,
            },
        );
        f.add(
            "fstyle",
            p,
            [321.0, 133.0, 166.0, 107.0],
            Kind::List {
                items: styles,
                sel: Some(style_sel),
                top: 0,
            },
        );
        f.add(
            "sizeLabel",
            p,
            [498.0, 88.0, 0.0, 0.0],
            Kind::Label(t("sheetFc.size", "Size:")),
        );
        let size_text = format!("{}", fmt.size);
        f.add(
            "size",
            p,
            [498.0, 108.0, 162.0, 24.0],
            Kind::Edit {
                value: size_text.chars().collect(),
                caret: size_text.chars().count(),
            },
        );
        let sizes: Vec<String> = [8, 9, 10, 11, 12, 14, 16, 18, 20, 22, 24, 26, 28, 36, 48, 72]
            .iter()
            .map(ToString::to_string)
            .collect();
        let size_sel = sizes.iter().position(|s| *s == size_text);
        f.add(
            "sizes",
            p,
            [498.0, 133.0, 162.0, 107.0],
            Kind::List {
                items: sizes,
                sel: None,
                top: 0,
            },
        );
        f.set_sel("sizes", size_sel);
        f.add(
            "ulLabel",
            p,
            [32.0, 245.0, 0.0, 0.0],
            Kind::Label(t("sheetFc.underline", "Underline:")),
        );
        f.add(
            "ul",
            p,
            [32.0, 265.0, 275.0, 28.0],
            Kind::Combo {
                items: vec![
                    t("sheetFc.ulNone", "None"),
                    t("sheetFc.ulSingle", "Single"),
                    t("sheetFc.ulDouble", "Double"),
                    t("sheetFc.ulSingleAcc", "Single Accounting"),
                    t("sheetFc.ulDoubleAcc", "Double Accounting"),
                ],
                sel: usize::from(fmt.underline),
            },
        );
        f.add(
            "colorLabel",
            p,
            [321.0, 245.0, 0.0, 0.0],
            Kind::Label(t("sheetFc.color", "Color:")),
        );
        let colors: Vec<String> = NAMED_COLORS.iter().map(|(k, _)| t(k, k)).collect();
        let color_sel = NAMED_COLORS
            .iter()
            .position(|(_, h)| h.eq_ignore_ascii_case(&fmt.color))
            .unwrap_or(0);
        f.add(
            "color",
            p,
            [321.0, 265.0, 155.0, 28.0],
            Kind::Combo {
                items: colors.clone(),
                sel: color_sel,
            },
        );
        f.add(
            "normal",
            p,
            [498.0, 270.0, 160.0, 20.0],
            Kind::Check {
                label: t("sheetFc.normalFont", "Normal font"),
                on: false,
            },
        );
        f.add(
            "fxGroup",
            p,
            [32.0, 298.0, 275.0, 104.0],
            Kind::Group(t("sheetFc.effects", "Effects")),
        );
        let va = self.fc_vert_align();
        for (id, y, k, d, on) in [
            (
                "strike",
                326.0,
                "sheetMenu.fcStrike",
                "Strikethrough",
                fmt.strike,
            ),
            (
                "sup",
                351.0,
                "sheetFc.superscript",
                "Superscript",
                va == "superscript",
            ),
            (
                "sub",
                376.0,
                "sheetFc.subscript",
                "Subscript",
                va == "subscript",
            ),
        ] {
            f.add(
                id,
                p,
                [46.0, y, 200.0, 20.0],
                Kind::Check { label: t(k, d), on },
            );
        }
        f.add(
            "pvGroup",
            p,
            [321.0, 298.0, 338.0, 104.0],
            Kind::Group(t("sheetFc.preview", "Preview")),
        );
        f.add("fpreview", p, [335.0, 318.0, 310.0, 72.0], Kind::Canvas);
        f.add(
            "fontNote",
            p,
            [32.0, 445.0, 0.0, 0.0],
            Kind::Note(t(
                "sheetFc.fontNote",
                "This is a TrueType font. The same font will be used on both your printer and your screen.",
            )),
        );
        // ------------------------------------------------------------ Border
        let p = Some(3);
        f.add(
            "lineGroup",
            p,
            [32.0, 88.0, 170.0, 298.0],
            Kind::Group(t("sheetFc.line", "Line")),
        );
        f.add(
            "lineLabel",
            p,
            [46.0, 114.0, 0.0, 0.0],
            Kind::Label(t("sheetFc.style", "Style:")),
        );
        f.add("lines", p, [46.0, 134.0, 143.0, 150.0], Kind::Canvas);
        f.add(
            "bcolorLabel",
            p,
            [46.0, 301.0, 0.0, 0.0],
            Kind::Label(t("sheetFc.color", "Color:")),
        );
        f.add(
            "bcolor",
            p,
            [46.0, 320.0, 143.0, 26.0],
            Kind::Combo {
                items: colors.clone(),
                sel: 0,
            },
        );
        f.add(
            "presetGroup",
            p,
            [217.0, 88.0, 340.0, 0.0],
            Kind::Group(t("sheetFc.presets", "Presets")),
        );
        f.add("presets", p, [282.0, 113.0, 224.0, 72.0], Kind::Canvas);
        f.add(
            "borderGroup",
            p,
            [217.0, 196.0, 340.0, 0.0],
            Kind::Group(t("sheetFc.border", "Border")),
        );
        f.add("border", p, [232.0, 220.0, 324.0, 162.0], Kind::Canvas);
        f.add(
            "borderNote",
            p,
            [32.0, 405.0, 0.0, 0.0],
            Kind::Note(t(
                "sheetFc.borderNote",
                "The selected border style can be applied by clicking the presets, preview diagram or the buttons above.",
            )),
        );
        // ------------------------------------------------------------ Fill
        let p = Some(4);
        f.add(
            "bgLabel",
            p,
            [42.0, 88.0, 0.0, 0.0],
            Kind::Label(t("sheetFc.background", "Background Color:")),
        );
        f.add(
            "noColor",
            p,
            [48.0, 113.0, 218.0, 20.0],
            Kind::Button {
                label: t("sheetFc.noColor", "No Color"),
                default: false,
            },
        );
        let palette = self.fc_palette();
        let fill_sel = palette.iter().position(|h| h.eq_ignore_ascii_case(&x.3));
        f.add(
            "bg",
            p,
            [49.0, 145.0, 218.0, 170.0],
            Kind::Swatches {
                colors: palette.iter().filter_map(|h| rgba(h)).collect(),
                cols: 10,
                sel: fill_sel,
                breaks: vec![1, 6],
            },
        );
        f.add(
            "effects",
            p,
            [43.0, 328.0, 112.0, 28.0],
            Kind::Button {
                label: t("sheetFc.fillEffects", "Fill Effects..."),
                default: false,
            },
        );
        f.enable("effects", false);
        f.add(
            "more",
            p,
            [170.0, 328.0, 112.0, 28.0],
            Kind::Button {
                label: t("sheetFc.moreColors", "More Colors..."),
                default: false,
            },
        );
        f.enable("more", false);
        f.add(
            "pcLabel",
            p,
            [359.0, 88.0, 0.0, 0.0],
            Kind::Label(t("sheetFc.patternColor", "Pattern Color:")),
        );
        f.add(
            "pcolor",
            p,
            [359.0, 108.0, 261.0, 28.0],
            Kind::Combo {
                items: colors,
                sel: 0,
            },
        );
        f.add(
            "psLabel",
            p,
            [359.0, 143.0, 0.0, 0.0],
            Kind::Label(t("sheetFc.patternStyle", "Pattern Style:")),
        );
        f.add(
            "pstyle",
            p,
            [359.0, 163.0, 261.0, 28.0],
            Kind::Combo {
                items: PATTERNS.iter().map(|s| (*s).to_owned()).collect(),
                sel: PATTERNS.iter().position(|s| *s == x.4).unwrap_or(0),
            },
        );
        f.add(
            "fsGroup",
            p,
            [32.0, 468.0, 627.0, 90.0],
            Kind::Group(t("sheetFc.sample", "Sample")),
        );
        f.add("fsample", p, [40.0, 488.0, 611.0, 62.0], Kind::Canvas);
        // ------------------------------------------------------------ Protection
        let p = Some(5);
        f.add(
            "locked",
            p,
            [32.0, 92.0, 200.0, 20.0],
            Kind::Check {
                label: t("sheetMenu.fcLocked", "Locked"),
                on: x.5,
            },
        );
        f.add(
            "hidden",
            p,
            [32.0, 117.0, 200.0, 20.0],
            Kind::Check {
                label: t("sheetMenu.fcHidden", "Hidden"),
                on: x.6,
            },
        );
        f.add(
            "protNote",
            p,
            [32.0, 150.0, 0.0, 0.0],
            Kind::Note(t(
                "sheetFc.protectionNote",
                "Locking cells or hiding formulas has no effect until you protect the worksheet (Review tab, Protect Sheet button).",
            )),
        );
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        f.set_tab(page.min(5));
        self.fc = FcState {
            edges: self.fc_edges(),
            border_changed: false,
            line: 6,
            degrees,
            vertical: rot == 255,
            fill: None,
            palette,
            code: fmt.num_fmt.clone(),
        };
        self.form = Some(f);
        self.form_kind = super::form_impl::FormKind::FormatCells;
        self.fc_refresh(true);
        true
    }

    /// Indent, rotation, shrink, fill colour, pattern, locked, hidden of the active cell.
    #[allow(clippy::type_complexity)]
    fn fc_cell(&self) -> (u32, u32, bool, String, String, bool, bool) {
        let (r, c) = self.view.active();
        let mut out = (0, 0, false, String::new(), "none".to_owned(), true, false);
        let Some(w) = self.view.engine().workbook() else {
            return out;
        };
        let style = w
            .sheets
            .get(self.view.sheet())
            .and_then(|ws| ws.cell(r, c))
            .map_or(0, |cell| cell.style);
        let Some(xf) = w.styles.xf(style) else {
            return out;
        };
        if let Some(a) = &xf.alignment {
            out.0 = a.indent.unwrap_or(0);
            out.1 = a.text_rotation.unwrap_or(0);
            out.2 = a.shrink_to_fit;
        }
        if let Some(p) = &xf.protection {
            out.5 = p.locked.unwrap_or(true);
            out.6 = p.hidden.unwrap_or(false);
        }
        if let Some(bo_model::sheet::Fill::Pattern(p)) = w.styles.fills.get(xf.fill_id as usize) {
            let kind = p.pattern.clone().unwrap_or_else(|| "none".into());
            let color = if kind == "solid" { &p.fg } else { &p.bg };
            out.3 = color
                .as_ref()
                .and_then(|c| bo_sheet::color::resolve_color(c, w.theme.as_ref()))
                .map(hex)
                .unwrap_or_default();
            out.4 = if kind == "solid" { "none".into() } else { kind };
        }
        out
    }

    fn fc_vert_align(&self) -> String {
        let (r, c) = self.view.active();
        let Some(w) = self.view.engine().workbook() else {
            return String::new();
        };
        let style = w
            .sheets
            .get(self.view.sheet())
            .and_then(|ws| ws.cell(r, c))
            .map_or(0, |cell| cell.style);
        w.styles
            .xf(style)
            .and_then(|xf| w.styles.fonts.get(xf.font_id as usize))
            .and_then(|f| f.vert_align.clone())
            .unwrap_or_default()
    }

    fn fc_merged(&self) -> bool {
        let (r0, c0, r1, c1) = self.view.range_bounds();
        self.view.engine().workbook().is_some_and(|w| {
            w.sheets.get(self.view.sheet()).is_some_and(|ws| {
                ws.merges.iter().any(|m| {
                    m.first.row <= r1 && m.last.row >= r0 && m.first.col <= c1 && m.last.col >= c0
                })
            })
        })
    }

    /// Edges of the active cell's border (top, bottom, left, right).
    fn fc_edges(&self) -> [(usize, String); EDGES] {
        let mut out: [(usize, String); EDGES] = Default::default();
        let (r, c) = self.view.active();
        let Some(w) = self.view.engine().workbook() else {
            return out;
        };
        let style = w
            .sheets
            .get(self.view.sheet())
            .and_then(|ws| ws.cell(r, c))
            .map_or(0, |cell| cell.style);
        let Some(b) = w
            .styles
            .xf(style)
            .and_then(|xf| w.styles.borders.get(xf.border_id as usize))
        else {
            return out;
        };
        let theme = w.theme.as_ref();
        for (slot, e) in [(0, &b.top), (2, &b.bottom), (3, &b.left), (5, &b.right)] {
            if let Some(st) = e.style.as_deref() {
                let i = LINE_STYLES.iter().position(|s| *s == st).unwrap_or(6);
                let color = e
                    .color
                    .as_ref()
                    .and_then(|c| bo_sheet::color::resolve_color(c, theme))
                    .map(hex)
                    .unwrap_or_default();
                out[slot] = (i, color);
            }
        }
        out
    }

    /// Theme colours with their tints, then the standard colours (reference palette).
    fn fc_palette(&self) -> Vec<String> {
        let theme = self.view.engine().workbook().and_then(|w| w.theme.clone());
        let col = |i: u32, tint: f64| {
            let c = SheetColor {
                theme: Some(i),
                tint: (tint != 0.0).then_some(tint),
                ..SheetColor::default()
            };
            bo_sheet::color::resolve_color(&c, theme.as_ref()).map_or_else(|| "000000".into(), hex)
        };
        let mut out = Vec::new();
        for i in 0..10 {
            out.push(col(i, 0.0));
        }
        let tints = |i: u32| -> [f64; 5] {
            match i {
                0 => [-0.05, -0.15, -0.25, -0.35, -0.5],
                1 => [0.5, 0.35, 0.25, 0.15, 0.05],
                2 => [-0.1, -0.25, -0.5, -0.75, -0.9],
                _ => [0.8, 0.6, 0.4, -0.25, -0.5],
            }
        };
        for row in 0..5 {
            for i in 0..10 {
                out.push(col(i, tints(i)[row]));
            }
        }
        for h in [
            "C00000", "FF0000", "FFC000", "FFFF00", "92D050", "00B050", "00B0F0", "0070C0",
            "002060", "7030A0",
        ] {
            out.push(h.to_owned());
        }
        out
    }

    /// Show the Number page's controls for the chosen category, fill its lists, and update
    /// the description; `keep` keeps the format code of the cell as the custom type.
    fn fc_refresh(&mut self, keep: bool) {
        let desc_keys = [
            ("sheetFc.descGeneral", "General format cells have no specific number format."),
            ("sheetFc.descNumber", "Number is used for general display of numbers. Currency and Accounting offer specialized formatting for monetary value."),
            ("sheetFc.descCurrency", "Currency formats are used for general monetary values. Use Accounting formats to align decimal points in a column."),
            ("sheetFc.descAccounting", "Accounting formats line up the currency symbols and decimal points in a column."),
            ("sheetFc.descDate", "Date formats display date and time serial numbers as date values."),
            ("sheetFc.descTime", "Time formats display date and time serial numbers as time values."),
            ("sheetFc.descPercentage", "Percentage formats multiply the cell value by 100 and display the result with a percent symbol."),
            ("sheetFc.descFraction", "Fraction formats display numbers as fractions."),
            ("sheetFc.descScientific", "Scientific formats display numbers in exponential notation."),
            ("sheetFc.descText", "Text format cells are treated as text even when a number is in the cell. The cell is displayed exactly as entered."),
            ("sheetFc.descSpecial", "Special formats are useful for tracking list and database values."),
            ("sheetFc.descCustom", "Type the number format code, using one of the existing codes as a starting point."),
        ];
        let descs: Vec<String> = desc_keys.iter().map(|(k, d)| self.t(k, d)).collect();
        let code_now = self.fc.code.clone();
        let Some(f) = self.form.as_mut() else {
            return;
        };
        let cat = f.sel("cat").unwrap_or(0);
        let dec = matches!(cat, 1 | 2 | 3 | 6 | 8);
        f.show("decLabel", dec);
        f.show("dec", dec);
        f.show("sep", cat == 1);
        f.show("symLabel", matches!(cat, 2 | 3));
        f.show("sym", matches!(cat, 2 | 3));
        let neg = matches!(cat, 1 | 2);
        f.show("negLabel", neg);
        f.show("neg", neg);
        let typed = matches!(cat, 4 | 5 | 7 | 10 | 11);
        f.show("typeLabel", typed);
        f.show("type", typed);
        f.show("code", cat == 11);
        if let Some(c) = f.get_mut("type") {
            c.rect = if cat == 11 {
                [225.0, 218.0, 434.0, 208.0]
            } else {
                [225.0, 188.0, 434.0, 208.0]
            };
        }
        if let Some(c) = f.get_mut("sep") {
            c.rect[1] = if cat == 1 { 198.0 } else { 0.0 };
        }
        if let Some(c) = f.get_mut("negLabel") {
            c.rect[1] = if cat == 1 { 232.0 } else { 236.0 };
        }
        let types: Vec<String> = match cat {
            4 => DATE_TYPES.iter().map(|s| (*s).to_owned()).collect(),
            5 => TIME_TYPES.iter().map(|s| (*s).to_owned()).collect(),
            7 => FRACTION_TYPES.iter().map(|s| (*s).to_owned()).collect(),
            10 => SPECIAL_TYPES.iter().map(|s| (*s).to_owned()).collect(),
            11 => {
                let mut v: Vec<String> = CUSTOM_TYPES.iter().map(|s| (*s).to_owned()).collect();
                if !code_now.is_empty() && !v.contains(&code_now) {
                    v.push(code_now.clone());
                }
                v
            }
            _ => Vec::new(),
        };
        let pick = types
            .iter()
            .position(|t| *t == code_now)
            .or((!types.is_empty()).then_some(0));
        let same = matches!(f.get("type").map(|c| &c.kind), Some(Kind::List { items, .. }) if *items == types);
        if !same {
            f.set_items("type", types, pick);
        }
        if cat == 11 && !keep {
            let ty = f.text("type");
            f.set_text("code", &ty);
        }
        // The description sits under the sample when the page has no other controls
        // (General, Text), else at the bottom (reference).
        // Under the sample it starts right of the category list, not over it.
        if let Some(c) = f.get_mut("desc") {
            let beside = matches!(cat, 0 | 9);
            c.rect[0] = if beside { 225.0 } else { 32.0 };
            c.rect[1] = if beside { 168.0 } else { 480.0 };
        }
        if let Some(Kind::Note(s)) = f.get_mut("desc").map(|c| &mut c.kind) {
            s.clone_from(&descs[cat]);
        }
        self.fc_negatives();
    }

    /// Sample texts of the negative-number list (−1234.10 in the chosen format).
    fn fc_negatives(&mut self) {
        let Some(f) = self.form.as_ref() else {
            return;
        };
        let cat = f.sel("cat").unwrap_or(0);
        if !matches!(cat, 1 | 2) {
            return;
        }
        let (dec, sep, sym) = (f.num("dec"), f.on("sep"), f.sel("sym").unwrap_or(1));
        let items: Vec<String> = (0..4)
            .map(|neg| {
                let code = code_for(cat, dec, sep || cat == 2, neg, sym, "");
                self.fc_format(&CellValue::Number(-1234.1), &code)
            })
            .collect();
        if let Some(f) = self.form.as_mut() {
            let sel = f.sel("neg").unwrap_or(0);
            if let Some(Kind::List { items: it, .. }) = f.get_mut("neg").map(|c| &mut c.kind) {
                *it = items;
            }
            f.set_sel("neg", Some(sel));
        }
    }

    fn fc_format(&self, v: &CellValue, code: &str) -> String {
        let opts = bo_sheet::numfmt::FormatOptions {
            locale: *self.view.engine().number_locale(),
            date1904: self.view.engine().workbook().is_some_and(|w| w.date1904),
        };
        bo_sheet::numfmt::format_value(v, code, &opts).text
    }

    /// The format code the Number page describes.
    fn fc_code(&self) -> String {
        let Some(f) = self.form.as_ref() else {
            return String::new();
        };
        let cat = f.sel("cat").unwrap_or(0);
        let ty = if cat == 11 {
            f.text("code")
        } else {
            f.text("type")
        };
        code_for(
            cat,
            f.num("dec"),
            f.on("sep"),
            f.sel("neg").unwrap_or(0),
            f.sel("sym").unwrap_or(1),
            &ty,
        )
    }

    /// A form event of the Format Cells dialog.
    pub(super) fn fc_event(&mut self, ev: &Event) -> bool {
        match ev {
            Event::Cancel => {
                self.form = None;
            }
            Event::Ok => {
                self.fc_apply();
                self.form = None;
            }
            Event::Changed("cat") => self.fc_refresh(false),
            Event::Changed("dec" | "sep" | "sym") => self.fc_negatives(),
            Event::Changed("type") => {
                if let Some(f) = self.form.as_mut() {
                    if f.sel("cat") == Some(11) {
                        let ty = f.text("type");
                        f.set_text("code", &ty);
                    }
                }
            }
            Event::Changed("fonts") => {
                if let Some(f) = self.form.as_mut() {
                    let n = f.text("fonts");
                    f.set_text("font", &n);
                }
            }
            Event::Changed("fstyle") => {
                if let Some(f) = self.form.as_mut() {
                    let n = f.text("fstyle");
                    f.set_text("styleName", &n);
                }
            }
            Event::Changed("sizes") => {
                if let Some(f) = self.form.as_mut() {
                    let n = f.text("sizes");
                    f.set_text("size", &n);
                }
            }
            Event::Changed("sup") => {
                if let Some(f) = self.form.as_mut() {
                    if f.on("sup") {
                        f.set_on("sub", false);
                    }
                }
            }
            Event::Changed("sub") => {
                if let Some(f) = self.form.as_mut() {
                    if f.on("sub") {
                        f.set_on("sup", false);
                    }
                }
            }
            Event::Changed("normal") => {
                if let Some(f) = self.form.as_mut() {
                    if f.on("normal") {
                        f.set_text("font", "Calibri");
                        f.set_text("size", "11");
                        f.set_sel("fstyle", Some(0));
                        f.set_text("styleName", &f.text("fstyle"));
                        f.set_sel("ul", Some(0));
                        f.set_sel("color", Some(0));
                        for id in ["strike", "sup", "sub"] {
                            f.set_on(id, false);
                        }
                    }
                }
            }
            Event::Changed("deg") => {
                if let Some(f) = self.form.as_ref() {
                    self.fc.degrees = f.num("deg");
                    self.fc.vertical = false;
                }
            }
            Event::Changed("bg") => {
                if let Some(i) = self.form.as_ref().and_then(|f| f.sel("bg")) {
                    self.fc.fill = self.fc.palette.get(i).cloned();
                }
            }
            Event::Clicked("noColor") => {
                self.fc.fill = Some(String::new());
                if let Some(f) = self.form.as_mut() {
                    f.set_sel("bg", None);
                    f.set_sel("pstyle", Some(0));
                }
            }
            Event::Clicked("fonts" | "sizes" | "fstyle" | "type" | "cat") => {
                // Double click on a list item: OK.
                self.fc_apply();
                self.form = None;
            }
            Event::Canvas(id, x, y) => self.fc_canvas_click(id, *x, *y),
            _ => {}
        }
        true
    }

    fn fc_canvas_click(&mut self, id: &str, x: f32, y: f32) {
        match id {
            "lines" => {
                let col = usize::from(x >= 72.0);
                let row = ((y - 4.0) / 20.5).floor().clamp(0.0, 6.0) as usize;
                self.fc.line = col * 7 + row;
            }
            "presets" => {
                let i = (x / 80.0).floor() as usize;
                let line = (self.fc.line.max(1), self.fc_bcolor());
                match i {
                    0 => self.fc.edges = Default::default(),
                    1 => {
                        for e in [0, 2, 3, 5] {
                            self.fc.edges[e] = line.clone();
                        }
                    }
                    _ => {
                        let (r0, c0, r1, c1) = self.view.range_bounds();
                        if r1 > r0 {
                            self.fc.edges[1] = line.clone();
                        }
                        if c1 > c0 {
                            self.fc.edges[4] = line;
                        }
                    }
                }
                self.fc.border_changed = true;
            }
            "border" => {
                // Buttons left (top, middle, bottom, diagonal up) and below (diagonal
                // down… left, centre, right, diagonal down); clicks in the box toggle the
                // nearest edge.
                let edge = if x < 40.0 {
                    match y {
                        y if y < 36.0 => Some(0),
                        y if y < 84.0 => Some(1),
                        y if y < 120.0 => Some(2),
                        _ => Some(6),
                    }
                } else if y > 132.0 {
                    match x {
                        x if x < 90.0 => Some(3),
                        x if x < 200.0 => Some(4),
                        x if x < 280.0 => Some(5),
                        _ => Some(7),
                    }
                } else {
                    let (bx, by, bw, bh) = (50.0, 1.0, 225.0, 128.0);
                    let d = [
                        (y - by).abs(),
                        (y - (by + bh / 2.0)).abs(),
                        (y - (by + bh)).abs(),
                        (x - bx).abs(),
                        (x - (bx + bw / 2.0)).abs(),
                        (x - (bx + bw)).abs(),
                    ];
                    d.iter()
                        .enumerate()
                        .min_by(|a, b| a.1.total_cmp(b.1))
                        .map(|(i, _)| i)
                };
                if let Some(e) = edge {
                    let line = (self.fc.line.max(1), self.fc_bcolor());
                    self.fc.edges[e] = if self.fc.edges[e] == line {
                        (0, String::new())
                    } else {
                        line
                    };
                    self.fc.border_changed = true;
                }
            }
            "vtext" => self.fc.vertical = !self.fc.vertical,
            "dial" => {
                // Angle of the point from the dial's centre on its left edge.
                let (cx, cy) = (6.0, 75.0);
                let a = (-(y - cy)).atan2((x - cx).max(0.0)).to_degrees().round() as i32;
                self.fc.degrees = a.clamp(-90, 90);
                self.fc.vertical = false;
                let d = self.fc.degrees;
                if let Some(f) = self.form.as_mut() {
                    f.set_num("deg", d);
                }
            }
            _ => {}
        }
    }

    fn fc_bcolor(&self) -> String {
        let i = self
            .form
            .as_ref()
            .and_then(|f| f.sel("bcolor"))
            .unwrap_or(0);
        NAMED_COLORS
            .get(i)
            .map(|(_, h)| (*h).to_owned())
            .unwrap_or_default()
    }

    /// Strings the owner-drawn areas show (measured at layout time).
    pub(super) fn fc_strings(&self) -> Vec<String> {
        let mut v = vec![
            self.t("sheetFc.text", "Text"),
            self.t("sheetFc.none", "None"),
            self.t("sheetFc.outline", "Outline"),
            self.t("sheetFc.inside", "Inside"),
            self.fc_sample(),
        ];
        if let Some(f) = self.form.as_ref() {
            v.push(f.text("font"));
        }
        v
    }

    pub(super) fn fc_sample(&self) -> String {
        let (r, c) = self.view.active();
        let value = self
            .view
            .engine()
            .workbook()
            .and_then(|w| w.sheets.get(self.view.sheet()))
            .and_then(|ws| ws.cell(r, c))
            .map_or(CellValue::Empty, |cell| cell.value.clone());
        if matches!(value, CellValue::Empty) {
            return String::new();
        }
        self.fc_format(&value, &self.fc_code())
    }

    /// Draw an owner-drawn area of the Format Cells dialog.
    #[allow(clippy::too_many_lines)]
    pub(super) fn fc_paint_canvas(&self, f: &Form, id: &str, r: [f32; 4], p: &mut dyn Painter) {
        let k = r[2] / f.get(id).map_or(r[2], |c| c.rect[2]).max(1.0);
        let font = f.font();
        let ink = Rgba(0, 0, 0, 255);
        let line_h = font.size * 1.33;
        match id {
            "sample" => {
                let s = self.fc_sample();
                p.text_font(&s, r[0] + 2.0 * k, r[1] + (r[3] - line_h) / 2.0, &font, ink);
            }
            "fpreview" => {
                let name = f.text("font");
                let pt = f
                    .text("size")
                    .replace(',', ".")
                    .parse::<f32>()
                    .unwrap_or(11.0);
                let style = f.sel("fstyle").unwrap_or(0);
                let pf = crate::Font {
                    family: if name.trim().is_empty() {
                        "Calibri"
                    } else {
                        name.trim()
                    },
                    size: (pt * 96.0 / 72.0 * 1.25 * k).clamp(8.0, 60.0),
                    weight: if style >= 2 { 700 } else { 400 },
                    italic: style % 2 == 1,
                    underline: f.sel("ul").unwrap_or(0) > 0,
                };
                p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0x64, 0x64, 0x64, 255));
                p.fill_rect(
                    r[0] + 1.0,
                    r[1] + 1.0,
                    r[2] - 2.0,
                    r[3] - 2.0,
                    Rgba(255, 255, 255, 255),
                );
                let color = NAMED_COLORS
                    .get(f.sel("color").unwrap_or(0))
                    .and_then(|(_, h)| rgba(h))
                    .unwrap_or(ink);
                let tw = f.width(&name).max(10.0) * pf.size / font.size;
                let tx = (r[0] + (r[2] - tw) / 2.0).round();
                let ty = (r[1] + (r[3] - pf.size * 1.33) / 2.0).round();
                p.push_clip(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0);
                p.text_font(&name, tx, ty, &pf, color);
                if f.on("strike") {
                    p.fill_rect(tx, ty + pf.size * 0.75, tw, 1.0, color);
                }
                p.pop_clip();
                // Baseline rules either side, as on the reference.
                let by = (r[1] + r[3] * 0.62).round();
                p.fill_rect(r[0] + 1.0, by, (tx - r[0] - 8.0 * k).max(0.0), 1.0, ink);
                let rx = tx + tw + 8.0 * k;
                p.fill_rect(rx, by, (r[0] + r[2] - 1.0 - rx).max(0.0), 1.0, ink);
            }
            "lines" => {
                p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0x82, 0x87, 0x90, 255));
                p.fill_rect(
                    r[0] + 1.0,
                    r[1] + 1.0,
                    r[2] - 2.0,
                    r[3] - 2.0,
                    Rgba(255, 255, 255, 255),
                );
                for (i, st) in LINE_STYLES.iter().enumerate() {
                    let (col, row) = (i / 7, i % 7);
                    let x0 = r[0] + (6.0 + col as f32 * 75.0) * k;
                    let y = (r[1] + (14.0 + row as f32 * 20.5) * k).round();
                    let w = 58.0 * k;
                    if self.fc.line == i {
                        p.fill_rect(
                            x0 - 3.0 * k,
                            y - 8.0 * k,
                            w + 6.0 * k,
                            16.0 * k,
                            Rgba(0xcc, 0xe8, 0xff, 255),
                        );
                    }
                    if *st == "none" {
                        let s = self.t("sheetFc.none", "None");
                        p.text_font(&s, x0 + 12.0 * k, y - line_h / 2.0, &font, ink);
                    } else {
                        paint_line(p, x0, y, w, st, ink);
                    }
                }
            }
            "presets" => {
                let labels = [
                    self.t("sheetFc.none", "None"),
                    self.t("sheetFc.outline", "Outline"),
                    self.t("sheetFc.inside", "Inside"),
                ];
                let (r0, c0, r1, c1) = self.view.range_bounds();
                let inside_ok = r1 > r0 || c1 > c0;
                for (i, label) in labels.iter().enumerate() {
                    let bx = r[0] + i as f32 * 80.0 * k;
                    let (bw, bh) = (64.0 * k, 50.0 * k);
                    p.fill_rect(bx, r[1], bw, bh, Rgba(0xd0, 0xd0, 0xd0, 255));
                    p.fill_rect(
                        bx + 1.0,
                        r[1] + 1.0,
                        bw - 2.0,
                        bh - 2.0,
                        Rgba(0xfd, 0xfd, 0xfd, 255),
                    );
                    let (gx, gy, gw, gh) = (bx + 16.0 * k, r[1] + 9.0 * k, 32.0 * k, 32.0 * k);
                    let light = Rgba(0xb4, 0xb4, 0xb4, 255);
                    let dark = Rgba(0x40, 0x40, 0x40, 255);
                    let outer = if i == 1 { dark } else { light };
                    let inner = if i == 2 && inside_ok { dark } else { light };
                    p.fill_rect(gx, gy, gw, 1.0, outer);
                    p.fill_rect(gx, gy + gh, gw + 1.0, 1.0, outer);
                    p.fill_rect(gx, gy, 1.0, gh, outer);
                    p.fill_rect(gx + gw, gy, 1.0, gh, outer);
                    p.fill_rect(gx, (gy + gh / 2.0).round(), gw, 1.0, inner);
                    p.fill_rect((gx + gw / 2.0).round(), gy, 1.0, gh, inner);
                    let tw = f.width(label);
                    let col = if i == 2 && !inside_ok {
                        Rgba(0x83, 0x83, 0x83, 255)
                    } else {
                        ink
                    };
                    p.text_font(
                        label,
                        (bx + (bw - tw) / 2.0).round(),
                        r[1] + 56.0 * k,
                        &font,
                        col,
                    );
                }
            }
            "border" => {
                let btn = |p: &mut dyn Painter, x: f32, y: f32, on: bool| {
                    let s = 32.0 * k;
                    p.fill_rect(
                        r[0] + x * k,
                        r[1] + y * k,
                        s,
                        s,
                        Rgba(0xd0, 0xd0, 0xd0, 255),
                    );
                    p.fill_rect(
                        r[0] + x * k + 1.0,
                        r[1] + y * k + 1.0,
                        s - 2.0,
                        s - 2.0,
                        if on {
                            Rgba(0xcc, 0xe8, 0xff, 255)
                        } else {
                            Rgba(0xfd, 0xfd, 0xfd, 255)
                        },
                    );
                };
                let e = &self.fc.edges;
                // Each button shows a dotted square with its edge drawn solid.
                let glyph = |p: &mut dyn Painter, bx: f32, by: f32, slot: usize| {
                    let (x0, y0) = (r[0] + (bx + 8.0) * k, r[1] + (by + 8.0) * k);
                    let s = 16.0 * k;
                    let grey = Rgba(0xa0, 0xa0, 0xa0, 255);
                    let mut d = 0.0;
                    while d < s {
                        for (x, y) in [
                            (x0 + d, y0),
                            (x0 + d, y0 + s),
                            (x0, y0 + d),
                            (x0 + s, y0 + d),
                        ] {
                            p.fill_rect(x.round(), y.round(), 1.0, 1.0, grey);
                        }
                        d += 2.0;
                    }
                    let seg = match slot {
                        0 => [x0, y0, x0 + s, y0],
                        1 => [x0, y0 + s / 2.0, x0 + s, y0 + s / 2.0],
                        2 => [x0, y0 + s, x0 + s, y0 + s],
                        3 => [x0, y0, x0, y0 + s],
                        4 => [x0 + s / 2.0, y0, x0 + s / 2.0, y0 + s],
                        5 => [x0 + s, y0, x0 + s, y0 + s],
                        6 => [x0, y0 + s, x0 + s, y0],
                        _ => [x0, y0, x0 + s, y0 + s],
                    };
                    p.path(
                        &[PathSeg::M(seg[0], seg[1]), PathSeg::L(seg[2], seg[3])],
                        None,
                        Some((ink, 1.0)),
                    );
                };
                for (slot, y) in [(0, 2.0), (1, 51.0), (2, 100.0), (6, 134.0)] {
                    btn(p, 0.0, y, e[slot].0 > 0);
                    glyph(p, 0.0, y, slot);
                }
                for (slot, x) in [(3, 50.0), (4, 146.0), (5, 241.0), (7, 290.0)] {
                    btn(p, x, 134.0, e[slot].0 > 0);
                    glyph(p, x, 134.0, slot);
                }
                // The preview box with the text and the chosen edges.
                let (bx, by, bw, bh) = (r[0] + 50.0 * k, r[1] + 1.0 * k, 225.0 * k, 128.0 * k);
                p.fill_rect(bx, by, bw, bh, Rgba(0x82, 0x87, 0x90, 255));
                p.fill_rect(
                    bx + 1.0,
                    by + 1.0,
                    bw - 2.0,
                    bh - 2.0,
                    Rgba(255, 255, 255, 255),
                );
                let pad = 10.0 * k;
                let (ix, iy, iw, ih) = (bx + pad, by + pad, bw - 2.0 * pad, bh - 2.0 * pad);
                let text = self.t("sheetFc.text", "Text");
                let tw = f.width(&text);
                let (r0, c0, r1, c1) = self.view.range_bounds();
                let rows = if r1 > r0 { 2 } else { 1 };
                let cols = if c1 > c0 { 2 } else { 1 };
                for ry in 0..rows {
                    for cx in 0..cols {
                        let cw = iw / cols as f32;
                        let chh = ih / rows as f32;
                        p.text_font(
                            &text,
                            (ix + cx as f32 * cw + (cw - tw) / 2.0).round(),
                            (iy + ry as f32 * chh + (chh - line_h) / 2.0).round(),
                            &font,
                            ink,
                        );
                    }
                }
                let col = |i: usize| rgba(&e[i].1).unwrap_or(ink);
                let st = |i: usize| LINE_STYLES.get(e[i].0).copied().unwrap_or("thin");
                if e[0].0 > 0 {
                    paint_line(p, ix, iy, iw, st(0), col(0));
                }
                if e[1].0 > 0 {
                    paint_line(p, ix, (iy + ih / 2.0).round(), iw, st(1), col(1));
                }
                if e[2].0 > 0 {
                    paint_line(p, ix, iy + ih, iw, st(2), col(2));
                }
                for (slot, x) in [(3, ix), (4, (ix + iw / 2.0).round()), (5, ix + iw)] {
                    if e[slot].0 > 0 {
                        p.fill_rect(
                            x,
                            iy,
                            if st(slot) == "thin" { 1.0 } else { 2.0 },
                            ih,
                            col(slot),
                        );
                    }
                }
                if e[6].0 > 0 {
                    p.path(
                        &[PathSeg::M(ix, iy + ih), PathSeg::L(ix + iw, iy)],
                        None,
                        Some((col(6), 1.0)),
                    );
                }
                if e[7].0 > 0 {
                    p.path(
                        &[PathSeg::M(ix, iy), PathSeg::L(ix + iw, iy + ih)],
                        None,
                        Some((col(7), 1.0)),
                    );
                }
            }
            "vtext" => {
                let on = self.fc.vertical;
                p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0x82, 0x87, 0x90, 255));
                p.fill_rect(
                    r[0] + 1.0,
                    r[1] + 1.0,
                    r[2] - 2.0,
                    r[3] - 2.0,
                    if on {
                        Rgba(0xcc, 0xe8, 0xff, 255)
                    } else {
                        Rgba(255, 255, 255, 255)
                    },
                );
                let text = self.t("sheetFc.text", "Text");
                for (i, ch) in text.chars().enumerate() {
                    p.text_font(
                        &ch.to_string(),
                        r[0] + r[2] / 2.0 - 4.0 * k,
                        r[1] + (42.0 + i as f32 * 17.0) * k,
                        &font,
                        ink,
                    );
                }
            }
            "dial" => {
                p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0x82, 0x87, 0x90, 255));
                p.fill_rect(
                    r[0] + 1.0,
                    r[1] + 1.0,
                    r[2] - 2.0,
                    r[3] - 2.0,
                    Rgba(255, 255, 255, 255),
                );
                let (cx, cy) = (r[0] + 6.0 * k, r[1] + 75.0 * k);
                let rad = 66.0 * k;
                for step in 0..=12 {
                    let a = (-90.0 + step as f32 * 15.0).to_radians();
                    let (x, y) = (cx + rad * a.cos(), cy - rad * a.sin());
                    let s = if step % 6 == 0 { 5.0 * k } else { 2.0 * k };
                    p.fill_rect(
                        (x - s / 2.0).round(),
                        (y - s / 2.0).round(),
                        s.round(),
                        s.round(),
                        ink,
                    );
                }
                let text = self.t("sheetFc.text", "Text");
                let deg = if self.fc.vertical { 0 } else { self.fc.degrees };
                let a = (deg as f32).to_radians();
                let tw = f.width(&text);
                p.text_font(&text, cx + 2.0 * k, cy - line_h / 2.0, &font, ink);
                let (x0, y0) = (cx + (tw + 6.0 * k) * a.cos(), cy - (tw + 6.0 * k) * a.sin());
                let (x1, y1) = (
                    cx + (rad - 4.0 * k) * a.cos(),
                    cy - (rad - 4.0 * k) * a.sin(),
                );
                p.path(
                    &[PathSeg::M(x0, y0), PathSeg::L(x1, y1)],
                    None,
                    Some((ink, 1.0)),
                );
                let red = Rgba(0xd4, 0x1e, 0x1e, 255);
                p.fill_rect(
                    (x1 - 2.5 * k).round(),
                    (y1 - 2.5 * k).round(),
                    (5.0 * k).round(),
                    (5.0 * k).round(),
                    red,
                );
            }
            "fsample" => {
                let bg = match &self.fc.fill {
                    Some(h) => rgba(h),
                    None => rgba(&self.fc_cell().3),
                }
                .unwrap_or(Rgba(255, 255, 255, 255));
                p.fill_rect(r[0], r[1], r[2], r[3], bg);
                let pat = PATTERNS
                    .get(f.sel("pstyle").unwrap_or(0))
                    .copied()
                    .unwrap_or("none");
                if !matches!(pat, "none" | "solid") {
                    let fg = NAMED_COLORS
                        .get(f.sel("pcolor").unwrap_or(0))
                        .and_then(|(_, h)| rgba(h))
                        .unwrap_or(ink);
                    paint_pattern(p, r, pat, fg);
                }
            }
            _ => {}
        }
    }

    /// Apply the dialog to the selection.
    #[allow(clippy::too_many_lines)]
    fn fc_apply(&mut self) {
        let Some(f) = self.form.as_ref() else {
            return;
        };
        let fmt = self.view.format().clone();
        let x = self.fc_cell();
        let mut patch = serde_json::Map::new();
        let code = self.fc_code();
        let had = if fmt.num_fmt.is_empty() {
            "General".to_owned()
        } else {
            fmt.num_fmt.clone()
        };
        if code != had && !code.is_empty() {
            patch.insert("numFmt".into(), json!(code));
        }
        let h = [
            "",
            "left",
            "center",
            "right",
            "fill",
            "justify",
            "centerContinuous",
            "distributed",
        ][f.sel("h").unwrap_or(0).min(7)];
        if h != fmt.h_align {
            patch.insert("hAlign".into(), json!(h));
        }
        let v =
            ["top", "center", "bottom", "justify", "distributed"][f.sel("v").unwrap_or(2).min(4)];
        let v_now = if fmt.v_align.is_empty() {
            "bottom"
        } else {
            fmt.v_align.as_str()
        };
        if v != v_now {
            patch.insert("vAlign".into(), json!(v));
        }
        let indent = f.num("indent").max(0) as u32;
        if indent != x.0 {
            patch.insert("indent".into(), json!(indent));
        }
        let rot = if self.fc.vertical {
            255
        } else if self.fc.degrees >= 0 {
            self.fc.degrees as u32
        } else {
            (90 - self.fc.degrees) as u32
        };
        if rot != x.1 {
            patch.insert("rotation".into(), json!(rot));
        }
        for (id, key, was) in [
            ("wrap", "wrap", fmt.wrap),
            ("shrink", "shrink", x.2),
            ("strike", "strike", fmt.strike),
            ("locked", "locked", x.5),
            ("hidden", "formulaHidden", x.6),
        ] {
            if f.on(id) != was {
                patch.insert(key.into(), json!(f.on(id)));
            }
        }
        let font = f.text("font").trim().to_owned();
        if !font.is_empty() && font != fmt.font {
            patch.insert("fontName".into(), json!(font));
        }
        if let Ok(pt) = f.text("size").trim().replace(',', ".").parse::<f64>() {
            if (1.0..=409.0).contains(&pt) && (pt - fmt.size).abs() > 1e-6 {
                patch.insert("fontSize".into(), json!(pt));
            }
        }
        let style = f.sel("fstyle").unwrap_or(0);
        if (style >= 2) != fmt.bold {
            patch.insert("bold".into(), json!(style >= 2));
        }
        if (style % 2 == 1) != fmt.italic {
            patch.insert("italic".into(), json!(style % 2 == 1));
        }
        let ul = f.sel("ul").unwrap_or(0);
        if (ul > 0) != fmt.underline {
            let kind = match ul {
                0 => "none",
                2 => "double",
                3 => "singleAccounting",
                4 => "doubleAccounting",
                _ => "single",
            };
            patch.insert("underline".into(), json!(kind));
        }
        let color = NAMED_COLORS
            .get(f.sel("color").unwrap_or(0))
            .map_or("", |(_, h)| *h);
        if !color.eq_ignore_ascii_case(&fmt.color) && (!color.is_empty() || fmt.color != "000000") {
            patch.insert("fontColor".into(), json!(color));
        }
        let va = self.fc_vert_align();
        let want = if f.on("sup") {
            "superscript"
        } else if f.on("sub") {
            "subscript"
        } else {
            ""
        };
        if want != va && !(want.is_empty() && va == "baseline") {
            patch.insert(
                "vertAlign".into(),
                json!(if want.is_empty() { "baseline" } else { want }),
            );
        }
        let pat = PATTERNS
            .get(f.sel("pstyle").unwrap_or(0))
            .copied()
            .unwrap_or("none");
        if !matches!(pat, "none" | "solid") && pat != x.4 {
            let pc = NAMED_COLORS
                .get(f.sel("pcolor").unwrap_or(0))
                .map_or("", |(_, h)| *h);
            patch.insert("fillPattern".into(), json!(pat));
            patch.insert("patternColor".into(), json!(pc));
            patch.insert(
                "fillColor".into(),
                json!(self.fc.fill.clone().unwrap_or_else(|| x.3.clone())),
            );
        } else if let Some(fill) = &self.fc.fill {
            if !fill.eq_ignore_ascii_case(&x.3) || x.4 != "none" {
                patch.insert("fillColor".into(), json!(fill));
            }
        }
        let range = self.view.rng();
        let sheet = self.view.sheet();
        let merge = f.on("merge");
        let mut cmds = Vec::new();
        if !patch.is_empty() {
            cmds.push(json!({ "cmd": "format", "sheet": sheet, "range": range, "patch": patch }));
        }
        if merge != self.fc_merged() {
            cmds.push(if merge {
                json!({ "cmd": "merge", "sheet": sheet, "range": range, "center": false })
            } else {
                json!({ "cmd": "unmerge", "sheet": sheet, "range": range })
            });
        }
        if self.fc.border_changed {
            cmds.push(json!({
                "cmd": "borders", "sheet": sheet, "range": range, "spec": { "preset": "none" },
            }));
            let e = &self.fc.edges;
            let mut add = |preset: &str, i: usize| {
                if e[i].0 > 0 {
                    let mut spec = json!({ "preset": preset, "style": LINE_STYLES[e[i].0] });
                    if !e[i].1.is_empty() {
                        spec["color"] = json!(e[i].1);
                    }
                    cmds.push(
                        json!({ "cmd": "borders", "sheet": sheet, "range": range, "spec": spec }),
                    );
                }
            };
            add("top", 0);
            add("bottom", 2);
            add("left", 3);
            add("right", 5);
            if e[1].0 > 0 || e[4].0 > 0 {
                add("inside", if e[1].0 > 0 { 1 } else { 4 });
            }
        }
        if !cmds.is_empty() {
            self.view.run(json!({ "cmd": "batch", "cmds": cmds }));
        }
    }
}

/// A border line style drawn horizontally from (x, y) over `w` pixels: dash patterns of
/// (on, off) lengths and the line's thickness.
fn paint_line(p: &mut dyn Painter, x: f32, y: f32, w: f32, style: &str, c: Rgba) {
    let (pattern, h): (&[f32], f32) = match style {
        "hair" => (&[1.0, 1.0], 1.0),
        "dotted" => (&[2.0, 2.0], 1.0),
        "dashDotDot" => (&[6.0, 2.0, 2.0, 2.0, 2.0, 2.0], 1.0),
        "dashDot" => (&[6.0, 2.0, 2.0, 2.0], 1.0),
        "dashed" => (&[4.0, 2.0], 1.0),
        "mediumDashDotDot" => (&[8.0, 3.0, 3.0, 3.0, 3.0, 3.0], 2.0),
        "slantDashDot" => (&[9.0, 2.0, 3.0, 2.0], 2.0),
        "mediumDashDot" => (&[9.0, 3.0, 3.0, 3.0], 2.0),
        "mediumDashed" => (&[8.0, 4.0], 2.0),
        "medium" => (&[], 2.0),
        "thick" => (&[], 3.0),
        _ => (&[], 1.0),
    };
    if style == "double" {
        p.fill_rect(x, y - 1.0, w, 1.0, c);
        p.fill_rect(x, y + 1.0, w, 1.0, c);
        return;
    }
    if pattern.is_empty() {
        p.fill_rect(x, y, w, h, c);
        return;
    }
    let mut at = x;
    let mut k = 0;
    while at < x + w {
        let len = pattern[k % pattern.len()];
        if k % 2 == 0 {
            p.fill_rect(at, y, len.min(x + w - at), h, c);
        }
        at += len;
        k += 1;
    }
}

/// A fill pattern (§18.18.55) in `r` drawn in `fg` (approximate pixel patterns).
fn paint_pattern(p: &mut dyn Painter, r: [f32; 4], kind: &str, fg: Rgba) {
    let step = 4.0;
    let (horiz, vert, diag_down, diag_up, density) = match kind {
        "darkHorizontal" => (2.0, 0.0, false, false, 0.0),
        "lightHorizontal" => (1.0, 0.0, false, false, 0.0),
        "darkVertical" => (0.0, 2.0, false, false, 0.0),
        "lightVertical" => (0.0, 1.0, false, false, 0.0),
        "darkDown" | "lightDown" => (0.0, 0.0, true, false, 0.0),
        "darkUp" | "lightUp" => (0.0, 0.0, false, true, 0.0),
        "darkGrid" => (2.0, 2.0, false, false, 0.0),
        "lightGrid" => (1.0, 1.0, false, false, 0.0),
        "darkTrellis" | "lightTrellis" => (0.0, 0.0, true, true, 0.0),
        "darkGray" => (0.0, 0.0, false, false, 0.75),
        "mediumGray" => (0.0, 0.0, false, false, 0.5),
        "lightGray" => (0.0, 0.0, false, false, 0.25),
        "gray125" => (0.0, 0.0, false, false, 0.125),
        _ => (0.0, 0.0, false, false, 0.0625),
    };
    p.push_clip(r[0], r[1], r[2], r[3]);
    if density > 0.0 {
        let a = (density * 255.0) as u8;
        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(fg.0, fg.1, fg.2, a));
    }
    let mut y = r[1];
    while horiz > 0.0 && y < r[1] + r[3] {
        p.fill_rect(r[0], y, r[2], horiz, fg);
        y += step;
    }
    let mut x = r[0];
    while vert > 0.0 && x < r[0] + r[2] {
        p.fill_rect(x, r[1], vert, r[3], fg);
        x += step;
    }
    if diag_down || diag_up {
        let mut d = -r[3];
        while d < r[2] {
            if diag_down {
                p.path(
                    &[
                        PathSeg::M(r[0] + d, r[1]),
                        PathSeg::L(r[0] + d + r[3], r[1] + r[3]),
                    ],
                    None,
                    Some((fg, 1.0)),
                );
            }
            if diag_up {
                p.path(
                    &[
                        PathSeg::M(r[0] + d, r[1] + r[3]),
                        PathSeg::L(r[0] + d + r[3], r[1]),
                    ],
                    None,
                    Some((fg, 1.0)),
                );
            }
            d += step * 1.5;
        }
    }
    p.pop_clip();
}

#[cfg(test)]
mod tests {
    use super::{classify, code_for};

    #[test]
    fn number_codes_round_trip_through_the_categories() {
        assert_eq!(classify("General").0, 0);
        assert_eq!(classify("#,##0.00").0, 1);
        assert_eq!(classify("#,##0.00"), (1, 2, true, 0));
        assert_eq!(classify("0.0%"), (6, 1, false, 0));
        assert_eq!(classify("m/d/yyyy").0, 4);
        assert_eq!(classify("h:mm:ss").0, 5);
        assert_eq!(classify("@").0, 9);
        assert_eq!(classify("0.00E+00").0, 8);
        assert_eq!(code_for(1, 2, true, 0, 1, ""), "#,##0.00");
        assert_eq!(code_for(1, 1, false, 3, 1, ""), "0.0_);[Red](0.0)");
        assert_eq!(code_for(6, 0, false, 0, 1, ""), "0%");
        assert_eq!(code_for(2, 2, true, 0, 1, ""), "\"$\"#,##0.00");
        assert_eq!(code_for(4, 0, false, 0, 1, "d-mmm"), "d-mmm");
        assert_eq!(classify(&code_for(2, 2, true, 0, 1, "")).0, 2);
        assert_eq!(classify(&code_for(3, 2, true, 0, 1, "")).0, 3);
    }
}
