//! Table styles — ECMA-376 Part 1, §20.1.4.2.27 tblStyleLst / §20.1.4.2.26
//! tblStyle, applied by `a:tblPr/a:tableStyleId` with the banding flags of
//! §21.1.3.15 tblPr (firstRow, lastRow, firstCol, lastCol, bandRow, bandCol).
//!
//! Styles come from the package's table styles part (`/ppt/tableStyles.xml`)
//! or, when the deck refers to a style it does not define, from BlueSlide's
//! own built-in styles below (authored in the same schema, own colours).

use bo_drawingml::color::find_color;
use bo_drawingml::props::{is_fill, parse_fill, parse_line};
use bo_model::slide::{Color, Fill, Line, Presentation, XmlElement};

/// Part name of the table styles part (§13.3.11).
pub const TABLE_STYLES_PART: &str = "/ppt/tableStyles.xml";

/// Content type of the table styles part.
pub const TABLE_STYLES_CT: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.tableStyles+xml";

/// Style of one table part (§20.1.4.2.* wholeTbl, band1H, firstRow, …).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PartStyle {
    /// Cell fill (tcStyle fill).
    pub fill: Option<Fill>,
    /// Text colour (tcTxStyle colour choice).
    pub color: Option<Color>,
    /// Bold text (tcTxStyle `b`), if set.
    pub bold: Option<bool>,
    /// Italic text (tcTxStyle `i`), if set.
    pub italic: Option<bool>,
    /// tcBdr: left, right, top, bottom, insideH, insideV.
    pub borders: [Option<Line>; 6],
}

/// A table style.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TableStyle {
    /// `styleId` (a GUID).
    pub id: String,
    /// `styleName`.
    pub name: String,
    /// Part styles by element name (`wholeTbl`, `firstRow`, …).
    pub parts: Vec<(String, PartStyle)>,
}

impl TableStyle {
    fn part(&self, name: &str) -> Option<&PartStyle> {
        self.parts.iter().find(|(n, _)| n == name).map(|(_, p)| p)
    }
}

fn on_off(v: Option<&str>) -> Option<bool> {
    match v {
        Some("on") => Some(true),
        Some("off") => Some(false),
        _ => None,
    }
}

/// Parse `a:tblStyle`.
pub fn parse_style(el: &XmlElement) -> TableStyle {
    let mut st = TableStyle {
        id: el.attr("styleId").unwrap_or_default().to_owned(),
        name: el.attr("styleName").unwrap_or_default().to_owned(),
        parts: Vec::new(),
    };
    for part in el.elements() {
        if part.local() == "tblBg" || part.local() == "extLst" {
            continue;
        }
        let mut ps = PartStyle::default();
        if let Some(tx) = part.child("a:tcTxStyle") {
            ps.bold = on_off(tx.attr("b"));
            ps.italic = on_off(tx.attr("i"));
            ps.color = find_color(tx);
        }
        if let Some(tc) = part.child("a:tcStyle") {
            if let Some(f) = tc
                .child("a:fill")
                .and_then(|f| f.elements().find(|e| is_fill(&e.name)))
            {
                ps.fill = parse_fill(f);
            }
            if let Some(b) = tc.child("a:tcBdr") {
                for (i, side) in [
                    "a:left",
                    "a:right",
                    "a:top",
                    "a:bottom",
                    "a:insideH",
                    "a:insideV",
                ]
                .iter()
                .enumerate()
                {
                    if let Some(ln) = b.child(side).and_then(|s| s.child("a:ln")) {
                        ps.borders[i] = Some(parse_line(ln));
                    }
                }
            }
        }
        st.parts.push((part.local().to_owned(), ps));
    }
    st
}

/// The banding / emphasis flags of `a:tblPr` (§21.1.3.15).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flags {
    /// `firstRow`: emphasise the first row.
    pub first_row: bool,
    /// `lastRow`: emphasise the last row.
    pub last_row: bool,
    /// `firstCol`: emphasise the first column.
    pub first_col: bool,
    /// `lastCol`: emphasise the last column.
    pub last_col: bool,
    /// `bandRow`: alternate row banding.
    pub band_row: bool,
    /// `bandCol`: alternate column banding.
    pub band_col: bool,
}

/// Flags and style id of a table's `a:tblPr`.
pub fn table_props(tbl_pr: Option<&XmlElement>) -> (Flags, Option<String>) {
    let Some(p) = tbl_pr else {
        return (Flags::default(), None);
    };
    let b = |n: &str| p.attr(n).is_some_and(|v| v == "1" || v == "true");
    (
        Flags {
            first_row: b("firstRow"),
            last_row: b("lastRow"),
            first_col: b("firstCol"),
            last_col: b("lastCol"),
            band_row: b("bandRow"),
            band_col: b("bandCol"),
        },
        p.child("a:tableStyleId")
            .map(|e| e.text().trim().to_owned()),
    )
}

/// Styles defined by the deck's table styles part.
pub fn deck_styles(pres: &Presentation) -> Vec<TableStyle> {
    let Some(data) = pres.part_data(TABLE_STYLES_PART) else {
        return Vec::new();
    };
    let Ok(root) = bo_drawingml::dom::parse(data, TABLE_STYLES_PART) else {
        return Vec::new();
    };
    root.elements()
        .filter(|e| e.local() == "tblStyle")
        .map(parse_style)
        .collect()
}

/// Ids of BlueSlide's built-in styles: one per accent colour (accent1..6).
pub const BUILTIN_IDS: [&str; 6] = [
    "{B5A1D7E2-1C0B-4E4A-9B51-0B5D1E000001}",
    "{B5A1D7E2-1C0B-4E4A-9B51-0B5D1E000002}",
    "{B5A1D7E2-1C0B-4E4A-9B51-0B5D1E000003}",
    "{B5A1D7E2-1C0B-4E4A-9B51-0B5D1E000004}",
    "{B5A1D7E2-1C0B-4E4A-9B51-0B5D1E000005}",
    "{B5A1D7E2-1C0B-4E4A-9B51-0B5D1E000006}",
];

/// Markup of built-in style `n` (0-based accent), in the §20.1.4.2.26 tblStyle schema.
pub fn builtin_xml(n: usize) -> String {
    let accent = format!("accent{}", n % 6 + 1);
    let id = BUILTIN_IDS[n % 6];
    let ln = |w: u32, clr: &str| {
        format!(
            r#"<a:ln w="{w}" cmpd="sng"><a:solidFill><a:schemeClr val="{clr}"/></a:solidFill></a:ln>"#
        )
    };
    let thin = ln(9525, "lt1");
    format!(
        concat!(
            r#"<a:tblStyle styleId="{id}" styleName="BlueSlide {name}">"#,
            r#"<a:wholeTbl><a:tcTxStyle><a:fontRef idx="minor"><a:prstClr val="black"/></a:fontRef><a:schemeClr val="dk1"/></a:tcTxStyle>"#,
            r#"<a:tcStyle><a:tcBdr><a:left>{thin}</a:left><a:right>{thin}</a:right><a:top>{thin}</a:top><a:bottom>{thin}</a:bottom>"#,
            r#"<a:insideH>{thin}</a:insideH><a:insideV>{thin}</a:insideV></a:tcBdr>"#,
            r#"<a:fill><a:solidFill><a:schemeClr val="{accent}"><a:tint val="15000"/></a:schemeClr></a:solidFill></a:fill></a:tcStyle></a:wholeTbl>"#,
            r#"<a:band1H><a:tcStyle><a:tcBdr/><a:fill><a:solidFill><a:schemeClr val="{accent}"><a:tint val="32000"/></a:schemeClr></a:solidFill></a:fill></a:tcStyle></a:band1H>"#,
            r#"<a:band2H><a:tcStyle><a:tcBdr/></a:tcStyle></a:band2H>"#,
            r#"<a:band1V><a:tcStyle><a:tcBdr/><a:fill><a:solidFill><a:schemeClr val="{accent}"><a:tint val="32000"/></a:schemeClr></a:solidFill></a:fill></a:tcStyle></a:band1V>"#,
            r#"<a:band2V><a:tcStyle><a:tcBdr/></a:tcStyle></a:band2V>"#,
            r#"<a:lastCol><a:tcTxStyle b="on"><a:fontRef idx="minor"><a:prstClr val="black"/></a:fontRef><a:schemeClr val="lt1"/></a:tcTxStyle>"#,
            r#"<a:tcStyle><a:tcBdr/><a:fill><a:solidFill><a:schemeClr val="{accent}"><a:shade val="75000"/></a:schemeClr></a:solidFill></a:fill></a:tcStyle></a:lastCol>"#,
            r#"<a:firstCol><a:tcTxStyle b="on"><a:fontRef idx="minor"><a:prstClr val="black"/></a:fontRef><a:schemeClr val="lt1"/></a:tcTxStyle>"#,
            r#"<a:tcStyle><a:tcBdr/><a:fill><a:solidFill><a:schemeClr val="{accent}"><a:shade val="75000"/></a:schemeClr></a:solidFill></a:fill></a:tcStyle></a:firstCol>"#,
            r#"<a:lastRow><a:tcTxStyle b="on"><a:fontRef idx="minor"><a:prstClr val="black"/></a:fontRef><a:schemeClr val="lt1"/></a:tcTxStyle>"#,
            r#"<a:tcStyle><a:tcBdr><a:top>{thick}</a:top></a:tcBdr><a:fill><a:solidFill><a:schemeClr val="{accent}"><a:shade val="75000"/></a:schemeClr></a:solidFill></a:fill></a:tcStyle></a:lastRow>"#,
            r#"<a:firstRow><a:tcTxStyle b="on"><a:fontRef idx="minor"><a:prstClr val="black"/></a:fontRef><a:schemeClr val="lt1"/></a:tcTxStyle>"#,
            r#"<a:tcStyle><a:tcBdr><a:bottom>{thick}</a:bottom></a:tcBdr><a:fill><a:solidFill><a:schemeClr val="{accent}"/></a:solidFill></a:fill></a:tcStyle></a:firstRow>"#,
            r#"</a:tblStyle>"#
        ),
        id = id,
        name = n % 6 + 1,
        accent = accent,
        thin = thin,
        thick = ln(28575, "lt1"),
    )
}

/// Built-in style `n` parsed.
pub fn builtin(n: usize) -> TableStyle {
    let xml = format!(
        r#"<a:root xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">{}</a:root>"#,
        builtin_xml(n)
    );
    bo_drawingml::dom::parse(xml.as_bytes(), "/builtin")
        .ok()
        .and_then(|r| r.child("a:tblStyle").map(parse_style))
        .unwrap_or_default()
}

/// The style a table uses: defined by the deck, else built in (unknown ids
/// use the first built-in style).
pub fn resolve(pres: &Presentation, id: &str) -> TableStyle {
    if let Some(s) = deck_styles(pres).into_iter().find(|s| s.id == id) {
        return s;
    }
    let n = BUILTIN_IDS.iter().position(|b| *b == id).unwrap_or(0);
    builtin(n)
}

/// Cell region of a table part: first/last row, first/last column.
type Region = (usize, usize, usize, usize);

/// Fill, text colour, bold, italic and borders (left, right, top, bottom) of a cell.
pub type CellLook = (
    Option<Fill>,
    Option<Color>,
    Option<bool>,
    Option<bool>,
    [Option<Line>; 4],
);

/// Effective style of cell (r, c) of a `rows × cols` table: fill, text
/// colour, bold, italic and borders (left, right, top, bottom).
pub fn cell_style(
    st: &TableStyle,
    f: Flags,
    r: usize,
    c: usize,
    rows: usize,
    cols: usize,
) -> CellLook {
    let last_r = rows.saturating_sub(1);
    let last_c = cols.saturating_sub(1);
    // Parts in increasing priority, each with the region it covers (r0, r1, c0, c1).
    let mut parts: Vec<(&str, Region)> = vec![("wholeTbl", (0, last_r, 0, last_c))];
    let body_r0 = usize::from(f.first_row);
    let body_c0 = usize::from(f.first_col);
    if f.band_col && c >= body_c0 && !(f.last_col && c == last_c) {
        let k = c - body_c0;
        parts.push((
            if k.is_multiple_of(2) {
                "band1V"
            } else {
                "band2V"
            },
            (0, last_r, c, c),
        ));
    }
    if f.band_row && r >= body_r0 && !(f.last_row && r == last_r) {
        let k = r - body_r0;
        parts.push((
            if k.is_multiple_of(2) {
                "band1H"
            } else {
                "band2H"
            },
            (r, r, 0, last_c),
        ));
    }
    if f.last_col && c == last_c {
        parts.push(("lastCol", (0, last_r, c, c)));
    }
    if f.first_col && c == 0 {
        parts.push(("firstCol", (0, last_r, 0, 0)));
    }
    if f.last_row && r == last_r {
        parts.push(("lastRow", (r, r, 0, last_c)));
    }
    if f.first_row && r == 0 {
        parts.push(("firstRow", (0, 0, 0, last_c)));
    }
    let mut fill = None;
    let mut color = None;
    let mut bold = None;
    let mut italic = None;
    let mut b: [Option<Line>; 4] = Default::default();
    for (name, (r0, r1, c0, c1)) in parts {
        let Some(p) = st.part(name) else { continue };
        if p.fill.is_some() {
            fill.clone_from(&p.fill);
        }
        if p.color.is_some() {
            color.clone_from(&p.color);
        }
        if p.bold.is_some() {
            bold = p.bold;
        }
        if p.italic.is_some() {
            italic = p.italic;
        }
        // left/right/top/bottom at the region's edges, insideH/insideV between cells.
        let pick = |edge: bool, outer: usize, inner: usize| {
            if edge {
                p.borders[outer].clone()
            } else {
                p.borders[inner].clone()
            }
        };
        let sides = [
            pick(c == c0, 0, 5),
            pick(c == c1, 1, 5),
            pick(r == r0, 2, 4),
            pick(r == r1, 3, 4),
        ];
        for (dst, src) in b.iter_mut().zip(sides) {
            if src.is_some() {
                *dst = src;
            }
        }
    }
    (fill, color, bold, italic, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_style_bands_and_header() {
        let st = builtin(0);
        assert_eq!(st.id, BUILTIN_IDS[0]);
        assert!(st.part("firstRow").is_some() && st.part("band1H").is_some());
        let f = Flags {
            first_row: true,
            band_row: true,
            ..Flags::default()
        };
        let (hf, hc, hb, _, hb4) = cell_style(&st, f, 0, 1, 4, 3);
        assert!(matches!(hf, Some(Fill::Solid(_))));
        assert_eq!(hb, Some(true));
        assert!(hc.is_some());
        assert!(hb4[3].as_ref().is_some_and(|l| l.w == Some(28575)));
        let (f1, _, b1, _, _) = cell_style(&st, f, 1, 0, 4, 3);
        let (f2, _, _, _, _) = cell_style(&st, f, 2, 0, 4, 3);
        assert_ne!(f1, f2, "banded rows alternate");
        assert_eq!(b1, None);
        // Without the header flag the first row is a body row.
        let (_, _, b0, _, _) = cell_style(&st, Flags::default(), 0, 0, 4, 3);
        assert_eq!(b0, None);
    }
}
