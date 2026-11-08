//! A small PresentationML package builder (ECMA-376 Part 1, §19) for decks
//! converted from other formats (`.ppt` [MS-PPT], `.odp` ODF 1.3): slides with
//! preset-geometry shapes, text, pictures and speaker notes on one master with
//! three layouts (title, title and content, blank) and one theme. The result is
//! an ordinary `.pptx` that [`crate::read_pptx`] opens and BlueSlide edits.
//!
//! Coordinates are EMU (§20.1.10.16 `ST_Coordinate`), font sizes hundredths of
//! a point (§21.1.10.72 `ST_TextFontSize`), colours `RRGGBB` (§20.1.2.3.32 `srgbClr`).

use bo_opc::xml::escape;
use bo_opc::{ContentTypes, Package};
use std::fmt::Write as _;

const NS: &str = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#;
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/";
const CT: &str = "application/vnd.openxmlformats-officedocument.";
const DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";
const TREE_HEAD: &str = r#"<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>"#;
const CLR_MAP: &str = r#"bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink""#;

/// A deck to write.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DeckSpec {
    /// Slide size in EMU (§19.2.1.39 `sldSz`; clamped to 914 400 – 51 206 400).
    pub width: i64,
    /// Slide height in EMU (same range as `width`).
    pub height: i64,
    /// Slides in presentation order.
    pub slides: Vec<SlideSpec>,
    /// `dc:title` of the core properties.
    pub title: Option<String>,
}

/// Slide layout used by a slide (§19.7.15 `ST_SlideLayoutType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutKind {
    /// Title slide (`title`): centred title and subtitle.
    Title,
    /// Title and content (`obj`): title and body placeholder.
    TitleContent,
    /// Blank layout (`blank`): no placeholders.
    #[default]
    Blank,
}

/// One slide.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SlideSpec {
    /// Layout the slide is based on.
    pub layout: LayoutKind,
    /// Shapes of the slide tree, back to front.
    pub shapes: Vec<ShapeSpec>,
    /// Speaker notes, one string per paragraph.
    pub notes: Vec<String>,
    /// Solid background colour (`p:bg/p:bgPr`).
    pub background: Option<String>,
}

/// Placeholder role of a shape (§19.7.10 `ST_PlaceholderType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placeholder {
    /// `title`.
    Title,
    /// `ctrTitle` (title slide).
    CenterTitle,
    /// `subTitle` (title slide).
    SubTitle,
    /// `body`.
    Body,
}

/// Fill of a shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Paint {
    /// `a:noFill`.
    None,
    /// `a:solidFill` / `a:srgbClr`.
    Solid(String),
}

/// Outline of a shape (§20.1.2.2.24 `ln`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineSpec {
    /// `None` = no line.
    pub color: Option<String>,
    /// Width in EMU.
    pub width: i64,
}

/// An embedded picture (§19.3.1.37 `pic`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    /// Encoded image bytes, stored as the media part.
    pub data: Vec<u8>,
    /// File extension: `png`, `jpeg`, `gif`, `bmp`, `tiff`, `emf`, `wmf`.
    pub ext: String,
}

/// A shape, text box or picture.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ShapeSpec {
    /// Shape name (`p:cNvPr/@name`).
    pub name: String,
    /// Left offset in EMU (`a:xfrm/a:off/@x`).
    pub x: i64,
    /// Top offset in EMU (`a:xfrm/a:off/@y`).
    pub y: i64,
    /// Width in EMU (`a:xfrm/a:ext/@cx`).
    pub cx: i64,
    /// Height in EMU (`a:xfrm/a:ext/@cy`).
    pub cy: i64,
    /// Rotation in 60 000ths of a degree (§20.1.10.3 `ST_Angle`).
    pub rot: i32,
    /// Horizontal flip (`a:xfrm/@flipH`).
    pub flip_h: bool,
    /// Vertical flip (`a:xfrm/@flipV`).
    pub flip_v: bool,
    /// Preset geometry (§20.1.10.56 `ST_ShapeType`), `rect` when empty.
    pub geometry: String,
    /// `None`: inherited (placeholders) / theme style.
    pub fill: Option<Paint>,
    /// Outline; `None` = inherited / theme style.
    pub line: Option<LineSpec>,
    /// Placeholder role (`p:nvPr/p:ph`); `None` for a free shape.
    pub placeholder: Option<Placeholder>,
    /// Paragraphs of the text body; empty = no text.
    pub text: Vec<ParaSpec>,
    /// `bodyPr/@anchor`: `t`, `ctr` or `b`.
    pub anchor: Option<String>,
    /// Picture content: the shape is written as `p:pic` instead of `p:sp`.
    pub picture: Option<Picture>,
}

/// A paragraph (§21.1.2.2.6 `p`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ParaSpec {
    /// Runs of the paragraph.
    pub runs: Vec<RunSpec>,
    /// Outline level 0–8.
    pub level: u8,
    /// `l`, `ctr`, `r`, `just`.
    pub align: Option<String>,
    /// `Some(true)` = bullet `•`, `Some(false)` = `buNone`, `None` = inherited.
    pub bullet: Option<bool>,
}

/// A text run (§21.1.2.3.8 `r`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RunSpec {
    /// Run text (`a:t`).
    pub text: String,
    /// Bold (`a:rPr/@b`).
    pub bold: bool,
    /// Italic (`a:rPr/@i`).
    pub italic: bool,
    /// Single underline (`a:rPr/@u="sng"`).
    pub underline: bool,
    /// Hundredths of a point.
    pub size: Option<u32>,
    /// Text colour as `RRGGBB` (`a:solidFill/a:srgbClr`).
    pub color: Option<String>,
    /// Latin typeface (`a:latin/@typeface`).
    pub font: Option<String>,
}

impl RunSpec {
    /// Plain text run.
    pub fn text(s: &str) -> RunSpec {
        RunSpec {
            text: s.to_owned(),
            ..RunSpec::default()
        }
    }
}

impl ParaSpec {
    /// A paragraph of one plain run.
    pub fn plain(s: &str) -> ParaSpec {
        ParaSpec {
            runs: vec![RunSpec::text(s)],
            ..ParaSpec::default()
        }
    }
}

fn hex(c: &str) -> Option<String> {
    let h = c.trim().trim_start_matches('#');
    (h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit())).then(|| h.to_ascii_uppercase())
}

fn solid(c: &str) -> String {
    hex(c).map_or_else(
        || "<a:noFill/>".to_owned(),
        |h| format!(r#"<a:solidFill><a:srgbClr val="{h}"/></a:solidFill>"#),
    )
}

fn run_xml(r: &RunSpec) -> String {
    let mut a = String::from(r#" lang="vi-VN""#);
    if let Some(sz) = r.size {
        // §21.1.10.72: 1–4000 pt.
        let _ = write!(a, r#" sz="{}""#, sz.clamp(100, 400_000));
    }
    if r.bold {
        a.push_str(r#" b="1""#);
    }
    if r.italic {
        a.push_str(r#" i="1""#);
    }
    if r.underline {
        a.push_str(r#" u="sng""#);
    }
    a.push_str(r#" dirty="0""#);
    let mut kids = String::new();
    if let Some(c) = r.color.as_deref().and_then(hex) {
        let _ = write!(kids, r#"<a:solidFill><a:srgbClr val="{c}"/></a:solidFill>"#);
    }
    if let Some(f) = r.font.as_deref().filter(|f| !f.is_empty()) {
        let _ = write!(kids, r#"<a:latin typeface="{}"/>"#, escape(f));
    }
    // Line breaks inside a paragraph become `a:br` (§21.1.2.2.1).
    let mut out = String::new();
    for (i, part) in r.text.split(['\u{2028}', '\u{b}']).enumerate() {
        if i > 0 {
            let _ = write!(out, "<a:br><a:rPr{a}/></a:br>");
        }
        if !part.is_empty() || i == 0 {
            let _ = write!(
                out,
                "<a:r><a:rPr{a}>{kids}</a:rPr><a:t>{}</a:t></a:r>",
                escape(part)
            );
        }
    }
    out
}

fn para_xml(p: &ParaSpec) -> String {
    let mut attrs = String::new();
    if p.level > 0 {
        let _ = write!(attrs, r#" lvl="{}""#, p.level.min(8));
    }
    if let Some(al) = p.align.as_deref() {
        if matches!(al, "l" | "ctr" | "r" | "just" | "dist") {
            let _ = write!(attrs, r#" algn="{al}""#);
        }
    }
    let bullet = match p.bullet {
        Some(true) => r#"<a:buFont typeface="Arial"/><a:buChar char="•"/>"#,
        Some(false) => "<a:buNone/>",
        None => "",
    };
    let ppr = if attrs.is_empty() && bullet.is_empty() {
        String::new()
    } else if bullet.is_empty() {
        format!("<a:pPr{attrs}/>")
    } else {
        format!("<a:pPr{attrs}>{bullet}</a:pPr>")
    };
    let runs: String = p.runs.iter().map(run_xml).collect();
    format!(r#"<a:p>{ppr}{runs}<a:endParaRPr lang="vi-VN" dirty="0"/></a:p>"#)
}

fn xfrm(s: &ShapeSpec) -> String {
    let mut a = String::new();
    if s.rot != 0 {
        let _ = write!(a, r#" rot="{}""#, s.rot.rem_euclid(21_600_000));
    }
    if s.flip_h {
        a.push_str(r#" flipH="1""#);
    }
    if s.flip_v {
        a.push_str(r#" flipV="1""#);
    }
    format!(
        r#"<a:xfrm{a}><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm>"#,
        s.x,
        s.y,
        s.cx.max(0),
        s.cy.max(0)
    )
}

fn geometry(s: &ShapeSpec) -> String {
    let g = if s.geometry.is_empty() {
        "rect"
    } else {
        s.geometry.as_str()
    };
    format!(
        r#"<a:prstGeom prst="{}"><a:avLst/></a:prstGeom>"#,
        escape(g)
    )
}

fn shape_xml(id: u32, s: &ShapeSpec, rid: Option<&str>) -> String {
    let name = if s.name.is_empty() {
        format!("Shape {id}")
    } else {
        s.name.clone()
    };
    let name = escape(&name);
    let mut sp_pr = format!("{}{}", xfrm(s), geometry(s));
    match &s.fill {
        Some(Paint::None) => sp_pr.push_str("<a:noFill/>"),
        Some(Paint::Solid(c)) => sp_pr.push_str(&solid(c)),
        None => {}
    }
    if let Some(l) = &s.line {
        match l.color.as_deref().and_then(hex) {
            Some(c) => {
                let _ = write!(
                    sp_pr,
                    r#"<a:ln w="{}"><a:solidFill><a:srgbClr val="{c}"/></a:solidFill></a:ln>"#,
                    l.width.clamp(0, 20_116_800)
                );
            }
            None => sp_pr.push_str("<a:ln><a:noFill/></a:ln>"),
        }
    }
    if let (Some(pic), Some(rid)) = (&s.picture, rid) {
        let _ = pic;
        return format!(
            r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="{name}"/><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="{rid}"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{sp_pr}</p:spPr></p:pic>"#
        );
    }
    let (ph, locks) = match s.placeholder {
        Some(Placeholder::Title) => (r#"<p:ph type="title"/>"#, true),
        Some(Placeholder::CenterTitle) => (r#"<p:ph type="ctrTitle"/>"#, true),
        Some(Placeholder::SubTitle) => (r#"<p:ph type="subTitle" idx="1"/>"#, true),
        Some(Placeholder::Body) => (r#"<p:ph idx="1"/>"#, true),
        None => ("", false),
    };
    let c_nv_sp = if locks {
        r#"<p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>"#
    } else if s.text.is_empty() {
        "<p:cNvSpPr/>"
    } else {
        r#"<p:cNvSpPr txBox="1"/>"#
    };
    let tx = if s.text.is_empty() && s.placeholder.is_none() {
        String::new()
    } else {
        let anchor = s
            .anchor
            .as_deref()
            .filter(|a| matches!(*a, "t" | "ctr" | "b"))
            .map(|a| format!(r#" anchor="{a}""#))
            .unwrap_or_default();
        let wrap = if s.placeholder.is_none() {
            r#" wrap="square" rtlCol="0""#
        } else {
            ""
        };
        let paras: String = if s.text.is_empty() {
            r#"<a:p><a:endParaRPr lang="vi-VN" dirty="0"/></a:p>"#.to_owned()
        } else {
            s.text.iter().map(para_xml).collect()
        };
        format!("<p:txBody><a:bodyPr{wrap}{anchor}/><a:lstStyle/>{paras}</p:txBody>")
    };
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{name}"/>{c_nv_sp}<p:nvPr>{ph}</p:nvPr></p:nvSpPr><p:spPr>{sp_pr}</p:spPr>{tx}</p:sp>"#
    )
}

fn rels_xml(rels: &[(String, String, String)]) -> Vec<u8> {
    let mut s = format!(
        r#"{DECL}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#
    );
    for (id, ty, target) in rels {
        let ty = if ty.contains("://") {
            ty.clone()
        } else {
            format!("{REL}{ty}")
        };
        let _ = write!(
            s,
            r#"<Relationship Id="{id}" Type="{ty}" Target="{}"/>"#,
            escape(target)
        );
    }
    s.push_str("</Relationships>");
    s.into_bytes()
}

fn rel(id: &str, ty: &str, target: &str) -> (String, String, String) {
    (id.to_owned(), ty.to_owned(), target.to_owned())
}

fn theme(name: &str) -> String {
    let clr = |n: &str, v: &str| format!(r#"<a:{n}><a:srgbClr val="{v}"/></a:{n}>"#);
    let ln = |w: u32| {
        format!(
            r#"<a:ln w="{w}" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:prstDash val="solid"/><a:miter lim="800000"/></a:ln>"#
        )
    };
    format!(
        r#"{DECL}<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="{name}"><a:themeElements><a:clrScheme name="{name}"><a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1><a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1>{dk2}{lt2}{a1}{a2}{a3}{a4}{a5}{a6}{h}{fh}</a:clrScheme><a:fontScheme name="{name}"><a:majorFont><a:latin typeface="Arial"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="Arial"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme><a:fmtScheme name="{name}"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"><a:tint val="50000"/></a:schemeClr></a:solidFill><a:solidFill><a:schemeClr val="phClr"><a:shade val="80000"/></a:schemeClr></a:solidFill></a:fillStyleLst><a:lnStyleLst>{l1}{l2}{l3}</a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"><a:tint val="95000"/></a:schemeClr></a:solidFill><a:solidFill><a:schemeClr val="phClr"><a:shade val="90000"/></a:schemeClr></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements><a:objectDefaults/><a:extraClrSchemeLst/></a:theme>"#,
        dk2 = clr("dk2", "1F3864"),
        lt2 = clr("lt2", "E7E6E6"),
        a1 = clr("accent1", "2E75B6"),
        a2 = clr("accent2", "ED7D31"),
        a3 = clr("accent3", "A5A5A5"),
        a4 = clr("accent4", "FFC000"),
        a5 = clr("accent5", "5B9BD5"),
        a6 = clr("accent6", "70AD47"),
        h = clr("hlink", "0563C1"),
        fh = clr("folHlink", "954F72"),
        l1 = ln(6350),
        l2 = ln(12700),
        l3 = ln(19050),
    )
}

#[allow(clippy::too_many_arguments)]
fn ph_shape(
    id: u32,
    name: &str,
    ph: &str,
    x: i64,
    y: i64,
    cx: i64,
    cy: i64,
    anchor: &str,
) -> String {
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{name}"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr>{ph}</p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr vert="horz" lIns="91440" tIns="45720" rIns="91440" bIns="45720" rtlCol="0"{anchor}><a:normAutofit/></a:bodyPr><a:lstStyle/><a:p><a:endParaRPr lang="vi-VN"/></a:p></p:txBody></p:sp>"#
    )
}

fn master(w: i64, h: i64) -> String {
    let m = w / 24;
    let title = ph_shape(
        2,
        "Title Placeholder 1",
        r#"<p:ph type="title"/>"#,
        m,
        h / 20,
        w - 2 * m,
        h / 6,
        r#" anchor="ctr""#,
    );
    let body = ph_shape(
        3,
        "Text Placeholder 2",
        r#"<p:ph type="body" idx="1"/>"#,
        m,
        h / 4,
        w - 2 * m,
        h * 2 / 3,
        "",
    );
    let lvl = |n: u8, mar: i64, sz: u32| {
        format!(
            r#"<a:lvl{n}pPr marL="{mar}" indent="-228600" algn="l" defTabSz="914400" rtl="0" eaLnBrk="1" latinLnBrk="0" hangingPunct="1"><a:spcBef><a:spcPts val="600"/></a:spcBef><a:buFont typeface="Arial"/><a:buChar char="•"/><a:defRPr sz="{sz}" kern="1200"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mn-lt"/><a:ea typeface="+mn-ea"/><a:cs typeface="+mn-cs"/></a:defRPr></a:lvl{n}pPr>"#
        )
    };
    format!(
        r#"{DECL}<p:sldMaster {NS}><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg><p:spTree>{TREE_HEAD}{title}{body}</p:spTree></p:cSld><p:clrMap {CLR_MAP}/><p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/><p:sldLayoutId id="2147483650" r:id="rId2"/><p:sldLayoutId id="2147483651" r:id="rId3"/></p:sldLayoutIdLst><p:txStyles><p:titleStyle><a:lvl1pPr algn="l" defTabSz="914400" rtl="0" eaLnBrk="1" latinLnBrk="0" hangingPunct="1"><a:spcBef><a:spcPct val="0"/></a:spcBef><a:buNone/><a:defRPr sz="4000" kern="1200"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mj-lt"/><a:ea typeface="+mj-ea"/><a:cs typeface="+mj-cs"/></a:defRPr></a:lvl1pPr></p:titleStyle><p:bodyStyle>{b1}{b2}{b3}</p:bodyStyle><p:otherStyle><a:defPPr><a:defRPr lang="vi-VN"/></a:defPPr><a:lvl1pPr marL="0" algn="l" defTabSz="914400" rtl="0" eaLnBrk="1" latinLnBrk="0" hangingPunct="1"><a:defRPr sz="1800" kern="1200"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mn-lt"/><a:ea typeface="+mn-ea"/><a:cs typeface="+mn-cs"/></a:defRPr></a:lvl1pPr></p:otherStyle></p:txStyles></p:sldMaster>"#,
        b1 = lvl(1, 228_600, 2800),
        b2 = lvl(2, 685_800, 2400),
        b3 = lvl(3, 1_143_000, 2000),
    )
}

fn layout(kind: &str, name: &str, shapes: &str) -> String {
    format!(
        r#"{DECL}<p:sldLayout {NS} type="{kind}" preserve="1"><p:cSld name="{name}"><p:spTree>{TREE_HEAD}{shapes}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#
    )
}

fn layouts(w: i64, h: i64) -> [String; 3] {
    let m = w / 12;
    [
        layout(
            "title",
            "Title Slide",
            &(ph_shape(
                2,
                "Title 1",
                r#"<p:ph type="ctrTitle"/>"#,
                m,
                h / 4,
                w - 2 * m,
                h / 4,
                r#" anchor="b""#,
            ) + &ph_shape(
                3,
                "Subtitle 2",
                r#"<p:ph type="subTitle" idx="1"/>"#,
                m,
                h * 9 / 16,
                w - 2 * m,
                h / 5,
                "",
            )),
        ),
        layout(
            "obj",
            "Title and Content",
            &(ph_shape(
                2,
                "Title 1",
                r#"<p:ph type="title"/>"#,
                w / 24,
                h / 20,
                w - w / 12,
                h / 6,
                r#" anchor="ctr""#,
            ) + &ph_shape(
                3,
                "Content Placeholder 2",
                r#"<p:ph idx="1"/>"#,
                w / 24,
                h / 4,
                w - w / 12,
                h * 2 / 3,
                "",
            )),
        ),
        layout("blank", "Blank", ""),
    ]
}

const NOTES_MASTER: &str = r#"<p:notesMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr><p:sp><p:nvSpPr><p:cNvPr id="2" name="Slide Image Placeholder 1"/><p:cNvSpPr><a:spLocks noGrp="1" noRot="1" noChangeAspect="1"/></p:cNvSpPr><p:nvPr><p:ph type="sldImg" idx="2"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="685800" y="1143000"/><a:ext cx="5486400" cy="3086100"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/><a:ln w="12700"><a:solidFill><a:prstClr val="black"/></a:solidFill></a:ln></p:spPr></p:sp><p:sp><p:nvSpPr><p:cNvPr id="3" name="Notes Placeholder 2"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="body" sz="quarter" idx="3"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="685800" y="4400550"/><a:ext cx="5486400" cy="3600450"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr lang="vi-VN"/></a:p></p:txBody></p:sp></p:spTree></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/><p:notesStyle><a:lvl1pPr marL="0" algn="l" defTabSz="914400" rtl="0" eaLnBrk="1" latinLnBrk="0" hangingPunct="1"><a:defRPr sz="1200" kern="1200"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mn-lt"/><a:ea typeface="+mn-ea"/><a:cs typeface="+mn-cs"/></a:defRPr></a:lvl1pPr></p:notesStyle></p:notesMaster>"#;

fn notes_xml(paras: &[String]) -> String {
    let body: String = paras
        .iter()
        .map(|p| para_xml(&ParaSpec::plain(p)))
        .collect();
    let body = if body.is_empty() {
        r#"<a:p><a:endParaRPr lang="vi-VN"/></a:p>"#.to_owned()
    } else {
        body
    };
    format!(
        r#"{DECL}<p:notes {NS}><p:cSld><p:spTree>{TREE_HEAD}<p:sp><p:nvSpPr><p:cNvPr id="2" name="Slide Image Placeholder 1"/><p:cNvSpPr><a:spLocks noGrp="1" noRot="1" noChangeAspect="1"/></p:cNvSpPr><p:nvPr><p:ph type="sldImg"/></p:nvPr></p:nvSpPr><p:spPr/></p:sp><p:sp><p:nvSpPr><p:cNvPr id="3" name="Notes Placeholder 2"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/>{body}</p:txBody></p:sp></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:notes>"#
    )
}

/// Media type of a picture extension (Part 2 §10.1.2.1 default content types).
fn media_type(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "png" => "image/png",
        "jpeg" | "jpg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "tiff" | "tif" => "image/tiff",
        "emf" => "image/x-emf",
        "wmf" => "image/x-wmf",
        _ => return None,
    })
}

/// Writes `deck` as a `.pptx` package.
pub fn build_pptx(deck: &DeckSpec) -> Result<Vec<u8>, bo_opc::OpcError> {
    let w = deck.width.clamp(914_400, 51_206_400);
    let h = deck.height.clamp(914_400, 51_206_400);
    let mut pkg = Package::new();
    let has_notes = deck.slides.iter().any(|s| !s.notes.is_empty());
    let mut ct = ContentTypes {
        defaults: vec![
            (
                "rels".into(),
                "application/vnd.openxmlformats-package.relationships+xml".into(),
            ),
            ("xml".into(), "application/xml".into()),
        ],
        overrides: Vec::new(),
    };
    let mut over = |name: &str, t: &str| ct.set_override(name, t);
    over(
        "/ppt/presentation.xml",
        &format!("{CT}presentationml.presentation.main+xml"),
    );
    over(
        "/ppt/slideMasters/slideMaster1.xml",
        &format!("{CT}presentationml.slideMaster+xml"),
    );
    over("/ppt/theme/theme1.xml", &format!("{CT}theme+xml"));
    over(
        "/docProps/core.xml",
        "application/vnd.openxmlformats-package.core-properties+xml",
    );
    pkg.put_part(
        "/_rels/.rels",
        rels_xml(&[
            rel(
                "rId2",
                "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties",
                "docProps/core.xml",
            ),
            rel("rId1", "officeDocument", "ppt/presentation.xml"),
        ]),
    );
    let title = deck.title.as_deref().map(escape).unwrap_or_default();
    pkg.put_part(
        "/docProps/core.xml",
        format!(r#"{DECL}<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>{title}</dc:title></cp:coreProperties>"#).into_bytes(),
    );
    let mut prels = vec![rel("rId1", "slideMaster", "slideMasters/slideMaster1.xml")];
    let mut sld_ids = String::new();
    for i in 0..deck.slides.len() {
        let rid = format!("rId{}", i + 2);
        let _ = write!(sld_ids, r#"<p:sldId id="{}" r:id="{rid}"/>"#, 256 + i);
        prels.push((rid, "slide".into(), format!("slides/slide{}.xml", i + 1)));
    }
    let n = deck.slides.len() + 2;
    let notes_lst = if has_notes {
        prels.push((
            format!("rId{n}"),
            "notesMaster".into(),
            "notesMasters/notesMaster1.xml".into(),
        ));
        format!(r#"<p:notesMasterIdLst><p:notesMasterId r:id="rId{n}"/></p:notesMasterIdLst>"#)
    } else {
        String::new()
    };
    prels.push((
        format!("rId{}", n + 1),
        "theme".into(),
        "theme/theme1.xml".into(),
    ));
    let sld_lst = if sld_ids.is_empty() {
        String::new()
    } else {
        format!("<p:sldIdLst>{sld_ids}</p:sldIdLst>")
    };
    pkg.put_part(
        "/ppt/presentation.xml",
        format!(r#"{DECL}<p:presentation {NS}><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst>{notes_lst}{sld_lst}<p:sldSz cx="{w}" cy="{h}"/><p:notesSz cx="6858000" cy="9144000"/><p:defaultTextStyle><a:defPPr><a:defRPr lang="vi-VN"/></a:defPPr></p:defaultTextStyle></p:presentation>"#).into_bytes(),
    );
    pkg.put_part("/ppt/_rels/presentation.xml.rels", rels_xml(&prels));
    pkg.put_part("/ppt/theme/theme1.xml", theme("BlueSlide").into_bytes());
    pkg.put_part(
        "/ppt/slideMasters/slideMaster1.xml",
        master(w, h).into_bytes(),
    );
    pkg.put_part(
        "/ppt/slideMasters/_rels/slideMaster1.xml.rels",
        rels_xml(&[
            rel("rId1", "slideLayout", "../slideLayouts/slideLayout1.xml"),
            rel("rId2", "slideLayout", "../slideLayouts/slideLayout2.xml"),
            rel("rId3", "slideLayout", "../slideLayouts/slideLayout3.xml"),
            rel("rId4", "theme", "../theme/theme1.xml"),
        ]),
    );
    for (i, l) in layouts(w, h).into_iter().enumerate() {
        let name = format!("/ppt/slideLayouts/slideLayout{}.xml", i + 1);
        pkg.put_part(&name, l.into_bytes());
        pkg.put_part(
            &format!("/ppt/slideLayouts/_rels/slideLayout{}.xml.rels", i + 1),
            rels_xml(&[rel(
                "rId1",
                "slideMaster",
                "../slideMasters/slideMaster1.xml",
            )]),
        );
        ct.set_override(&name, &format!("{CT}presentationml.slideLayout+xml"));
    }
    if has_notes {
        pkg.put_part(
            "/ppt/notesMasters/notesMaster1.xml",
            format!("{DECL}{NOTES_MASTER}").into_bytes(),
        );
        pkg.put_part(
            "/ppt/notesMasters/_rels/notesMaster1.xml.rels",
            rels_xml(&[rel("rId1", "theme", "../theme/theme2.xml")]),
        );
        pkg.put_part("/ppt/theme/theme2.xml", theme("Notes").into_bytes());
        ct.set_override(
            "/ppt/notesMasters/notesMaster1.xml",
            &format!("{CT}presentationml.notesMaster+xml"),
        );
        ct.set_override("/ppt/theme/theme2.xml", &format!("{CT}theme+xml"));
    }
    let mut media_n = 0;
    let mut notes_n = 0;
    let mut exts: Vec<String> = Vec::new();
    for (i, s) in deck.slides.iter().enumerate() {
        let layout = match s.layout {
            LayoutKind::Title => 1,
            LayoutKind::TitleContent => 2,
            LayoutKind::Blank => 3,
        };
        let mut rels = vec![rel(
            "rId1",
            "slideLayout",
            &format!("../slideLayouts/slideLayout{layout}.xml"),
        )];
        let mut shapes = String::new();
        for (k, sh) in s.shapes.iter().enumerate() {
            let id = u32::try_from(k).unwrap_or(u32::MAX - 2).saturating_add(2);
            let rid = match &sh.picture {
                Some(p) if media_type(&p.ext).is_some() => {
                    media_n += 1;
                    let ext = if p.ext == "jpg" {
                        "jpeg"
                    } else {
                        p.ext.as_str()
                    };
                    let target = format!("../media/image{media_n}.{ext}");
                    pkg.put_part(&format!("/ppt/media/image{media_n}.{ext}"), p.data.clone());
                    if !exts.iter().any(|e| e == ext) {
                        exts.push(ext.to_owned());
                    }
                    let rid = format!("rId{}", rels.len() + 1);
                    rels.push((rid.clone(), "image".into(), target));
                    Some(rid)
                }
                _ => None,
            };
            if sh.picture.is_some() && rid.is_none() {
                continue;
            }
            shapes.push_str(&shape_xml(id, sh, rid.as_deref()));
        }
        let bg = s
            .background
            .as_deref()
            .and_then(hex)
            .map(|c| {
                format!(
                    r#"<p:bg><p:bgPr><a:solidFill><a:srgbClr val="{c}"/></a:solidFill><a:effectLst/></p:bgPr></p:bg>"#
                )
            })
            .unwrap_or_default();
        let name = format!("/ppt/slides/slide{}.xml", i + 1);
        pkg.put_part(
            &name,
            format!(
                r"{DECL}<p:sld {NS}><p:cSld>{bg}<p:spTree>{TREE_HEAD}{shapes}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"
            )
            .into_bytes(),
        );
        if !s.notes.is_empty() {
            notes_n += 1;
            let nname = format!("/ppt/notesSlides/notesSlide{notes_n}.xml");
            pkg.put_part(&nname, notes_xml(&s.notes).into_bytes());
            pkg.put_part(
                &format!("/ppt/notesSlides/_rels/notesSlide{notes_n}.xml.rels"),
                rels_xml(&[
                    rel("rId1", "notesMaster", "../notesMasters/notesMaster1.xml"),
                    rel("rId2", "slide", &format!("../slides/slide{}.xml", i + 1)),
                ]),
            );
            ct.set_override(&nname, &format!("{CT}presentationml.notesSlide+xml"));
            let rid = format!("rId{}", rels.len() + 1);
            rels.push((
                rid,
                "notesSlide".into(),
                format!("../notesSlides/notesSlide{notes_n}.xml"),
            ));
        }
        pkg.put_part(
            &format!("/ppt/slides/_rels/slide{}.xml.rels", i + 1),
            rels_xml(&rels),
        );
        ct.set_override(&name, &format!("{CT}presentationml.slide+xml"));
    }
    for e in exts {
        if let Some(t) = media_type(&e) {
            ct.defaults.push((e, t.to_owned()));
        }
    }
    pkg.set_content_types(&ct);
    pkg.write()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deck() -> DeckSpec {
        let png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        DeckSpec {
            width: 9_144_000,
            height: 6_858_000,
            title: Some("Thử & kiểm".into()),
            slides: vec![
                SlideSpec {
                    layout: LayoutKind::Title,
                    shapes: vec![ShapeSpec {
                        placeholder: Some(Placeholder::CenterTitle),
                        x: 685_800,
                        y: 2_130_425,
                        cx: 7_772_400,
                        cy: 1_470_025,
                        text: vec![ParaSpec::plain("Báo cáo <năm>")],
                        ..ShapeSpec::default()
                    }],
                    notes: vec!["Ghi chú 1".into()],
                    background: Some("F2F2F2".into()),
                },
                SlideSpec {
                    shapes: vec![
                        ShapeSpec {
                            geometry: "ellipse".into(),
                            x: 100,
                            y: 200,
                            cx: 3_000_000,
                            cy: 2_000_000,
                            rot: 30 * 60_000,
                            fill: Some(Paint::Solid("C00000".into())),
                            line: Some(LineSpec {
                                color: Some("000000".into()),
                                width: 12_700,
                            }),
                            text: vec![ParaSpec {
                                runs: vec![RunSpec {
                                    text: "Đậm\u{2028}dòng 2".into(),
                                    bold: true,
                                    size: Some(2400),
                                    color: Some("FFFFFF".into()),
                                    font: Some("Arial".into()),
                                    ..RunSpec::default()
                                }],
                                align: Some("ctr".into()),
                                ..ParaSpec::default()
                            }],
                            anchor: Some("ctr".into()),
                            ..ShapeSpec::default()
                        },
                        ShapeSpec {
                            name: "Ảnh".into(),
                            x: 4_000_000,
                            cx: 1_000_000,
                            cy: 1_000_000,
                            picture: Some(Picture {
                                data: png,
                                ext: "png".into(),
                            }),
                            ..ShapeSpec::default()
                        },
                    ],
                    ..SlideSpec::default()
                },
            ],
        }
    }

    #[test]
    fn builds_a_deck_the_reader_opens() {
        let bytes = build_pptx(&deck()).unwrap();
        let p = crate::read_pptx(&bytes).unwrap();
        assert_eq!(p.slides.len(), 2);
        let pkg = Package::read(&bytes, &bo_opc::Limits::default()).unwrap();
        let s2 =
            String::from_utf8(pkg.part("/ppt/slides/slide2.xml").unwrap().data.clone()).unwrap();
        assert!(s2.contains(r#"<a:prstGeom prst="ellipse">"#));
        assert!(s2.contains(r#"rot="1800000""#));
        assert!(s2.contains("<a:br>"));
        assert!(s2.contains(r#"<a:blip r:embed="rId2"/>"#));
        assert!(pkg.part("/ppt/media/image1.png").is_some());
        assert!(pkg.part("/ppt/notesSlides/notesSlide1.xml").is_some());
        let s1 =
            String::from_utf8(pkg.part("/ppt/slides/slide1.xml").unwrap().data.clone()).unwrap();
        assert!(s1.contains("Báo cáo &lt;năm&gt;"));
        assert!(s1.contains(r#"<p:ph type="ctrTitle"/>"#));
        // Saved again by the regular writer.
        assert!(crate::write_pptx(&p).is_ok());
    }
}
