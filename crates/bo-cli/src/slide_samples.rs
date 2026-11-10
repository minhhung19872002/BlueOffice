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
    // §20.1.8.32 glow (alone and with a soft shadow, §20.1.8.49), on a shape and a
    // rotated arrow.
    let glow = sp(
        80,
        "Glow 79",
        "",
        &(xfrm(IN, 2 * IN, 3 * IN, IN + IN / 2)
            + &prst("roundRect")
            + &solid("FFFFFF")
            + r#"<a:effectLst><a:glow rad="139700"><a:srgbClr val="FFC000"><a:alpha val="60000"/></a:srgbClr></a:glow></a:effectLst>"#),
        Some(&body(r#" anchor="ctr""#, &para("Phát sáng"))),
        "",
    );
    let glow_shadow = sp(
        81,
        "Glow shadow 80",
        "",
        &(r#"<a:xfrm rot="1800000"><a:off x="5486400" y="1828800"/><a:ext cx="2743200" cy="1371600"/></a:xfrm>"#.to_owned()
            + &prst("rightArrow")
            + &solid("4472C4")
            + r#"<a:effectLst><a:glow rad="101600"><a:schemeClr val="accent2"><a:satMod val="175000"/><a:alpha val="40000"/></a:schemeClr></a:glow><a:outerShdw blurRad="114300" dist="76200" dir="2700000" algn="tl" rotWithShape="0"><a:prstClr val="black"><a:alpha val="50000"/></a:prstClr></a:outerShdw></a:effectLst>"#),
        None,
        "",
    );
    let deck4 = build(
        &[blank_slide("Phát sáng và đổ bóng mờ", glow + &glow_shadow)],
        &Vec::new(),
        THEME_BLUE,
    );
    // §20.1.5 3-D: oblique extrusion with an extrusion colour, an isometric cube with a top bevel and a
    // contour, a perspective camera with an explicit rotation (§20.1.5.11 rot).
    let solid3d = |id: u32, x: i64, geom: &str, rgb: &str, scene: &str, sp3d: &str, text: &str| {
        sp(
            id,
            &format!("3D {id}"),
            "",
            &(xfrm(x, 2 * IN, 2 * IN, 2 * IN)
                + &prst(geom)
                + &solid(rgb)
                + &format!(
                    "<a:scene3d>{scene}<a:lightRig rig=\"threePt\" dir=\"t\"/></a:scene3d>{sp3d}"
                )),
            Some(&body(r#" anchor="ctr""#, &para(text))),
            "",
        )
    };
    let deck5 = build(
        &[blank_slide(
            "Hình 3D: đùn, vát cạnh, phối cảnh",
            solid3d(
                90,
                IN,
                "rect",
                "4472C4",
                r#"<a:camera prst="obliqueTopRight"/>"#,
                r#"<a:sp3d extrusionH="381000"><a:extrusionClr><a:srgbClr val="1F3864"/></a:extrusionClr></a:sp3d>"#,
                "Đùn",
            ) + &solid3d(
                91,
                4 * IN,
                "rect",
                "ED7D31",
                r#"<a:camera prst="isometricLeftDown"/>"#,
                r#"<a:sp3d extrusionH="762000" contourW="12700"><a:bevelT w="101600" h="101600"/><a:contourClr><a:srgbClr val="FFFFFF"/></a:contourClr></a:sp3d>"#,
                "Khối",
            ) + &solid3d(
                92,
                7 * IN,
                "ellipse",
                "70AD47",
                r#"<a:camera prst="perspectiveFront"><a:rot lat="1200000" lon="19800000" rev="0"/></a:camera>"#,
                r#"<a:sp3d extrusionH="254000" prstMaterial="plastic"><a:bevelT w="63500" h="63500"/></a:sp3d>"#,
                "Trụ",
            ),
        )],
        &Vec::new(),
        THEME_BLUE,
    );
    vec![
        ("slide-shapes-presets.pptx", deck1),
        ("slide-shapes-lines.pptx", deck2),
        ("slide-shapes-groups.pptx", deck3),
        ("slide-shapes-glow.pptx", deck4),
        ("slide-shapes-3d.pptx", deck5),
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

/// SmartArt: graphic frame with `dgm:relIds` (ECMA-376 Part 1, §21.4.2.6) and the
/// diagram parts; the drawing fallback ([MS-ODRAWXML] `dsp:drawing`) is what
/// BlueSlide shows. `dsp:dataModelExt@relId` names the slide's relationship to it.
fn smartart_decks() -> Vec<(&'static str, Vec<u8>)> {
    const DGM: &str = "http://schemas.openxmlformats.org/drawingml/2006/diagram";
    const DSP: &str = "http://schemas.microsoft.com/office/drawing/2008/diagram";
    const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
    let frame = |id: u32, x: i64, y: i64, cx: i64, cy: i64| {
        format!(
            r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="{id}" name="Sơ đồ {id}"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><p:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></p:xfrm><a:graphic><a:graphicData uri="{DGM}"><dgm:relIds xmlns:dgm="{DGM}" r:dm="rId2" r:lo="rId3" r:qs="rId4" r:cs="rId5"/></a:graphicData></a:graphic></p:graphicFrame>"#
        )
    };
    let dsp_sp = |n: u32, x: i64, y: i64, cx: i64, cy: i64, geom: &str, rgb: &str, text: &str| {
        format!(
            r#"<dsp:sp modelId="{{00000000-0000-0000-0000-0000000000{n:02}}}"><dsp:nvSpPr><dsp:cNvPr id="0" name=""/><dsp:cNvSpPr/></dsp:nvSpPr><dsp:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="{geom}"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="{rgb}"/></a:solidFill><a:ln w="12700"><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill></a:ln></dsp:spPr><dsp:txBody><a:bodyPr anchor="ctr"/><a:lstStyle/><a:p><a:pPr algn="ctr"/><a:r><a:rPr lang="vi-VN" sz="2000"><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill></a:rPr><a:t>{text}</a:t></a:r></a:p></dsp:txBody></dsp:sp>"#
        )
    };
    let parts = |shapes: &str| -> Parts {
        let x = |root: &str, inner: &str| {
            format!(r#"{XML_DECL}<{root} xmlns:dgm="{DGM}" xmlns:a="{A}">{inner}</{root}>"#)
                .into_bytes()
        };
        vec![
            (
                "/ppt/diagrams/data1.xml".into(),
                format!("{CT}drawingml.diagramData+xml"),
                x(
                    "dgm:dataModel",
                    &format!(
                        r#"<dgm:ptLst><dgm:pt modelId="{{00000000-0000-0000-0000-000000000001}}" type="doc"/></dgm:ptLst><dgm:extLst><a:ext uri="http://schemas.microsoft.com/office/drawing/2008/diagram"><dsp:dataModelExt xmlns:dsp="{DSP}" relId="rId6" minVer="http://schemas.openxmlformats.org/drawingml/2006/diagram"/></a:ext></dgm:extLst>"#
                    ),
                ),
            ),
            (
                "/ppt/diagrams/layout1.xml".into(),
                format!("{CT}drawingml.diagramLayout+xml"),
                x("dgm:layoutDef", "<dgm:layoutNode/>"),
            ),
            (
                "/ppt/diagrams/quickStyle1.xml".into(),
                format!("{CT}drawingml.diagramStyle+xml"),
                x("dgm:styleDef", r#"<dgm:styleLbl name="node0"/>"#),
            ),
            (
                "/ppt/diagrams/colors1.xml".into(),
                format!("{CT}drawingml.diagramColors+xml"),
                x("dgm:colorsDef", ""),
            ),
            (
                "/ppt/diagrams/drawing1.xml".into(),
                "application/vnd.ms-office.drawingml.diagramDrawing+xml".into(),
                format!(
                    r#"{XML_DECL}<dsp:drawing xmlns:dgm="{DGM}" xmlns:dsp="{DSP}" xmlns:a="{A}"><dsp:spTree><dsp:nvGrpSpPr><dsp:cNvPr id="0" name=""/><dsp:cNvGrpSpPr/></dsp:nvGrpSpPr><dsp:grpSpPr/>{shapes}</dsp:spTree></dsp:drawing>"#
                )
                .into_bytes(),
            ),
        ]
    };
    let rels = || {
        vec![
            r("rId2", "diagramData", "../diagrams/data1.xml"),
            r("rId3", "diagramLayout", "../diagrams/layout1.xml"),
            r("rId4", "diagramQuickStyle", "../diagrams/quickStyle1.xml"),
            r("rId5", "diagramColors", "../diagrams/colors1.xml"),
            r(
                "rId6",
                "http://schemas.microsoft.com/office/2007/relationships/diagramDrawing",
                "../diagrams/drawing1.xml",
            ),
        ]
    };
    let (w, h) = (3 * IN, IN + IN / 2);
    let mut process = blank_slide(
        "Quy trình 3 bước (SmartArt)",
        frame(4, IN, 2 * IN, 3 * w + IN, h),
    );
    process.rels = rels();
    let process_shapes = [
        dsp_sp(1, 0, 0, w, h, "homePlate", "2E75B6", "Tiếp nhận"),
        dsp_sp(2, w + IN / 2, 0, w, h, "chevron", "548235", "Thẩm định"),
        dsp_sp(3, 2 * w + IN, 0, w, h, "chevron", "C55A11", "Trả kết quả"),
    ]
    .concat();
    let mut cycle = blank_slide(
        "Chu trình xoay (SmartArt, khung xoay 15°)",
        frame(4, 3 * IN, IN + IN / 2, 6 * IN, 5 * IN)
            .replace("<p:xfrm>", r#"<p:xfrm rot="900000">"#),
    );
    cycle.rels = rels();
    let c = IN + IN / 2;
    let cycle_shapes = [
        dsp_sp(
            1,
            2 * IN + IN / 4,
            0,
            c,
            c,
            "ellipse",
            "7030A0",
            "Lập kế hoạch",
        ),
        dsp_sp(
            2,
            4 * IN + IN / 2,
            2 * IN,
            c,
            c,
            "ellipse",
            "2E75B6",
            "Thực hiện",
        ),
        dsp_sp(
            3,
            2 * IN + IN / 4,
            3 * IN + IN / 2,
            c,
            c,
            "ellipse",
            "548235",
            "Kiểm tra",
        ),
        dsp_sp(4, 0, 2 * IN, c, c, "ellipse", "C55A11", "Cải tiến"),
    ]
    .concat();
    // Hierarchy (organisation chart): boxes and the connector lines between them.
    let mut hierarchy = blank_slide(
        "Sơ đồ tổ chức (SmartArt phân cấp)",
        frame(4, IN, IN + IN / 2, 8 * IN, 4 * IN),
    );
    hierarchy.rels = rels();
    let (bw, bh) = (2 * IN, IN);
    let hierarchy_shapes = [
        dsp_sp(1, 3 * IN, 0, bw, bh, "roundRect", "1F4E79", "Giám đốc"),
        dsp_sp(
            2,
            0,
            2 * IN,
            bw,
            bh,
            "roundRect",
            "2E75B6",
            "Phòng Kỹ thuật",
        ),
        dsp_sp(
            3,
            3 * IN,
            2 * IN,
            bw,
            bh,
            "roundRect",
            "2E75B6",
            "Phòng Kinh doanh",
        ),
        dsp_sp(
            4,
            6 * IN,
            2 * IN,
            bw,
            bh,
            "roundRect",
            "2E75B6",
            "Phòng Hành chính",
        ),
        dsp_sp(5, 4 * IN - IN / 40, bh, IN / 20, IN, "rect", "7F7F7F", ""),
        dsp_sp(
            6,
            IN,
            3 * IN / 2 - IN / 40,
            6 * IN,
            IN / 20,
            "rect",
            "7F7F7F",
            "",
        ),
        dsp_sp(
            7,
            IN - IN / 40,
            3 * IN / 2,
            IN / 20,
            IN / 2,
            "rect",
            "7F7F7F",
            "",
        ),
        dsp_sp(
            8,
            7 * IN - IN / 40,
            3 * IN / 2,
            IN / 20,
            IN / 2,
            "rect",
            "7F7F7F",
            "",
        ),
    ]
    .concat();
    vec![
        (
            "slide-smartart-hierarchy.pptx",
            build(&[hierarchy], &parts(&hierarchy_shapes), THEME_BLUE),
        ),
        (
            "slide-smartart-process.pptx",
            build(&[process], &parts(&process_shapes), THEME_BLUE),
        ),
        (
            "slide-smartart-cycle.pptx",
            build(&[cycle], &parts(&cycle_shapes), THEME_BLUE),
        ),
    ]
}

/// SmartArt saved without a drawing part: only the data model (ECMA-376 Part 1, §21.4.3 `dgm:pt` /
/// `dgm:cxn`) and a layout definition naming its category (§21.4.2.3 `dgm:cat`), so the diagram is laid
/// out by BlueOffice (CORE-SMART-002).
pub(crate) fn smartart_data_model(lo: &str, nodes: &[(&str, Option<usize>)]) -> String {
    use std::fmt::Write as _;
    let mut pts = format!(
        r#"<dgm:pt modelId="{{00000000-0000-0000-0000-0000000000D0}}" type="doc"><dgm:prSet loTypeId="{lo}"/><dgm:spPr/><dgm:t><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr lang="vi-VN"/></a:p></dgm:t></dgm:pt>"#
    );
    let mut cxns = String::new();
    let mut ord = std::collections::HashMap::new();
    for (i, (t, parent)) in nodes.iter().enumerate() {
        let _ = write!(
            pts,
            r#"<dgm:pt modelId="{{00000000-0000-0000-0000-0000000001{i:02}}}"><dgm:prSet phldrT="[Văn bản]"/><dgm:spPr/><dgm:t><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="vi-VN"/><a:t>{t}</a:t></a:r></a:p></dgm:t></dgm:pt>"#
        );
        let src = parent.map_or("00000000-0000-0000-0000-0000000000D0".to_owned(), |p| {
            format!("00000000-0000-0000-0000-0000000001{p:02}")
        });
        let o = ord.entry(src.clone()).or_insert(0u32);
        let _ = write!(
            cxns,
            r#"<dgm:cxn modelId="{{00000000-0000-0000-0000-0000000002{i:02}}}" srcId="{{{src}}}" destId="{{00000000-0000-0000-0000-0000000001{i:02}}}" srcOrd="{o}" destOrd="0"/>"#
        );
        *o += 1;
    }
    format!(r#"<dgm:ptLst>{pts}</dgm:ptLst><dgm:cxnLst>{cxns}</dgm:cxnLst><dgm:bg/><dgm:whole/>"#)
}

/// Decks whose SmartArt has no drawing part (laid out from the data model).
fn smartart_data_decks() -> Vec<(&'static str, Vec<u8>)> {
    const DGM: &str = "http://schemas.openxmlformats.org/drawingml/2006/diagram";
    const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
    let frame = |x: i64, y: i64, cx: i64, cy: i64| {
        format!(
            r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="4" name="Sơ đồ 4"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><p:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></p:xfrm><a:graphic><a:graphicData uri="{DGM}"><dgm:relIds xmlns:dgm="{DGM}" r:dm="rId2" r:lo="rId3" r:qs="rId4" r:cs="rId5"/></a:graphicData></a:graphic></p:graphicFrame>"#
        )
    };
    let parts = |lo: &str, cat: &str, nodes: &[(&str, Option<usize>)]| -> Parts {
        let x = |root: &str, attrs: &str, inner: &str| {
            format!(r#"{XML_DECL}<{root} xmlns:dgm="{DGM}" xmlns:a="{A}"{attrs}>{inner}</{root}>"#)
                .into_bytes()
        };
        vec![
            (
                "/ppt/diagrams/data1.xml".into(),
                format!("{CT}drawingml.diagramData+xml"),
                x("dgm:dataModel", "", &smartart_data_model(lo, nodes)),
            ),
            (
                "/ppt/diagrams/layout1.xml".into(),
                format!("{CT}drawingml.diagramLayout+xml"),
                x(
                    "dgm:layoutDef",
                    &format!(r#" uniqueId="{lo}""#),
                    &format!(
                        r#"<dgm:catLst><dgm:cat type="{cat}" pri="1000"/></dgm:catLst><dgm:layoutNode name="diagram"/>"#
                    ),
                ),
            ),
            (
                "/ppt/diagrams/quickStyle1.xml".into(),
                format!("{CT}drawingml.diagramStyle+xml"),
                x("dgm:styleDef", "", r#"<dgm:styleLbl name="node0"/>"#),
            ),
            (
                "/ppt/diagrams/colors1.xml".into(),
                format!("{CT}drawingml.diagramColors+xml"),
                x("dgm:colorsDef", "", ""),
            ),
        ]
    };
    let rels = || {
        vec![
            r("rId2", "diagramData", "../diagrams/data1.xml"),
            r("rId3", "diagramLayout", "../diagrams/layout1.xml"),
            r("rId4", "diagramQuickStyle", "../diagrams/quickStyle1.xml"),
            r("rId5", "diagramColors", "../diagrams/colors1.xml"),
        ]
    };
    let deck = |title: &str, lo: &str, cat: &str, nodes: &[(&str, Option<usize>)]| {
        let mut slide = blank_slide(title, frame(IN, IN + IN / 2, 8 * IN, 4 * IN));
        slide.rels = rels();
        build(&[slide], &parts(lo, cat, nodes), THEME_BLUE)
    };
    vec![
        (
            "slide-smartart-data-process.pptx",
            deck(
                "Quy trình (SmartArt chỉ có data model)",
                "urn:bluestar.vn/layout/process",
                "process",
                &[
                    ("Tiếp nhận", None),
                    ("Xử lý", None),
                    ("Phê duyệt", None),
                    ("Trả kết quả", None),
                ],
            ),
        ),
        (
            "slide-smartart-data-hierarchy.pptx",
            deck(
                "Sơ đồ tổ chức (SmartArt chỉ có data model)",
                "urn:bluestar.vn/layout/hierarchy",
                "hierarchy",
                &[
                    ("Giám đốc", None),
                    ("Kỹ thuật", Some(0)),
                    ("Kinh doanh", Some(0)),
                    ("Phần mềm", Some(1)),
                    ("Hạ tầng", Some(1)),
                ],
            ),
        ),
        (
            "slide-smartart-data-cycle.pptx",
            deck(
                "Chu trình PDCA (SmartArt chỉ có data model)",
                "urn:bluestar.vn/layout/cycle",
                "cycle",
                &[
                    ("Kế hoạch", None),
                    ("Thực hiện", None),
                    ("Kiểm tra", None),
                    ("Cải tiến", None),
                ],
            ),
        ),
        (
            "slide-smartart-data-pyramid.pptx",
            deck(
                "Kim tự tháp (SmartArt chỉ có data model)",
                "urn:bluestar.vn/layout/pyramid",
                "pyramid",
                &[
                    ("Tầm nhìn", None),
                    ("Chiến lược", None),
                    ("Kế hoạch", None),
                    ("Hành động", None),
                ],
            ),
        ),
    ]
}

/// Decks with chartex charts ([MS-ODRAWXML] 2.24) in `mc:AlternateContent` frames (CORE-CHART-004).
fn chartex_decks() -> Vec<(&'static str, Vec<u8>)> {
    use bo_model::sheet as m;
    let frame = |req: &str, req_ns: &str| {
        format!(
            r#"<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:Choice xmlns:{req}="{req_ns}" Requires="{req}"><p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="4" name="Biểu đồ 4"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><p:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></p:xfrm><a:graphic><a:graphicData uri="{ns}"><cx:chart xmlns:cx="{ns}" r:id="rId2"/></a:graphicData></a:graphic></p:graphicFrame></mc:Choice><mc:Fallback><p:sp><p:nvSpPr><p:cNvPr id="4" name="Biểu đồ 4"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="vi-VN"/><a:t>Biểu đồ mở rộng (chartex)</a:t></a:r></a:p></p:txBody></p:sp></mc:Fallback></mc:AlternateContent>"#,
            x = IN,
            y = IN + IN / 2,
            cx = 8 * IN,
            cy = 4 * IN,
            ns = bo_xlsx::chartex::CX_NS
        )
    };
    let series = |cats: &[&str], vals: &[f64]| m::ChartSeries {
        name: Some("Giá trị".into()),
        categories: cats.iter().map(|c| (*c).to_owned()).collect(),
        values: vals.iter().map(|v| Some(*v)).collect(),
        ..m::ChartSeries::default()
    };
    let deck = |title: &str, kind: m::ChartKind, s: m::ChartSeries, funnel: bool| {
        let chart = m::Chart {
            title: Some(title.to_owned()),
            plots: vec![m::ChartPlot {
                kind,
                series: vec![s],
                ..m::ChartPlot::default()
            }],
            ..m::Chart::default()
        };
        let (req, ns) = if funnel {
            (
                "cx2",
                "http://schemas.microsoft.com/office/drawing/2015/10/21/chartex",
            )
        } else {
            (
                "cx1",
                "http://schemas.microsoft.com/office/drawing/2015/9/8/chartex",
            )
        };
        let mut slide = blank_slide(title, frame(req, ns));
        slide.rels = vec![r(
            "rId2",
            bo_xlsx::chartex::REL_CHARTEX,
            "../charts/chartEx1.xml",
        )];
        let parts: Parts = vec![(
            "/ppt/charts/chartEx1.xml".into(),
            bo_xlsx::chartex::CT_CHARTEX.into(),
            bo_xlsx::chartex::chartex_space_xml(&chart).into_bytes(),
        )];
        build(&[slide], &parts, THEME_BLUE)
    };
    let mut water = series(
        &["Đầu kỳ", "Thu", "Chi", "Cuối kỳ"],
        &[100.0, 45.0, -30.0, 115.0],
    );
    water.subtotals = vec![3];
    let mut tree = series(
        &["Ba Đình", "Cầu Giấy", "Hải Châu", "Quận 1", "Thủ Đức"],
        &[34.0, 28.0, 22.0, 41.0, 37.0],
    );
    tree.levels = vec![
        tree.categories.clone(),
        ["Hà Nội", "Hà Nội", "Đà Nẵng", "TP.HCM", "TP.HCM"]
            .map(String::from)
            .to_vec(),
    ];
    vec![
        (
            "slide-chartex-waterfall.pptx",
            deck(
                "Dòng tiền (waterfall)",
                m::ChartKind::Waterfall,
                water,
                false,
            ),
        ),
        (
            "slide-chartex-treemap.pptx",
            deck(
                "Doanh số theo quận (treemap)",
                m::ChartKind::Treemap,
                tree,
                false,
            ),
        ),
        (
            "slide-chartex-funnel.pptx",
            deck(
                "Phễu bán hàng (funnel)",
                m::ChartKind::Funnel,
                series(&["Truy cập", "Đăng ký", "Mua"], &[5000.0, 1800.0, 240.0]),
                true,
            ),
        ),
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
    // Deck 4: effects BlueSlide does not model (emphasis §19.5.4 animScale, motion
    // path §19.5.6 animMotion) next to a fade, and a dissolve transition (§19.3.1.18).
    let emph = r#"<p:par><p:cTn id="9" presetID="6" presetClass="emph" presetSubtype="0" fill="hold" nodeType="withEffect"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst><p:animScale><p:cBhvr><p:cTn id="10" dur="1000" fill="hold"/><p:tgtEl><p:spTgt spid="5"/></p:tgtEl></p:cBhvr><p:by x="150000" y="150000"/></p:animScale></p:childTnLst></p:cTn></p:par>"#;
    let path = r#"<p:par><p:cTn id="14" presetID="42" presetClass="path" presetSubtype="0" accel="50000" decel="50000" fill="hold" nodeType="clickEffect"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst><p:animMotion origin="layout" path="M 0 0 L 0.25 0 E" pathEditMode="relative" ptsTypes="AA"><p:cBhvr><p:cTn id="15" dur="2000" fill="hold"/><p:tgtEl><p:spTgt spid="6"/></p:tgtEl><p:attrNameLst><p:attrName>ppt_x</p:attrName><p:attrName>ppt_y</p:attrName></p:attrNameLst></p:cBhvr></p:animMotion></p:childTnLst></p:cTn></p:par>"#;
    let g4 = click(
        3,
        &(effect(5, 4, 10, 0, "clickEffect", "0", &fx_fade(7, 4)) + emph),
    ) + &click(12, path);
    let mut u1 = blank_slide("Hiệu ứng nhấn mạnh và đường chuyển động", shapes(3));
    u1.tail = format!(
        r#"<p:transition spd="med"><p:dissolve/></p:transition>{}"#,
        timing(&g4, "")
    );
    vec![
        (
            "slide-anim-unknown.pptx",
            build(&[u1], &Vec::new(), THEME_BLUE),
        ),
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
    // Notes on every slide: several paragraphs, an empty line kept as an empty
    // paragraph, XML-escaped characters (§19.3.1.26 notes, §21.1.2.3.11 t).
    let mut q1 = title_slide("Báo cáo quý", "Ghi chú nhiều đoạn");
    q1.notes = Some(
        "Mở đầu: chào hỏi.\n\nPhần 1: doanh thu &amp; lợi nhuận.\nPhần 2: chi phí &lt; kế hoạch."
            .into(),
    );
    let mut q2 = content_slide(
        "Số liệu",
        &body("", &(para_lvl(0, "Doanh thu") + &para_lvl(1, "Tăng 12%"))),
    );
    q2.notes = Some(
        "Đọc chậm từng con số; nhấn mạnh tháng 9 “kỷ lục”. Nếu bị hỏi về tỷ giá, trả lời theo bảng phụ lục trang 4 và đề nghị gửi file sau buổi họp."
            .into(),
    );
    let mut q3 = content_slide("Kết luận", &body("", &para("Cảm ơn")));
    q3.notes = Some("Hỏi đáp 10 phút.".into());
    // Notes only on some slides (the others have no notes slide part).
    let s1 = title_slide("Chỉ slide 2 và 4 có ghi chú", "Ghi chú rải rác");
    let mut s2 = content_slide("Slide 2", &body("", &para("Có ghi chú")));
    s2.notes = Some("Ghi chú của slide 2.".into());
    let s3 = content_slide("Slide 3", &body("", &para("Không có ghi chú")));
    let mut s4 = blank_slide("Slide 4", String::new());
    s4.notes = Some("Ghi chú của slide 4 (bố cục trống).".into());
    // Theme fonts and the six accents (§20.1.4.1.9 clrScheme, §20.1.4.1.18
    // fontScheme): shapes filled by scheme colour, text in +mj-lt / +mn-lt.
    let accents: String = (1..=6u32)
        .map(|i| {
            let x = IN / 2 + i64::from(i - 1) * (2 * IN);
            sp(
                3 + i,
                &format!("accent{i}"),
                "",
                &(xfrm(x, 2 * IN, IN + IN / 2, IN + IN / 2)
                    + &prst("roundRect")
                    + &scheme(&format!("accent{i}"))),
                Some(&body(r#" anchor="ctr""#, &para(&format!("accent{i}")))),
                STYLE,
            )
        })
        .collect();
    let fonts = sp(
        10,
        "Fonts",
        "",
        &(xfrm(IN / 2, 4 * IN, 12 * IN, IN + IN / 2) + &prst("rect")),
        Some(&body(
            "",
            r#"<a:p><a:r><a:rPr lang="vi-VN" sz="2800" dirty="0"><a:solidFill><a:schemeClr val="tx2"/></a:solidFill><a:latin typeface="+mj-lt"/></a:rPr><a:t>Phông tiêu đề (+mj-lt)</a:t></a:r></a:p><a:p><a:r><a:rPr lang="vi-VN" sz="2000" dirty="0"><a:solidFill><a:schemeClr val="accent1"><a:lumMod val="75000"/></a:schemeClr></a:solidFill><a:latin typeface="+mn-lt"/></a:rPr><a:t>Phông nội dung (+mn-lt), accent1 tối 25%</a:t></a:r></a:p>"#,
        )),
        "",
    );
    let f1 = blank_slide("Sáu màu nhấn và phông theme", accents + &fonts);
    let f2 = content_slide(
        "Bố cục kế thừa phông theme",
        &body(
            "",
            &(para("Tiêu đề: Times New Roman") + &para("Nội dung: Tahoma")),
        ),
    );
    vec![
        (
            "slide-notes-basic.pptx",
            build(&[n1, n2], &Vec::new(), THEME_BLUE),
        ),
        (
            "slide-notes-multi.pptx",
            build(&[q1, q2, q3], &Vec::new(), THEME_BLUE),
        ),
        (
            "slide-notes-sparse.pptx",
            build(&[s1, s2, s3, s4], &Vec::new(), THEME_BLUE),
        ),
        ("slide-theme-colormap.pptx", deck2),
        (
            "slide-theme-fonts.pptx",
            build(
                &[f1, f2],
                &Vec::new(),
                ("Violet", "7030A0", "Times New Roman", "Tahoma"),
            ),
        ),
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

// ---------------------------------------------------------------------------
// 3D models (CORE-3DMODEL-001)
// ---------------------------------------------------------------------------

/// [MS-ODRAWXML] relationship type of `am3d:model3d@r:embed`.
const REL_MODEL3D: &str = "http://schemas.microsoft.com/office/2017/06/relationships/model3d";

/// A self-made model seen with rotation `rot` (degrees) in a frame at most `max` EMU:
/// (GLB, PNG picture of the view, the view, frame size EMU).
fn model3d(
    scene: &bo_model3d::Scene,
    rot: [f64; 3],
    max: (f64, f64),
) -> (Vec<u8>, Vec<u8>, bo_model3d::view::View3d, (i64, i64)) {
    use bo_model3d::view::{View3d, ANGLE};
    let (lo, hi) = scene.bounds().unwrap_or(([-0.5; 3], [0.5; 3]));
    let mut v = View3d::fit(lo, hi);
    v.rot = rot.map(|d| d * ANGLE);
    let (w, h) = v.frame_for(scene, max.0, max.1);
    let (pw, ph) = bo_model3d::raster::picture_size(w, h, 96.0, 512);
    let png = bo_model3d::raster::render_png(scene, &v, w, h, pw, ph);
    (bo_model3d::glb::write(scene), png, v, (w as i64, h as i64))
}

fn model3d_decks() -> Vec<(&'static str, Vec<u8>)> {
    use bo_model3d::sample;
    let glb_part = |n: usize, data: Vec<u8>| {
        (
            format!("/ppt/media/model3d{n}.glb"),
            "model/gltf-binary".to_owned(),
            data,
        )
    };
    let png_part =
        |n: usize, data: Vec<u8>| (format!("/ppt/media/image{n}.png"), String::new(), data);
    let rels = |m: &str, mi: usize, i: &str, ii: usize| {
        vec![
            r(m, REL_MODEL3D, &format!("../media/model3d{mi}.glb")),
            r(i, "image", &format!("../media/image{ii}.png")),
        ]
    };
    // Deck 1: one cube seen from above front left.
    let (glb1, png1, v1, (w1, h1)) = model3d(
        &sample::cube(),
        [25.0, 35.0, 0.0],
        (4.0 * IN as f64, 4.0 * IN as f64),
    );
    let mut s1 = blank_slide(
        "Mô hình 3D: khối lập phương",
        bo_pptx::model3d::node_xml(
            4,
            "3D Model 3",
            "Khối lập phương sáu màu",
            ((13 * IN - w1) / 2, IN + (5 * IN - h1) / 2),
            (w1, h1),
            "rId2",
            "rId3",
            &v1,
        ),
    );
    s1.rels = rels("rId2", 1, "rId3", 1);
    // Deck 2: a textured cube and a ring (top view), then a pyramid on a second slide.
    let (glb2, png2, v2, (w2, h2)) = model3d(
        &sample::textured_cube(),
        [20.0, 330.0, 0.0],
        (3.5 * IN as f64, 3.5 * IN as f64),
    );
    let (glb3, png3, v3, (w3, h3)) = model3d(
        &sample::ring(),
        [60.0, 0.0, 0.0],
        (4.0 * IN as f64, 3.0 * IN as f64),
    );
    let (glb4, png4, v4, (w4, h4)) = model3d(
        &sample::pyramid(),
        [15.0, 45.0, 0.0],
        (4.0 * IN as f64, 4.0 * IN as f64),
    );
    let mut s2 = blank_slide(
        "Hai mô hình: khối có vân và vòng xuyến",
        bo_pptx::model3d::node_xml(
            4,
            "3D Model 3",
            "Khối vân ca-rô",
            (IN, 2 * IN),
            (w2, h2),
            "rId2",
            "rId3",
            &v2,
        ) + &bo_pptx::model3d::node_xml(
            5,
            "3D Model 4",
            "Vòng xuyến",
            (7 * IN, 2 * IN),
            (w3, h3),
            "rId4",
            "rId5",
            &v3,
        ),
    );
    s2.rels = rels("rId2", 1, "rId3", 1);
    s2.rels.extend(rels("rId4", 2, "rId5", 2));
    let mut s3 = blank_slide(
        "Kim tự tháp",
        bo_pptx::model3d::node_xml(
            4,
            "3D Model 3",
            "",
            (4 * IN, 2 * IN),
            (w4, h4),
            "rId2",
            "rId3",
            &v4,
        ),
    );
    s3.rels = rels("rId2", 3, "rId3", 3);
    // Deck 3: written by hand from the schema — orthographic camera, window viewport,
    // point / spot / directional lights, extension lists, a model inside a group.
    let (glb5, png5, _, _) = model3d(
        &sample::pyramid(),
        [0.0, 0.0, 0.0],
        (3.0 * IN as f64, 3.0 * IN as f64),
    );
    let am3d = "http://schemas.microsoft.com/office/drawing/2017/model3d";
    let hand = |id: u32, off: (i64, i64)| {
        format!(
            concat!(
                r#"<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:Choice xmlns:am3d="{am3d}" Requires="am3d">"#,
                r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="{id}" name="Mô hình {id}" descr="Kim tự tháp, camera trực giao"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>"#,
                r#"<p:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{s}" cy="{s}"/></p:xfrm><a:graphic><a:graphicData uri="{am3d}"><am3d:model3d r:embed="rId2">"#,
                r#"<am3d:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{s}" cy="{s}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></am3d:spPr>"#,
                r#"<am3d:camera><am3d:pos x="0" y="0" z="90000000"/><am3d:up dx="0" dy="36000000" dz="0"/><am3d:lookAt x="0" y="0" z="0"/>"#,
                r#"<am3d:orthographic><am3d:sz n="40000000" d="1"/></am3d:orthographic></am3d:camera>"#,
                r#"<am3d:trans><am3d:meterPerModelUnit n="1" d="1"/><am3d:preTrans dx="0" dy="0" dz="0"/><am3d:scale><am3d:sx n="1" d="1"/><am3d:sy n="1" d="1"/><am3d:sz n="1" d="1"/></am3d:scale>"#,
                r#"<am3d:rot ax="1200000" ay="2700000" az="0"/><am3d:postTrans dx="0" dy="0" dz="0"/></am3d:trans>"#,
                r#"<am3d:raster rName="BlueOffice" rVer="1"><am3d:blip r:embed="rId3"/></am3d:raster>"#,
                r#"<am3d:extLst><a:ext uri="{{9A65E2B6-3D5C-4F5E-8A1B-000000000001}}"/></am3d:extLst>"#,
                r#"<am3d:winViewport/>"#,
                r#"<am3d:ambientLight enabled="1"><am3d:clr><a:srgbClr val="404040"/></am3d:clr><am3d:illuminance n="1" d="2"/></am3d:ambientLight>"#,
                r#"<am3d:ptLight rad="0"><am3d:clr><a:srgbClr val="FFF2CC"/></am3d:clr><am3d:intensity n="3" d="1"/><am3d:pos x="10000000" y="30000000" z="50000000"/></am3d:ptLight>"#,
                r#"<am3d:spotLight rad="0" spotAng="1800000"><am3d:clr><a:srgbClr val="DDEBF7"/></am3d:clr><am3d:intensity n="2" d="1"/><am3d:pos x="-30000000" y="10000000" z="40000000"/><am3d:lookAt x="0" y="0" z="0"/></am3d:spotLight>"#,
                r#"<am3d:dirLight angularRad="0" enabled="0"><am3d:clr><a:schemeClr val="accent1"/></am3d:clr><am3d:illuminance n="1" d="1"/><am3d:pos x="0" y="36000000" z="0"/><am3d:lookAt x="0" y="0" z="0"/></am3d:dirLight>"#,
                r#"</am3d:model3d></a:graphicData></a:graphic></p:graphicFrame></mc:Choice>"#,
                r#"<mc:Fallback><p:pic><p:nvPicPr><p:cNvPr id="{id}" name="Mô hình {id}" descr="Kim tự tháp, camera trực giao"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr>"#,
                r#"<p:blipFill><a:blip r:embed="rId3"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>"#,
                r#"<p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{s}" cy="{s}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic></mc:Fallback></mc:AlternateContent>"#
            ),
            am3d = am3d,
            id = id,
            x = off.0,
            y = off.1,
            s = 3 * IN,
        )
    };
    let group = format!(
        concat!(
            r#"<p:grpSp><p:nvGrpSpPr><p:cNvPr id="6" name="Nhóm 5"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>"#,
            r#"<p:grpSpPr><a:xfrm><a:off x="{gx}" y="{gy}"/><a:ext cx="{s}" cy="{s}"/><a:chOff x="{gx}" y="{gy}"/><a:chExt cx="{s}" cy="{s}"/></a:xfrm></p:grpSpPr>{m}</p:grpSp>"#
        ),
        gx = 7 * IN,
        gy = 2 * IN,
        s = 3 * IN,
        m = hand(7, (7 * IN, 2 * IN)),
    );
    let mut s4 = blank_slide(
        "Mô hình viết tay theo lược đồ",
        hand(4, (2 * IN, 2 * IN)) + &group,
    );
    s4.rels = rels("rId2", 1, "rId3", 1);
    vec![
        (
            "slide-model3d-cube.pptx",
            build(
                &[s1],
                &vec![glb_part(1, glb1), png_part(1, png1)],
                THEME_BLUE,
            ),
        ),
        (
            "slide-model3d-scenes.pptx",
            build(
                &[s2, s3],
                &vec![
                    glb_part(1, glb2),
                    png_part(1, png2),
                    glb_part(2, glb3),
                    png_part(2, png3),
                    glb_part(3, glb4),
                    png_part(3, png4),
                ],
                THEME_BLUE,
            ),
        ),
        (
            "slide-model3d-handwritten.pptx",
            build(
                &[s4],
                &vec![glb_part(1, glb5), png_part(1, png5)],
                THEME_BLUE,
            ),
        ),
    ]
}

/// Benchmark deck of CORE-PERF-001 ("`.pptx` 300 slide có video mở < 5 s"): `slides`
/// title-and-content slides with three bullets and a picture (cycling through
/// `distinct` photo-like PNG parts), speaker notes on every tenth slide, and
/// `videos` slides showing an embedded video (§19.3.1.37 pic with `a:videoFile`,
/// §20.1.3.6, and the `p14:media` extension) of `video_kb` KiB each. The video parts
/// are an `ftyp` box followed by incompressible pseudo-random bytes: open time is
/// measured, not decoding. Generated for benchmarks, never committed.
pub fn big_deck(slides: usize, videos: usize, video_kb: usize, distinct: usize) -> Vec<u8> {
    let distinct = distinct.max(1);
    let every = slides.checked_div(videos).map_or(usize::MAX, |n| n.max(1));
    let mut parts: Parts = (0..distinct)
        .map(|k| {
            let seed = u32::try_from(k).unwrap_or(0) + 101;
            (
                format!("/ppt/media/image{}.png", k + 1),
                String::new(),
                crate::corpus::photo_png(800, 600, seed),
            )
        })
        .collect();
    let mut video_n = 0;
    let mut specs = Vec::with_capacity(slides);
    for i in 0..slides {
        let bullets = body(
            "",
            &(para(&format!("Nội dung {} – mục tiêu và kết quả", i + 1))
                + &para_lvl(1, "Số liệu giả định của quý")
                + &para("Kế hoạch tiếp theo")),
        );
        let mut s = content_slide(&format!("Trang trình chiếu {}", i + 1), &bullets);
        let img = i % distinct + 1;
        if i % every == every - 1 && video_n < videos {
            video_n += 1;
            let target = format!("../media/media{video_n}.mp4");
            s.shapes.push_str(&format!(
                r#"<p:pic><p:nvPicPr><p:cNvPr id="4" name="clip{video_n}.mp4"><a:hlinkClick r:id="" action="ppaction://media"/></p:cNvPr><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr><a:videoFile r:link="rId2"/><p:extLst><p:ext uri="{{DAA4B4D4-6D71-4841-9C94-3DE7FCFB9230}}"><p14:media xmlns:p14="{P14}" r:embed="rId3"/></p:ext></p:extLst></p:nvPr></p:nvPicPr><p:blipFill><a:blip r:embed="rId4"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{}{}</p:spPr></p:pic>"#,
                xfrm(7 * IN, 2 * IN, 5 * IN, 3 * IN),
                prst("rect")
            ));
            s.rels = vec![
                r("rId2", "video", &target),
                r(
                    "rId3",
                    "http://schemas.microsoft.com/office/2007/relationships/media",
                    &target,
                ),
                r("rId4", "image", &format!("../media/image{img}.png")),
            ];
            let mut bytes = b"\x00\x00\x00\x18ftypmp42\x00\x00\x00\x00mp42isom".to_vec();
            let mut state = u32::try_from(video_n)
                .unwrap_or(1)
                .wrapping_mul(2_654_435_761)
                | 1;
            bytes.extend((0..video_kb * 1024).map(|_| {
                // xorshift32: incompressible like real video
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state.to_le_bytes()[0]
            }));
            parts.push((
                format!("/ppt/media/media{video_n}.mp4"),
                String::new(),
                bytes,
            ));
        } else {
            s.shapes.push_str(&pic(
                4,
                &format!("Picture {}", i + 1),
                "rId2",
                (7 * IN, 2 * IN, 4 * IN, 3 * IN),
                "",
                &prst("rect"),
            ));
            s.rels = vec![r("rId2", "image", &format!("../media/image{img}.png"))];
        }
        if i % 10 == 0 {
            s.notes = Some(format!("Ghi chú người trình bày cho trang {}", i + 1));
        }
        specs.push(s);
    }
    build(&specs, &parts, THEME_BLUE)
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
    v.extend(smartart_decks());
    v.extend(smartart_data_decks());
    v.extend(chartex_decks());
    v.extend(model3d_decks());
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
