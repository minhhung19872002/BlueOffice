//! Self-generated BlueSlide corpus (`tests/corpus/slide/`): small `.pptx`
//! decks written as raw PresentationML (ECMA-376 Part 1, §19) the way common
//! producers lay them out — master/layout/slide placeholder chains, theme,
//! extension lists, `mc:AlternateContent`, presProps/viewProps/tableStyles,
//! docProps — so the round-trip and rendering tests exercise real structure.
//! All texts, numbers and images are invented.

// Test-data generator: XML templates read better as raw strings built with `format!`.
#![allow(clippy::needless_raw_string_hashes, clippy::format_collect)]

use bo_opc::Package;

const NS: &str = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#;
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/";
const CT: &str = "application/vnd.openxmlformats-officedocument.";
const P14: &str = "http://schemas.microsoft.com/office/powerpoint/2010/main";
const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

/// One inch in EMU (§20.1.10.16).
const IN: i64 = 914_400;

/// A slide of a sample deck.
#[derive(Default)]
struct SlideSpec {
    /// 1 = title, 2 = title and content, 3 = blank.
    layout: usize,
    /// Shapes inside `p:spTree` (after the group properties).
    shapes: String,
    /// Background (`p:bg`) or empty.
    bg: String,
    /// Children after `p:cSld` (clrMapOvr, transition, timing, extLst).
    tail: String,
    /// Extra relationships `(id, type suffix or full URI, target)`; rId1 is the layout.
    rels: Vec<(String, String, String)>,
    /// Speaker notes text.
    notes: Option<String>,
}

/// Additional package parts (media, charts) `(part name, content type, bytes)`.
type Parts = Vec<(String, String, Vec<u8>)>;

fn xfrm(x: i64, y: i64, cx: i64, cy: i64) -> String {
    format!(r#"<a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm>"#)
}

fn ph(kind: &str, idx: Option<u32>) -> String {
    let t = if kind.is_empty() {
        String::new()
    } else {
        format!(r#" type="{kind}""#)
    };
    let i = idx.map(|i| format!(r#" idx="{i}""#)).unwrap_or_default();
    format!("<p:ph{t}{i}/>")
}

/// `p:sp` with optional placeholder, spPr inner XML and txBody inner XML.
fn sp(id: u32, name: &str, ph_xml: &str, sp_pr: &str, tx: Option<&str>, style: &str) -> String {
    let locks = if ph_xml.is_empty() {
        "<p:cNvSpPr/>".to_owned()
    } else {
        r#"<p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>"#.to_owned()
    };
    let tx = tx
        .map(|t| format!("<p:txBody>{t}</p:txBody>"))
        .unwrap_or_default();
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{name}"/>{locks}<p:nvPr>{ph_xml}</p:nvPr></p:nvSpPr><p:spPr>{sp_pr}</p:spPr>{style}{tx}</p:sp>"#
    )
}

fn body(body_pr: &str, paras: &str) -> String {
    format!("<a:bodyPr{body_pr}/><a:lstStyle/>{paras}")
}

/// Text body whose `bodyPr` has an autofit child (§21.1.2.1.1).
fn body_fit(body_pr: &str, fit: &str, paras: &str) -> String {
    format!("<a:bodyPr{body_pr}>{fit}</a:bodyPr><a:lstStyle/>{paras}")
}

fn para(text: &str) -> String {
    format!(r#"<a:p><a:r><a:rPr lang="vi-VN" dirty="0"/><a:t>{text}</a:t></a:r></a:p>"#)
}

fn para_lvl(lvl: u8, text: &str) -> String {
    format!(
        r#"<a:p><a:pPr lvl="{lvl}"/><a:r><a:rPr lang="vi-VN" dirty="0"/><a:t>{text}</a:t></a:r></a:p>"#
    )
}

fn prst(name: &str) -> String {
    format!(r#"<a:prstGeom prst="{name}"><a:avLst/></a:prstGeom>"#)
}

fn solid(rgb: &str) -> String {
    format!(r#"<a:solidFill><a:srgbClr val="{rgb}"/></a:solidFill>"#)
}

fn scheme(name: &str) -> String {
    format!(r#"<a:solidFill><a:schemeClr val="{name}"/></a:solidFill>"#)
}

/// Shape style referencing the theme matrices (§19.3.1.46 style).
const STYLE: &str = r#"<p:style><a:lnRef idx="2"><a:schemeClr val="accent1"><a:shade val="50000"/></a:schemeClr></a:lnRef><a:fillRef idx="1"><a:schemeClr val="accent1"/></a:fillRef><a:effectRef idx="0"><a:schemeClr val="accent1"/></a:effectRef><a:fontRef idx="minor"><a:schemeClr val="lt1"/></a:fontRef></p:style>"#;

fn title_slide(title: &str, sub: &str) -> SlideSpec {
    SlideSpec {
        layout: 1,
        shapes: sp(
            2,
            "Title 1",
            &ph("ctrTitle", None),
            "",
            Some(&body("", &para(title))),
            "",
        ) + &sp(
            3,
            "Subtitle 2",
            &ph("subTitle", Some(1)),
            "",
            Some(&body("", &para(sub))),
            "",
        ),
        ..SlideSpec::default()
    }
}

fn content_slide(title: &str, body_xml: &str) -> SlideSpec {
    SlideSpec {
        layout: 2,
        shapes: sp(
            2,
            "Title 1",
            &ph("title", None),
            "",
            Some(&body("", &para(title))),
            "",
        ) + &sp(
            3,
            "Content Placeholder 2",
            &ph("", Some(1)),
            "",
            Some(body_xml),
            "",
        ),
        ..SlideSpec::default()
    }
}

fn blank_slide(title: &str, shapes: impl AsRef<str>) -> SlideSpec {
    let t = sp(
        20,
        "TextBox 19",
        "",
        &(xfrm(IN / 2, IN / 4, 12 * IN, 3 * IN / 4) + &prst("rect") + "<a:noFill/>"),
        Some(&body_fit(
            r#" wrap="square" rtlCol="0""#,
            "<a:spAutoFit/>",
            &format!(
                r#"<a:p><a:r><a:rPr lang="vi-VN" sz="2800" b="1" dirty="0"/><a:t>{title}</a:t></a:r></a:p>"#
            ),
        )),
        "",
    );
    SlideSpec {
        layout: 3,
        shapes: t + shapes.as_ref(),
        ..SlideSpec::default()
    }
}

// ---------------------------------------------------------------------------
// Fixed parts
// ---------------------------------------------------------------------------

fn theme(name: &str, accent1: &str, major: &str, minor: &str) -> String {
    let clr = |n: &str, v: &str| format!(r#"<a:{n}><a:srgbClr val="{v}"/></a:{n}>"#);
    let grad = |a: &str, b: &str, c: &str| {
        format!(
            r#"<a:gradFill rotWithShape="1"><a:gsLst><a:gs pos="0"><a:schemeClr val="phClr"><a:lumMod val="{a}"/><a:satMod val="103000"/><a:tint val="73000"/></a:schemeClr></a:gs><a:gs pos="50000"><a:schemeClr val="phClr"><a:lumMod val="{b}"/><a:satMod val="109000"/><a:tint val="81000"/></a:schemeClr></a:gs><a:gs pos="100000"><a:schemeClr val="phClr"><a:satMod val="109000"/><a:lumMod val="{c}"/><a:shade val="94000"/></a:schemeClr></a:gs></a:gsLst><a:lin ang="5400000" scaled="0"/></a:gradFill>"#
        )
    };
    let ln = |w: u32| {
        format!(
            r#"<a:ln w="{w}" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:prstDash val="solid"/><a:miter lim="800000"/></a:ln>"#
        )
    };
    format!(
        r#"{XML_DECL}<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="{name}"><a:themeElements><a:clrScheme name="{name}"><a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1><a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1>{dk2}{lt2}{a1}{a2}{a3}{a4}{a5}{a6}{h}{fh}</a:clrScheme><a:fontScheme name="{name}"><a:majorFont><a:latin typeface="{major}" panose="020F0302020204030204"/><a:ea typeface=""/><a:cs typeface=""/><a:font script="Jpan" typeface="Yu Gothic Light"/></a:majorFont><a:minorFont><a:latin typeface="{minor}" panose="020F0502020204030204"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme><a:fmtScheme name="Office"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill>{g1}{g2}</a:fillStyleLst><a:lnStyleLst>{l1}{l2}{l3}</a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst><a:outerShdw blurRad="57150" dist="19050" dir="5400000" algn="ctr" rotWithShape="0"><a:srgbClr val="000000"><a:alpha val="63000"/></a:srgbClr></a:outerShdw></a:effectLst></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"><a:tint val="95000"/><a:satMod val="170000"/></a:schemeClr></a:solidFill>{g3}</a:bgFillStyleLst></a:fmtScheme></a:themeElements><a:objectDefaults/><a:extraClrSchemeLst/><a:extLst><a:ext uri="{{05A4C25C-085E-4340-85A3-A5531E510DB2}}"><thm15:themeFamily xmlns:thm15="http://schemas.microsoft.com/office/thememl/2012/main" name="{name}" id="{{62F939B6-93AF-4DB8-9C6B-D6C7DFDC589F}}" vid="{{4A3C46E8-61CC-4603-A589-7422A47A8E4A}}"/></a:ext></a:extLst></a:theme>"#,
        dk2 = clr("dk2", "1F3864"),
        lt2 = clr("lt2", "E7E6E6"),
        a1 = clr("accent1", accent1),
        a2 = clr("accent2", "ED7D31"),
        a3 = clr("accent3", "A5A5A5"),
        a4 = clr("accent4", "FFC000"),
        a5 = clr("accent5", "5B9BD5"),
        a6 = clr("accent6", "70AD47"),
        h = clr("hlink", "0563C1"),
        fh = clr("folHlink", "954F72"),
        g1 = grad("110000", "105000", "105000"),
        g2 = grad("102000", "103000", "99000"),
        g3 = grad("102000", "103000", "99000"),
        l1 = ln(6350),
        l2 = ln(12700),
        l3 = ln(19050),
    )
}

const CLR_MAP: &str = r#"bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink""#;

const TREE_HEAD: &str = r#"<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>"#;

fn creation_ext(val: u32) -> String {
    format!(
        r#"<p:extLst><p:ext uri="{{BB962C8B-B14F-4D97-AF65-F5344CB8AC3E}}"><p14:creationId xmlns:p14="{P14}" val="{val}"/></p:ext></p:extLst>"#
    )
}

fn footer_phs(start: u32) -> String {
    let w = 4 * IN;
    let y = 7 * IN - IN / 2;
    sp(
        start,
        "Date Placeholder",
        &ph("dt", Some(10)),
        &(xfrm(IN / 2, y, w, IN / 3) + &prst("rect")),
        Some(&body(
            r#" vert="horz" lIns="91440" tIns="45720" rIns="91440" bIns="45720" rtlCol="0" anchor="ctr""#,
            r#"<a:p><a:fld id="{B1C2D3E4-0000-4000-8000-000000000001}" type="datetimeFigureOut"><a:rPr lang="vi-VN"/><a:t>07/11/2026</a:t></a:fld><a:endParaRPr lang="vi-VN"/></a:p>"#,
        )),
        "",
    ) + &sp(
        start + 1,
        "Footer Placeholder",
        &ph("ftr", Some(11)),
        &(xfrm(IN / 2 + w + IN / 4, y, w, IN / 3) + &prst("rect")),
        Some(&body(
            r#" anchor="ctr""#,
            r#"<a:p><a:endParaRPr lang="vi-VN"/></a:p>"#,
        )),
        "",
    ) + &sp(
        start + 2,
        "Slide Number Placeholder",
        &ph("sldNum", Some(12)),
        &(xfrm(13 * IN - IN / 2 - 2 * IN, y, 2 * IN, IN / 3) + &prst("rect")),
        Some(&body(
            r#" anchor="ctr""#,
            r#"<a:p><a:pPr algn="r"/><a:fld id="{B1C2D3E4-0000-4000-8000-000000000002}" type="slidenum"><a:rPr lang="vi-VN"/><a:t>‹#›</a:t></a:fld><a:endParaRPr lang="vi-VN"/></a:p>"#,
        )),
        "",
    )
}

fn master(layouts: usize) -> String {
    let title = sp(
        2,
        "Title Placeholder 1",
        &ph("title", None),
        &(xfrm(IN / 2, IN / 3, 12 * IN, IN + IN / 4) + &prst("rect")),
        Some(&body_fit(
            r#" vert="horz" lIns="91440" tIns="45720" rIns="91440" bIns="45720" rtlCol="0" anchor="ctr""#,
            "<a:normAutofit/>",
            r#"<a:p><a:r><a:rPr lang="vi-VN"/><a:t>Tiêu đề mẫu</a:t></a:r><a:endParaRPr lang="vi-VN"/></a:p>"#,
        )),
        "",
    );
    let bodyph = sp(
        3,
        "Text Placeholder 2",
        &ph("body", Some(1)),
        &(xfrm(IN / 2, IN + 3 * IN / 4, 12 * IN, 4 * IN + IN / 4) + &prst("rect")),
        Some(&body_fit(
            r#" vert="horz" lIns="91440" tIns="45720" rIns="91440" bIns="45720" rtlCol="0""#,
            "<a:normAutofit/>",
            &(para("Cấp 1") + &para_lvl(1, "Cấp 2") + &para_lvl(2, "Cấp 3")),
        )),
        "",
    );
    let lvl = |n: u8, mar: i64, sz: u32, ch: &str| {
        format!(
            r#"<a:lvl{n}pPr marL="{mar}" indent="-228600" algn="l" defTabSz="914400" rtl="0" eaLnBrk="1" latinLnBrk="0" hangingPunct="1"><a:lnSpc><a:spcPct val="90000"/></a:lnSpc><a:spcBef><a:spcPts val="1000"/></a:spcBef><a:buFont typeface="Arial" panose="020B0604020202020204" pitchFamily="34" charset="0"/><a:buChar char="{ch}"/><a:defRPr sz="{sz}" kern="1200"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mn-lt"/><a:ea typeface="+mn-ea"/><a:cs typeface="+mn-cs"/></a:defRPr></a:lvl{n}pPr>"#
        )
    };
    let ids: String = (0..layouts)
        .map(|i| {
            format!(
                r#"<p:sldLayoutId id="{}" r:id="rId{}"/>"#,
                2_147_483_649 + i,
                i + 1
            )
        })
        .collect();
    format!(
        r#"{XML_DECL}<p:sldMaster {NS}><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg><p:spTree>{TREE_HEAD}{title}{bodyph}{foot}</p:spTree><p:extLst><p:ext uri="{{BB962C8B-B14F-4D97-AF65-F5344CB8AC3E}}"><p14:creationId xmlns:p14="{P14}" val="2469031417"/></p:ext></p:extLst></p:cSld><p:clrMap {CLR_MAP}/><p:sldLayoutIdLst>{ids}</p:sldLayoutIdLst><p:txStyles><p:titleStyle><a:lvl1pPr algn="l" defTabSz="914400" rtl="0" eaLnBrk="1" latinLnBrk="0" hangingPunct="1"><a:lnSpc><a:spcPct val="90000"/></a:lnSpc><a:spcBef><a:spcPct val="0"/></a:spcBef><a:buNone/><a:defRPr sz="4400" kern="1200"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mj-lt"/><a:ea typeface="+mj-ea"/><a:cs typeface="+mj-cs"/></a:defRPr></a:lvl1pPr></p:titleStyle><p:bodyStyle>{b1}{b2}{b3}{b4}{b5}</p:bodyStyle><p:otherStyle><a:defPPr><a:defRPr lang="vi-VN"/></a:defPPr><a:lvl1pPr marL="0" algn="l" defTabSz="914400" rtl="0" eaLnBrk="1" latinLnBrk="0" hangingPunct="1"><a:defRPr sz="1800" kern="1200"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mn-lt"/><a:ea typeface="+mn-ea"/><a:cs typeface="+mn-cs"/></a:defRPr></a:lvl1pPr></p:otherStyle></p:txStyles></p:sldMaster>"#,
        foot = footer_phs(4),
        b1 = lvl(1, 228_600, 2800, "•"),
        b2 = lvl(2, 685_800, 2400, "–"),
        b3 = lvl(3, 1_143_000, 2000, "•"),
        b4 = lvl(4, 1_600_200, 1800, "–"),
        b5 = lvl(5, 2_057_400, 1800, "»"),
    )
}

fn layout(kind: &str, name: &str, shapes: &str, creation: u32) -> String {
    format!(
        r#"{XML_DECL}<p:sldLayout {NS} type="{kind}" preserve="1"><p:cSld name="{name}"><p:spTree>{TREE_HEAD}{shapes}</p:spTree>{ext}</p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#,
        ext = creation_ext(creation)
    )
}

fn layouts() -> Vec<String> {
    let ctr = sp(
        2,
        "Title 1",
        &ph("ctrTitle", None),
        &xfrm(IN + IN / 2, IN + IN / 8, 10 * IN, 2 * IN + IN / 4),
        Some(&body_fit(
            r#" anchor="b""#,
            "<a:normAutofit/>",
            r#"<a:p><a:r><a:rPr lang="vi-VN"/><a:t>Tiêu đề</a:t></a:r><a:endParaRPr lang="vi-VN"/></a:p>"#,
        )
        .replace(
            "<a:lstStyle/>",
            r#"<a:lstStyle><a:lvl1pPr algn="ctr"><a:defRPr sz="6000"/></a:lvl1pPr></a:lstStyle>"#,
        )),
        "",
    );
    let sub = sp(
        3,
        "Subtitle 2",
        &ph("subTitle", Some(1)),
        &xfrm(IN + IN / 2, 3 * IN + IN / 2, 10 * IN, IN + IN / 2),
        Some(&body("", r#"<a:p><a:r><a:rPr lang="vi-VN"/><a:t>Phụ đề</a:t></a:r><a:endParaRPr lang="vi-VN"/></a:p>"#).replace(
            "<a:lstStyle/>",
            r#"<a:lstStyle><a:lvl1pPr marL="0" indent="0" algn="ctr"><a:buNone/><a:defRPr sz="2400"><a:solidFill><a:schemeClr val="tx1"><a:lumMod val="65000"/><a:lumOff val="35000"/></a:schemeClr></a:solidFill></a:defRPr></a:lvl1pPr></a:lstStyle>"#,
        )),
        "",
    );
    let title = sp(
        2,
        "Title 1",
        &ph("title", None),
        "",
        Some(&body("", &para("Tiêu đề"))),
        "",
    );
    let content = sp(
        3,
        "Content Placeholder 2",
        &ph("", Some(1)),
        "",
        Some(&body("", &para("Nội dung"))),
        "",
    );
    vec![
        layout(
            "title",
            "Title Slide",
            &(ctr + &sub + &footer_phs(4)),
            1_111_111_111,
        ),
        layout(
            "obj",
            "Title and Content",
            &(title + &content + &footer_phs(4)),
            2_222_222_222,
        ),
        layout("blank", "Blank", &footer_phs(2), 3_333_333_333),
    ]
}

const NOTES_MASTER: &str = r#"<p:notesMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr><p:sp><p:nvSpPr><p:cNvPr id="2" name="Slide Image Placeholder 1"/><p:cNvSpPr><a:spLocks noGrp="1" noRot="1" noChangeAspect="1"/></p:cNvSpPr><p:nvPr><p:ph type="sldImg" idx="2"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="685800" y="1143000"/><a:ext cx="5486400" cy="3086100"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/><a:ln w="12700"><a:solidFill><a:prstClr val="black"/></a:solidFill></a:ln></p:spPr></p:sp><p:sp><p:nvSpPr><p:cNvPr id="3" name="Notes Placeholder 2"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="body" sz="quarter" idx="3"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="685800" y="4400550"/><a:ext cx="5486400" cy="3600450"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="vi-VN"/><a:t>Ghi chú</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/><p:notesStyle><a:lvl1pPr marL="0" algn="l" defTabSz="914400" rtl="0" eaLnBrk="1" latinLnBrk="0" hangingPunct="1"><a:defRPr sz="1200" kern="1200"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mn-lt"/><a:ea typeface="+mn-ea"/><a:cs typeface="+mn-cs"/></a:defRPr></a:lvl1pPr></p:notesStyle></p:notesMaster>"#;

fn notes_slide(text: &str) -> String {
    let paras: String = text.lines().map(para).collect();
    format!(
        r#"{XML_DECL}<p:notes {NS}><p:cSld><p:spTree>{TREE_HEAD}<p:sp><p:nvSpPr><p:cNvPr id="2" name="Slide Image Placeholder 1"/><p:cNvSpPr><a:spLocks noGrp="1" noRot="1" noChangeAspect="1"/></p:cNvSpPr><p:nvPr><p:ph type="sldImg"/></p:nvPr></p:nvSpPr><p:spPr/></p:sp><p:sp><p:nvSpPr><p:cNvPr id="3" name="Notes Placeholder 2"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/>{paras}</p:txBody></p:sp></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:notes>"#
    )
}

fn rels_xml(rels: &[(String, String, String)]) -> String {
    let mut s = format!(
        r#"{XML_DECL}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#
    );
    for (id, ty, target) in rels {
        let ty = if ty.contains("://") {
            ty.clone()
        } else {
            format!("{REL}{ty}")
        };
        let ext = if target.starts_with("http") {
            r#" TargetMode="External""#
        } else {
            ""
        };
        s.push_str(&format!(
            r#"<Relationship Id="{id}" Type="{ty}" Target="{target}"{ext}/>"#
        ));
    }
    s.push_str("</Relationships>");
    s
}

fn r(id: &str, ty: &str, target: &str) -> (String, String, String) {
    (id.to_owned(), ty.to_owned(), target.to_owned())
}

/// Theme variants: (name, accent1, major font, minor font).
type ThemeSpec = (&'static str, &'static str, &'static str, &'static str);
const THEME_BLUE: ThemeSpec = ("BlueStar", "2E75B6", "Calibri Light", "Calibri");

fn build(slides: &[SlideSpec], extra_parts: &Parts, th: ThemeSpec) -> Vec<u8> {
    let mut pkg = Package::new();
    let has_notes = slides.iter().any(|s| s.notes.is_some());
    let mut overrides: Vec<(String, String)> = vec![
        (
            "/ppt/presentation.xml".into(),
            format!("{CT}presentationml.presentation.main+xml"),
        ),
        (
            "/ppt/slideMasters/slideMaster1.xml".into(),
            format!("{CT}presentationml.slideMaster+xml"),
        ),
        ("/ppt/theme/theme1.xml".into(), format!("{CT}theme+xml")),
        (
            "/ppt/presProps.xml".into(),
            format!("{CT}presentationml.presProps+xml"),
        ),
        (
            "/ppt/viewProps.xml".into(),
            format!("{CT}presentationml.viewProps+xml"),
        ),
        (
            "/ppt/tableStyles.xml".into(),
            format!("{CT}presentationml.tableStyles+xml"),
        ),
        (
            "/docProps/core.xml".into(),
            "application/vnd.openxmlformats-package.core-properties+xml".into(),
        ),
        (
            "/docProps/app.xml".into(),
            format!("{CT}extended-properties+xml"),
        ),
    ];
    pkg.put_part(
        "/_rels/.rels",
        rels_xml(&[
            r("rId3", "extended-properties", "docProps/app.xml"),
            r(
                "rId2",
                "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties",
                "docProps/core.xml",
            ),
            r("rId1", "officeDocument", "ppt/presentation.xml"),
        ])
        .into_bytes(),
    );
    pkg.put_part(
        "/docProps/core.xml",
        format!(r#"{XML_DECL}<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:title>Mẫu BlueSlide</dc:title><dc:creator>BlueOffice corpus</dc:creator><dcterms:created xsi:type="dcterms:W3CDTF">2026-11-07T00:00:00Z</dcterms:created></cp:coreProperties>"#).into_bytes(),
    );
    pkg.put_part(
        "/docProps/app.xml",
        format!(r#"{XML_DECL}<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Application>BlueOffice corpus generator</Application><Slides>{}</Slides></Properties>"#, slides.len()).into_bytes(),
    );
    // presentation.xml + rels
    let mut prels = vec![r("rId1", "slideMaster", "slideMasters/slideMaster1.xml")];
    let mut sld_ids = String::new();
    for (i, _) in slides.iter().enumerate() {
        let rid = format!("rId{}", i + 2);
        sld_ids.push_str(&format!(r#"<p:sldId id="{}" r:id="{rid}"/>"#, 256 + i));
        prels.push((rid, "slide".into(), format!("slides/slide{}.xml", i + 1)));
    }
    let n = slides.len() + 2;
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
        "presProps".into(),
        "presProps.xml".into(),
    ));
    prels.push((
        format!("rId{}", n + 2),
        "viewProps".into(),
        "viewProps.xml".into(),
    ));
    prels.push((
        format!("rId{}", n + 3),
        "theme".into(),
        "theme/theme1.xml".into(),
    ));
    prels.push((
        format!("rId{}", n + 4),
        "tableStyles".into(),
        "tableStyles.xml".into(),
    ));
    let def_lvl = |n: u8| {
        format!(
            r#"<a:lvl{n}pPr marL="{}" algn="l" defTabSz="914400" rtl="0" eaLnBrk="1" latinLnBrk="0" hangingPunct="1"><a:defRPr sz="1800" kern="1200"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mn-lt"/><a:ea typeface="+mn-ea"/><a:cs typeface="+mn-cs"/></a:defRPr></a:lvl{n}pPr>"#,
            (u32::from(n) - 1) * 457_200
        )
    };
    let def_style: String = (1..=3).map(def_lvl).collect();
    pkg.put_part(
        "/ppt/presentation.xml",
        format!(r#"{XML_DECL}<p:presentation {NS} saveSubsetFonts="1"><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst>{notes_lst}<p:sldIdLst>{sld_ids}</p:sldIdLst><p:sldSz cx="12192000" cy="6858000"/><p:notesSz cx="6858000" cy="9144000"/><p:defaultTextStyle><a:defPPr><a:defRPr lang="vi-VN"/></a:defPPr>{def_style}</p:defaultTextStyle><p:extLst><p:ext uri="{{EFAFB233-063F-42B5-8137-9DF3F51BA10A}}"><p15:sldGuideLst xmlns:p15="http://schemas.microsoft.com/office/powerpoint/2012/main"/></p:ext></p:extLst></p:presentation>"#).into_bytes(),
    );
    pkg.put_part(
        "/ppt/_rels/presentation.xml.rels",
        rels_xml(&prels).into_bytes(),
    );
    pkg.put_part(
        "/ppt/presProps.xml",
        format!(r#"{XML_DECL}<p:presentationPr {NS}><p:extLst><p:ext uri="{{E76CE94A-603C-4142-B9EB-6D1370010A27}}"><p14:discardImageEditData xmlns:p14="{P14}" val="0"/></p:ext></p:extLst></p:presentationPr>"#).into_bytes(),
    );
    pkg.put_part(
        "/ppt/viewProps.xml",
        format!(r#"{XML_DECL}<p:viewPr {NS}><p:normalViewPr><p:restoredLeft sz="15620"/><p:restoredTop sz="94660"/></p:normalViewPr><p:gridSpacing cx="76200" cy="76200"/></p:viewPr>"#).into_bytes(),
    );
    pkg.put_part(
        "/ppt/tableStyles.xml",
        format!(r#"{XML_DECL}<a:tblStyleLst xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" def="{{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}}"/>"#).into_bytes(),
    );
    pkg.put_part(
        "/ppt/theme/theme1.xml",
        theme(th.0, th.1, th.2, th.3).into_bytes(),
    );
    // Master and layouts.
    let ls = layouts();
    pkg.put_part(
        "/ppt/slideMasters/slideMaster1.xml",
        master(ls.len()).into_bytes(),
    );
    let mut mrels: Vec<_> = (0..ls.len())
        .map(|i| {
            (
                format!("rId{}", i + 1),
                "slideLayout".to_owned(),
                format!("../slideLayouts/slideLayout{}.xml", i + 1),
            )
        })
        .collect();
    mrels.push((
        format!("rId{}", ls.len() + 1),
        "theme".into(),
        "../theme/theme1.xml".into(),
    ));
    pkg.put_part(
        "/ppt/slideMasters/_rels/slideMaster1.xml.rels",
        rels_xml(&mrels).into_bytes(),
    );
    for (i, l) in ls.iter().enumerate() {
        let name = format!("/ppt/slideLayouts/slideLayout{}.xml", i + 1);
        pkg.put_part(&name, l.clone().into_bytes());
        pkg.put_part(
            &format!("/ppt/slideLayouts/_rels/slideLayout{}.xml.rels", i + 1),
            rels_xml(&[r("rId1", "slideMaster", "../slideMasters/slideMaster1.xml")]).into_bytes(),
        );
        overrides.push((name, format!("{CT}presentationml.slideLayout+xml")));
    }
    if has_notes {
        pkg.put_part(
            "/ppt/notesMasters/notesMaster1.xml",
            format!("{XML_DECL}{NOTES_MASTER}").into_bytes(),
        );
        pkg.put_part(
            "/ppt/notesMasters/_rels/notesMaster1.xml.rels",
            rels_xml(&[r("rId1", "theme", "../theme/theme2.xml")]).into_bytes(),
        );
        pkg.put_part(
            "/ppt/theme/theme2.xml",
            theme("Notes", "4472C4", "Calibri Light", "Calibri").into_bytes(),
        );
        overrides.push((
            "/ppt/notesMasters/notesMaster1.xml".into(),
            format!("{CT}presentationml.notesMaster+xml"),
        ));
        overrides.push(("/ppt/theme/theme2.xml".into(), format!("{CT}theme+xml")));
    }
    // Slides.
    let mut notes_n = 0;
    for (i, s) in slides.iter().enumerate() {
        let name = format!("/ppt/slides/slide{}.xml", i + 1);
        let xml = format!(
            r#"{XML_DECL}<p:sld {NS}><p:cSld>{bg}<p:spTree>{TREE_HEAD}{shapes}</p:spTree>{ext}</p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>{tail}</p:sld>"#,
            bg = s.bg,
            shapes = s.shapes,
            ext = creation_ext(4_000_000 + i as u32),
            tail = s.tail,
        );
        pkg.put_part(&name, xml.into_bytes());
        let mut rels = vec![r(
            "rId1",
            "slideLayout",
            &format!("../slideLayouts/slideLayout{}.xml", s.layout.clamp(1, 3)),
        )];
        rels.extend(s.rels.iter().cloned());
        if let Some(text) = &s.notes {
            notes_n += 1;
            let nname = format!("/ppt/notesSlides/notesSlide{notes_n}.xml");
            pkg.put_part(&nname, notes_slide(text).into_bytes());
            pkg.put_part(
                &format!("/ppt/notesSlides/_rels/notesSlide{notes_n}.xml.rels"),
                rels_xml(&[
                    r("rId2", "slide", &format!("../slides/slide{}.xml", i + 1)),
                    r("rId1", "notesMaster", "../notesMasters/notesMaster1.xml"),
                ])
                .into_bytes(),
            );
            overrides.push((nname, format!("{CT}presentationml.notesSlide+xml")));
            rels.push((
                format!("rId{}", rels.len() + 1),
                "notesSlide".into(),
                format!("../notesSlides/notesSlide{notes_n}.xml"),
            ));
        }
        pkg.put_part(
            &format!("/ppt/slides/_rels/slide{}.xml.rels", i + 1),
            rels_xml(&rels).into_bytes(),
        );
        overrides.push((name, format!("{CT}presentationml.slide+xml")));
    }
    for (name, ct, data) in extra_parts {
        pkg.put_part(name, data.clone());
        if !ct.is_empty() {
            overrides.push((name.clone(), ct.clone()));
        }
    }
    let mut ct = bo_opc::ContentTypes {
        defaults: vec![
            (
                "rels".into(),
                "application/vnd.openxmlformats-package.relationships+xml".into(),
            ),
            ("xml".into(), "application/xml".into()),
            ("png".into(), "image/png".into()),
            ("mp4".into(), "video/mp4".into()),
            ("mp3".into(), "audio/mpeg".into()),
        ],
        overrides: Vec::new(),
    };
    for (n, t) in overrides {
        ct.set_override(&n, &t);
    }
    pkg.set_content_types(&ct);
    pkg.write().unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Images (generated)
// ---------------------------------------------------------------------------

/// A `w × h` PNG with a diagonal gradient (optionally with alpha).
fn png_image(w: u32, h: u32, alpha: bool, tint: [u8; 3]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(if alpha {
            png::ColorType::Rgba
        } else {
            png::ColorType::Rgb
        });
        enc.set_depth(png::BitDepth::Eight);
        let Ok(mut wr) = enc.write_header() else {
            return Vec::new();
        };
        let mut data = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let t = (x + y) * 255 / (w + h).max(1);
                let px = [
                    (u32::from(tint[0]) * t / 255) as u8,
                    (u32::from(tint[1]) * (255 - t) / 255) as u8,
                    tint[2],
                ];
                data.extend_from_slice(&px);
                if alpha {
                    data.push(if (x / 8 + y / 8) % 2 == 0 { 255 } else { 96 });
                }
            }
        }
        if wr.write_image_data(&data).is_err() {
            return Vec::new();
        }
    }
    out
}

fn pic(
    id: u32,
    name: &str,
    rid: &str,
    x: (i64, i64, i64, i64),
    extra_blip: &str,
    geom: &str,
) -> String {
    format!(
        r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="{name}" descr="Ảnh minh hoạ"/><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="{rid}"/>{extra_blip}<a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{xf}{geom}</p:spPr></p:pic>"#,
        xf = xfrm(x.0, x.1, x.2, x.3)
    )
}

// ---------------------------------------------------------------------------
// Feature decks
// ---------------------------------------------------------------------------

fn text_decks() -> Vec<(&'static str, Vec<u8>)> {
    // 1: placeholder inheritance + multi-level bullets.
    let bullets = body(
        "",
        &(para("Mục tiêu năm 2026")
            + &para_lvl(1, "Tăng doanh thu 15%")
            + &para_lvl(1, "Mở rộng thị trường miền Trung")
            + &para_lvl(2, "Đà Nẵng, Huế, Quảng Nam")
            + &para("Kế hoạch triển khai")
            + &para_lvl(1, "Quý I: khảo sát")
            + &para_lvl(3, "Cấp 4: chi tiết")
            + &para_lvl(4, "Cấp 5: ghi chú")),
    );
    let numbered = body(
        "",
        r#"<a:p><a:pPr marL="457200" indent="-457200"><a:buFont typeface="+mj-lt"/><a:buAutoNum type="arabicPeriod"/></a:pPr><a:r><a:rPr lang="vi-VN"/><a:t>Bước một</a:t></a:r></a:p><a:p><a:pPr marL="457200" indent="-457200"><a:buFont typeface="+mj-lt"/><a:buAutoNum type="arabicPeriod"/></a:pPr><a:r><a:rPr lang="vi-VN"/><a:t>Bước hai</a:t></a:r></a:p><a:p><a:pPr marL="914400" lvl="1" indent="-457200"><a:buFont typeface="+mj-lt"/><a:buAutoNum type="alphaLcParenR"/></a:pPr><a:r><a:rPr lang="vi-VN"/><a:t>Bước con</a:t></a:r></a:p><a:p><a:pPr marL="457200" indent="-457200"><a:buFont typeface="+mj-lt"/><a:buAutoNum type="arabicPeriod"/></a:pPr><a:r><a:rPr lang="vi-VN" b="1"/><a:t>Bước ba</a:t></a:r><a:r><a:rPr lang="vi-VN" i="1"><a:solidFill><a:srgbClr val="C00000"/></a:solidFill></a:rPr><a:t> (quan trọng)</a:t></a:r></a:p>"#,
    );
    let deck1 = build(
        &[
            title_slide("Báo cáo kết quả kinh doanh", "Công ty giả định ABC – 2026"),
            content_slide("Nội dung chính", &bullets),
            content_slide("Danh sách đánh số", &numbered),
        ],
        &Vec::new(),
        THEME_BLUE,
    );
    // 2: normAutofit with fontScale / lnSpcReduction, overflowing text.
    let long: String = (1..=12)
        .map(|i| {
            para(&format!(
                "Dòng nội dung số {i} có độ dài vừa phải để kiểm tra co chữ tự động"
            ))
        })
        .collect();
    let autofit = body_fit(
        "",
        r#"<a:normAutofit fontScale="62500" lnSpcReduction="20000"/>"#,
        &long,
    );
    let shrink = body_fit("", "<a:normAutofit/>", &long);
    let deck2 = build(
        &[
            content_slide("Co chữ (fontScale 62.5%)", &autofit),
            content_slide("Co chữ khi tràn (normAutofit)", &shrink),
            content_slide("Không co chữ", &body("", &long)),
        ],
        &Vec::new(),
        THEME_BLUE,
    );
    // 3: columns in a text box, alignment, anchors, vertical text, runs.
    let cols: String = (1..=10)
        .map(|i| para(&format!("Đoạn {i}: văn bản chia cột trong hộp văn bản.")))
        .collect();
    let box1 = sp(
        4,
        "TextBox 3",
        "",
        &(xfrm(IN / 2, IN + IN / 4, 8 * IN, 3 * IN) + &prst("rect") + &solid("F2F2F2")),
        Some(&body_fit(
            r#" wrap="square" numCol="2" spcCol="365760" rtlCol="0""#,
            "<a:noAutofit/>",
            &cols,
        )),
        "",
    );
    let box2 = sp(
        5,
        "TextBox 4",
        "",
        &(xfrm(9 * IN, IN + IN / 4, 3 * IN + IN / 2, 2 * IN)
            + &prst("roundRect")
            + &scheme("accent1")),
        Some(&body(
            r#" anchor="ctr""#,
            r#"<a:p><a:pPr algn="ctr"/><a:r><a:rPr lang="vi-VN" sz="2000" b="1"><a:solidFill><a:schemeClr val="bg1"/></a:solidFill></a:rPr><a:t>Căn giữa</a:t></a:r><a:br><a:rPr lang="vi-VN"/></a:br><a:r><a:rPr lang="vi-VN" sz="1400" u="sng"><a:solidFill><a:schemeClr val="bg1"/></a:solidFill></a:rPr><a:t>gạch chân</a:t></a:r></a:p>"#,
        )),
        "",
    );
    let box3 = sp(
        6,
        "TextBox 5",
        "",
        &(xfrm(9 * IN, 3 * IN + IN / 2, 3 * IN + IN / 2, IN + IN / 2) + &prst("rect")),
        Some(&body(
            r#" anchor="b""#,
            r#"<a:p><a:pPr algn="r"/><a:r><a:rPr lang="vi-VN" sz="1600" strike="sngStrike"/><a:t>Căn phải, đáy</a:t></a:r></a:p><a:p><a:pPr algn="r"/><a:r><a:rPr lang="vi-VN" sz="1600" baseline="30000"/><a:t>chỉ số trên</a:t></a:r><a:r><a:rPr lang="vi-VN" sz="1600" cap="all"/><a:t> chữ hoa</a:t></a:r></a:p>"#,
        )),
        "",
    );
    let deck3 = build(
        &[
            blank_slide("Văn bản chia cột", box1 + &box2 + &box3),
            blank_slide(
                "Ba cột, khoảng cách lớn",
                sp(
                    4,
                    "TextBox 3",
                    "",
                    &(xfrm(IN / 2, IN + IN / 4, 12 * IN, 4 * IN) + &prst("rect")),
                    Some(&body_fit(
                        r#" numCol="3" spcCol="457200""#,
                        "<a:normAutofit/>",
                        &cols,
                    )),
                    "",
                ),
            ),
            content_slide(
                "Chữ trong placeholder",
                &body("", &para("Một đoạn duy nhất.")),
            ),
        ],
        &Vec::new(),
        THEME_BLUE,
    );
    vec![
        ("slide-text-bullets.pptx", deck1),
        ("slide-text-autofit.pptx", deck2),
        ("slide-text-columns.pptx", deck3),
    ]
}

fn shape_grid(names: &[&str], fill: &dyn Fn(usize) -> String) -> String {
    let mut s = String::new();
    let (w, h) = (IN + IN / 2, IN);
    for (i, n) in names.iter().enumerate() {
        let col = (i % 7) as i64;
        let row = (i / 7) as i64;
        let x = IN / 3 + col * (w + IN / 4);
        let y = IN + IN / 4 + row * (h + IN / 3);
        s.push_str(&sp(
            10 + i as u32,
            n,
            "",
            &(xfrm(x, y, w, h) + &prst(n) + &fill(i)),
            None,
            STYLE,
        ));
    }
    s
}

fn shape_decks() -> Vec<(&'static str, Vec<u8>)> {
    let basic = [
        "rect",
        "roundRect",
        "ellipse",
        "triangle",
        "rtTriangle",
        "diamond",
        "pentagon",
        "hexagon",
        "octagon",
        "star5",
        "star12",
        "plus",
        "heart",
        "cloud",
        "rightArrow",
        "leftRightArrow",
        "chevron",
        "can",
        "cube",
        "donut",
        "smileyFace",
        "sun",
        "moon",
        "lightningBolt",
        "wave",
        "flowChartDecision",
        "wedgeRoundRectCallout",
        "gear6",
    ];
    let fills = |i: usize| -> String {
        match i % 4 {
            0 => String::new(), // style fillRef
            1 => solid("70AD47"),
            2 => r#"<a:gradFill><a:gsLst><a:gs pos="0"><a:srgbClr val="FFC000"/></a:gs><a:gs pos="100000"><a:srgbClr val="C00000"/></a:gs></a:gsLst><a:lin ang="2700000" scaled="1"/></a:gradFill>"#.to_owned(),
            _ => r#"<a:pattFill prst="dkUpDiag"><a:fgClr><a:schemeClr val="accent1"/></a:fgClr><a:bgClr><a:schemeClr val="bg1"/></a:bgClr></a:pattFill>"#.to_owned(),
        }
    };
    let deck1 = build(
        &[
            blank_slide("Hình preset", shape_grid(&basic, &fills)),
            blank_slide(
                "Hình preset (2)",
                shape_grid(
                    &[
                        "flowChartProcess",
                        "flowChartTerminator",
                        "flowChartDocument",
                        "flowChartMultidocument",
                        "flowChartMagneticDisk",
                        "actionButtonHome",
                        "mathPlus",
                        "mathMultiply",
                        "mathEqual",
                        "bevel",
                        "frame",
                        "teardrop",
                        "pie",
                        "arc",
                        "blockArc",
                        "chord",
                        "curvedRightArrow",
                        "uturnArrow",
                        "circularArrow",
                        "ribbon2",
                        "ellipseRibbon",
                        "horizontalScroll",
                        "verticalScroll",
                        "doubleWave",
                        "irregularSeal1",
                        "irregularSeal2",
                        "star32",
                        "funnel",
                    ],
                    &|i| {
                        if i % 2 == 0 {
                            solid("5B9BD5")
                        } else {
                            String::new()
                        }
                    },
                ),
            ),
        ],
        &Vec::new(),
        THEME_BLUE,
    );
    // 2: lines, dashes, arrows, connectors, custom geometry, adjust values.
    let ln = |w: u32, dash: &str, head: &str, tail: &str| {
        format!(
            r#"<a:ln w="{w}"><a:solidFill><a:srgbClr val="1F3864"/></a:solidFill><a:prstDash val="{dash}"/><a:headEnd type="{head}"/><a:tailEnd type="{tail}" w="lg" len="lg"/></a:ln>"#
        )
    };
    let mut lines = String::new();
    for (i, (dash, head, tail)) in [
        ("solid", "none", "triangle"),
        ("dash", "oval", "arrow"),
        ("dot", "none", "stealth"),
        ("lgDashDot", "diamond", "triangle"),
        ("sysDash", "none", "none"),
    ]
    .iter()
    .enumerate()
    {
        let y = IN + IN / 2;
        lines.push_str(&format!(
            r#"<p:cxnSp><p:nvCxnSpPr><p:cNvPr id="{}" name="Straight Arrow Connector {}"/><p:cNvCxnSpPr/><p:nvPr/></p:nvCxnSpPr><p:spPr>{}{}{}</p:spPr><p:style><a:lnRef idx="1"><a:schemeClr val="accent1"/></a:lnRef><a:fillRef idx="0"><a:schemeClr val="accent1"/></a:fillRef><a:effectRef idx="0"><a:schemeClr val="accent1"/></a:effectRef><a:fontRef idx="minor"><a:schemeClr val="tx1"/></a:fontRef></p:style></p:cxnSp>"#,
            30 + i,
            i + 1,
            xfrm(IN / 2, y + i as i64 * IN / 2, 5 * IN, IN / 4),
            prst("straightConnector1"),
            ln(19_050 + i as u32 * 6350, dash, head, tail)
        ));
    }
    let bent = format!(
        r#"<p:cxnSp><p:nvCxnSpPr><p:cNvPr id="40" name="Elbow Connector 39"/><p:cNvCxnSpPr><a:stCxn id="41" idx="3"/><a:endCxn id="42" idx="1"/></p:cNvCxnSpPr><p:nvPr/></p:nvCxnSpPr><p:spPr>{}{}{}</p:spPr></p:cxnSp>"#,
        r#"<a:xfrm flipV="1"><a:off x="8229600" y="1828800"/><a:ext cx="1371600" cy="914400"/></a:xfrm>"#,
        r#"<a:prstGeom prst="bentConnector3"><a:avLst><a:gd name="adj1" fmla="val 50000"/></a:avLst></a:prstGeom>"#,
        ln(12_700, "solid", "none", "triangle")
    );
    let cust = sp(
        43,
        "Freeform 42",
        "",
        &(xfrm(7 * IN, 4 * IN, 2 * IN, 2 * IN)
            + r#"<a:custGeom><a:avLst/><a:gdLst><a:gd name="hx" fmla="*/ w 1 2"/></a:gdLst><a:ahLst/><a:cxnLst/><a:rect l="l" t="t" r="r" b="b"/><a:pathLst><a:path w="200" h="200"><a:moveTo><a:pt x="0" y="200"/></a:moveTo><a:cubicBezTo><a:pt x="50" y="0"/><a:pt x="150" y="0"/><a:pt x="200" y="200"/></a:cubicBezTo><a:quadBezTo><a:pt x="100" y="120"/><a:pt x="0" y="200"/></a:quadBezTo><a:close/></a:path><a:path w="200" h="200" fill="none"><a:moveTo><a:pt x="0" y="0"/></a:moveTo><a:arcTo wR="100" hR="50" stAng="10800000" swAng="10800000"/></a:path></a:pathLst></a:custGeom>"#
            + &solid("ED7D31")
            + &ln(9525, "solid", "none", "none")),
        None,
        "",
    );
    let adj = sp(
        44,
        "Rounded Rectangle 43",
        "",
        &(xfrm(10 * IN, 4 * IN, 2 * IN, IN + IN / 2)
            + r#"<a:prstGeom prst="roundRect"><a:avLst><a:gd name="adj" fmla="val 40000"/></a:avLst></a:prstGeom>"#
            + &solid("A5A5A5")),
        None,
        "",
    );
    let boxes = sp(
        41,
        "Rectangle 40",
        "",
        &(xfrm(6 * IN + IN / 2, IN + IN / 2, 2 * IN - IN / 2, IN) + &prst("rect")),
        Some(&body(r#" anchor="ctr""#, &para("Nguồn"))),
        STYLE,
    ) + &sp(
        42,
        "Rectangle 41",
        "",
        &(xfrm(9 * IN + IN / 2, 2 * IN + IN / 2, 2 * IN, IN) + &prst("rect")),
        Some(&body(r#" anchor="ctr""#, &para("Đích"))),
        STYLE,
    );
    let deck2 = build(
        &[blank_slide(
            "Đường, mũi tên, connector, hình tự do",
            lines + &boxes + &bent + &cust + &adj,
        )],
        &Vec::new(),
        THEME_BLUE,
    );
    // 3: groups (nested, rotated, flipped), shadow, rotation, radial gradient.
    let child = |id: u32, x: i64, y: i64, g: &str, f: &str| {
        sp(
            id,
            &format!("Shape {id}"),
            "",
            &(xfrm(x, y, IN, IN) + &prst(g) + f),
            None,
            "",
        )
    };
    let inner = format!(
        r#"<p:grpSp><p:nvGrpSpPr><p:cNvPr id="60" name="Group 59"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm rot="1800000"><a:off x="914400" y="914400"/><a:ext cx="914400" cy="914400"/><a:chOff x="0" y="0"/><a:chExt cx="1828800" cy="1828800"/></a:xfrm></p:grpSpPr>{}{}</p:grpSp>"#,
        child(61, 0, 0, "ellipse", &solid("FF0000")),
        child(62, IN, IN, "triangle", &solid("0070C0"))
    );
    let group = format!(
        r#"<p:grpSp><p:nvGrpSpPr><p:cNvPr id="50" name="Group 49"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm flipH="1"><a:off x="914400" y="1371600"/><a:ext cx="3657600" cy="3657600"/><a:chOff x="0" y="0"/><a:chExt cx="2743200" cy="2743200"/></a:xfrm></p:grpSpPr>{}{}{}</p:grpSp>"#,
        child(51, 0, 0, "rtTriangle", &solid("70AD47")),
        child(52, 2 * IN, 0, "rightArrow", &solid("FFC000")),
        inner
    );
    let shadow = sp(
        70,
        "Shadowed 69",
        "",
        &(xfrm(6 * IN, IN + IN / 2, 2 * IN + IN / 2, IN + IN / 2)
            + &prst("roundRect")
            + &solid("FFFFFF")
            + r#"<a:ln w="12700"><a:solidFill><a:srgbClr val="7F7F7F"/></a:solidFill></a:ln><a:effectLst><a:outerShdw blurRad="76200" dist="76200" dir="2700000" algn="tl" rotWithShape="0"><a:prstClr val="black"><a:alpha val="40000"/></a:prstClr></a:outerShdw></a:effectLst>"#),
        Some(&body(r#" anchor="ctr""#, &para("Đổ bóng"))),
        "",
    );
    let themed_shadow = sp(
        71,
        "Theme effect 70",
        "",
        &(xfrm(9 * IN, IN + IN / 2, 2 * IN + IN / 2, IN + IN / 2) + &prst("ellipse")),
        None,
        &STYLE.replace(r#"<a:effectRef idx="0">"#, r#"<a:effectRef idx="3">"#),
    );
    let rotated = sp(
        72,
        "Rotated 71",
        "",
        &(r#"<a:xfrm rot="2700000" flipV="1"><a:off x="6400800" y="3962400"/><a:ext cx="2286000" cy="1143000"/></a:xfrm>"#.to_owned()
            + &prst("homePlate")
            + r#"<a:gradFill rotWithShape="1"><a:gsLst><a:gs pos="0"><a:schemeClr val="accent1"><a:lumMod val="20000"/><a:lumOff val="80000"/></a:schemeClr></a:gs><a:gs pos="100000"><a:schemeClr val="accent1"/></a:gs></a:gsLst><a:path path="circle"><a:fillToRect l="50000" t="50000" r="50000" b="50000"/></a:path></a:gradFill>"#),
        Some(&body(r#" anchor="ctr""#, &para("Xoay 45°"))),
        "",
    );
    let deck3 = build(
        &[blank_slide(
            "Nhóm, xoay, đổ bóng",
            group + &shadow + &themed_shadow + &rotated,
        )],
        &Vec::new(),
        THEME_BLUE,
    );
    vec![
        ("slide-shapes-presets.pptx", deck1),
        ("slide-shapes-lines.pptx", deck2),
        ("slide-shapes-groups.pptx", deck3),
    ]
}

fn image_decks() -> Vec<(&'static str, Vec<u8>)> {
    let media = |n: usize, w: u32, h: u32, alpha: bool, tint: [u8; 3]| {
        (
            format!("/ppt/media/image{n}.png"),
            String::new(),
            png_image(w, h, alpha, tint),
        )
    };
    let img_rel = |rid: &str, n: usize| r(rid, "image", &format!("../media/image{n}.png"));
    let mut s1 = blank_slide(
        "Ảnh, cắt ảnh, ảnh trong suốt",
        pic(
            4,
            "Picture 3",
            "rId2",
            (IN / 2, IN + IN / 4, 4 * IN, 3 * IN),
            "",
            &prst("rect"),
        ) + &pic(
            5,
            "Picture 4",
            "rId2",
            (5 * IN, IN + IN / 4, 3 * IN, 3 * IN),
            r#"<a:srcRect l="25000" t="10000" r="25000" b="10000"/>"#,
            &prst("rect"),
        ) + &pic(
            6,
            "Picture 5",
            "rId3",
            (9 * IN, IN + IN / 4, 3 * IN, 3 * IN),
            "",
            &prst("rect"),
        ),
    );
    s1.rels = vec![img_rel("rId2", 1), img_rel("rId3", 2)];
    let mut s2 = blank_slide(
        "Ảnh theo hình (ellipse, star) và viền",
        pic(
            4,
            "Picture 3",
            "rId2",
            (IN, IN + IN / 2, 3 * IN, 3 * IN),
            "",
            &prst("ellipse"),
        ) + &pic(
            5,
            "Picture 4",
            "rId2",
            (5 * IN, IN + IN / 2, 3 * IN, 3 * IN),
            "",
            &(prst("star5")
                + r#"<a:ln w="38100"><a:solidFill><a:srgbClr val="FFC000"/></a:solidFill></a:ln>"#),
        ) + &pic(
            6,
            "Picture 5",
            "rId2",
            (9 * IN, IN + IN / 2, 3 * IN, 2 * IN),
            "",
            r#"<a:prstGeom prst="roundRect"><a:avLst><a:gd name="adj" fmla="val 30000"/></a:avLst></a:prstGeom>"#,
        ),
    );
    s2.rels = vec![img_rel("rId2", 1)];
    let mut s3 = blank_slide(
        "Ảnh làm nền hình và nền slide",
        sp(
            4,
            "Picture fill 3",
            "",
            &(xfrm(IN, IN + IN / 2, 5 * IN, 3 * IN)
                + &prst("wave")
                + r#"<a:blipFill dpi="0" rotWithShape="1"><a:blip r:embed="rId3"/><a:srcRect/><a:stretch><a:fillRect/></a:stretch></a:blipFill>"#),
            Some(&body(r#" anchor="ctr""#, &para("Chữ trên ảnh"))),
            "",
        ),
    );
    s3.bg = r#"<p:bg><p:bgPr><a:blipFill dpi="0" rotWithShape="1"><a:blip r:embed="rId2"><a:alphaModFix amt="35000"/></a:blip><a:srcRect/><a:stretch><a:fillRect/></a:stretch></a:blipFill><a:effectLst/></p:bgPr></p:bg>"#.into();
    s3.rels = vec![img_rel("rId2", 1), img_rel("rId3", 2)];
    let parts = vec![
        media(1, 64, 48, false, [46, 117, 182]),
        media(2, 32, 32, true, [237, 125, 49]),
    ];
    vec![
        ("slide-images-crop.pptx", build(&[s1], &parts, THEME_BLUE)),
        (
            "slide-images-shapes.pptx",
            build(&[s2], &parts[..1].to_vec(), THEME_BLUE),
        ),
        ("slide-images-fill.pptx", build(&[s3], &parts, THEME_BLUE)),
    ]
}

fn table_xml(
    rows: &[Vec<&str>],
    col_w: i64,
    style: &str,
    cell_extra: &dyn Fn(usize, usize) -> String,
) -> String {
    let ncol = rows.first().map_or(0, Vec::len);
    let grid: String = (0..ncol)
        .map(|_| format!(r#"<a:gridCol w="{col_w}"/>"#))
        .collect();
    let mut body = String::new();
    for (ri, row) in rows.iter().enumerate() {
        body.push_str(r#"<a:tr h="370840">"#);
        for (ci, text) in row.iter().enumerate() {
            let extra = cell_extra(ri, ci);
            let (attrs, tcpr) = extra.split_once('|').unwrap_or(("", extra.as_str()));
            let t = if text.is_empty() {
                r#"<a:p><a:endParaRPr lang="vi-VN"/></a:p>"#.to_owned()
            } else {
                format!(r#"<a:p><a:r><a:rPr lang="vi-VN" dirty="0"/><a:t>{text}</a:t></a:r></a:p>"#)
            };
            body.push_str(&format!(
                r#"<a:tc{attrs}><a:txBody><a:bodyPr/><a:lstStyle/>{t}</a:txBody><a:tcPr{tcpr}</a:tcPr></a:tc>"#
            ));
        }
        body.push_str("</a:tr>");
    }
    format!(
        r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="4" name="Table 3"/><p:cNvGraphicFramePr><a:graphicFrameLocks noGrp="1"/></p:cNvGraphicFramePr><p:nvPr/></p:nvGraphicFramePr><p:xfrm><a:off x="457200" y="1371600"/><a:ext cx="{w}" cy="{h}"/></p:xfrm><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table"><a:tbl><a:tblPr firstRow="1" bandRow="1"><a:tableStyleId>{style}</a:tableStyleId></a:tblPr><a:tblGrid>{grid}</a:tblGrid>{body}</a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#,
        w = col_w * ncol as i64,
        h = 370_840 * rows.len() as i64,
    )
}

fn table_decks() -> Vec<(&'static str, Vec<u8>)> {
    let border = |c: &str| {
        format!(
            r#"<a:lnL w="12700"><a:solidFill><a:srgbClr val="{c}"/></a:solidFill></a:lnL><a:lnR w="12700"><a:solidFill><a:srgbClr val="{c}"/></a:solidFill></a:lnR><a:lnT w="12700"><a:solidFill><a:srgbClr val="{c}"/></a:solidFill></a:lnT><a:lnB w="12700"><a:solidFill><a:srgbClr val="{c}"/></a:solidFill></a:lnB>"#
        )
    };
    let rows = vec![
        vec!["Chỉ tiêu", "Quý I", "Quý II", "Quý III"],
        vec!["Doanh thu", "1.200", "1.350", "1.480"],
        vec!["Chi phí", "900", "950", "1.010"],
        vec!["Lợi nhuận", "300", "400", "470"],
    ];
    let t1 = table_xml(
        &rows,
        2_743_200,
        "{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}",
        &|r, _| {
            let fill = if r == 0 {
                solid("2E75B6")
            } else if r % 2 == 1 {
                solid("DEEAF6")
            } else {
                solid("FFFFFF")
            };
            format!(r#"|>{}{fill}"#, border("FFFFFF"))
        },
    );
    let t2 = table_xml(
        &[
            vec!["Gộp ngang", "", "C"],
            vec!["Gộp dọc", "B2", "C2"],
            vec!["", "B3", "C3"],
        ],
        3_048_000,
        "{2D5ABB26-0587-4C30-8999-92F81FD0307C}",
        &|r, c| match (r, c) {
            (0, 0) => format!(r#" gridSpan="2"|>{}{}"#, border("000000"), solid("FFF2CC")),
            (0, 1) => format!(r#" hMerge="1"|>{}"#, border("000000")),
            (1, 0) => format!(
                r#" rowSpan="2"|anchor="ctr">{}{}"#,
                border("000000"),
                solid("E2EFDA")
            )
            .replace("|anchor", "| anchor"),
            (2, 0) => format!(r#" vMerge="1"|>{}"#, border("000000")),
            _ => format!(r#"|>{}"#, border("000000")),
        },
    );
    let t3 = table_xml(
        &[
            vec!["STT", "Họ và tên", "Đơn vị", "Ghi chú"],
            vec!["1", "Nguyễn Văn An", "Phòng Kế hoạch", "Lề rộng"],
            vec!["2", "Trần Thị Bình", "Phòng Kỹ thuật", ""],
        ],
        2_590_800,
        "{073A0DAA-6AF3-43AB-8588-CEC1D06C72B9}",
        &|r, c| {
            if r == 1 && c == 3 {
                format!(
                    r#"|marL="182880" marR="182880" marT="91440" marB="91440" anchor="b">{}"#,
                    border("7F7F7F")
                )
                .replace("|marL", "| marL")
            } else {
                format!(r#"|>{}"#, border("7F7F7F"))
            }
        },
    );
    vec![
        (
            "slide-tables-basic.pptx",
            build(&[blank_slide("Bảng số liệu", t1)], &Vec::new(), THEME_BLUE),
        ),
        (
            "slide-tables-merge.pptx",
            build(&[blank_slide("Bảng gộp ô", t2)], &Vec::new(), THEME_BLUE),
        ),
        (
            "slide-tables-margins.pptx",
            build(&[blank_slide("Bảng lề ô", t3)], &Vec::new(), THEME_BLUE),
        ),
    ]
}

fn chart_part(plot: &str, legend: bool) -> Vec<u8> {
    let legend = if legend {
        r#"<c:legend><c:legendPos val="r"/><c:overlay val="0"/></c:legend>"#
    } else {
        ""
    };
    format!(
        r#"{XML_DECL}<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><c:date1904 val="0"/><c:lang val="vi-VN"/><c:roundedCorners val="0"/><c:chart><c:title><c:tx><c:rich><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="vi-VN"/><a:t>Doanh thu theo quý</a:t></a:r></a:p></c:rich></c:tx><c:overlay val="0"/></c:title><c:autoTitleDeleted val="0"/><c:plotArea><c:layout/>{plot}</c:plotArea>{legend}<c:plotVisOnly val="1"/><c:dispBlanksAs val="gap"/></c:chart><c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr><a:defRPr sz="1200"/></a:pPr><a:endParaRPr lang="vi-VN"/></a:p></c:txPr></c:chartSpace>"#
    )
    .into_bytes()
}

fn series(idx: usize, name: &str, vals: &[f64], color: Option<&str>) -> String {
    let cats = ["Quý I", "Quý II", "Quý III", "Quý IV"];
    let cat_pts: String = cats
        .iter()
        .take(vals.len())
        .enumerate()
        .map(|(i, c)| format!(r#"<c:pt idx="{i}"><c:v>{c}</c:v></c:pt>"#))
        .collect();
    let val_pts: String = vals
        .iter()
        .enumerate()
        .map(|(i, v)| format!(r#"<c:pt idx="{i}"><c:v>{v}</c:v></c:pt>"#))
        .collect();
    let sp_pr = color
        .map(|c| format!(r#"<c:spPr><a:solidFill><a:srgbClr val="{c}"/></a:solidFill><a:ln w="28575"><a:solidFill><a:srgbClr val="{c}"/></a:solidFill></a:ln></c:spPr>"#))
        .unwrap_or_default();
    let col = char::from(b'B' + idx as u8);
    format!(
        r#"<c:ser><c:idx val="{idx}"/><c:order val="{idx}"/><c:tx><c:strRef><c:f>Sheet1!${col}$1</c:f><c:strCache><c:ptCount val="1"/><c:pt idx="0"><c:v>{name}</c:v></c:pt></c:strCache></c:strRef></c:tx>{sp_pr}<c:cat><c:strRef><c:f>Sheet1!$A$2:$A$5</c:f><c:strCache><c:ptCount val="{n}"/>{cat_pts}</c:strCache></c:strRef></c:cat><c:val><c:numRef><c:f>Sheet1!${col}$2:${col}$5</c:f><c:numCache><c:formatCode>General</c:formatCode><c:ptCount val="{n}"/>{val_pts}</c:numCache></c:numRef></c:val></c:ser>"#,
        n = vals.len()
    )
}

const AXES: &str = r#"<c:axId val="111"/><c:axId val="222"/></c:XCHART><c:catAx><c:axId val="111"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="b"/><c:crossAx val="222"/></c:catAx><c:valAx><c:axId val="222"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="l"/><c:majorGridlines/><c:crossAx val="111"/></c:valAx>"#;

fn chart_frame(id: u32, rid: &str) -> String {
    format!(
        r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="{id}" name="Chart {}"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><p:xfrm><a:off x="914400" y="1280160"/><a:ext cx="10363200" cy="5029200"/></p:xfrm><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" r:id="{rid}"/></a:graphicData></a:graphic></p:graphicFrame>"#,
        id - 1
    )
}

fn chart_decks() -> Vec<(&'static str, Vec<u8>)> {
    let ct = format!("{CT}drawingml.chart+xml");
    let deck = |title: &str, plot: String, legend: bool| {
        let mut s = blank_slide(title, chart_frame(4, "rId2"));
        s.rels = vec![r("rId2", "chart", "../charts/chart1.xml")];
        build(
            &[s],
            &vec![(
                "/ppt/charts/chart1.xml".to_owned(),
                ct.clone(),
                chart_part(&plot, legend),
            )],
            THEME_BLUE,
        )
    };
    let bar = format!(
        r#"<c:barChart><c:barDir val="col"/><c:grouping val="clustered"/><c:varyColors val="0"/>{}{}{}<c:gapWidth val="150"/>{}"#,
        series(0, "2024", &[12.0, 15.5, 14.0, 18.0], None),
        series(1, "2025", &[14.0, 17.0, 16.5, 21.0], None),
        series(2, "2026", &[16.0, 19.5, 20.0, 24.0], Some("70AD47")),
        AXES.replace("XCHART", "barChart")
    );
    let stacked = format!(
        r#"<c:barChart><c:barDir val="bar"/><c:grouping val="stacked"/><c:varyColors val="0"/>{}{}<c:gapWidth val="80"/><c:overlap val="100"/>{}"#,
        series(0, "Miền Bắc", &[5.0, 6.0, 7.0, 8.0], None),
        series(1, "Miền Nam", &[4.0, 5.5, 6.0, 9.0], None),
        AXES.replace("XCHART", "barChart")
    );
    let line = format!(
        r#"<c:lineChart><c:grouping val="standard"/><c:varyColors val="0"/>{}{}<c:marker val="1"/>{}"#,
        series(0, "Kế hoạch", &[10.0, 12.0, 15.0, 17.0], Some("2E75B6")),
        series(1, "Thực hiện", &[9.0, 13.5, 14.0, 19.0], Some("ED7D31")),
        AXES.replace("XCHART", "lineChart")
    );
    let pie = format!(
        r#"<c:pieChart><c:varyColors val="1"/>{}<c:firstSliceAng val="0"/></c:pieChart>"#,
        series(0, "Cơ cấu", &[45.0, 25.0, 20.0, 10.0], None)
    );
    vec![
        ("slide-charts-column.pptx", deck("Biểu đồ cột", bar, true)),
        (
            "slide-charts-bar-stacked.pptx",
            deck("Biểu đồ thanh chồng", stacked, true),
        ),
        ("slide-charts-line.pptx", deck("Biểu đồ đường", line, true)),
        ("slide-charts-pie.pptx", deck("Biểu đồ tròn", pie, true)),
    ]
}

fn anim_decks() -> Vec<(&'static str, Vec<u8>)> {
    let fade = r#"<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:Choice xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main" Requires="p14"><p:transition spd="slow" p14:dur="1500"><p:fade/></p:transition></mc:Choice><mc:Fallback><p:transition spd="slow"><p:fade/></p:transition></mc:Fallback></mc:AlternateContent>"#;
    let push =
        r#"<p:transition spd="med" advClick="0" advTm="3000"><p:push dir="u"/></p:transition>"#;
    let wipe = r#"<p:transition><p:wipe dir="r"/></p:transition>"#;
    let cover = r#"<p:transition spd="fast"><p:cover dir="ld"/></p:transition>"#;
    let shapes = |n: usize| -> String {
        (0..n)
            .map(|i| {
                sp(
                    4 + i as u32,
                    &format!("Shape {}", 3 + i),
                    "",
                    &(xfrm(IN / 2 + i as i64 * 3 * IN, 2 * IN, 2 * IN + IN / 2, 2 * IN)
                        + &prst("roundRect")),
                    Some(&body(
                        r#" anchor="ctr""#,
                        &para(&format!("Hiệu ứng {}", i + 1)),
                    )),
                    STYLE,
                )
            })
            .collect()
    };
    let effect = |id: u32, spid: u32, preset: u32, sub: u32, node: &str, delay: &str, bhv: &str| {
        format!(
            r#"<p:par><p:cTn id="{id}" presetID="{preset}" presetClass="entr" presetSubtype="{sub}" fill="hold" nodeType="{node}"><p:stCondLst><p:cond delay="{delay}"/></p:stCondLst><p:childTnLst><p:set><p:cBhvr><p:cTn id="{}" dur="1" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst></p:cTn><p:tgtEl><p:spTgt spid="{spid}"/></p:tgtEl><p:attrNameLst><p:attrName>style.visibility</p:attrName></p:attrNameLst></p:cBhvr><p:to><p:strVal val="visible"/></p:to></p:set>{bhv}</p:childTnLst></p:cTn></p:par>"#,
            id + 1
        )
    };
    let fx_fade = |id: u32, spid: u32| {
        format!(
            r#"<p:animEffect transition="in" filter="fade"><p:cBhvr><p:cTn id="{id}" dur="500"/><p:tgtEl><p:spTgt spid="{spid}"/></p:tgtEl></p:cBhvr></p:animEffect>"#
        )
    };
    let fx_wipe = |id: u32, spid: u32| {
        format!(
            r#"<p:animEffect transition="in" filter="wipe(down)"><p:cBhvr><p:cTn id="{id}" dur="500"/><p:tgtEl><p:spTgt spid="{spid}"/></p:tgtEl></p:cBhvr></p:animEffect>"#
        )
    };
    let fx_fly = |id: u32, spid: u32| {
        format!(
            r##"<p:anim calcmode="lin" valueType="num"><p:cBhvr additive="base"><p:cTn id="{id}" dur="500" fill="hold"/><p:tgtEl><p:spTgt spid="{spid}"/></p:tgtEl><p:attrNameLst><p:attrName>ppt_x</p:attrName></p:attrNameLst></p:cBhvr><p:tavLst><p:tav tm="0"><p:val><p:strVal val="#ppt_x"/></p:val></p:tav><p:tav tm="100000"><p:val><p:strVal val="#ppt_x"/></p:val></p:tav></p:tavLst></p:anim><p:anim calcmode="lin" valueType="num"><p:cBhvr additive="base"><p:cTn id="{}" dur="500" fill="hold"/><p:tgtEl><p:spTgt spid="{spid}"/></p:tgtEl><p:attrNameLst><p:attrName>ppt_y</p:attrName></p:attrNameLst></p:cBhvr><p:tavLst><p:tav tm="0"><p:val><p:strVal val="1+#ppt_h/2"/></p:val></p:tav><p:tav tm="100000"><p:val><p:strVal val="#ppt_y"/></p:val></p:tav></p:tavLst></p:anim>"##,
            id + 1
        )
    };
    let click = |id: u32, inner: &str| {
        format!(
            r#"<p:par><p:cTn id="{id}" fill="hold"><p:stCondLst><p:cond delay="indefinite"/></p:stCondLst><p:childTnLst><p:par><p:cTn id="{}" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst>{inner}</p:childTnLst></p:cTn></p:par></p:childTnLst></p:cTn></p:par>"#,
            id + 1
        )
    };
    let timing = |groups: &str, bld: &str| {
        format!(
            r#"<p:timing><p:tnLst><p:par><p:cTn id="1" dur="indefinite" restart="never" nodeType="tmRoot"><p:childTnLst><p:seq concurrent="1" nextAc="seek"><p:cTn id="2" dur="indefinite" nodeType="mainSeq"><p:childTnLst>{groups}</p:childTnLst></p:cTn><p:prevCondLst><p:cond evt="onPrev" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:prevCondLst><p:nextCondLst><p:cond evt="onNext" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:nextCondLst></p:seq></p:childTnLst></p:cTn></p:par></p:tnLst>{bld}</p:timing>"#
        )
    };
    let bld = r#"<p:bldLst><p:bldP spid="4" grpId="0" animBg="1"/><p:bldP spid="5" grpId="0" animBg="1"/><p:bldP spid="6" grpId="0" animBg="1"/></p:bldLst>"#;
    // Deck 1: fade / appear on click.
    let g1 = click(3, &effect(5, 4, 10, 0, "clickEffect", "0", &fx_fade(7, 4)))
        + &click(8, &effect(10, 5, 1, 0, "clickEffect", "0", ""));
    let mut a1 = blank_slide("Mờ dần và xuất hiện", shapes(2));
    a1.tail = format!(
        "{fade}{}",
        timing(
            &g1,
            &bld.replace(r#"<p:bldP spid="6" grpId="0" animBg="1"/>"#, "")
        )
    );
    let mut a1b = title_slide("Chuyển trang đẩy lên", "Tự chuyển sau 3 giây");
    a1b.tail = push.into();
    // Deck 2: fly in + wipe with-previous / after-previous.
    let g2 = click(
        3,
        &(effect(5, 4, 2, 4, "clickEffect", "0", &fx_fly(7, 4))
            + &effect(9, 5, 22, 1, "withEffect", "0", &fx_wipe(11, 5))),
    ) + &click(
        12,
        &effect(14, 6, 10, 0, "afterEffect", "250", &fx_fade(16, 6)),
    );
    let mut a2 = blank_slide("Bay vào và quét", shapes(3));
    a2.tail = format!("{wipe}{}", timing(&g2, bld));
    // Deck 3: transitions only, several slides.
    let mut t1 = content_slide("Chuyển trang che phủ", &body("", &para("Hiệu ứng che phủ")));
    t1.tail = cover.into();
    let mut t2 = content_slide("Chuyển trang mờ", &body("", &para("Hiệu ứng mờ dần")));
    t2.tail = fade.into();
    let mut t3 = content_slide("Chuyển trang ngẫu nhiên", &body("", &para("Ngẫu nhiên")));
    t3.tail = r#"<p:transition spd="slow"><p:random/></p:transition>"#.into();
    vec![
        (
            "slide-anim-fade-appear.pptx",
            build(&[a1, a1b], &Vec::new(), THEME_BLUE),
        ),
        (
            "slide-anim-fly-wipe.pptx",
            build(&[a2], &Vec::new(), THEME_BLUE),
        ),
        (
            "slide-anim-transitions.pptx",
            build(&[t1, t2, t3], &Vec::new(), THEME_BLUE),
        ),
    ]
}

fn media_decks() -> Vec<(&'static str, Vec<u8>)> {
    let poster = (
        "/ppt/media/image1.png".to_owned(),
        String::new(),
        png_image(64, 36, false, [31, 56, 100]),
    );
    // Not real media streams: a few header bytes, enough to test byte-for-byte preservation.
    let mut mp4 = b"\x00\x00\x00\x18ftypmp42\x00\x00\x00\x00mp42isom".to_vec();
    mp4.extend(std::iter::repeat_n(0u8, 64));
    let mut mp3 = b"ID3\x04\x00\x00\x00\x00\x00\x00".to_vec();
    mp3.extend(std::iter::repeat_n(0u8, 32));
    let video = |id: u32, link: &str, embed: &str| {
        format!(
            r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="clip{id}.mp4"><a:hlinkClick r:id="" action="ppaction://media"/></p:cNvPr><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr><a:videoFile r:link="{link}"/><p:extLst><p:ext uri="{{DAA4B4D4-6D71-4841-9C94-3DE7FCFB9230}}"><p14:media xmlns:p14="{P14}" r:embed="{embed}"><p14:trim st="500"/></p14:media></p:ext></p:extLst></p:nvPr></p:nvPicPr><p:blipFill><a:blip r:embed="rId4"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{}{}</p:spPr></p:pic>"#,
            xfrm(2 * IN, IN + IN / 2, 8 * IN, 4 * IN + IN / 2),
            prst("rect")
        )
    };
    let audio = format!(
        r#"<p:pic><p:nvPicPr><p:cNvPr id="5" name="sound.mp3"><a:hlinkClick r:id="" action="ppaction://media"/></p:cNvPr><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr><a:audioFile r:link="rId5"/><p:extLst><p:ext uri="{{DAA4B4D4-6D71-4841-9C94-3DE7FCFB9230}}"><p14:media xmlns:p14="{P14}" r:embed="rId6"/></p:ext></p:extLst></p:nvPr></p:nvPicPr><p:blipFill><a:blip r:embed="rId4"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{}{}</p:spPr></p:pic>"#,
        xfrm(11 * IN, 6 * IN, IN / 2, IN / 2),
        prst("rect")
    );
    let media_timing = r#"<p:timing><p:tnLst><p:par><p:cTn id="1" dur="indefinite" restart="never" nodeType="tmRoot"><p:childTnLst><p:video><p:cMediaNode vol="80000"><p:cTn id="2" fill="hold" display="0"><p:stCondLst><p:cond delay="indefinite"/></p:stCondLst></p:cTn><p:tgtEl><p:spTgt spid="4"/></p:tgtEl></p:cMediaNode></p:video></p:childTnLst></p:cTn></p:par></p:tnLst></p:timing>"#;
    let vrels = |s: &mut SlideSpec| {
        s.rels = vec![
            r("rId2", "video", "../media/media1.mp4"),
            r(
                "rId3",
                "http://schemas.microsoft.com/office/2007/relationships/media",
                "../media/media1.mp4",
            ),
            r("rId4", "image", "../media/image1.png"),
        ];
    };
    let mut s1 = blank_slide("Video nhúng", video(4, "rId2", "rId3"));
    vrels(&mut s1);
    s1.tail = media_timing.into();
    let mut s2 = blank_slide("Âm thanh nhúng", audio.clone());
    s2.rels = vec![
        r("rId4", "image", "../media/image1.png"),
        r("rId5", "audio", "../media/media2.mp3"),
        r(
            "rId6",
            "http://schemas.microsoft.com/office/2007/relationships/media",
            "../media/media2.mp3",
        ),
    ];
    let mut s3 = blank_slide(
        "Video liên kết ngoài và âm thanh",
        video(4, "rId2", "rId3") + &audio,
    );
    s3.rels = vec![
        r("rId2", "video", "https://example.invalid/clip.mp4"),
        r(
            "rId3",
            "http://schemas.microsoft.com/office/2007/relationships/media",
            "https://example.invalid/clip.mp4",
        ),
        r("rId4", "image", "../media/image1.png"),
        r("rId5", "audio", "../media/media2.mp3"),
        r(
            "rId6",
            "http://schemas.microsoft.com/office/2007/relationships/media",
            "../media/media2.mp3",
        ),
    ];
    let v = ("/ppt/media/media1.mp4".to_owned(), String::new(), mp4);
    let a = ("/ppt/media/media2.mp3".to_owned(), String::new(), mp3);
    vec![
        (
            "slide-media-video.pptx",
            build(&[s1], &vec![poster.clone(), v], THEME_BLUE),
        ),
        (
            "slide-media-audio.pptx",
            build(&[s2], &vec![poster.clone(), a.clone()], THEME_BLUE),
        ),
        (
            "slide-media-linked.pptx",
            build(&[s3], &vec![poster, a], THEME_BLUE),
        ),
    ]
}

fn notes_theme_decks() -> Vec<(&'static str, Vec<u8>)> {
    let mut n1 = title_slide("Có ghi chú", "Ghi chú người trình bày");
    n1.notes = Some("Chào mừng quý vị.\nGiới thiệu mục tiêu buổi họp.".into());
    let mut n2 = content_slide("Slide thứ hai", &body("", &para("Nội dung")));
    n2.notes = Some("Nhấn mạnh số liệu quý III.".into());
    let n3 = content_slide("Slide không ghi chú", &body("", &para("Không có ghi chú")));
    // Theme variant + colour map override + backgrounds.
    let mut c1 = content_slide(
        "Nền gradient, đổi bảng màu",
        &body("", &para("Chữ dùng tx1 (đã đổi sang lt1)")),
    );
    c1.bg = r#"<p:bg><p:bgPr><a:gradFill flip="none" rotWithShape="1"><a:gsLst><a:gs pos="0"><a:schemeClr val="accent1"><a:lumMod val="50000"/></a:schemeClr></a:gs><a:gs pos="100000"><a:schemeClr val="accent1"/></a:gs></a:gsLst><a:lin ang="5400000" scaled="1"/></a:gradFill><a:effectLst/></p:bgPr></p:bg>"#.into();
    let mut c2 = content_slide("Nền theo theme (bgRef)", &body("", &para("bgRef idx=1003")));
    c2.bg = r#"<p:bg><p:bgRef idx="1003"><a:schemeClr val="accent2"/></p:bgRef></p:bg>"#.into();
    let mut c3 = blank_slide(
        "Nền hoa văn",
        sp(
            4,
            "Shape 3",
            "",
            &(xfrm(4 * IN, 2 * IN, 5 * IN, 3 * IN) + &prst("ellipse")),
            Some(&body(r#" anchor="ctr""#, &para("Màu theme"))),
            STYLE,
        ),
    );
    c3.bg = r#"<p:bg><p:bgPr><a:pattFill prst="smGrid"><a:fgClr><a:schemeClr val="accent1"><a:lumMod val="40000"/><a:lumOff val="60000"/></a:schemeClr></a:fgClr><a:bgClr><a:schemeClr val="bg1"/></a:bgClr></a:pattFill><a:effectLst/></p:bgPr></p:bg>"#.into();
    let mut slides = vec![c1, c2, c3];
    // Swap the slide's colour mapping on the first slide (§19.3.1.7 overrideClrMapping).
    slides[0].tail = String::new();
    let ovr = r#"<p:clrMapOvr><a:overrideClrMapping bg1="dk1" tx1="lt1" bg2="dk2" tx2="lt2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/></p:clrMapOvr>"#;
    let deck2 = build(
        &slides,
        &Vec::new(),
        ("Green", "548235", "Cambria", "Arial"),
    );
    let deck2 = replace_in_part(
        &deck2,
        "/ppt/slides/slide1.xml",
        "<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>",
        ovr,
    );
    let mut m1 = title_slide("Ẩn hình của master", "showMasterSp=0");
    m1.notes = Some("Slide ẩn.".into());
    let deck3 = build(
        &[m1, n3],
        &Vec::new(),
        ("Orange", "C55A11", "Georgia", "Verdana"),
    );
    let deck3 = replace_in_part(
        &deck3,
        "/ppt/slides/slide1.xml",
        "<p:sld ",
        r#"<p:sld show="0" showMasterSp="0" "#,
    );
    vec![
        (
            "slide-notes-basic.pptx",
            build(&[n1, n2], &Vec::new(), THEME_BLUE),
        ),
        ("slide-theme-colormap.pptx", deck2),
        ("slide-theme-hidden.pptx", deck3),
    ]
}

/// Textual replacement inside one part of a package.
fn replace_in_part(pkg: &[u8], part: &str, from: &str, to: &str) -> Vec<u8> {
    let Ok(mut p) = Package::read(pkg, &bo_opc::Limits::default()) else {
        return pkg.to_vec();
    };
    let Some(data) = p
        .part(part)
        .map(|x| String::from_utf8_lossy(&x.data).replacen(from, to, 1))
    else {
        return pkg.to_vec();
    };
    p.put_part(part, data.into_bytes());
    p.write().unwrap_or_else(|_| pkg.to_vec())
}

/// Every corpus deck: (file name, bytes) — generated decks plus decks
/// edited through the slide edit API (`slide_edits`).
pub fn decks() -> Vec<(&'static str, Vec<u8>)> {
    let mut v = base_decks();
    let edited = crate::slide_edits::decks(&v, &sample_png());
    v.extend(edited);
    v
}

/// Picture inserted by the edited decks.
pub fn sample_png() -> Vec<u8> {
    png_image(48, 32, false, [46, 117, 182])
}

/// Decks generated from markup.
pub fn base_decks() -> Vec<(&'static str, Vec<u8>)> {
    let mut v = Vec::new();
    v.extend(text_decks());
    v.extend(shape_decks());
    v.extend(image_decks());
    v.extend(table_decks());
    v.extend(chart_decks());
    v.extend(anim_decks());
    v.extend(media_decks());
    v.extend(notes_theme_decks());
    v
}

#[cfg(test)]
mod tests {
    /// The committed corpus is exactly what the generator produces
    /// (regenerate with `cargo run -p bo-cli -- sample-slides tests/corpus/slide`).
    #[test]
    fn committed_corpus_is_up_to_date() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/slide");
        let decks = super::decks();
        assert!(decks.len() >= 24);
        for (name, bytes) in decks {
            let on_disk = std::fs::read(dir.join(name)).unwrap_or_default();
            assert!(on_disk == bytes, "{name} is stale: regenerate the corpus");
        }
    }
}
