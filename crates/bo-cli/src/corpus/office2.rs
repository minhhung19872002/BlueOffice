//! More Vietnamese office documents for the layout-fidelity corpus (BlueDoc layout round 4):
//! contracts with automatically numbered clauses and signature tables, reports with charts
//! and captions, invitations, meeting minutes, a CV, multi-level outline numbering, tables of
//! contents with PAGEREF page numbers, cross-references, equations, SmartArt-style drawings
//! with fallback pictures, text boxes anchored to paragraphs, long tables with repeated
//! header rows and merged cells, landscape pages mid-document, different first page and
//! odd / even headers, "Trang X/Y" with SECTIONPAGES. Every name, number and organisation in
//! them is invented.

use super::office::{
    admin_header, body, center, font, heading, note, note_ref, page_break, para, r, sect_a4,
    sect_a4_landscape, sect_para, sents, signature, spacing, styles, sz, tab, tbl, tc, thousands,
    tr, SENT, TEXT_W, TIGHT,
};
use super::{field_complex, inline_pic, png, Doc};

const REL_CHART: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart";
const CT_CHART: &str = "application/vnd.openxmlformats-officedocument.drawingml.chart+xml";
const NS_C: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const NS_WPS: &str = "http://schemas.microsoft.com/office/word/2010/wordprocessingShape";
const NS_WPG: &str = "http://schemas.microsoft.com/office/word/2010/wordprocessingGroup";

/// Settings with §17.10.1 evenAndOddHeaders (after defaultTabStop in CT_Settings order).
fn settings_even_odd() -> String {
    r#"<w:zoom w:percent="100"/><w:defaultTabStop w:val="720"/><w:evenAndOddHeaders/><w:characterSpacingControl w:val="doNotCompress"/><w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat>"#.to_owned()
}

/// A complex field whose instruction and result are given, the result with run properties.
fn field_rpr(rpr: &str, instr: &str, cached: &str) -> String {
    let props = if rpr.is_empty() {
        String::new()
    } else {
        format!("<w:rPr>{rpr}</w:rPr>")
    };
    format!(
        r#"<w:r>{props}<w:fldChar w:fldCharType="begin"/></w:r><w:r>{props}<w:instrText xml:space="preserve"> {instr} </w:instrText></w:r><w:r>{props}<w:fldChar w:fldCharType="separate"/></w:r><w:r>{props}<w:t>{cached}</w:t></w:r><w:r>{props}<w:fldChar w:fldCharType="end"/></w:r>"#
    )
}

/// Bookmark around `inner` (§17.13.6.2 bookmarkStart / §17.13.6.1 bookmarkEnd).
fn bookmark(id: u32, name: &str, inner: &str) -> String {
    format!(r#"<w:bookmarkStart w:id="{id}" w:name="{name}"/>{inner}<w:bookmarkEnd w:id="{id}"/>"#)
}

/// A caption paragraph "Label N. text" with the number as a SEQ field (§17.16.5.56),
/// "Label N" bookmarked as `bm` for REF cross-references. `label` is the SEQ identifier
/// (no spaces: `_` stands for one in the shown text).
fn caption(id: u32, bm: &str, label: &str, n: u32, text: &str) -> String {
    para(
        r#"<w:pStyle w:val="Caption"/>"#,
        &[
            bookmark(
                id,
                bm,
                &[
                    r("", &format!("{} ", label.replace('_', " "))),
                    field_complex(&format!("SEQ {label} \\* ARABIC"), &n.to_string()),
                ]
                .concat(),
            ),
            r("", &format!(". {text}")),
        ]
        .concat(),
    )
}

/// A chart part (ECMA-376 Part 1, §21.2.2.29 chartSpace) with the cached data of a
/// clustered column (§21.2.2.16 barChart), line (§21.2.2.97 lineChart) or pie chart
/// (§21.2.2.141 pieChart). `series` = (name, values).
fn chart_xml(kind: &str, title: &str, cats: &[&str], series: &[(&str, &[f64])]) -> String {
    let colors = ["4472C4", "ED7D31", "A5A5A5", "FFC000"];
    let str_cache = |items: &[&str]| {
        let pts: String = items
            .iter()
            .enumerate()
            .map(|(i, t)| format!(r#"<c:pt idx="{i}"><c:v>{t}</c:v></c:pt>"#))
            .collect();
        format!(
            r#"<c:strCache><c:ptCount val="{}"/>{pts}</c:strCache>"#,
            items.len()
        )
    };
    let mut ser = String::new();
    for (k, (name, vals)) in series.iter().enumerate() {
        let pts: String = vals
            .iter()
            .enumerate()
            .map(|(i, v)| format!(r#"<c:pt idx="{i}"><c:v>{v}</c:v></c:pt>"#))
            .collect();
        let sp = match kind {
            "line" => format!(
                r#"<c:spPr><a:ln w="28575" cap="rnd"><a:solidFill><a:srgbClr val="{}"/></a:solidFill><a:round/></a:ln></c:spPr><c:marker><c:symbol val="none"/></c:marker>"#,
                colors[k % 4]
            ),
            "pie" => {
                let dpts: String = (0..vals.len())
                    .map(|i| format!(r#"<c:dPt><c:idx val="{i}"/><c:bubble3D val="0"/><c:spPr><a:solidFill><a:srgbClr val="{}"/></a:solidFill></c:spPr></c:dPt>"#, colors[i % 4]))
                    .collect();
                dpts
            }
            _ => format!(
                r#"<c:spPr><a:solidFill><a:srgbClr val="{}"/></a:solidFill></c:spPr><c:invertIfNegative val="0"/>"#,
                colors[k % 4]
            ),
        };
        ser.push_str(&format!(
            r#"<c:ser><c:idx val="{k}"/><c:order val="{k}"/><c:tx><c:strRef><c:f>Sheet1!${col}$1</c:f>{name_cache}</c:strRef></c:tx>{sp}<c:cat><c:strRef><c:f>Sheet1!$A$2:$A${last}</c:f>{cat_cache}</c:strRef></c:cat><c:val><c:numRef><c:f>Sheet1!${col}$2:${col}${last}</c:f><c:numCache><c:formatCode>General</c:formatCode><c:ptCount val="{n}"/>{pts}</c:numCache></c:numRef></c:val>{smooth}</c:ser>"#,
            col = char::from(b'B' + k as u8),
            name_cache = str_cache(&[name]),
            last = cats.len() + 1,
            cat_cache = str_cache(cats),
            n = vals.len(),
            smooth = if kind == "line" { r#"<c:smooth val="0"/>"# } else { "" },
        ));
    }
    let axes = r#"<c:catAx><c:axId val="111"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="b"/><c:numFmt formatCode="General" sourceLinked="1"/><c:majorTickMark val="none"/><c:minorTickMark val="none"/><c:tickLblPos val="nextTo"/><c:crossAx val="222"/><c:crosses val="autoZero"/><c:auto val="1"/><c:lblAlgn val="ctr"/><c:lblOffset val="100"/><c:noMultiLvlLbl val="0"/></c:catAx><c:valAx><c:axId val="222"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="l"/><c:majorGridlines/><c:numFmt formatCode="General" sourceLinked="1"/><c:majorTickMark val="none"/><c:minorTickMark val="none"/><c:tickLblPos val="nextTo"/><c:crossAx val="111"/><c:crosses val="autoZero"/><c:crossBetween val="between"/></c:valAx>"#;
    let plot = match kind {
        "line" => format!(
            r#"<c:lineChart><c:grouping val="standard"/><c:varyColors val="0"/>{ser}<c:marker val="1"/><c:axId val="111"/><c:axId val="222"/></c:lineChart>{axes}"#
        ),
        "pie" => format!(
            r#"<c:pieChart><c:varyColors val="1"/>{ser}<c:firstSliceAng val="0"/></c:pieChart>"#
        ),
        _ => format!(
            r#"<c:barChart><c:barDir val="col"/><c:grouping val="clustered"/><c:varyColors val="0"/>{ser}<c:gapWidth val="219"/><c:overlap val="-27"/><c:axId val="111"/><c:axId val="222"/></c:barChart>{axes}"#
        ),
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<c:chartSpace xmlns:c="{NS_C}" xmlns:a="{NS_A}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><c:roundedCorners val="0"/><c:chart><c:title><c:tx><c:rich><a:bodyPr/><a:lstStyle/><a:p><a:pPr><a:defRPr sz="1200" b="1"/></a:pPr><a:r><a:rPr lang="vi-VN" sz="1200" b="1"/><a:t>{title}</a:t></a:r></a:p></c:rich></c:tx><c:overlay val="0"/></c:title><c:autoTitleDeleted val="0"/><c:plotArea><c:layout/>{plot}</c:plotArea><c:legend><c:legendPos val="b"/><c:overlay val="0"/></c:legend><c:plotVisOnly val="1"/><c:dispBlanksAs val="gap"/></c:chart><c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr><a:defRPr sz="1000"><a:latin typeface="Times New Roman"/></a:defRPr></a:pPr><a:endParaRPr lang="vi-VN"/></a:p></c:txPr></c:chartSpace>"#
    )
}

/// An inline chart (§21.2.2.27 chart reference in a §20.4.2.8 inline drawing).
fn inline_chart(id: u32, rid: &str, cx: i64, cy: i64) -> String {
    format!(
        r#"<w:r><w:rPr><w:noProof/></w:rPr><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{cx}" cy="{cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="{id}" name="Biểu đồ {id}"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="{NS_C}"><c:chart xmlns:c="{NS_C}" r:id="{rid}"/></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#
    )
}

/// Numbering part from raw abstractNum bodies (`w:lvl` elements); num i+1 → abstract i.
fn numbering_raw(abstracts: &[String]) -> String {
    let mut s = String::new();
    for (id, lvls) in abstracts.iter().enumerate() {
        s.push_str(&format!(
            r#"<w:abstractNum w:abstractNumId="{id}"><w:nsid w:val="6C1D{id:04X}"/><w:multiLevelType w:val="multilevel"/>{lvls}</w:abstractNum>"#
        ));
    }
    for id in 0..abstracts.len() {
        s.push_str(&format!(
            r#"<w:num w:numId="{}"><w:abstractNumId w:val="{id}"/></w:num>"#,
            id + 1
        ));
    }
    s
}

/// One `w:lvl` (§17.9.6): format, text, optional pStyle link, suffix (tab / space),
/// left and hanging indent (twips), label run properties.
#[allow(clippy::too_many_arguments)]
fn lvl(
    ilvl: u8,
    fmt: &str,
    text: &str,
    pstyle: &str,
    suff: &str,
    left: i32,
    hanging: i32,
    rpr: &str,
) -> String {
    let ps = if pstyle.is_empty() {
        String::new()
    } else {
        format!(r#"<w:pStyle w:val="{pstyle}"/>"#)
    };
    let sf = if suff.is_empty() {
        String::new()
    } else {
        format!(r#"<w:suff w:val="{suff}"/>"#)
    };
    format!(
        r#"<w:lvl w:ilvl="{ilvl}"><w:start w:val="1"/><w:numFmt w:val="{fmt}"/>{ps}{sf}<w:lvlText w:val="{text}"/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="{left}" w:hanging="{hanging}"/></w:pPr><w:rPr>{rpr}</w:rPr></w:lvl>"#
    )
}

/// Paragraph numbered with `num` at level `ilvl`; `before` = pPr children before numPr
/// (pStyle, keepNext), `after` = the ones after it (spacing, ind, jc).
fn numbered(num: i32, ilvl: u8, before: &str, after: &str, runs: &str) -> String {
    para(
        &format!(
            r#"{before}<w:numPr><w:ilvl w:val="{ilvl}"/><w:numId w:val="{num}"/></w:numPr>{after}"#
        ),
        runs,
    )
}

/// Paragraph with a bottom border (§17.3.1.24 pBdr) — CV section titles.
fn ruled(rpr: &str, t: &str) -> String {
    para(
        r#"<w:keepNext/><w:pBdr><w:bottom w:val="single" w:sz="8" w:space="1" w:color="1F4E79"/></w:pBdr><w:spacing w:before="200" w:after="80"/>"#,
        &r(rpr, t),
    )
}

/// Text box anchored to its paragraph (§20.4.2.3 anchor, positionV relativeFrom paragraph)
/// with horizontal position relative to `rel_h`.
fn para_box(
    id: u32,
    rel_h: &str,
    at: (i64, i64),
    size: (i64, i64),
    wrap: &str,
    content: &str,
) -> String {
    let ((x, y), (cx, cy)) = (at, size);
    format!(
        r#"<w:r><w:rPr><w:noProof/></w:rPr><w:drawing><wp:anchor distT="0" distB="0" distL="114300" distR="114300" simplePos="0" relativeHeight="{id}" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="{rel_h}"><wp:posOffset>{x}</wp:posOffset></wp:positionH><wp:positionV relativeFrom="paragraph"><wp:posOffset>{y}</wp:posOffset></wp:positionV><wp:extent cx="{cx}" cy="{cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/>{wrap}<wp:docPr id="{id}" name="Hộp {id}"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="{NS_WPS}"><wps:wsp xmlns:wps="{NS_WPS}"><wps:cNvSpPr txBox="1"/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="F2F2F2"/></a:solidFill><a:ln w="12700"><a:solidFill><a:srgbClr val="2F5496"/></a:solidFill></a:ln></wps:spPr><wps:txbx><w:txbxContent>{content}</w:txbxContent></wps:txbx><wps:bodyPr rot="0" vert="horz" wrap="square" lIns="91440" tIns="45720" rIns="91440" bIns="45720" anchor="t" anchorCtr="0"><a:noAutofit/></wps:bodyPr></wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r>"#
    )
}

/// A process diagram drawn as a group of shapes (`wpg:wgp`, [MS-DOCX] 2.2.2) in
/// `mc:AlternateContent` (ECMA-376 Part 3, §10.2) with an inline picture as the fallback —
/// how a word processor saves a SmartArt it converted to shapes for older readers.
fn diagram_with_fallback(id: u32, rid: &str, steps: &[&str]) -> String {
    let n = steps.len() as i64;
    let (cx, cy) = (5_400_000_i64, 900_000_i64);
    let w = (cx - (n - 1) * 180_000) / n;
    let mut shapes = String::new();
    for (k, t) in steps.iter().enumerate() {
        let x = k as i64 * (w + 180_000);
        shapes.push_str(&format!(
            r#"<wps:wsp><wps:cNvPr id="{}" name="Bước {}"/><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="{x}" y="0"/><a:ext cx="{w}" cy="{cy}"/></a:xfrm><a:prstGeom prst="roundRect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="4472C4"/></a:solidFill><a:ln w="12700"><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill></a:ln></wps:spPr><wps:txbx><w:txbxContent><w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="240" w:lineRule="auto"/><w:jc w:val="center"/></w:pPr><w:r><w:rPr><w:color w:val="FFFFFF"/><w:sz w:val="24"/></w:rPr><w:t>{t}</w:t></w:r></w:p></w:txbxContent></wps:txbx><wps:bodyPr rot="0" vert="horz" wrap="square" lIns="45720" tIns="45720" rIns="45720" bIns="45720" anchor="ctr" anchorCtr="0"><a:noAutofit/></wps:bodyPr></wps:wsp>"#,
            id * 10 + k as u32 + 1,
            k + 1
        ));
    }
    let fallback = inline_pic(id + 500, rid, cx, cy, "", "");
    let fallback = fallback
        .trim_start_matches("<w:r><w:rPr><w:noProof/></w:rPr>")
        .trim_end_matches("</w:r>");
    format!(
        r#"<w:r><w:rPr><w:noProof/></w:rPr><mc:AlternateContent xmlns:wps="{NS_WPS}" xmlns:wpg="{NS_WPG}"><mc:Choice Requires="wpg"><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{cx}" cy="{cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:docPr id="{id}" name="Sơ đồ quy trình {id}"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="{NS_WPG}"><wpg:wgp><wpg:cNvGrpSpPr/><wpg:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/><a:chOff x="0" y="0"/><a:chExt cx="{cx}" cy="{cy}"/></a:xfrm></wpg:grpSpPr>{shapes}</wpg:wgp></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Choice><mc:Fallback>{fallback}</mc:Fallback></mc:AlternateContent></w:r>"#
    )
}

/// Office Math run (§22.1.2.87 r) with plain text.
fn mr(t: &str) -> String {
    format!("<m:r><m:t>{t}</m:t></m:r>")
}

/// Fraction (§22.1.2.36 f).
fn mfrac(num: &str, den: &str) -> String {
    format!("<m:f><m:num>{num}</m:num><m:den>{den}</m:den></m:f>")
}

/// Superscript (§22.1.2.105 sSup).
fn msup(base: &str, sup: &str) -> String {
    format!("<m:sSup><m:e>{base}</m:e><m:sup>{sup}</m:sup></m:sSup>")
}

/// Subscript (§22.1.2.101 sSub).
fn msub(base: &str, sub: &str) -> String {
    format!("<m:sSub><m:e>{base}</m:e><m:sub>{sub}</m:sub></m:sSub>")
}

/// Radical without degree (§22.1.2.88 rad, degHide).
fn msqrt(e: &str) -> String {
    format!(r#"<m:rad><m:radPr><m:degHide m:val="1"/></m:radPr><m:deg/><m:e>{e}</m:e></m:rad>"#)
}

/// n-ary sum with limits (§22.1.2.70 nary).
fn msum(lo: &str, hi: &str, e: &str) -> String {
    format!(
        r#"<m:nary><m:naryPr><m:chr m:val="∑"/><m:limLoc m:val="undOvr"/></m:naryPr><m:sub>{lo}</m:sub><m:sup>{hi}</m:sup><m:e>{e}</m:e></m:nary>"#
    )
}

/// An equation on its own line: centre tab, the equation, right tab, "(n)".
fn numbered_equation(n: u32, math: &str) -> String {
    para(
        &format!(
            r#"<w:tabs><w:tab w:val="center" w:pos="{}"/><w:tab w:val="right" w:pos="{TEXT_W}"/></w:tabs><w:spacing w:before="120" w:after="120"/>"#,
            TEXT_W / 2
        ),
        &[
            tab(),
            format!("<m:oMath>{math}</m:oMath>"),
            tab(),
            r("", &format!("({n})")),
        ]
        .concat(),
    )
}

/// Run with right-aligned number text in a tight paragraph.
fn right(rpr: &str, t: &str) -> String {
    para(&format!(r#"{TIGHT}<w:jc w:val="right"/>"#), &r(rpr, t))
}

/// Left-aligned tight paragraph.
fn left(rpr: &str, t: &str) -> String {
    para(TIGHT, &r(rpr, t))
}

/// Number with `,` decimals and `.` thousands (Vietnamese), two decimals.
fn money2(v: u64) -> String {
    format!("{},{:02}", thousands(v / 100), v % 100)
}

#[allow(clippy::too_many_lines)]
pub(super) fn documents() -> Vec<(&'static str, &'static str, &'static str, Doc)> {
    let mut v = Vec::new();

    // ---- 21 hợp đồng: automatic "Điều N." numbering (suffix space), clauses N.M.,
    // points a), bordered two-column signature table.
    let clause_lvls = [
        lvl(
            0,
            "decimal",
            "Điều %1.",
            "",
            "space",
            0,
            0,
            "<w:b/><w:bCs/>",
        ),
        lvl(1, "decimal", "%1.%2.", "", "tab", 567, 567, ""),
        lvl(2, "lowerLetter", "%3)", "", "tab", 851, 284, ""),
    ]
    .concat();
    let mut b21 = [
        center(&format!("<w:b/>{}", sz(13.0)), "CỘNG HÒA XÃ HỘI CHỦ NGHĨA VIỆT NAM"),
        center(&format!("<w:b/>{}", sz(13.0)), "Độc lập - Tự do - Hạnh phúc"),
        center("", "———————"),
        para(r#"<w:spacing w:before="240" w:after="0"/><w:jc w:val="center"/>"#, &r(&format!("<w:b/>{}", sz(15.0)), "HỢP ĐỒNG CUNG CẤP DỊCH VỤ PHẦN MỀM")),
        center("<w:i/>", "Số: 27/2026/HĐDV-AS"),
        para(r#"<w:ind w:firstLine="567"/><w:jc w:val="both"/>"#, &r("<w:i/>", "Căn cứ Bộ luật Dân sự năm 2015 và nhu cầu, khả năng của hai bên, hôm nay, tại Hà Nội, chúng tôi gồm:")),
        para(r#"<w:spacing w:before="120"/>"#, &r("<w:b/>", "BÊN A (Bên sử dụng dịch vụ): CÔNG TY TNHH GIẢ ĐỊNH MIỀN BẮC")),
        para(r#"<w:tabs><w:tab w:val="left" w:pos="2268"/></w:tabs>"#, &[r("", "Đại diện:"), tab(), r("", "Ông Nguyễn Văn Bình – Giám đốc")].concat()),
        para(r#"<w:spacing w:before="120"/>"#, &r("<w:b/>", "BÊN B (Bên cung cấp dịch vụ): CÔNG TY CỔ PHẦN ÁNH SAO")),
        para(r#"<w:tabs><w:tab w:val="left" w:pos="2268"/></w:tabs>"#, &[r("", "Đại diện:"), tab(), r("", "Bà Lê Thu Hà – Tổng giám đốc")].concat()),
        para(r#"<w:ind w:firstLine="567"/><w:jc w:val="both"/>"#, &r("", "Hai bên thống nhất ký kết hợp đồng với các điều khoản sau:")),
    ]
    .concat();
    let titles = [
        "Đối tượng của hợp đồng",
        "Giá trị hợp đồng và phương thức thanh toán",
        "Quyền và nghĩa vụ của Bên A",
        "Quyền và nghĩa vụ của Bên B",
        "Bảo mật thông tin",
        "Điều khoản chung",
    ];
    for (k, t) in titles.iter().enumerate() {
        b21.push_str(&numbered(
            1,
            0,
            "<w:keepNext/>",
            r#"<w:spacing w:before="160"/><w:ind w:left="0" w:firstLine="567"/>"#,
            &r("<w:b/>", t),
        ));
        for m in 0..(2 + k % 2) {
            b21.push_str(&numbered(
                1,
                1,
                "",
                r#"<w:jc w:val="both"/>"#,
                &r("", &sents(k + m, 1 + m % 2)),
            ));
            if m == 1 {
                for q in 0..3 {
                    b21.push_str(&numbered(
                        1,
                        2,
                        "",
                        r#"<w:jc w:val="both"/>"#,
                        &r("", SENT[(k + q + 2) % SENT.len()]),
                    ));
                }
            }
        }
    }
    let sig_cell = |t: &str, name: &str| {
        [
            center("<w:b/>", t),
            center("<w:i/>", "(Ký, ghi rõ họ tên, đóng dấu)"),
            center("", ""),
            center("", ""),
            center("", ""),
            center("<w:b/>", name),
        ]
        .concat()
    };
    b21.push_str(&para(r#"<w:spacing w:before="240"/>"#, ""));
    b21.push_str(&tbl(
        &[4677, 4678],
        true,
        r#"<w:tblLayout w:type="fixed"/>"#,
        &[tr(
            "<w:cantSplit/>",
            &[
                tc(4677, "", &sig_cell("ĐẠI DIỆN BÊN A", "Nguyễn Văn Bình")),
                tc(4678, "", &sig_cell("ĐẠI DIỆN BÊN B", "Lê Thu Hà")),
            ],
        )],
    ));
    v.push((
        "office-21-hop-dong-dieu-khoan.docx",
        "office",
        "Hợp đồng: \"Điều N.\" đánh số tự động (suff space), khoản N.M., điểm a), bảng chữ ký hai bên có viền, Times New Roman 13 giãn 1,2",
        Doc {
            body: b21,
            sect: sect_a4("", ""),
            styles: Some(styles("Times New Roman", 13.0, &spacing(0, 80, 288, false))),
            numbering: Some(numbering_raw(&[clause_lvls])),
            ..Doc::default()
        },
    ));

    // ---- 22 báo cáo có biểu đồ và chú thích (SEQ).
    let cats = ["Quý I", "Quý II", "Quý III", "Quý IV"];
    let b22 = [
        heading(1, "1. KẾT QUẢ KINH DOANH"),
        body(567, &sents(0, 3)),
        para(
            r#"<w:keepNext/><w:spacing w:before="120" w:after="0"/><w:jc w:val="center"/>"#,
            &inline_chart(41, "rIdX1", 5_486_400, 2_743_200),
        ),
        caption(
            101,
            "_Ref410000001",
            "Biểu_đồ",
            1,
            "Doanh thu theo quý năm 2026 (tỷ đồng)",
        ),
        body(567, &sents(3, 3)),
        body(567, &sents(5, 2)),
        para(
            r#"<w:keepNext/><w:spacing w:before="120" w:after="0"/><w:jc w:val="center"/>"#,
            &inline_chart(42, "rIdX2", 5_486_400, 2_560_320),
        ),
        caption(
            102,
            "_Ref410000002",
            "Biểu_đồ",
            2,
            "Số hồ sơ giải quyết đúng hạn",
        ),
        heading(1, "2. CƠ CẤU CHI PHÍ"),
        body(567, &sents(2, 2)),
        tbl(
            &[4677, 4678],
            false,
            r#"<w:tblLayout w:type="fixed"/>"#,
            &[tr(
                "",
                &[
                    tc(
                        4677,
                        r#"<w:vAlign w:val="center"/>"#,
                        &[
                            para(
                                r#"<w:spacing w:after="0"/><w:jc w:val="center"/>"#,
                                &inline_chart(43, "rIdX3", 2_743_200, 2_286_000),
                            ),
                            caption(103, "_Ref410000003", "Biểu_đồ", 3, "Cơ cấu chi phí"),
                        ]
                        .concat(),
                    ),
                    tc(
                        4678,
                        r#"<w:vAlign w:val="center"/>"#,
                        &[body(0, &sents(4, 2)), body(0, &sents(6, 1))].concat(),
                    ),
                ],
            )],
        ),
        body(567, &sents(1, 3)),
    ]
    .concat();
    v.push((
        "office-22-bao-cao-bieu-do.docx",
        "office",
        "Báo cáo có biểu đồ cột / đường / tròn nội dòng (chart part), chú thích \"Biểu đồ N\" bằng trường SEQ, biểu đồ trong ô bảng, Times New Roman 13 giãn 1,15",
        Doc {
            body: b22,
            sect: sect_a4("", ""),
            styles: Some(styles("Times New Roman", 13.0, &spacing(60, 60, 276, false))),
            extra: vec![
                ("charts/chart1.xml", CT_CHART, REL_CHART, chart_xml("bar", "Doanh thu (tỷ đồng)", &cats, &[("Hà Nội", &[12.5, 14.0, 13.2, 16.8]), ("Đà Nẵng", &[8.1, 9.4, 10.0, 11.3])])),
                ("charts/chart2.xml", CT_CHART, REL_CHART, chart_xml("line", "Hồ sơ đúng hạn (%)", &cats, &[("2025", &[88.0, 90.5, 91.2, 93.0]), ("2026", &[91.0, 93.4, 95.1, 96.8])])),
                ("charts/chart3.xml", CT_CHART, REL_CHART, chart_xml("pie", "Chi phí", &["Nhân sự", "Hạ tầng", "Đào tạo", "Khác"], &[("Chi phí", &[55.0, 25.0, 12.0, 8.0])])),
            ],
            ..Doc::default()
        },
    ));

    // ---- 23 giấy mời: page border, big title, tab-leader fill lines, schedule table.
    let fill = |label: &str| {
        para(
            &format!(
                r#"<w:tabs><w:tab w:val="right" w:leader="dot" w:pos="{TEXT_W}"/></w:tabs><w:spacing w:before="120"/>"#
            ),
            &[r("", label), tab()].concat(),
        )
    };
    let sched: Vec<String> = [
        ("08:00 – 08:30", "Đón tiếp đại biểu, ổn định tổ chức"),
        ("08:30 – 09:00", "Khai mạc, phát biểu chào mừng"),
        (
            "09:00 – 10:15",
            "Báo cáo kết quả hoạt động năm 2026 và phương hướng năm 2027",
        ),
        ("10:15 – 10:30", "Nghỉ giải lao"),
        ("10:30 – 11:30", "Thảo luận, trao đổi ý kiến của các đơn vị"),
        ("11:30", "Bế mạc"),
    ]
    .iter()
    .map(|(t, c)| {
        tr(
            "",
            &[
                tc(2268, r#"<w:vAlign w:val="center"/>"#, &center("<w:b/>", t)),
                tc(6520, "", &left("", c)),
            ],
        )
    })
    .collect();
    v.push((
        "office-23-giay-moi.docx",
        "office",
        "Giấy mời: viền trang (pgBorders), tiêu đề 22 pt, dòng điền tab phải dẫn chấm, bảng chương trình, Arial 13",
        Doc {
            body: [
                admin_header(13.0, "CÔNG TY CỔ PHẦN ÁNH SAO", "BAN TỔ CHỨC HỘI NGHỊ", "Số: 09/GM-AS", ""),
                para(r#"<w:spacing w:before="480" w:after="240"/><w:jc w:val="center"/>"#, &r(&format!("<w:b/><w:color w:val=\"C00000\"/>{}", sz(22.0)), "GIẤY MỜI")),
                fill("Kính gửi: "),
                fill("Chức vụ: "),
                fill("Đơn vị: "),
                body(567, "Công ty Cổ phần Ánh Sao trân trọng kính mời Quý vị tới dự Hội nghị tổng kết công tác năm 2026 và triển khai nhiệm vụ năm 2027."),
                para(r#"<w:ind w:left="567"/>"#, &[r("<w:b/>", "Thời gian: "), r("", "08 giờ 00, thứ Sáu, ngày 18 tháng 12 năm 2026.")].concat()),
                para(r#"<w:ind w:left="567"/>"#, &[r("<w:b/>", "Địa điểm: "), r("", "Hội trường tầng 5, số 12 phố Giả Định, quận Ba Đình, Hà Nội.")].concat()),
                para(r#"<w:spacing w:before="120"/><w:ind w:left="567"/>"#, &r("<w:b/>", "Chương trình hội nghị:")),
                tbl(&[2268, 6520], true, r#"<w:tblInd w:w="567" w:type="dxa"/><w:tblLayout w:type="fixed"/>"#, &sched),
                body(567, "Sự hiện diện của Quý vị là niềm vinh dự cho Hội nghị. Đề nghị xác nhận tham dự trước ngày 15 tháng 12 năm 2026."),
                signature(13.0, "TM. BAN TỔ CHỨC", "Phạm Quốc Bảo"),
            ]
            .concat(),
            sect: sect_a4("", r#"<w:pgBorders w:offsetFrom="page"><w:top w:val="double" w:sz="6" w:space="24" w:color="1F4E79"/><w:left w:val="double" w:sz="6" w:space="24" w:color="1F4E79"/><w:bottom w:val="double" w:sz="6" w:space="24" w:color="1F4E79"/><w:right w:val="double" w:sz="6" w:space="24" w:color="1F4E79"/></w:pgBorders>"#),
            styles: Some(styles("Arial", 13.0, &spacing(60, 60, 276, false))),
            ..Doc::default()
        },
    ));

    // ---- 24 biên bản họp: bullets "-", numbered agenda with a) points, 3-column signatures.
    let mut b24 = [
        admin_header(
            14.0,
            "CÔNG TY CỔ PHẦN ÁNH SAO",
            "PHÒNG KỸ THUẬT",
            "Số: 14/BB-KT",
            "",
        ),
        para(
            r#"<w:spacing w:before="360" w:after="120"/><w:jc w:val="center"/>"#,
            &r(&format!("<w:b/>{}", sz(15.0)), "BIÊN BẢN HỌP"),
        ),
        center(
            "<w:b/>",
            "Về việc đánh giá tiến độ dự án chuyển đổi số giai đoạn 2",
        ),
        para(
            r#"<w:tabs><w:tab w:val="left" w:pos="2268"/></w:tabs><w:spacing w:before="240"/>"#,
            &[
                r("<w:b/>", "Thời gian:"),
                tab(),
                r("", "14 giờ 00, ngày 20 tháng 11 năm 2026"),
            ]
            .concat(),
        ),
        para(
            r#"<w:tabs><w:tab w:val="left" w:pos="2268"/></w:tabs>"#,
            &[
                r("<w:b/>", "Địa điểm:"),
                tab(),
                r("", "Phòng họp số 2, tầng 3"),
            ]
            .concat(),
        ),
        para(
            r#"<w:tabs><w:tab w:val="left" w:pos="2268"/></w:tabs>"#,
            &[
                r("<w:b/>", "Chủ trì:"),
                tab(),
                r("", "Ông Hoàng Minh Đức – Trưởng phòng Kỹ thuật"),
            ]
            .concat(),
        ),
        para(
            r#"<w:tabs><w:tab w:val="left" w:pos="2268"/></w:tabs>"#,
            &[
                r("<w:b/>", "Thư ký:"),
                tab(),
                r("", "Bà Vũ Thị Lan – Chuyên viên"),
            ]
            .concat(),
        ),
        para("<w:keepNext/>", &r("<w:b/>", "Thành phần tham dự:")),
    ]
    .concat();
    for who in [
        "Ông Trần Văn Nam – Phó Trưởng phòng Kỹ thuật;",
        "Bà Đỗ Thị Hương – Trưởng nhóm phát triển phần mềm;",
        "Ông Lý Quang Huy – Trưởng nhóm hạ tầng và an toàn thông tin;",
        "Đại diện Phòng Tài chính – Kế toán và Phòng Hành chính – Nhân sự.",
    ] {
        b24.push_str(&numbered(1, 0, "", r#"<w:jc w:val="both"/>"#, &r("", who)));
    }
    b24.push_str(&para(
        r#"<w:keepNext/><w:spacing w:before="120"/>"#,
        &r("<w:b/>", "Nội dung cuộc họp:"),
    ));
    for (k, t) in [
        "Báo cáo tiến độ",
        "Ý kiến thảo luận",
        "Kết luận của chủ trì",
    ]
    .iter()
    .enumerate()
    {
        b24.push_str(&numbered(2, 0, "<w:keepNext/>", "", &r("<w:b/>", t)));
        for m in 0..3 {
            b24.push_str(&numbered(
                2,
                1,
                "",
                r#"<w:jc w:val="both"/>"#,
                &r("", &sents(k * 2 + m, 1 + (m + k) % 2)),
            ));
        }
    }
    b24.push_str(&body(567, "Cuộc họp kết thúc vào lúc 16 giờ 30 cùng ngày. Biên bản được lập thành 03 bản, các thành viên đã đọc lại và nhất trí ký tên."));
    let sig3 = |t: &str, n: &str| {
        [
            center("<w:b/>", t),
            center("", ""),
            center("", ""),
            center("", ""),
            center("<w:b/>", n),
        ]
        .concat()
    };
    b24.push_str(&tbl(
        &[3118, 3118, 3119],
        false,
        r#"<w:tblLayout w:type="fixed"/>"#,
        &[tr(
            "<w:cantSplit/>",
            &[
                tc(3118, "", &sig3("THƯ KÝ", "Vũ Thị Lan")),
                tc(3118, "", &sig3("ĐẠI DIỆN CÁC NHÓM", "Đỗ Thị Hương")),
                tc(3119, "", &sig3("CHỦ TRÌ", "Hoàng Minh Đức")),
            ],
        )],
    ));
    v.push((
        "office-24-bien-ban-hop.docx",
        "office",
        "Biên bản họp: dòng thông tin thẳng tab, gạch đầu dòng \"-\" (numFmt bullet), nội dung đánh số 1. / a), chữ ký ba cột, Times New Roman 14 giãn 1,15",
        Doc {
            body: b24,
            sect: sect_a4("", ""),
            styles: Some(styles("Times New Roman", 14.0, &spacing(40, 40, 276, false))),
            numbering: Some(numbering_raw(&[
                lvl(0, "bullet", "-", "", "tab", 851, 284, ""),
                [lvl(0, "decimal", "%1.", "", "tab", 567, 283, "<w:b/><w:bCs/>"), lvl(1, "lowerLetter", "%2)", "", "tab", 851, 284, "")].concat(),
            ])),
            ..Doc::default()
        },
    ));

    // ---- 25 CV: shaded left column with photo, right column with ruled section titles,
    // bullets and right-tabbed dates.
    let small_with = |mid: &str| format!("{}{mid}{}", font("Arial"), sz(10.0));
    let small = small_with("");
    let leftc = [
        para(
            r#"<w:spacing w:before="120" w:after="120"/><w:jc w:val="center"/>"#,
            &inline_pic(51, "rIdP0", 1_080_000, 1_440_000, "", ""),
        ),
        center(
            &format!("<w:b/><w:color w:val=\"1F4E79\"/>{}", sz(14.0)),
            "NGUYỄN THỊ MINH ANH",
        ),
        center(&small_with("<w:i/>"), "Chuyên viên phân tích dữ liệu"),
        ruled(&small_with("<w:b/>"), "LIÊN HỆ"),
        left(&small, "Điện thoại: 0900 000 000"),
        left(&small, "Thư điện tử: minhanh@vidu.vn"),
        left(&small, "Địa chỉ: Cầu Giấy, Hà Nội"),
        ruled(&small_with("<w:b/>"), "KỸ NĂNG"),
        numbered(1, 0, "", TIGHT, &r(&small, "Phân tích số liệu, thống kê")),
        numbered(1, 0, "", TIGHT, &r(&small, "Lập báo cáo, trình bày")),
        numbered(1, 0, "", TIGHT, &r(&small, "Tiếng Anh giao tiếp tốt")),
        numbered(1, 0, "", TIGHT, &r(&small, "Làm việc nhóm")),
    ]
    .concat();
    let job = |title: &str, dates: &str, place: &str, items: &[&str]| {
        let mut s = para(
            r#"<w:keepNext/><w:tabs><w:tab w:val="right" w:pos="5900"/></w:tabs><w:spacing w:before="120" w:after="0"/>"#,
            &[r("<w:b/>", title), tab(), r("<w:i/>", dates)].concat(),
        );
        s.push_str(&para(
            r#"<w:keepNext/><w:spacing w:before="0" w:after="40"/>"#,
            &r("<w:i/><w:color w:val=\"595959\"/>", place),
        ));
        for it in items {
            s.push_str(&numbered(
                1,
                0,
                "",
                r#"<w:spacing w:before="0" w:after="20"/><w:jc w:val="both"/>"#,
                &r("", it),
            ));
        }
        s
    };
    let rightc = [
        ruled("<w:b/><w:color w:val=\"1F4E79\"/>", "MỤC TIÊU NGHỀ NGHIỆP"),
        para(r#"<w:jc w:val="both"/>"#, &r("", "Phát triển trong lĩnh vực phân tích dữ liệu phục vụ quản trị, đóng góp vào việc nâng cao chất lượng ra quyết định của doanh nghiệp.")),
        ruled("<w:b/><w:color w:val=\"1F4E79\"/>", "KINH NGHIỆM LÀM VIỆC"),
        job("Chuyên viên phân tích", "03/2023 – nay", "Công ty Cổ phần Ánh Sao", &["Xây dựng báo cáo quản trị hằng tháng cho Ban Giám đốc.", "Tự động hoá quy trình tổng hợp số liệu từ các chi nhánh.", "Hướng dẫn nhân viên mới sử dụng công cụ phân tích."]),
        job("Nhân viên kế hoạch", "07/2020 – 02/2023", "Công ty TNHH Giả Định Miền Bắc", &["Theo dõi tiến độ thực hiện kế hoạch kinh doanh.", "Lập dự toán chi phí và đề xuất điều chỉnh."]),
        ruled("<w:b/><w:color w:val=\"1F4E79\"/>", "HỌC VẤN"),
        job("Cử nhân Kinh tế", "2016 – 2020", "Trường Đại học Giả Định", &["Tốt nghiệp loại Giỏi, chuyên ngành Thống kê kinh tế."]),
        ruled("<w:b/><w:color w:val=\"1F4E79\"/>", "CHỨNG CHỈ"),
        job("Chứng chỉ phân tích dữ liệu", "2022", "Trung tâm Đào tạo Giả Định", &[]),
    ]
    .concat();
    v.push((
        "office-25-cv.docx",
        "office",
        "CV: bảng hai cột không viền, cột trái tô nền có ảnh 3×4, tiêu đề mục có viền dưới, gạch đầu dòng, ngày tháng tab phải, Arial 10–11",
        Doc {
            body: tbl(&[3175, 6180], false, r#"<w:tblLayout w:type="fixed"/>"#, &[tr("", &[tc(3175, r#"<w:shd w:val="clear" w:color="auto" w:fill="DEEAF6"/>"#, &leftc), tc(6180, "", &rightc)])]),
            sect: sect_a4("", ""),
            styles: Some(styles("Arial", 10.5, &spacing(0, 60, 252, false))),
            numbering: Some(numbering_raw(&[lvl(0, "bullet", "•", "", "tab", 284, 284, "")])),
            images: vec![("rIdP0".into(), png(24, 32, [0x8E, 0xA9, 0xC8]))],
            ..Doc::default()
        },
    ));

    // ---- 26 đề cương nhiều cấp: headings linked to a multilevel list 1. / 1.1. / 1.1.1.,
    // body lists a) and "-".
    let outline = [
        lvl(0, "decimal", "%1.", "Heading1", "tab", 432, 432, ""),
        lvl(1, "decimal", "%1.%2.", "Heading2", "tab", 576, 576, ""),
        lvl(2, "decimal", "%1.%2.%3.", "Heading3", "tab", 720, 720, ""),
    ]
    .concat();
    let points = [
        lvl(0, "lowerLetter", "%1)", "", "tab", 851, 284, ""),
        lvl(1, "bullet", "-", "", "tab", 1135, 284, ""),
    ]
    .concat();
    let mut b26 = center(
        &format!("<w:b/>{}", sz(15.0)),
        "ĐỀ CƯƠNG ĐỀ ÁN CHUYỂN ĐỔI SỐ",
    );
    let h = |lvl: u8, t: &str| {
        para(
            &format!(r#"<w:pStyle w:val="Heading{}"/>"#, lvl + 1),
            &r("", t),
        )
    };
    let mut list_n = 2;
    for (i, t1) in [
        "Sự cần thiết",
        "Mục tiêu",
        "Nhiệm vụ và giải pháp",
        "Tổ chức thực hiện",
    ]
    .iter()
    .enumerate()
    {
        b26.push_str(&h(0, t1));
        b26.push_str(&body(567, &sents(i, 2)));
        for j in 0..2 {
            b26.push_str(&h(1, &format!("{} {}", ["Nội dung", "Yêu cầu"][j], i + 1)));
            b26.push_str(&body(567, &sents(i + j + 2, 1)));
            if j == 1 {
                b26.push_str(&h(2, "Chỉ tiêu cụ thể"));
                for k in 0..3 {
                    b26.push_str(&numbered(
                        list_n,
                        0,
                        "",
                        r#"<w:jc w:val="both"/>"#,
                        &r("", SENT[(i + k) % SENT.len()]),
                    ));
                    if k == 0 {
                        for q in 0..2 {
                            b26.push_str(&numbered(
                                list_n,
                                1,
                                "",
                                r#"<w:jc w:val="both"/>"#,
                                &r(
                                    "",
                                    SENT[(i + q + 4) % SENT.len()]
                                        .split(',')
                                        .next()
                                        .unwrap_or(""),
                                ),
                            ));
                        }
                    }
                }
                list_n += 1;
            }
        }
    }
    let mut abstracts26 = vec![outline];
    for _ in 2..list_n {
        abstracts26.push(points.clone());
    }
    let styles26 = styles("Times New Roman", 13.0, &spacing(60, 60, 288, false))
        .replace(
            r#"<w:keepNext/><w:keepLines/><w:spacing w:before="240" w:after="120"/><w:outlineLvl w:val="0"/>"#,
            r#"<w:keepNext/><w:keepLines/><w:numPr><w:numId w:val="1"/></w:numPr><w:spacing w:before="240" w:after="120"/><w:outlineLvl w:val="0"/>"#,
        )
        .replace(
            r#"<w:keepNext/><w:keepLines/><w:spacing w:before="120" w:after="60"/><w:outlineLvl w:val="1"/>"#,
            r#"<w:keepNext/><w:keepLines/><w:numPr><w:ilvl w:val="1"/><w:numId w:val="1"/></w:numPr><w:spacing w:before="120" w:after="60"/><w:outlineLvl w:val="1"/>"#,
        )
        .replace(
            r#"<w:keepNext/><w:keepLines/><w:spacing w:before="60" w:after="60"/><w:outlineLvl w:val="2"/>"#,
            r#"<w:keepNext/><w:keepLines/><w:numPr><w:ilvl w:val="2"/><w:numId w:val="1"/></w:numPr><w:spacing w:before="60" w:after="60"/><w:outlineLvl w:val="2"/>"#,
        );
    v.push((
        "office-26-de-cuong-nhieu-cap.docx",
        "office",
        "Đề cương: đề mục Heading 1–3 gắn danh sách nhiều cấp 1. / 1.1. / 1.1.1. (numPr trong style, pStyle trong lvl), điểm a) và gạch \"-\" lồng, Times New Roman 13",
        Doc {
            body: b26,
            sect: sect_a4("", ""),
            styles: Some(styles26),
            numbering: Some(numbering_raw(&abstracts26)),
            ..Doc::default()
        },
    ));

    // ---- 27 mục lục có số trang PAGEREF + bookmarks _Toc, sections on new pages, footer PAGE.
    let toc27: &[(u8, &str, u32)] = &[
        (1, "1. Giới thiệu chung", 2),
        (2, "1.1. Mục đích", 2),
        (2, "1.2. Phạm vi", 2),
        (1, "2. Hiện trạng", 3),
        (2, "2.1. Hạ tầng kỹ thuật", 3),
        (2, "2.2. Nguồn nhân lực", 3),
        (1, "3. Giải pháp", 4),
        (2, "3.1. Giải pháp kỹ thuật", 4),
        (3, "3.1.1. Máy chủ và lưu trữ", 4),
        (3, "3.1.2. Mạng và an toàn thông tin", 4),
        (2, "3.2. Giải pháp tổ chức", 5),
        (1, "4. Kết luận", 6),
    ];
    let mut b27 = para(
        r#"<w:spacing w:after="240"/><w:jc w:val="center"/>"#,
        &r(&format!("<w:b/>{}", sz(15.0)), "MỤC LỤC"),
    );
    for (k, (lvl, t, pg)) in toc27.iter().enumerate() {
        let begin = if k == 0 {
            r#"<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> TOC \o "1-3" \h \z \u </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r>"#
        } else {
            ""
        };
        let end = if k + 1 == toc27.len() {
            r#"<w:r><w:fldChar w:fldCharType="end"/></w:r>"#
        } else {
            ""
        };
        let bm = format!("_Toc27000{k:03}");
        b27.push_str(&para(
            &format!(r#"<w:pStyle w:val="TOC{lvl}"/>"#),
            &format!(
                r#"{begin}<w:hyperlink w:anchor="{bm}" w:history="1">{}{}{}</w:hyperlink>{end}"#,
                r("", t),
                tab(),
                field_complex(&format!("PAGEREF {bm} \\h"), &pg.to_string())
            ),
        ));
    }
    let mut last_pg = 1;
    for (k, (lvl, t, pg)) in toc27.iter().enumerate() {
        if *pg != last_pg {
            b27.push_str(&page_break());
            last_pg = *pg;
        }
        let bm = format!("_Toc27000{k:03}");
        b27.push_str(&para(
            &format!(r#"<w:pStyle w:val="Heading{lvl}"/>"#),
            &bookmark(200 + k as u32, &bm, &r("", t)),
        ));
        b27.push_str(&body(567, &sents(k, 2 + k % 2)));
        if *lvl > 1 {
            b27.push_str(&body(567, &sents(k + 3, 2)));
        }
    }
    v.push((
        "office-27-muc-luc-so-trang.docx",
        "office",
        "Mục lục: trường TOC có hyperlink + PAGEREF tới bookmark _Toc của đề mục (kết quả cache đúng trang), các phần sang trang, chân trang PAGE, Times New Roman 13",
        Doc {
            body: b27,
            sect: sect_a4(r#"<w:footerReference w:type="default" r:id="rIdF1"/>"#, ""),
            styles: Some(styles("Times New Roman", 13.0, &spacing(60, 60, 276, false))),
            hf: vec![("rIdF1".into(), "footer", para(r#"<w:pStyle w:val="Footer"/><w:jc w:val="center"/>"#, &field_complex("PAGE", "1")))],
            ..Doc::default()
        },
    ));

    // ---- 28 tham chiếu chéo: SEQ captions with _Ref bookmarks, REF / PAGEREF fields with
    // stale cached results (as saved before an update).
    let rows28: Vec<String> = (0..6)
        .map(|i| {
            tr(
                "",
                &[
                    tc(700, "", &center("", &(i + 1).to_string())),
                    tc(
                        4800,
                        "",
                        &left("", SENT[i % SENT.len()].split(',').next().unwrap_or("")),
                    ),
                    tc(
                        1900,
                        "",
                        &right("", &thousands(1_250_000 + i as u64 * 375_500)),
                    ),
                    tc(1955, "", &center("", ["Đạt", "Chưa đạt", "Đạt"][i % 3])),
                ],
            )
        })
        .collect();
    let b28 = [
        heading(1, "1. TỔNG HỢP SỐ LIỆU"),
        para(
            r#"<w:ind w:firstLine="567"/><w:jc w:val="both"/>"#,
            &[
                r("", "Kết quả chi tiết được trình bày tại "),
                field_complex("REF _Ref280000001 \\h", "Bảng 3"),
                r("", " (trang "),
                field_complex("PAGEREF _Ref280000001 \\h", "5"),
                r("", "); hình ảnh minh hoạ xem "),
                field_complex("REF _Ref280000002 \\h", "Hình 1"),
                r("", ". "),
                r("", &sents(0, 2)),
            ]
            .concat(),
        ),
        body(567, &sents(2, 3)),
        caption(
            301,
            "_Ref280000001",
            "Bảng",
            1,
            "Kết quả thực hiện các chỉ tiêu",
        ),
        tbl(
            &[700, 4800, 1900, 1955],
            true,
            r#"<w:tblLayout w:type="fixed"/>"#,
            &[
                vec![tr(
                    "<w:tblHeader/>",
                    &[
                        tc(700, "", &center("<w:b/>", "STT")),
                        tc(4800, "", &center("<w:b/>", "Chỉ tiêu")),
                        tc(1900, "", &center("<w:b/>", "Giá trị (đồng)")),
                        tc(1955, "", &center("<w:b/>", "Đánh giá")),
                    ],
                )],
                rows28,
            ]
            .concat(),
        ),
        body(567, &sents(4, 3)),
        para(
            r#"<w:keepNext/><w:spacing w:before="120" w:after="0"/><w:jc w:val="center"/>"#,
            &inline_pic(52, "rIdP0", 3_600_000, 1_800_000, "", ""),
        ),
        caption(302, "_Ref280000002", "Hình", 1, "Sơ đồ bố trí mặt bằng"),
        para(
            r#"<w:ind w:firstLine="567"/><w:jc w:val="both"/>"#,
            &[
                r("", "Như đã nêu ở "),
                field_complex("REF _Ref280000001 \\h", "Bảng 3"),
                r("", " và "),
                field_complex("REF _Ref280000002 \\h", "Hình 1"),
                r("", " ở trang "),
                field_complex("PAGEREF _Ref280000002 \\h", "1"),
                r("", ", "),
                r("", &sents(6, 2)),
            ]
            .concat(),
        ),
        body(567, &sents(1, 4)),
        body(567, &sents(5, 3)),
    ]
    .concat();
    v.push((
        "office-28-tham-chieu-cheo.docx",
        "office",
        "Tham chiếu chéo: chú thích Bảng / Hình bằng SEQ có bookmark _Ref, trường REF / PAGEREF với kết quả cache cũ (sai số bảng, sai trang), Times New Roman 13",
        Doc {
            body: b28,
            sect: sect_a4("", ""),
            styles: Some(styles("Times New Roman", 13.0, &spacing(60, 60, 276, false))),
            images: vec![("rIdP0".into(), png(32, 16, [0x70, 0xAD, 0x47]))],
            ..Doc::default()
        },
    ));

    // ---- 29 phương trình: inline and numbered display equations.
    let e1 = [
        mr("y="),
        mfrac(
            &[mr("a"), msup(&mr("x"), &mr("2")), mr("+b")].concat(),
            &mr("c"),
        ),
    ]
    .concat();
    let e2 = [
        msub(&mr("S"), &mr("n")),
        mr("="),
        msum(&mr("i=1"), &mr("n"), &msub(&mr("a"), &mr("i"))),
    ]
    .concat();
    let e3 = [
        mr("x="),
        mfrac(
            &[
                mr("−b±"),
                msqrt(&[msup(&mr("b"), &mr("2")), mr("−4ac")].concat()),
            ]
            .concat(),
            &mr("2a"),
        ),
    ]
    .concat();
    let e4 = [
        mr("V="),
        msub(&mr("V"), &mr("0")),
        msup(
            &[mr("("), mr("1+"), mfrac(&mr("r"), &mr("100")), mr(")")].concat(),
            &mr("t"),
        ),
    ]
    .concat();
    let inline = |pre: &str, math: &str, post: &str| {
        para(
            r#"<w:ind w:firstLine="567"/><w:jc w:val="both"/>"#,
            &[
                r("", pre),
                format!("<m:oMath>{math}</m:oMath>"),
                r("", post),
            ]
            .concat(),
        )
    };
    let b29 = [
        heading(1, "PHỤ LỤC: PHƯƠNG PHÁP TÍNH"),
        body(567, &sents(0, 2)),
        inline("Giá trị tính toán được xác định theo hàm bậc hai ", &[mr("f"), mr("(x)=a"), msup(&mr("x"), &mr("2")), mr("+bx+c")].concat(), ", trong đó các hệ số được ước lượng từ số liệu ba năm gần nhất của đơn vị."),
        numbered_equation(1, &e1),
        body(567, &sents(2, 2)),
        numbered_equation(2, &e2),
        inline("Với chỉ số tăng trưởng ", &[msub(&mr("g"), &mr("t")), mr("="), mfrac(&[msub(&mr("a"), &mr("t")), mr("−"), msub(&mr("a"), &mr("t−1"))].concat(), &msub(&mr("a"), &mr("t−1")))].concat(), " tính cho từng năm, giá trị trung bình được dùng làm căn cứ lập kế hoạch."),
        numbered_equation(3, &e3),
        body(567, &sents(4, 3)),
        numbered_equation(4, &e4),
        body(567, &sents(6, 2)),
        para("", &format!(r#"<m:oMathPara><m:oMathParaPr><m:jc m:val="center"/></m:oMathParaPr><m:oMath>{}</m:oMath></m:oMathPara>"#, [mr("T="), msum(&mr("k=1"), &mr("12"), &mfrac(&msub(&mr("C"), &mr("k")), &msup(&mr("(1+r)"), &mr("k"))))].concat())),
        body(567, &sents(1, 3)),
    ]
    .concat();
    v.push((
        "office-29-phuong-trinh.docx",
        "office",
        "Phương trình: Office Math nội dòng (phân số, chỉ số, căn) và phương trình riêng dòng đánh số (tab giữa + tab phải), oMathPara căn giữa, Times New Roman 13",
        Doc {
            body: b29,
            sect: sect_a4("", ""),
            styles: Some(styles("Times New Roman", 13.0, &spacing(60, 60, 276, false))),
            ..Doc::default()
        },
    ));

    // ---- 30 sơ đồ dạng SmartArt: shape groups in mc:AlternateContent with fallback pictures.
    let b30 = [
        heading(1, "QUY TRÌNH GIẢI QUYẾT HỒ SƠ"),
        body(567, &sents(0, 2)),
        para(
            r#"<w:spacing w:before="120" w:after="120"/><w:jc w:val="center"/>"#,
            &diagram_with_fallback(
                61,
                "rIdP0",
                &["Tiếp nhận", "Thẩm định", "Phê duyệt", "Trả kết quả"],
            ),
        ),
        body(567, &sents(2, 3)),
        heading(1, "QUY TRÌNH LƯU TRỮ"),
        body(567, &sents(4, 2)),
        para(
            r#"<w:spacing w:before="120" w:after="120"/><w:jc w:val="center"/>"#,
            &diagram_with_fallback(62, "rIdP1", &["Phân loại", "Số hoá", "Lưu trữ"]),
        ),
        body(567, &sents(5, 3)),
    ]
    .concat();
    v.push((
        "office-30-so-do-du-phong.docx",
        "office",
        "Sơ đồ quy trình: nhóm hình (wpg) trong mc:AlternateContent với ảnh dự phòng (mc:Fallback), chữ trong hình, Times New Roman 13",
        Doc {
            body: b30,
            sect: sect_a4("", ""),
            styles: Some(styles("Times New Roman", 13.0, &spacing(60, 60, 276, false))),
            images: vec![("rIdP0".into(), png(60, 10, [0x44, 0x72, 0xC4])), ("rIdP1".into(), png(60, 10, [0x44, 0x72, 0xC4]))],
            ..Doc::default()
        },
    ));

    // ---- 31 text boxes anchored to paragraphs: right / left of the column, square wrap.
    let note_box = |t: &str| {
        [
            para(TIGHT, &r(&format!("<w:b/>{}", sz(11.0)), "Lưu ý")),
            para(&format!(r#"{TIGHT}<w:jc w:val="both"/>"#), &r(&sz(11.0), t)),
        ]
        .concat()
    };
    let b31 = [
        heading(1, "HƯỚNG DẪN THỰC HIỆN"),
        para(
            r#"<w:ind w:firstLine="567"/><w:jc w:val="both"/>"#,
            &[
                para_box(
                    71,
                    "column",
                    (3_960_000, 0),
                    (1_980_000, 1_260_000),
                    r#"<wp:wrapSquare wrapText="bothSides"/>"#,
                    &note_box("Hồ sơ nộp trực tuyến được ưu tiên giải quyết."),
                ),
                r("", &sents(0, 4)),
            ]
            .concat(),
        ),
        body(567, &sents(4, 3)),
        para(
            r#"<w:ind w:firstLine="567"/><w:jc w:val="both"/>"#,
            &[
                para_box(
                    72,
                    "column",
                    (0, 180_000),
                    (1_800_000, 1_080_000),
                    r#"<wp:wrapSquare wrapText="bothSides"/>"#,
                    &note_box("Thời hạn: 05 ngày làm việc."),
                ),
                r("", &sents(2, 5)),
            ]
            .concat(),
        ),
        body(567, &sents(6, 3)),
        para(
            r#"<w:ind w:firstLine="567"/><w:jc w:val="both"/>"#,
            &[
                para_box(
                    73,
                    "margin",
                    (1_440_000, 0),
                    (3_240_000, 640_000),
                    "<wp:wrapTopAndBottom/>",
                    &note_box("Mẫu đơn được đăng tải trên cổng thông tin điện tử của đơn vị."),
                ),
                r("", &sents(1, 4)),
            ]
            .concat(),
        ),
        body(567, &sents(3, 4)),
    ]
    .concat();
    v.push((
        "office-31-hop-van-ban-neo-doan.docx",
        "office",
        "Hộp văn bản neo vào đoạn (positionV paragraph, positionH column / margin), bao vuông hai bên và trên-dưới, nhiều hộp trên một trang, Times New Roman 13",
        Doc {
            body: b31,
            sect: sect_a4("", ""),
            styles: Some(styles("Times New Roman", 13.0, &spacing(60, 60, 276, false))),
            ..Doc::default()
        },
    ));

    // ---- 32 long table: 2 repeated header rows, vertically merged group cells spanning
    // page breaks, rows that split, gridSpan subtotal rows.
    let g32 = [567, 1701, 3402, 1134, 1276, 1275];
    let hd = |t: &str, w: i32, extra: &str| {
        tc(
            w,
            &format!(
                r#"{extra}<w:shd w:val="clear" w:color="auto" w:fill="FBE4D5"/><w:vAlign w:val="center"/>"#
            ),
            &center("<w:b/>", t),
        )
    };
    let mut rows32 = vec![
        tr(
            "<w:tblHeader/>",
            &[
                hd("TT", 567, r#"<w:vMerge w:val="restart"/>"#),
                hd("Nhóm", 1701, r#"<w:vMerge w:val="restart"/>"#),
                hd("Nội dung", 3402, r#"<w:vMerge w:val="restart"/>"#),
                hd("Kế hoạch", 2410, r#"<w:gridSpan w:val="2"/>"#),
                hd("Ghi chú", 1275, r#"<w:vMerge w:val="restart"/>"#),
            ],
        ),
        tr(
            "<w:tblHeader/>",
            &[
                hd("", 567, "<w:vMerge/>"),
                hd("", 1701, "<w:vMerge/>"),
                hd("", 3402, "<w:vMerge/>"),
                hd("Bắt đầu", 1134, ""),
                hd("Hoàn thành", 1276, ""),
                hd("", 1275, "<w:vMerge/>"),
            ],
        ),
    ];
    let groups = [
        "Hạ tầng kỹ thuật",
        "Ứng dụng nghiệp vụ",
        "Dữ liệu",
        "Đào tạo",
    ];
    let mut n = 0;
    for (gi, g) in groups.iter().enumerate() {
        for k in 0..6 {
            n += 1;
            let merge = if k == 0 {
                r#"<w:vMerge w:val="restart"/><w:vAlign w:val="center"/>"#
            } else {
                "<w:vMerge/>"
            };
            let text = if k % 3 == 1 {
                sents(gi + k, 2)
            } else {
                SENT[(gi + k) % SENT.len()].to_owned()
            };
            rows32.push(tr(
                "",
                &[
                    tc(567, "", &center("", &n.to_string())),
                    tc(
                        1701,
                        merge,
                        &if k == 0 {
                            center("<w:b/>", g)
                        } else {
                            para(TIGHT, "")
                        },
                    ),
                    tc(
                        3402,
                        "",
                        &para(&format!(r#"{TIGHT}<w:jc w:val="both"/>"#), &r("", &text)),
                    ),
                    tc(1134, "", &center("", &format!("{:02}/2026", (n % 12) + 1))),
                    tc(
                        1276,
                        "",
                        &center("", &format!("{:02}/2027", ((n + 5) % 12) + 1)),
                    ),
                    tc(1275, "", &center("", "")),
                ],
            ));
        }
        rows32.push(tr("<w:cantSplit/>", &[tc(5670, r#"<w:gridSpan w:val="3"/><w:shd w:val="clear" w:color="auto" w:fill="F2F2F2"/>"#, &right("<w:b/>", &format!("Cộng nhóm {}", gi + 1))), tc(2410, r#"<w:gridSpan w:val="2"/><w:shd w:val="clear" w:color="auto" w:fill="F2F2F2"/>"#, &center("<w:b/>", "6 nhiệm vụ")), tc(1275, r#"<w:shd w:val="clear" w:color="auto" w:fill="F2F2F2"/>"#, &center("", ""))]));
    }
    v.push((
        "office-32-bang-tach-trang-gop-o.docx",
        "office",
        "Bảng dài qua nhiều trang: 2 hàng tiêu đề lặp lại, ô gộp dọc (vMerge) kéo qua chỗ ngắt trang, hàng tách trang, hàng cộng gộp ngang (gridSpan, cantSplit), Times New Roman 12",
        Doc {
            body: [
                center(&format!("<w:b/>{}", sz(14.0)), "DANH MỤC NHIỆM VỤ CHUYỂN ĐỔI SỐ"),
                body(567, &sents(0, 2)),
                tbl(&g32, true, r#"<w:tblLayout w:type="fixed"/>"#, &rows32),
                body(567, &sents(3, 2)),
            ]
            .concat(),
            sect: sect_a4("", ""),
            styles: Some(styles("Times New Roman", 12.0, &spacing(60, 60, 276, false))),
            ..Doc::default()
        },
    ));

    // ---- 33 landscape page in the middle, footer page numbers continue.
    let wide = [567, 2835, 1418, 1418, 1418, 1418, 1418, 1418, 2665];
    let mut rows33 = vec![tr(
        "<w:tblHeader/>",
        &[
            "STT",
            "Đơn vị",
            "Tháng 7",
            "Tháng 8",
            "Tháng 9",
            "Tháng 10",
            "Tháng 11",
            "Tháng 12",
            "Ghi chú",
        ]
        .iter()
        .zip(&wide)
        .map(|(t, w)| {
            tc(
                *w,
                r#"<w:shd w:val="clear" w:color="auto" w:fill="E2EFD9"/>"#,
                &center("<w:b/>", t),
            )
        })
        .collect::<Vec<_>>(),
    )];
    for i in 0..14u64 {
        let mut cells = vec![
            tc(567, "", &center("", &(i + 1).to_string())),
            tc(
                2835,
                "",
                &left(
                    "",
                    [
                        "Văn phòng",
                        "Phòng Kế hoạch",
                        "Phòng Kỹ thuật",
                        "Phòng Tài chính",
                        "Chi nhánh miền Trung",
                        "Chi nhánh miền Nam",
                        "Trung tâm dữ liệu",
                    ][(i % 7) as usize],
                ),
            ),
        ];
        for m in 0..6u64 {
            cells.push(tc(
                1418,
                "",
                &right("", &thousands(1_000 + (i * 37 + m * 113) % 900 * 10)),
            ));
        }
        cells.push(tc(
            2665,
            "",
            &left(
                "",
                if i % 4 == 0 {
                    "Vượt kế hoạch"
                } else {
                    ""
                },
            ),
        ));
        rows33.push(tr("", &cells));
    }
    v.push((
        "office-33-trang-ngang-giua.docx",
        "office",
        "Trang ngang giữa văn bản dọc (section nextPage), bảng 9 cột lặp tiêu đề, đầu / chân trang nối tiếp (PAGE), Times New Roman 13",
        Doc {
            body: [
                heading(1, "1. TÌNH HÌNH THỰC HIỆN"),
                body(567, &sents(0, 4)),
                body(567, &sents(4, 4)),
                body(567, &sents(1, 3)),
                sect_para(&sect_a4(r#"<w:headerReference w:type="default" r:id="rIdH1"/><w:footerReference w:type="default" r:id="rIdF1"/>"#, "")),
                para(r#"<w:jc w:val="center"/>"#, &r("<w:b/>", "Bảng 1. Số liệu sáu tháng cuối năm 2026 (triệu đồng)")),
                tbl(&wide, true, r#"<w:tblLayout w:type="fixed"/>"#, &rows33),
                body(567, &sents(2, 2)),
                sect_para(&sect_a4_landscape(r#"<w:headerReference w:type="default" r:id="rIdH1"/><w:footerReference w:type="default" r:id="rIdF1"/><w:type w:val="nextPage"/>"#, "")),
                heading(1, "2. ĐÁNH GIÁ"),
                body(567, &sents(3, 5)),
                body(567, &sents(5, 3)),
            ]
            .concat(),
            sect: sect_a4(r#"<w:headerReference w:type="default" r:id="rIdH1"/><w:footerReference w:type="default" r:id="rIdF1"/><w:type w:val="nextPage"/>"#, ""),
            styles: Some(styles("Times New Roman", 13.0, &spacing(60, 60, 276, false))),
            hf: vec![
                ("rIdH1".into(), "header", para(r#"<w:pStyle w:val="Header"/><w:jc w:val="right"/>"#, &r(&format!("<w:i/>{}", sz(11.0)), "Báo cáo sáu tháng cuối năm 2026"))),
                ("rIdF1".into(), "footer", para(r#"<w:pStyle w:val="Footer"/><w:jc w:val="center"/>"#, &field_complex("PAGE", "1"))),
            ],
            ..Doc::default()
        },
    ));

    // ---- 34 different first page, odd / even headers and footers.
    let mut b34 = [
        para(
            r#"<w:spacing w:before="2400" w:after="240"/><w:jc w:val="center"/>"#,
            &r(&format!("<w:b/>{}", sz(20.0)), "SỔ TAY NHÂN VIÊN"),
        ),
        center(
            &format!("<w:i/>{}", sz(14.0)),
            "Công ty Cổ phần Ánh Sao – phiên bản 2026",
        ),
        page_break(),
    ]
    .concat();
    for k in 0..5 {
        b34.push_str(&heading(
            1,
            &format!(
                "Chương {}. {}",
                k + 1,
                [
                    "Giới thiệu",
                    "Nội quy lao động",
                    "Chế độ làm việc",
                    "Phúc lợi",
                    "Đánh giá và khen thưởng"
                ][k]
            ),
        ));
        for m in 0..5 {
            b34.push_str(&body(567, &sents(k + m, 3)));
        }
    }
    let hpara = |jc: &str, t: &str| {
        para(
            &format!(
                r#"<w:pStyle w:val="Header"/><w:pBdr><w:bottom w:val="single" w:sz="4" w:space="1" w:color="auto"/></w:pBdr><w:jc w:val="{jc}"/>"#
            ),
            &r(&format!("<w:i/>{}", sz(11.0)), t),
        )
    };
    let fpara = |jc: &str| {
        para(
            &format!(r#"<w:pStyle w:val="Footer"/><w:jc w:val="{jc}"/>"#),
            &[r(&sz(11.0), "Trang "), field_rpr(&sz(11.0), "PAGE", "2")].concat(),
        )
    };
    v.push((
        "office-34-trang-dau-khac-chan-le.docx",
        "office",
        "Trang bìa riêng (titlePg, không đầu / chân trang), đầu / chân trang trang lẻ căn phải, trang chẵn căn trái (evenAndOddHeaders), Times New Roman 13",
        Doc {
            body: b34,
            sect: sect_a4(r#"<w:headerReference w:type="even" r:id="rIdH2"/><w:headerReference w:type="default" r:id="rIdH1"/><w:headerReference w:type="first" r:id="rIdH3"/><w:footerReference w:type="even" r:id="rIdF2"/><w:footerReference w:type="default" r:id="rIdF1"/><w:footerReference w:type="first" r:id="rIdF3"/>"#, "<w:titlePg/>"),
            styles: Some(styles("Times New Roman", 13.0, &spacing(60, 60, 276, false))),
            settings: Some(settings_even_odd()),
            hf: vec![
                ("rIdH1".into(), "header", hpara("right", "Sổ tay nhân viên – Công ty Cổ phần Ánh Sao")),
                ("rIdH2".into(), "header", hpara("left", "Phiên bản 2026")),
                ("rIdH3".into(), "header", para(r#"<w:pStyle w:val="Header"/>"#, "")),
                ("rIdF1".into(), "footer", fpara("right")),
                ("rIdF2".into(), "footer", fpara("left")),
                ("rIdF3".into(), "footer", para(r#"<w:pStyle w:val="Footer"/>"#, "")),
            ],
            ..Doc::default()
        },
    ));

    // ---- 35 "Trang X/Y": NUMPAGES in section 1, SECTIONPAGES + restart in the appendix.
    let mut b35 = heading(1, "BÁO CÁO TỔNG HỢP");
    for k in 0..14 {
        b35.push_str(&body(0, &sents(k, 3)));
    }
    b35.push_str(&sect_para(&sect_a4(
        r#"<w:footerReference w:type="default" r:id="rIdF1"/>"#,
        "",
    )));
    b35.push_str(&heading(1, "PHỤ LỤC"));
    for k in 0..10 {
        b35.push_str(&body(0, &sents(k + 3, 3)));
    }
    v.push((
        "office-35-trang-x-tren-y-section.docx",
        "office",
        "\"Trang X/Y\": section 1 dùng NUMPAGES, phụ lục đánh số lại từ 1 (pgNumType start) với \"Phụ lục – trang X/Y\" bằng SECTIONPAGES, Arial 11 giãn 1,15",
        Doc {
            body: b35,
            sect: sect_a4(r#"<w:footerReference w:type="default" r:id="rIdF2"/><w:type w:val="nextPage"/>"#, r#"<w:pgNumType w:start="1"/>"#),
            styles: Some(styles("Arial", 11.0, &spacing(0, 120, 276, false))),
            hf: vec![
                ("rIdF1".into(), "footer", para(r#"<w:pStyle w:val="Footer"/><w:jc w:val="right"/>"#, &[r("", "Trang "), field_complex("PAGE", "1"), r("", "/"), field_complex("NUMPAGES", "1")].concat())),
                ("rIdF2".into(), "footer", para(r#"<w:pStyle w:val="Footer"/><w:jc w:val="right"/>"#, &[r("", "Phụ lục – trang "), field_complex("PAGE", "1"), r("", "/"), field_complex("SECTIONPAGES", "1")].concat())),
            ],
            ..Doc::default()
        },
    ));

    // ---- 36 dense number appendix: Arial 9, 8 columns, decimals with comma.
    let g36 = [454, 2268, 1077, 1077, 1134, 1134, 1134, 1077];
    let a9 = sz(9.0);
    let mut rows36 = vec![tr("<w:tblHeader/>", &["TT", "Khoản mục", "Mã số", "Đơn vị", "Năm 2024", "Năm 2025", "Năm 2026", "Tăng (%)"].iter().zip(&g36).map(|(t, w)| tc(*w, r#"<w:shd w:val="clear" w:color="auto" w:fill="D9D9D9"/><w:vAlign w:val="center"/>"#, &center(&format!("<w:b/>{a9}"), t))).collect::<Vec<_>>())];
    for i in 0..42u64 {
        let base = 1_234_567 + i * 98_765;
        rows36.push(tr(
            "",
            &[
                tc(454, "", &center(&a9, &(i + 1).to_string())),
                tc(
                    2268,
                    "",
                    &left(
                        &a9,
                        [
                            "Doanh thu bán hàng",
                            "Giá vốn hàng bán",
                            "Chi phí quản lý",
                            "Chi phí bán hàng",
                            "Lợi nhuận thuần",
                            "Thu nhập khác",
                        ][(i % 6) as usize],
                    ),
                ),
                tc(1077, "", &center(&a9, &format!("{}", 100 + i * 10))),
                tc(1077, "", &center(&a9, "triệu đồng")),
                tc(1134, "", &right(&a9, &money2(base * 13))),
                tc(1134, "", &right(&a9, &money2(base * 14 + 777))),
                tc(1134, "", &right(&a9, &money2(base * 15 + 1_234))),
                tc(1077, "", &right(&a9, &format!("{},{}", 3 + i % 9, i % 10))),
            ],
        ));
    }
    v.push((
        "office-36-phu-luc-so-lieu.docx",
        "office",
        "Phụ lục số liệu dày: bảng 8 cột Arial 9, số thập phân dấu phẩy căn phải, hàng tiêu đề tô xám lặp lại, 42 hàng, Arial 11",
        Doc {
            body: [
                center(&format!("<w:b/>{}", sz(13.0)), "PHỤ LỤC 02. CHỈ TIÊU TÀI CHÍNH 2024 – 2026"),
                center("<w:i/>", "(Kèm theo Báo cáo số 51/BC-AS ngày 20/11/2026)"),
                tbl(&g36, true, r#"<w:tblLayout w:type="fixed"/>"#, &rows36),
                body(0, "Ghi chú: số liệu năm 2026 là số ước thực hiện."),
            ]
            .concat(),
            sect: sect_a4("", ""),
            styles: Some(styles("Arial", 11.0, &spacing(0, 60, 240, false))),
            ..Doc::default()
        },
    ));

    // ---- 37 thông báo: justified TNR 13 with bold / underlined runs, "-" bullets, footnote.
    let mut fns37 = String::new();
    let mut b37 = [
        admin_header(
            13.0,
            "CÔNG TY CỔ PHẦN ÁNH SAO",
            "PHÒNG HÀNH CHÍNH – NHÂN SỰ",
            "Số: 31/TB-HCNS",
            "",
        ),
        para(
            r#"<w:spacing w:before="360" w:after="0"/><w:jc w:val="center"/>"#,
            &r(&format!("<w:b/>{}", sz(14.0)), "THÔNG BÁO"),
        ),
        center("<w:b/>", "Tuyển dụng nhân sự năm 2027"),
    ]
    .concat();
    for k in 0..4 {
        let mut runs = r("", &sents(k, 1));
        runs.push_str(&r("<w:b/>", " Hạn nộp hồ sơ: ngày 31/12/2026. "));
        runs.push_str(&r("<w:u w:val=\"single\"/>", SENT[(k + 2) % SENT.len()]));
        runs.push_str(&note_ref("footnote", k as i32 + 1));
        fns37.push_str(&note(
            "footnote",
            k as i32 + 1,
            &format!("Thông tin chi tiết tại bảng tin số {} của đơn vị.", k + 1),
        ));
        runs.push_str(&r("", &format!(" {}", sents(k + 4, 2))));
        b37.push_str(&para(
            r#"<w:ind w:firstLine="567"/><w:jc w:val="both"/>"#,
            &runs,
        ));
        for m in 0..3 {
            b37.push_str(&numbered(
                1,
                0,
                "",
                r#"<w:jc w:val="both"/>"#,
                &r("", SENT[(k + m + 1) % SENT.len()]),
            ));
        }
    }
    b37.push_str(&signature(13.0, "TRƯỞNG PHÒNG", "Vũ Thị Lan"));
    v.push((
        "office-37-thong-bao.docx",
        "office",
        "Thông báo: đoạn đều hai bên có run đậm / gạch chân, chú thích cuối trang, gạch đầu dòng \"-\" thụt treo, Times New Roman 13 giãn 1,5",
        Doc {
            body: b37,
            sect: sect_a4("", ""),
            styles: Some(styles("Times New Roman", 13.0, &spacing(0, 60, 360, false))),
            numbering: Some(numbering_raw(&[lvl(0, "bullet", "-", "", "tab", 851, 284, "")])),
            footnotes: Some(fns37),
            ..Doc::default()
        },
    ));

    // ---- 38 báo cáo tài chính: decimal tab stops in cells, indented row labels, totals
    // with a top border.
    let g38 = [4535, 850, 1134, 1418, 1418];
    let dec = |t: &str, b: bool| {
        para(
            &format!(r#"<w:tabs><w:tab w:val="decimal" w:pos="1000"/></w:tabs>{TIGHT}"#),
            &[tab(), r(if b { "<w:b/>" } else { "" }, t)].concat(),
        )
    };
    let mut rows38 = vec![tr(
        "<w:tblHeader/>",
        &[
            tc(4535, "", &center("<w:b/>", "CHỈ TIÊU")),
            tc(850, "", &center("<w:b/>", "Mã số")),
            tc(1134, "", &center("<w:b/>", "Thuyết minh")),
            tc(1418, "", &center("<w:b/>", "Số cuối năm")),
            tc(1418, "", &center("<w:b/>", "Số đầu năm")),
        ],
    )];
    let items38: &[(u8, &str)] = &[
        (0, "A. TÀI SẢN NGẮN HẠN"),
        (1, "I. Tiền và các khoản tương đương tiền"),
        (2, "1. Tiền"),
        (2, "2. Các khoản tương đương tiền"),
        (1, "II. Đầu tư tài chính ngắn hạn"),
        (2, "1. Chứng khoán kinh doanh"),
        (1, "III. Các khoản phải thu ngắn hạn"),
        (2, "1. Phải thu ngắn hạn của khách hàng"),
        (2, "2. Trả trước cho người bán ngắn hạn"),
        (2, "3. Phải thu ngắn hạn khác"),
        (1, "IV. Hàng tồn kho"),
        (2, "1. Hàng tồn kho"),
        (0, "B. TÀI SẢN DÀI HẠN"),
        (1, "I. Tài sản cố định"),
        (2, "1. Tài sản cố định hữu hình"),
        (3, "- Nguyên giá"),
        (3, "- Giá trị hao mòn luỹ kế"),
        (2, "2. Tài sản cố định vô hình"),
        (1, "II. Tài sản dở dang dài hạn"),
        (1, "III. Đầu tư tài chính dài hạn"),
    ];
    for (k, (lv, t)) in items38.iter().enumerate() {
        let b = *lv <= 1;
        let rp = if b {
            "<w:b/>"
        } else if *lv == 3 {
            "<w:i/>"
        } else {
            ""
        };
        let v1 = 1_234_567_890u64 / (k as u64 + 3) * 7;
        let v2 = v1 / 10 * 9 + 123;
        rows38.push(tr(
            "",
            &[
                tc(
                    4535,
                    "",
                    &para(
                        &format!(r#"{TIGHT}<w:ind w:left="{}"/>"#, i32::from(*lv) * 227),
                        &r(rp, t),
                    ),
                ),
                tc(850, "", &center(rp, &(100 + k * 10).to_string())),
                tc(1134, "", &center("", if k % 3 == 1 { "V.01" } else { "" })),
                tc(1418, "", &dec(&thousands(v1), b)),
                tc(1418, "", &dec(&thousands(v2), b)),
            ],
        ));
    }
    rows38.push(tr("", &[
        tc(4535, r#"<w:tcBorders><w:top w:val="double" w:sz="4" w:space="0" w:color="auto"/></w:tcBorders>"#, &center("<w:b/>", "TỔNG CỘNG TÀI SẢN (270 = 100 + 200)")),
        tc(850, r#"<w:tcBorders><w:top w:val="double" w:sz="4" w:space="0" w:color="auto"/></w:tcBorders>"#, &center("<w:b/>", "270")),
        tc(1134, r#"<w:tcBorders><w:top w:val="double" w:sz="4" w:space="0" w:color="auto"/></w:tcBorders>"#, &center("", "")),
        tc(1418, r#"<w:tcBorders><w:top w:val="double" w:sz="4" w:space="0" w:color="auto"/></w:tcBorders>"#, &dec("9.876.543.210", true)),
        tc(1418, r#"<w:tcBorders><w:top w:val="double" w:sz="4" w:space="0" w:color="auto"/></w:tcBorders>"#, &dec("8.765.432.109", true)),
    ]));
    v.push((
        "office-38-bao-cao-tai-chinh.docx",
        "office",
        "Bảng cân đối kế toán: số tiền thẳng hàng bằng tab thập phân trong ô (tab decimal), chỉ tiêu thụt lề theo cấp, hàng tổng có viền trên kép, Times New Roman 11",
        Doc {
            body: [
                center(&format!("<w:b/>{}", sz(14.0)), "BẢNG CÂN ĐỐI KẾ TOÁN"),
                center("<w:i/>", "Tại ngày 31 tháng 12 năm 2026 – Đơn vị tính: đồng"),
                para("", ""),
                tbl(&g38, true, r#"<w:tblLayout w:type="fixed"/>"#, &rows38),
                para("", ""),
                signature(11.0, "KẾ TOÁN TRƯỞNG", "Đặng Thu Trang"),
            ]
            .concat(),
            sect: sect_a4("", ""),
            styles: Some(styles("Times New Roman", 11.0, &spacing(0, 0, 264, false))),
            ..Doc::default()
        },
    ));

    // ---- 39 Gantt plan: landscape, 15 columns, shaded month cells, vertical header text.
    let mut g39 = vec![454, 4082];
    g39.extend(std::iter::repeat_n(737, 12));
    g39.push(1418);
    let t10 = sz(10.0);
    let mut hdr39 = vec![
        tc(
            454,
            r#"<w:shd w:val="clear" w:color="auto" w:fill="BDD7EE"/><w:vAlign w:val="center"/>"#,
            &center(&format!("<w:b/>{t10}"), "TT"),
        ),
        tc(
            4082,
            r#"<w:shd w:val="clear" w:color="auto" w:fill="BDD7EE"/><w:vAlign w:val="center"/>"#,
            &center(&format!("<w:b/>{t10}"), "Công việc"),
        ),
    ];
    for m in 1..=12 {
        hdr39.push(tc(737, r#"<w:shd w:val="clear" w:color="auto" w:fill="BDD7EE"/><w:textDirection w:val="btLr"/><w:vAlign w:val="center"/>"#, &center(&format!("<w:b/>{t10}"), &format!("Tháng {m}"))));
    }
    hdr39.push(tc(
        1418,
        r#"<w:shd w:val="clear" w:color="auto" w:fill="BDD7EE"/><w:vAlign w:val="center"/>"#,
        &center(&format!("<w:b/>{t10}"), "Phụ trách"),
    ));
    let mut rows39 = vec![tr(r#"<w:trHeight w:val="1021"/><w:tblHeader/>"#, &hdr39)];
    for i in 0..16usize {
        let start = (i * 5) % 9;
        let len = 2 + i % 4;
        let mut cells = vec![
            tc(454, "", &center(&t10, &(i + 1).to_string())),
            tc(
                4082,
                "",
                &left(&t10, SENT[i % SENT.len()].split(',').next().unwrap_or("")),
            ),
        ];
        for m in 0..12 {
            let fill = if m >= start && m < start + len {
                r#"<w:shd w:val="clear" w:color="auto" w:fill="5B9BD5"/>"#
            } else {
                ""
            };
            cells.push(tc(737, fill, &center(&t10, "")));
        }
        cells.push(tc(
            1418,
            "",
            &center(
                &t10,
                ["Văn phòng", "Kỹ thuật", "Kế hoạch", "Tài chính"][i % 4],
            ),
        ));
        rows39.push(tr("", &cells));
    }
    v.push((
        "office-39-ke-hoach-gantt.docx",
        "office",
        "Kế hoạch dạng Gantt khổ ngang: bảng 15 cột, ô tháng tô màu, tiêu đề tháng chữ dọc (textDirection btLr) hàng cao cố định, Times New Roman 10–13",
        Doc {
            body: [
                center(&format!("<w:b/>{}", sz(14.0)), "KẾ HOẠCH TRIỂN KHAI NĂM 2027"),
                para("", ""),
                tbl(&g39, true, r#"<w:tblLayout w:type="fixed"/>"#, &rows39),
                body(0, "Ghi chú: ô tô màu là thời gian thực hiện dự kiến."),
            ]
            .concat(),
            sect: sect_a4_landscape("", ""),
            styles: Some(styles("Times New Roman", 13.0, &spacing(0, 60, 240, false))),
            ..Doc::default()
        },
    ));

    // ---- 40 tờ khai: exact / at-least row heights, □ boxes, underscore-leader lines.
    let under = |label: &str| {
        para(
            &format!(
                r#"<w:tabs><w:tab w:val="right" w:leader="underscore" w:pos="{TEXT_W}"/></w:tabs><w:spacing w:before="120"/>"#
            ),
            &[r("", label), tab()].concat(),
        )
    };
    let mut rows40 = Vec::new();
    for (k, (label, h, rule)) in [
        ("Họ và tên", 454, "exact"),
        ("Ngày sinh", 454, "exact"),
        ("Số định danh cá nhân", 454, "exact"),
        ("Địa chỉ thường trú", 851, "atLeast"),
        ("Nghề nghiệp", 454, "exact"),
        ("Nội dung đề nghị", 1701, "atLeast"),
    ]
    .iter()
    .enumerate()
    {
        let content = if k == 5 {
            body(0, &sents(2, 3))
        } else {
            left("", "")
        };
        rows40.push(tr(
            &format!(r#"<w:trHeight w:val="{h}" w:hRule="{rule}"/>"#),
            &[
                tc(
                    2835,
                    r#"<w:vAlign w:val="center"/>"#,
                    &left("<w:b/>", label),
                ),
                tc(6520, r#"<w:vAlign w:val="center"/>"#, &content),
            ],
        ));
    }
    v.push((
        "office-40-to-khai.docx",
        "office",
        "Tờ khai: bảng hàng cao chính xác / tối thiểu (trHeight exact, atLeast), ô □ đánh dấu, dòng kẻ bằng tab dẫn gạch dưới, Times New Roman 13",
        Doc {
            body: [
                center(&format!("<w:b/>{}", sz(13.0)), "CỘNG HÒA XÃ HỘI CHỦ NGHĨA VIỆT NAM"),
                center(&format!("<w:b/>{}", sz(13.0)), "Độc lập - Tự do - Hạnh phúc"),
                para(r#"<w:spacing w:before="240" w:after="120"/><w:jc w:val="center"/>"#, &r(&format!("<w:b/>{}", sz(15.0)), "TỜ KHAI ĐỀ NGHỊ CẤP GIẤY XÁC NHẬN")),
                para(r#"<w:jc w:val="center"/>"#, &r("", "Kính gửi: Ủy ban nhân dân phường Giả Định")),
                tbl(&[2835, 6520], true, r#"<w:tblLayout w:type="fixed"/>"#, &rows40),
                para(r#"<w:spacing w:before="200"/>"#, &r("<w:b/>", "Hình thức nhận kết quả:")),
                para(r#"<w:tabs><w:tab w:val="left" w:pos="3402"/><w:tab w:val="left" w:pos="6237"/></w:tabs>"#, &[r("", "□ Trực tiếp"), tab(), r("", "□ Qua bưu điện"), tab(), r("", "□ Trực tuyến")].concat()),
                under("Số điện thoại liên hệ: "),
                under("Thư điện tử: "),
                under("Ghi chú khác: "),
                under(""),
                body(567, "Tôi xin cam đoan những thông tin kê khai trên là đúng sự thật và chịu trách nhiệm trước pháp luật về nội dung đã kê khai."),
                signature(13.0, "NGƯỜI KÊ KHAI", "Nguyễn Văn An"),
            ]
            .concat(),
            sect: sect_a4("", ""),
            styles: Some(styles("Times New Roman", 13.0, &spacing(60, 60, 276, false))),
            ..Doc::default()
        },
    ));

    v
}
