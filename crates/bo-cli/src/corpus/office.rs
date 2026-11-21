//! Vietnamese office documents for the layout-fidelity corpus (BlueDoc layout round 3):
//! administrative letters and decisions with two-column header tables (thể thức văn bản
//! hành chính), "Điều 1." articles, tables of contents with dot leaders, "Trang X/Y" footers,
//! landscape sections, text boxes, pictures in tables, long tables with repeated header rows,
//! footnotes and endnotes, Times New Roman / Arial 13–14 pt. Every name, number and
//! organisation in them is invented.

use super::{field_complex, field_simple, inline_pic, png, Doc};

/// Text width of the A4 page with the 3 cm / 1.5 cm side margins of `sect_a4` (twips).
const TEXT_W: i32 = 11906 - 1701 - 850;

const SENT: &[&str] = &[
    "Thực hiện kế hoạch công tác năm 2026, đơn vị đã triển khai đồng bộ các giải pháp nhằm nâng cao chất lượng phục vụ người dân và doanh nghiệp.",
    "Các phòng, ban chuyên môn chủ động rà soát quy trình nội bộ, cắt giảm thủ tục không cần thiết và rút ngắn thời gian giải quyết hồ sơ.",
    "Việc ứng dụng công nghệ thông tin trong quản lý văn bản, hồ sơ công việc được đẩy mạnh, tỷ lệ văn bản điện tử đạt trên chín mươi phần trăm.",
    "Công tác kiểm tra, giám sát được duy trì thường xuyên, kịp thời phát hiện và chấn chỉnh những tồn tại, hạn chế trong quá trình thực hiện nhiệm vụ.",
    "Bên cạnh kết quả đạt được, vẫn còn một số khó khăn về nguồn nhân lực, kinh phí và cơ sở vật chất cần được tháo gỡ trong thời gian tới.",
    "Đề nghị các đơn vị nghiêm túc triển khai thực hiện, định kỳ báo cáo kết quả về Văn phòng trước ngày 25 hằng tháng để tổng hợp.",
    "Trong quá trình thực hiện, nếu có vướng mắc phát sinh, các đơn vị phản ánh kịp thời để được hướng dẫn, giải quyết theo thẩm quyền.",
    "Kinh phí thực hiện được bố trí từ nguồn ngân sách được giao và các nguồn hợp pháp khác theo quy định hiện hành của pháp luật.",
];

/// `n` sentences starting at `k`, joined into one paragraph text.
fn sents(k: usize, n: usize) -> String {
    (0..n)
        .map(|i| SENT[(k + i) % SENT.len()])
        .collect::<Vec<_>>()
        .join(" ")
}

fn esc(t: &str) -> String {
    t.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// A run with run properties `rpr` (children of `w:rPr`, §17.3.2) and text `t`.
fn r(rpr: &str, t: &str) -> String {
    let props = if rpr.is_empty() {
        String::new()
    } else {
        format!("<w:rPr>{rpr}</w:rPr>")
    };
    format!(
        r#"<w:r>{props}<w:t xml:space="preserve">{}</w:t></w:r>"#,
        esc(t)
    )
}

/// A tab run (§17.3.3.32).
fn tab() -> String {
    "<w:r><w:tab/></w:r>".to_owned()
}

/// A paragraph with paragraph properties `ppr` (children of `w:pPr` in CT_PPrBase order,
/// ECMA-376 Part 1, §17.3.1.26) and runs `runs`.
fn para(ppr: &str, runs: &str) -> String {
    if ppr.is_empty() {
        format!("<w:p>{runs}</w:p>")
    } else {
        format!("<w:p><w:pPr>{ppr}</w:pPr>{runs}</w:p>")
    }
}

/// A body paragraph: justified (§17.3.1.13 jc both) with a first-line indent of `first` twips.
fn body(first: i32, t: &str) -> String {
    para(
        &format!(r#"<w:ind w:firstLine="{first}"/><w:jc w:val="both"/>"#),
        &r("", t),
    )
}

/// Font family run property (§17.3.2.26 rFonts).
fn font(f: &str) -> String {
    format!(r#"<w:rFonts w:ascii="{f}" w:hAnsi="{f}" w:cs="{f}"/>"#)
}

/// Size in points as §17.3.2.38 sz / §17.3.2.39 szCs.
fn sz(pt: f32) -> String {
    let hp = (pt * 2.0).round() as i32;
    format!(r#"<w:sz w:val="{hp}"/><w:szCs w:val="{hp}"/>"#)
}

/// Section properties (§17.6.17): A4 portrait, margins 2 / 2 / 3 / 1.5 cm (top, bottom,
/// left, right) as the Vietnamese administrative format asks. `refs` = header / footer
/// references and §17.11.11 footnotePr / §17.6.22 type before pgSz; `after` = elements
/// after pgMar (pgNumType, cols, titlePg) in CT_SectPr order.
fn sect_a4(refs: &str, after: &str) -> String {
    format!(
        r#"<w:sectPr>{refs}<w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="850" w:bottom="1134" w:left="1701" w:header="709" w:footer="709" w:gutter="0"/>{after}<w:docGrid w:linePitch="360"/></w:sectPr>"#
    )
}

/// Landscape A4 (§17.6.13 pgSz orient) with 2 cm margins.
fn sect_a4_landscape(refs: &str, after: &str) -> String {
    format!(
        r#"<w:sectPr>{refs}<w:pgSz w:w="16838" w:h="11906" w:orient="landscape"/><w:pgMar w:top="1134" w:right="1134" w:bottom="1134" w:left="1134" w:header="709" w:footer="709" w:gutter="0"/>{after}<w:docGrid w:linePitch="360"/></w:sectPr>"#
    )
}

/// An empty paragraph ending a section (§17.6.17 sectPr in pPr).
fn sect_para(sect: &str) -> String {
    format!("<w:p><w:pPr>{sect}</w:pPr></w:p>")
}

/// Styles part content: document defaults `font` `pt` with line spacing `line` (240ths,
/// auto) or exact (`exact` twips) and space after `after` twips; Normal, Table Grid,
/// headings 1–3, TOC 1–3, header / footer, foot / end notes, caption, list paragraph.
fn styles(f: &str, pt: f32, spacing: &str) -> String {
    let hp = (pt * 2.0).round() as i32;
    format!(
        r#"<w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="{f}" w:eastAsia="{f}" w:hAnsi="{f}" w:cs="{f}"/><w:sz w:val="{hp}"/><w:szCs w:val="{hp}"/><w:lang w:val="vi-VN" w:eastAsia="en-US" w:bidi="ar-SA"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr>{spacing}</w:pPr></w:pPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style><w:style w:type="character" w:default="1" w:styleId="DefaultParagraphFont"><w:name w:val="Default Paragraph Font"/><w:uiPriority w:val="1"/><w:semiHidden/><w:unhideWhenUsed/></w:style><w:style w:type="table" w:default="1" w:styleId="TableNormal"><w:name w:val="Normal Table"/><w:uiPriority w:val="99"/><w:semiHidden/><w:unhideWhenUsed/><w:tblPr><w:tblInd w:w="0" w:type="dxa"/><w:tblCellMar><w:top w:w="0" w:type="dxa"/><w:left w:w="108" w:type="dxa"/><w:bottom w:w="0" w:type="dxa"/><w:right w:w="108" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style><w:style w:type="numbering" w:default="1" w:styleId="NoList"><w:name w:val="No List"/><w:uiPriority w:val="99"/><w:semiHidden/><w:unhideWhenUsed/></w:style><w:style w:type="table" w:styleId="TableGrid"><w:name w:val="Table Grid"/><w:basedOn w:val="TableNormal"/><w:uiPriority w:val="39"/><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:tblPr><w:tblBorders><w:top w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:left w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:bottom w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:right w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:insideH w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:insideV w:val="single" w:sz="4" w:space="0" w:color="auto"/></w:tblBorders></w:tblPr></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="9"/><w:qFormat/><w:pPr><w:keepNext/><w:keepLines/><w:spacing w:before="240" w:after="120"/><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:bCs/><w:sz w:val="{h1}"/><w:szCs w:val="{h1}"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="9"/><w:unhideWhenUsed/><w:qFormat/><w:pPr><w:keepNext/><w:keepLines/><w:spacing w:before="120" w:after="60"/><w:outlineLvl w:val="1"/></w:pPr><w:rPr><w:b/><w:bCs/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading3"><w:name w:val="heading 3"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="9"/><w:unhideWhenUsed/><w:qFormat/><w:pPr><w:keepNext/><w:keepLines/><w:spacing w:before="60" w:after="60"/><w:outlineLvl w:val="2"/></w:pPr><w:rPr><w:b/><w:bCs/><w:i/><w:iCs/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="TOC1"><w:name w:val="toc 1"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="39"/><w:unhideWhenUsed/><w:pPr><w:tabs><w:tab w:val="right" w:leader="dot" w:pos="{tw}"/></w:tabs><w:spacing w:after="100"/></w:pPr><w:rPr><w:b/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="TOC2"><w:name w:val="toc 2"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="39"/><w:unhideWhenUsed/><w:pPr><w:tabs><w:tab w:val="right" w:leader="dot" w:pos="{tw}"/></w:tabs><w:spacing w:after="100"/><w:ind w:left="280"/></w:pPr></w:style><w:style w:type="paragraph" w:styleId="TOC3"><w:name w:val="toc 3"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="39"/><w:unhideWhenUsed/><w:pPr><w:tabs><w:tab w:val="right" w:leader="dot" w:pos="{tw}"/></w:tabs><w:spacing w:after="100"/><w:ind w:left="560"/></w:pPr><w:rPr><w:i/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Header"><w:name w:val="header"/><w:basedOn w:val="Normal"/><w:uiPriority w:val="99"/><w:unhideWhenUsed/><w:pPr><w:tabs><w:tab w:val="center" w:pos="4680"/><w:tab w:val="right" w:pos="{tw}"/></w:tabs><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr></w:style><w:style w:type="paragraph" w:styleId="Footer"><w:name w:val="footer"/><w:basedOn w:val="Normal"/><w:uiPriority w:val="99"/><w:unhideWhenUsed/><w:pPr><w:tabs><w:tab w:val="center" w:pos="4680"/><w:tab w:val="right" w:pos="{tw}"/></w:tabs><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr></w:style><w:style w:type="paragraph" w:styleId="FootnoteText"><w:name w:val="footnote text"/><w:basedOn w:val="Normal"/><w:uiPriority w:val="99"/><w:unhideWhenUsed/><w:pPr><w:spacing w:before="0" w:after="0" w:line="240" w:lineRule="auto"/><w:jc w:val="both"/></w:pPr><w:rPr><w:sz w:val="20"/><w:szCs w:val="20"/></w:rPr></w:style><w:style w:type="character" w:styleId="FootnoteReference"><w:name w:val="footnote reference"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="99"/><w:unhideWhenUsed/><w:rPr><w:vertAlign w:val="superscript"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="EndnoteText"><w:name w:val="endnote text"/><w:basedOn w:val="Normal"/><w:uiPriority w:val="99"/><w:unhideWhenUsed/><w:pPr><w:spacing w:before="0" w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:rPr><w:sz w:val="20"/><w:szCs w:val="20"/></w:rPr></w:style><w:style w:type="character" w:styleId="EndnoteReference"><w:name w:val="endnote reference"/><w:basedOn w:val="DefaultParagraphFont"/><w:uiPriority w:val="99"/><w:unhideWhenUsed/><w:rPr><w:vertAlign w:val="superscript"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Caption"><w:name w:val="caption"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="35"/><w:unhideWhenUsed/><w:qFormat/><w:pPr><w:spacing w:before="60" w:after="120" w:line="240" w:lineRule="auto"/><w:jc w:val="center"/></w:pPr><w:rPr><w:i/><w:iCs/><w:sz w:val="22"/><w:szCs w:val="22"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="ListParagraph"><w:name w:val="List Paragraph"/><w:basedOn w:val="Normal"/><w:uiPriority w:val="34"/><w:qFormat/><w:pPr><w:ind w:left="720"/><w:contextualSpacing/></w:pPr></w:style>"#,
        h1 = hp + 2,
        tw = TEXT_W,
    )
}

/// Paragraph spacing for `pPrDefault`: space before / after (twips) and line spacing
/// (§17.3.1.33): `line` in 240ths of a line (auto) or, with `exact`, in twips.
fn spacing(before: i32, after: i32, line: i32, exact: bool) -> String {
    let rule = if exact { "exact" } else { "auto" };
    format!(
        r#"<w:spacing w:before="{before}" w:after="{after}" w:line="{line}" w:lineRule="{rule}"/>"#
    )
}

/// No spacing around a paragraph, single lines (for header tables and signatures).
const TIGHT: &str = r#"<w:spacing w:before="0" w:after="0" w:line="240" w:lineRule="auto"/>"#;

/// A table cell (§17.4.66 tcPr) of width `w` twips with extra properties and paragraphs.
fn tc(w: i32, extra: &str, paras: &str) -> String {
    format!(r#"<w:tc><w:tcPr><w:tcW w:w="{w}" w:type="dxa"/>{extra}</w:tcPr>{paras}</w:tc>"#)
}

/// A table row (§17.4.79 trPr).
fn tr(trpr: &str, cells: &[String]) -> String {
    let props = if trpr.is_empty() {
        String::new()
    } else {
        format!("<w:trPr>{trpr}</w:trPr>")
    };
    format!("<w:tr>{props}{}</w:tr>", cells.concat())
}

/// A table on `grid` (twips); `grid_style` = with Table Grid borders, else no borders.
fn tbl(grid: &[i32], grid_style: bool, tblpr: &str, rows: &[String]) -> String {
    let g: String = grid
        .iter()
        .map(|w| format!(r#"<w:gridCol w:w="{w}"/>"#))
        .collect();
    let total: i32 = grid.iter().sum();
    let style = if grid_style {
        r#"<w:tblStyle w:val="TableGrid"/>"#
    } else {
        ""
    };
    format!(
        r#"<w:tbl><w:tblPr>{style}<w:tblW w:w="{total}" w:type="dxa"/>{tblpr}<w:tblLook w:val="04A0" w:firstRow="1" w:lastRow="0" w:firstColumn="1" w:lastColumn="0" w:noHBand="0" w:noVBand="1"/></w:tblPr><w:tblGrid>{g}</w:tblGrid>{}</w:tbl>"#,
        rows.concat()
    )
}

/// Centred tight paragraph.
fn center(rpr: &str, t: &str) -> String {
    para(&format!(r#"{TIGHT}<w:jc w:val="center"/>"#), &r(rpr, t))
}

/// The two-column header of a Vietnamese administrative document (agency, number and
/// subject left; national title, motto and place / date right), as a borderless table.
fn admin_header(pt: f32, agency: &str, unit: &str, number: &str, subject: &str) -> String {
    let small = sz(pt - 1.0);
    let b = format!("<w:b/>{}", sz(pt - 1.0));
    let left = [
        center(&small, agency),
        center(&b, unit),
        center(&small, "———"),
        center(&small, number),
        if subject.is_empty() {
            String::new()
        } else {
            center(&sz(pt - 2.0), subject)
        },
    ]
    .concat();
    let right = [
        center(
            &format!("<w:b/>{}", sz(pt - 1.0)),
            "CỘNG HÒA XÃ HỘI CHỦ NGHĨA VIỆT NAM",
        ),
        center(&format!("<w:b/>{}", sz(pt)), "Độc lập - Tự do - Hạnh phúc"),
        center(&small, "———————"),
        center(
            &format!("<w:i/>{}", sz(pt)),
            "Hà Nội, ngày 20 tháng 11 năm 2026",
        ),
    ]
    .concat();
    tbl(
        &[3686, 5669],
        false,
        r#"<w:tblLayout w:type="fixed"/>"#,
        &[tr("", &[tc(3686, "", &left), tc(5669, "", &right)])],
    )
}

/// Signature block: recipients left, title and (invented) signer right.
fn signature(pt: f32, title: &str, name: &str) -> String {
    let left = [
        para(
            TIGHT,
            &r(&format!("<w:b/><w:i/>{}", sz(pt - 2.0)), "Nơi nhận:"),
        ),
        para(TIGHT, &r(&sz(pt - 3.0), "- Như trên;")),
        para(TIGHT, &r(&sz(pt - 3.0), "- Ban Giám đốc (để b/c);")),
        para(TIGHT, &r(&sz(pt - 3.0), "- Lưu: VT, VP.")),
    ]
    .concat();
    let right = [
        center(&format!("<w:b/>{}", sz(pt)), title),
        center(&format!("<w:i/>{}", sz(pt)), "(Ký, ghi rõ họ tên)"),
        center("", ""),
        center("", ""),
        center("", ""),
        center(&format!("<w:b/>{}", sz(pt)), name),
    ]
    .concat();
    tbl(
        &[4535, 4820],
        false,
        r#"<w:tblLayout w:type="fixed"/>"#,
        &[tr("", &[tc(4535, "", &left), tc(4820, "", &right)])],
    )
}

fn page_break() -> String {
    r#"<w:p><w:r><w:br w:type="page"/></w:r></w:p>"#.to_owned()
}

/// `<w:pStyle>` for heading level `lvl`.
fn heading(lvl: u8, t: &str) -> String {
    para(&format!(r#"<w:pStyle w:val="Heading{lvl}"/>"#), &r("", t))
}

/// Footnote / endnote reference run (§17.11.14 footnoteReference, §17.11.2 endnoteReference).
fn note_ref(kind: &str, id: i32) -> String {
    let style = if kind == "footnote" {
        "FootnoteReference"
    } else {
        "EndnoteReference"
    };
    format!(
        r#"<w:r><w:rPr><w:rStyle w:val="{style}"/></w:rPr><w:{kind}Reference w:id="{id}"/></w:r>"#
    )
}

/// A footnote / endnote with one paragraph of text `t`.
fn note(kind: &str, id: i32, t: &str) -> String {
    let (ps, rs, mark) = if kind == "footnote" {
        ("FootnoteText", "FootnoteReference", "footnoteRef")
    } else {
        ("EndnoteText", "EndnoteReference", "endnoteRef")
    };
    format!(
        r#"<w:{kind} w:id="{id}"><w:p><w:pPr><w:pStyle w:val="{ps}"/></w:pPr><w:r><w:rPr><w:rStyle w:val="{rs}"/></w:rPr><w:{mark}/></w:r>{}</w:p></w:{kind}>"#,
        r("", &format!(" {t}"))
    )
}

/// A paragraph with the "Trang X/Y" page field pair (§17.16.5.48 PAGE, §17.16.5.38 NUMPAGES).
fn page_x_of_y(jc: &str, pt: f32) -> String {
    para(
        &format!(r#"<w:pStyle w:val="Footer"/><w:jc w:val="{jc}"/>"#),
        &[
            r(&sz(pt), "Trang "),
            field_complex("PAGE", "1"),
            r(&sz(pt), "/"),
            field_simple("NUMPAGES", "1"),
        ]
        .concat(),
    )
}

/// An anchored text box (wordprocessingShape `wps:wsp` with §17.17.1 txbxContent) at
/// `x`, `y` EMU from the margin / paragraph, `cx` × `cy` EMU, wrap element `wrap`
/// (§20.4.2.17 wrapSquare, §20.4.2.20 wrapTopAndBottom).
fn text_box(id: u32, at: (i64, i64), size: (i64, i64), wrap: &str, content: &str) -> String {
    let ((x, y), (cx, cy)) = (at, size);
    format!(
        r#"<w:r><w:rPr><w:noProof/></w:rPr><w:drawing><wp:anchor distT="45720" distB="45720" distL="114300" distR="114300" simplePos="0" relativeHeight="{id}" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="margin"><wp:posOffset>{x}</wp:posOffset></wp:positionH><wp:positionV relativeFrom="paragraph"><wp:posOffset>{y}</wp:posOffset></wp:positionV><wp:extent cx="{cx}" cy="{cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/>{wrap}<wp:docPr id="{id}" name="Hộp văn bản {id}"/><wp:cNvGraphicFramePr/><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:cNvSpPr txBox="1"/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill><a:ln w="9525"><a:solidFill><a:srgbClr val="000000"/></a:solidFill></a:ln></wps:spPr><wps:txbx><w:txbxContent>{content}</w:txbxContent></wps:txbx><wps:bodyPr rot="0" vert="horz" wrap="square" lIns="91440" tIns="45720" rIns="91440" bIns="45720" anchor="t" anchorCtr="0"><a:noAutofit/></wps:bodyPr></wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r>"#
    )
}

/// A numbering level: numFmt, lvlText, left indent, hanging indent, bold label.
type Lvl<'a> = (&'a str, &'a str, i32, i32, bool);

/// Amount with `.` between thousands (Vietnamese number format).
fn thousands(v: u64) -> String {
    let s = v.to_string();
    let mut out = String::new();
    for (k, ch) in s.chars().enumerate() {
        if k > 0 && (s.len() - k).is_multiple_of(3) {
            out.push('.');
        }
        out.push(ch);
    }
    out
}

/// Numbering part: abstract numbering definitions given as (id, levels), each level
/// (numFmt, lvlText, left indent, hanging, bold label), with `w:num` i+1 → abstract i
/// (ECMA-376 Part 1, §17.9.1 abstractNum, §17.9.6 lvlText).
fn numbering(abstracts: &[&[Lvl<'_>]]) -> String {
    let mut s = String::new();
    for (id, levels) in abstracts.iter().enumerate() {
        s.push_str(&format!(r#"<w:abstractNum w:abstractNumId="{id}"><w:nsid w:val="5B0C{id:04X}"/><w:multiLevelType w:val="multilevel"/>"#));
        for (lvl, (fmt, text, left, hanging, bold)) in levels.iter().enumerate() {
            let b = if *bold { "<w:b/><w:bCs/>" } else { "" };
            s.push_str(&format!(r#"<w:lvl w:ilvl="{lvl}"><w:start w:val="1"/><w:numFmt w:val="{fmt}"/><w:lvlText w:val="{text}"/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="{left}" w:hanging="{hanging}"/></w:pPr><w:rPr>{b}</w:rPr></w:lvl>"#));
        }
        s.push_str("</w:abstractNum>");
    }
    for id in 0..abstracts.len() {
        s.push_str(&format!(
            r#"<w:num w:numId="{}"><w:abstractNumId w:val="{id}"/></w:num>"#,
            id + 1
        ));
    }
    s
}

/// A numbered paragraph; `extra` = further pPr children (keepNext first: it precedes
/// numPr in CT_PPrBase, the others follow it).
fn num_para(num: i32, lvl: u8, extra: &str, runs: &str) -> String {
    let (keep, rest) = match extra.strip_prefix("<w:keepNext/>") {
        Some(rest) => ("<w:keepNext/>", rest),
        None => ("", extra),
    };
    para(
        &format!(
            r#"{keep}<w:numPr><w:ilvl w:val="{lvl}"/><w:numId w:val="{num}"/></w:numPr>{rest}"#
        ),
        runs,
    )
}

/// "Điều N. Title" article heading followed by its clauses.
fn article(n: usize, title: &str, clauses: &[String]) -> String {
    let mut s = para(
        r#"<w:keepNext/><w:spacing w:before="120"/><w:ind w:firstLine="567"/><w:jc w:val="both"/>"#,
        &[r("<w:b/>", &format!("Điều {n}. ")), r("<w:b/>", title)].concat(),
    );
    for c in clauses {
        s.push_str(&body(567, c));
    }
    s
}

#[allow(clippy::too_many_lines)]
pub(super) fn documents() -> Vec<(&'static str, &'static str, &'static str, Doc)> {
    let mut v = Vec::new();
    let tnr14 = || styles("Times New Roman", 14.0, &spacing(120, 0, 276, false));

    // ---- 01 công văn: header table, subject, "Kính gửi", body, signature.
    v.push((
        "office-01-cong-van.docx",
        "office",
        "Công văn: bảng 2 cột đầu văn bản (cơ quan / quốc hiệu), trích yếu, kính gửi, thân văn bản Times New Roman 14 giãn 1,15, khối nơi nhận / chữ ký",
        Doc {
            body: [
                admin_header(14.0, "TỔNG CÔNG TY GIẢ ĐỊNH", "CÔNG TY CỔ PHẦN ÁNH SAO", "Số: 245/CV-AS", "V/v triển khai phần mềm quản lý văn bản điện tử"),
                para(r#"<w:spacing w:before="360"/><w:ind w:left="1134"/>"#, &r("", "Kính gửi: Các phòng, ban và đơn vị trực thuộc.")),
                body(567, &sents(0, 3)),
                body(567, &sents(3, 4)),
                body(567, &sents(1, 2)),
                body(567, &sents(5, 3)),
                body(567, "Công ty đề nghị các đơn vị quan tâm, phối hợp thực hiện./."),
                signature(14.0, "GIÁM ĐỐC", "Trần Minh Khoa"),
            ]
            .concat(),
            sect: sect_a4("", ""),
            styles: Some(tnr14()),
            ..Doc::default()
        },
    ));

    // ---- 02 quyết định: articles "Điều 1." with clauses, exact 18 pt lines.
    let mut dieu = String::new();
    for n in 1..=7 {
        let clauses: Vec<String> = (0..=(n % 3))
            .map(|k| format!("{}. {}", k + 1, sents(n + k, 2)))
            .collect();
        dieu.push_str(&article(
            n,
            [
                "Ban hành kèm theo Quyết định này Quy chế quản lý văn bản.",
                "Phạm vi điều chỉnh và đối tượng áp dụng.",
                "Trách nhiệm của các phòng, ban.",
                "Chế độ báo cáo.",
                "Kinh phí thực hiện.",
                "Hiệu lực thi hành.",
                "Tổ chức thực hiện.",
            ][n - 1],
            &clauses,
        ));
    }
    v.push((
        "office-02-quyet-dinh.docx",
        "office",
        "Quyết định: đầu văn bản 2 cột, tiêu đề căn giữa, các căn cứ in nghiêng, \"Điều 1.\" … \"Điều 7.\" in đậm có khoản, giãn dòng chính xác 18 pt, Times New Roman 14",
        Doc {
            body: [
                admin_header(14.0, "CÔNG TY CỔ PHẦN ÁNH SAO", "PHÒNG HÀNH CHÍNH", "Số: 118/QĐ-AS", ""),
                para(r#"<w:spacing w:before="360" w:after="0"/><w:jc w:val="center"/>"#, &r(&format!("<w:b/>{}", sz(14.0)), "QUYẾT ĐỊNH")),
                para(r#"<w:spacing w:before="0"/><w:jc w:val="center"/>"#, &r("<w:b/>", "Về việc ban hành Quy chế quản lý văn bản điện tử")),
                para(r#"<w:spacing w:before="240"/><w:jc w:val="center"/>"#, &r("<w:b/>", "GIÁM ĐỐC CÔNG TY CỔ PHẦN ÁNH SAO")),
                para(r#"<w:ind w:firstLine="567"/><w:jc w:val="both"/>"#, &r("<w:i/>", "Căn cứ Luật Doanh nghiệp ngày 17 tháng 6 năm 2020;")),
                para(r#"<w:ind w:firstLine="567"/><w:jc w:val="both"/>"#, &r("<w:i/>", "Căn cứ Điều lệ tổ chức và hoạt động của Công ty Cổ phần Ánh Sao (giả định);")),
                para(r#"<w:ind w:firstLine="567"/><w:jc w:val="both"/>"#, &r("<w:i/>", "Theo đề nghị của Trưởng phòng Hành chính - Nhân sự.")),
                para(r#"<w:spacing w:before="240" w:after="120"/><w:jc w:val="center"/>"#, &r("<w:b/>", "QUYẾT ĐỊNH:")),
                dieu,
                signature(14.0, "GIÁM ĐỐC", "Lê Thu Hà"),
            ]
            .concat(),
            sect: sect_a4("", ""),
            styles: Some(styles("Times New Roman", 14.0, &spacing(120, 0, 360, true))),
            ..Doc::default()
        },
    ));

    // ---- 03 tờ trình: multilevel numbering I. / 1. / a), 1.5 lines, TNR 13.
    let mut items = String::new();
    for (i, title) in [
        "SỰ CẦN THIẾT",
        "MỤC TIÊU VÀ PHẠM VI",
        "NỘI DUNG ĐỀ XUẤT",
        "KINH PHÍ VÀ TIẾN ĐỘ",
    ]
    .iter()
    .enumerate()
    {
        items.push_str(&num_para(
            1,
            0,
            r#"<w:keepNext/><w:spacing w:before="120"/>"#,
            &r("<w:b/>", title),
        ));
        for k in 0..3 {
            items.push_str(&num_para(
                1,
                1,
                r#"<w:jc w:val="both"/>"#,
                &r("", &sents(i + k, 2)),
            ));
            if k == 1 {
                for m in 0..2 {
                    items.push_str(&num_para(
                        1,
                        2,
                        r#"<w:jc w:val="both"/>"#,
                        &r("", SENT[(i + k + m) % SENT.len()]),
                    ));
                }
            }
        }
    }
    v.push((
        "office-03-to-trinh.docx",
        "office",
        "Tờ trình: đánh số nhiều cấp I. / 1. / a) (numbering upperRoman, decimal, lowerLetter), giãn dòng 1,5, Times New Roman 13, nhiều trang",
        Doc {
            body: [
                admin_header(13.0, "CÔNG TY CỔ PHẦN ÁNH SAO", "BAN KẾ HOẠCH", "Số: 37/TTr-KH", "V/v đầu tư hệ thống lưu trữ"),
                para(r#"<w:spacing w:before="240"/><w:jc w:val="center"/>"#, &r(&format!("<w:b/>{}", sz(14.0)), "TỜ TRÌNH")),
                para(r#"<w:jc w:val="center"/>"#, &r("<w:b/>", "Về việc phê duyệt chủ trương đầu tư hệ thống lưu trữ dữ liệu")),
                para(r#"<w:ind w:left="1134"/>"#, &r("", "Kính gửi: Hội đồng quản trị Công ty.")),
                items,
                signature(13.0, "TRƯỞNG BAN", "Phạm Quốc Bảo"),
            ]
            .concat(),
            sect: sect_a4("", ""),
            styles: Some(styles("Times New Roman", 13.0, &spacing(0, 60, 360, false))),
            numbering: Some(numbering(&[&[
                ("upperRoman", "%1.", 567, 567, true),
                ("decimal", "%2.", 567, 283, false),
                ("lowerLetter", "%3)", 851, 284, false),
            ]])),
            ..Doc::default()
        },
    ));

    // ---- 04 báo cáo với mục lục (TOC field, dot leaders) + page number footer.
    let toc_entries: &[(u8, &str, &str)] = &[
        (1, "1. Tổng quan", "2"),
        (2, "1.1. Bối cảnh", "2"),
        (2, "1.2. Phạm vi báo cáo", "2"),
        (1, "2. Kết quả thực hiện", "3"),
        (2, "2.1. Công tác chuyên môn", "3"),
        (3, "2.1.1. Lĩnh vực văn bản", "3"),
        (2, "2.2. Công tác hành chính", "4"),
        (1, "3. Phương hướng nhiệm vụ", "4"),
        (1, "Phụ lục", "5"),
    ];
    let mut toc = para(
        r#"<w:jc w:val="center"/>"#,
        &r(&format!("<w:b/>{}", sz(14.0)), "MỤC LỤC"),
    );
    for (k, (lvl, t, pg)) in toc_entries.iter().enumerate() {
        let begin = if k == 0 {
            r#"<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> TOC \o "1-3" \h \z \u </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r>"#
        } else {
            ""
        };
        let end = if k + 1 == toc_entries.len() {
            r#"<w:r><w:fldChar w:fldCharType="end"/></w:r>"#
        } else {
            ""
        };
        toc.push_str(&para(
            &format!(r#"<w:pStyle w:val="TOC{lvl}"/>"#),
            &format!("{begin}{}{}{}{end}", r("", t), tab(), r("", pg)),
        ));
    }
    let mut report = page_break();
    for (lvl, t, _) in toc_entries {
        if *t == "Phụ lục" {
            report.push_str(&page_break());
        }
        report.push_str(&heading(*lvl, t));
        for k in 0..(if *lvl == 1 { 1 } else { 3 }) {
            report.push_str(&body(567, &sents(k + t.len(), 3)));
        }
    }
    v.push((
        "office-04-bao-cao-muc-luc.docx",
        "office",
        "Báo cáo có mục lục: trường TOC (kết quả cache) với TOC1–3 tab phải dẫn chấm, đề mục Heading 1–3 đánh số tay, chân trang số trang căn giữa",
        Doc {
            body: [toc, report].concat(),
            sect: sect_a4(r#"<w:footerReference w:type="default" r:id="rIdF1"/>"#, ""),
            styles: Some(styles("Times New Roman", 13.0, &spacing(60, 60, 288, false))),
            hf: vec![(
                "rIdF1".into(),
                "footer",
                para(r#"<w:pStyle w:val="Footer"/><w:jc w:val="center"/>"#, &field_complex("PAGE", "1")),
            )],
            ..Doc::default()
        },
    ));

    // ---- 05 trang X/Y, header, first page different.
    let mut b5 = para(
        r#"<w:jc w:val="center"/>"#,
        &r(&format!("<w:b/>{}", sz(16.0)), "BÁO CÁO QUÝ III NĂM 2026"),
    );
    for k in 0..26 {
        if k % 6 == 0 {
            b5.push_str(&heading(2, &format!("Phần {}", k / 6 + 1)));
        }
        b5.push_str(&body(0, &sents(k, 2 + k % 3)));
    }
    v.push((
        "office-05-trang-x-tren-y.docx",
        "office",
        "Đầu trang phải, chân trang \"Trang X/Y\" (PAGE / NUMPAGES), trang đầu khác (titlePg), Arial 12 giãn 1,15",
        Doc {
            body: b5,
            sect: sect_a4(
                r#"<w:headerReference w:type="default" r:id="rIdH1"/><w:footerReference w:type="default" r:id="rIdF1"/><w:footerReference w:type="first" r:id="rIdF2"/>"#,
                "<w:titlePg/>",
            ),
            styles: Some(styles("Arial", 12.0, &spacing(0, 120, 276, false))),
            hf: vec![
                ("rIdH1".into(), "header", para(r#"<w:pStyle w:val="Header"/><w:pBdr><w:bottom w:val="single" w:sz="4" w:space="1" w:color="auto"/></w:pBdr><w:jc w:val="right"/>"#, &r("<w:i/>", "Báo cáo quý III/2026 – Công ty Cổ phần Ánh Sao"))),
                ("rIdF1".into(), "footer", page_x_of_y("right", 10.0)),
                ("rIdF2".into(), "footer", para(r#"<w:pStyle w:val="Footer"/>"#, "")),
            ],
            ..Doc::default()
        },
    ));

    // ---- 06 landscape section with a wide table between portrait sections.
    let wide: Vec<i32> = vec![567, 3000, 1400, 1400, 1400, 1400, 1700, 1701, 1402];
    let mut rows6 = vec![tr(
        r#"<w:tblHeader/>"#,
        &["STT", "Nội dung công việc", "Đơn vị chủ trì", "Phối hợp", "Bắt đầu", "Kết thúc", "Kinh phí (triệu đồng)", "Sản phẩm", "Ghi chú"]
            .iter()
            .zip(&wide)
            .map(|(t, w)| tc(*w, r#"<w:shd w:val="clear" w:color="auto" w:fill="D9E2F3"/><w:vAlign w:val="center"/>"#, &center("<w:b/>", t)))
            .collect::<Vec<_>>(),
    )];
    for i in 1..=12 {
        let cells = [
            i.to_string(),
            SENT[i % SENT.len()]
                .split(',')
                .next()
                .unwrap_or("")
                .to_owned(),
            "Văn phòng".to_owned(),
            "Các phòng".to_owned(),
            format!("{:02}/2026", i % 12 + 1),
            format!("{:02}/2027", (i + 3) % 12 + 1),
            format!("{},5", 120 + i * 37),
            "Báo cáo".to_owned(),
            String::new(),
        ];
        rows6.push(tr(
            "",
            &cells
                .iter()
                .zip(&wide)
                .enumerate()
                .map(|(k, (t, w))| {
                    let jc = if k == 6 {
                        "right"
                    } else if k == 1 {
                        "both"
                    } else {
                        "center"
                    };
                    tc(
                        *w,
                        "",
                        &para(&format!(r#"<w:jc w:val="{jc}"/>"#), &r("", t)),
                    )
                })
                .collect::<Vec<_>>(),
        ));
    }
    v.push((
        "office-06-ngang-bang-rong.docx",
        "office",
        "Section dọc → ngang (orient landscape, bảng 9 cột có hàng tiêu đề tô nền) → dọc, Times New Roman 13",
        Doc {
            body: [
                heading(1, "KẾ HOẠCH TRIỂN KHAI"),
                body(567, &sents(0, 4)),
                body(567, &sents(4, 3)),
                sect_para(&sect_a4("", "")),
                para(r#"<w:jc w:val="center"/>"#, &r("<w:b/>", "Bảng 1. Danh mục nhiệm vụ năm 2026")),
                tbl(&wide, true, r#"<w:tblLayout w:type="fixed"/>"#, &rows6),
                para("", ""),
                sect_para(&sect_a4_landscape(r#"<w:type w:val="nextPage"/>"#, "")),
                heading(1, "TỔ CHỨC THỰC HIỆN"),
                body(567, &sents(2, 5)),
            ]
            .concat(),
            sect: sect_a4(r#"<w:type w:val="nextPage"/>"#, ""),
            styles: Some(styles("Times New Roman", 13.0, &spacing(60, 60, 276, false))),
            ..Doc::default()
        },
    ));

    // ---- 07 text boxes (square and top-and-bottom wrap).
    let box_text = |t: &str| {
        [
            para(TIGHT, &r(&format!("<w:b/>{}", sz(12.0)), "Ghi chú")),
            para(&format!(r#"{TIGHT}<w:jc w:val="both"/>"#), &r(&sz(12.0), t)),
        ]
        .concat()
    };
    v.push((
        "office-07-hop-van-ban.docx",
        "office",
        "Hộp văn bản (wps:txbx) bao chữ vuông bên phải và trên-dưới, chữ chảy quanh hộp, Times New Roman 13",
        Doc {
            body: [
                heading(1, "THÔNG TIN DỰ ÁN"),
                para(
                    r#"<w:ind w:firstLine="567"/><w:jc w:val="both"/>"#,
                    &[text_box(11, (3_600_000, 0), (2_160_000, 1_440_000), r#"<wp:wrapSquare wrapText="bothSides"/>"#, &box_text("Hồ sơ được tiếp nhận trực tuyến qua cổng dịch vụ công.")), r("", &sents(0, 5))].concat(),
                ),
                body(567, &sents(5, 4)),
                para(
                    r#"<w:ind w:firstLine="567"/><w:jc w:val="both"/>"#,
                    &[text_box(12, (720_000, 0), (4_320_000, 720_000), "<wp:wrapTopAndBottom/>", &box_text("Thời hạn nộp báo cáo: trước ngày 25 hằng tháng.")), r("", &sents(2, 4))].concat(),
                ),
                body(567, &sents(6, 4)),
                body(567, &sents(1, 3)),
            ]
            .concat(),
            sect: sect_a4("", ""),
            styles: Some(styles("Times New Roman", 13.0, &spacing(60, 60, 276, false))),
            ..Doc::default()
        },
    ));

    // ---- 08 pictures inline in table cells with captions.
    let colors = [
        [0x1F, 0x6F, 0xB2],
        [0x2E, 0x8B, 0x57],
        [0xC2, 0x57, 0x1A],
        [0x7A, 0x3E, 0x9D],
        [0xB0, 0x8D, 0x1A],
        [0x44, 0x44, 0x44],
    ];
    let mut cells8 = Vec::new();
    for (k, _) in colors.iter().enumerate() {
        let id = 21 + k as u32;
        let pic = para(
            r#"<w:spacing w:after="0"/><w:jc w:val="center"/>"#,
            &inline_pic(
                id,
                &format!("rIdP{k}"),
                1_620_000,
                1_080_000 + (k as i64 % 2) * 360_000,
                "",
                "",
            ),
        );
        let cap = para(
            r#"<w:pStyle w:val="Caption"/>"#,
            &r(
                "",
                &format!("Hình {}. Ảnh hiện trạng khu vực {}", k + 1, k + 1),
            ),
        );
        cells8.push(tc(
            3118,
            r#"<w:vAlign w:val="center"/>"#,
            &[pic, cap].concat(),
        ));
    }
    let mut rows8 = Vec::new();
    for ch in cells8.chunks(3) {
        rows8.push(tr("<w:cantSplit/>", ch));
    }
    v.push((
        "office-08-anh-trong-bang.docx",
        "office",
        "Ảnh nội dòng trong ô bảng (bảng 3 cột × 2 hàng, chú thích Caption dưới ảnh, cantSplit), bảng ảnh + mô tả hai cột",
        Doc {
            body: [
                heading(1, "HÌNH ẢNH KHẢO SÁT"),
                body(567, &sents(3, 3)),
                tbl(&[3118, 3118, 3119], true, r#"<w:tblLayout w:type="fixed"/>"#, &rows8),
                body(567, &sents(1, 2)),
                tbl(
                    &[2835, 6520],
                    true,
                    "",
                    &[
                        tr("", &[tc(2835, r#"<w:vAlign w:val="center"/>"#, &para(r#"<w:jc w:val="center"/>"#, &inline_pic(31, "rIdP0", 1_440_000, 1_080_000, "", ""))), tc(6520, "", &[para(TIGHT, &r("<w:b/>", "Khu vực 1")), para(r#"<w:jc w:val="both"/>"#, &r("", &sents(0, 3)))].concat())]),
                        tr("", &[tc(2835, r#"<w:vAlign w:val="center"/>"#, &para(r#"<w:jc w:val="center"/>"#, &inline_pic(32, "rIdP1", 1_440_000, 1_440_000, "", ""))), tc(6520, "", &[para(TIGHT, &r("<w:b/>", "Khu vực 2")), para(r#"<w:jc w:val="both"/>"#, &r("", &sents(4, 2)))].concat())]),
                    ],
                ),
            ]
            .concat(),
            sect: sect_a4("", ""),
            styles: Some(styles("Times New Roman", 13.0, &spacing(60, 60, 276, false))),
            images: colors
                .iter()
                .enumerate()
                .map(|(k, c)| (format!("rIdP{k}"), png(24, 16, *c)))
                .collect(),
            ..Doc::default()
        },
    ));

    // ---- 09 long table with two repeated header rows.
    let g9 = [624, 3402, 1134, 1134, 1531, 1530];
    let hdr9 = |t: &str, w: i32, extra: &str| {
        tc(
            w,
            &format!(
                r#"{extra}<w:shd w:val="clear" w:color="auto" w:fill="E7E6E6"/><w:vAlign w:val="center"/>"#
            ),
            &center("<w:b/>", t),
        )
    };
    let mut rows9 = vec![
        tr(
            "<w:tblHeader/>",
            &[
                hdr9("STT", 624, r#"<w:vMerge w:val="restart"/>"#),
                hdr9(
                    "Tên hàng hoá, dịch vụ",
                    3402,
                    r#"<w:vMerge w:val="restart"/>"#,
                ),
                hdr9("Đơn vị tính", 1134, r#"<w:vMerge w:val="restart"/>"#),
                hdr9("Số lượng", 1134, r#"<w:vMerge w:val="restart"/>"#),
                hdr9("Giá trị (đồng)", 3061, r#"<w:gridSpan w:val="2"/>"#),
            ],
        ),
        tr(
            "<w:tblHeader/>",
            &[
                hdr9("", 624, "<w:vMerge/>"),
                hdr9("", 3402, "<w:vMerge/>"),
                hdr9("", 1134, "<w:vMerge/>"),
                hdr9("", 1134, "<w:vMerge/>"),
                hdr9("Đơn giá", 1531, ""),
                hdr9("Thành tiền", 1530, ""),
            ],
        ),
    ];
    let goods = [
        "Giấy in A4 định lượng 70 g/m²",
        "Mực in laser đen",
        "Bìa hồ sơ nhựa",
        "Bút bi xanh",
        "Ổ cứng ngoài 2 TB dùng sao lưu dữ liệu định kỳ của các phòng",
        "Kẹp tài liệu",
        "Sổ ghi chép bìa cứng",
        "Băng dính trong",
        "Máy tính bỏ túi",
        "Ghim dập",
    ];
    for i in 0..64usize {
        let q = 3 + (i * 7) % 40;
        let price = 15_000 + (i * 3_571) % 480_000;
        let total = q * price;
        let money = |v: usize| thousands(v as u64);
        rows9.push(tr(
            "",
            &[
                tc(624, "", &center("", &(i + 1).to_string())),
                tc(3402, "", &para(TIGHT, &r("", goods[i % goods.len()]))),
                tc(1134, "", &center("", ["Ram", "Hộp", "Cái", "Chiếc"][i % 4])),
                tc(1134, "", &center("", &q.to_string())),
                tc(
                    1531,
                    "",
                    &para(
                        &format!(r#"{TIGHT}<w:jc w:val="right"/>"#),
                        &r("", &money(price)),
                    ),
                ),
                tc(
                    1530,
                    "",
                    &para(
                        &format!(r#"{TIGHT}<w:jc w:val="right"/>"#),
                        &r("", &money(total)),
                    ),
                ),
            ],
        ));
    }
    v.push((
        "office-09-bang-dai-lap-tieu-de.docx",
        "office",
        "Bảng dài 64 hàng qua nhiều trang, 2 hàng tiêu đề lặp lại (tblHeader, vMerge, gridSpan), số căn phải, Times New Roman 12",
        Doc {
            body: [
                para(r#"<w:jc w:val="center"/>"#, &r(&format!("<w:b/>{}", sz(14.0)), "BẢNG KÊ VẬT TƯ VĂN PHÒNG PHẨM")),
                body(567, &sents(0, 2)),
                tbl(&g9, true, r#"<w:tblLayout w:type="fixed"/>"#, &rows9),
                body(567, &sents(5, 2)),
            ]
            .concat(),
            sect: sect_a4("", ""),
            styles: Some(styles("Times New Roman", 12.0, &spacing(60, 60, 276, false))),
            ..Doc::default()
        },
    ));

    // ---- 10 footnotes + endnotes.
    let mut b10 = heading(1, "CƠ SỞ PHÁP LÝ VÀ THỰC TIỄN");
    let mut fns = String::new();
    let mut ens = String::new();
    for k in 0..12 {
        let mut runs = r("", &sents(k, 2));
        if k % 3 == 2 {
            let id = (k / 3 + 1) as i32;
            runs.push_str(&note_ref("endnote", id));
            ens.push_str(&note("endnote", id, &sents(k, 1)));
        } else {
            let id = (k / 3 * 2 + k % 3 + 1) as i32;
            runs.push_str(&note_ref("footnote", id));
            fns.push_str(&note(
                "footnote",
                id,
                &format!(
                    "Văn bản giả định số {}/2026, trích {}.",
                    10 + id,
                    SENT[k % SENT.len()].split(',').next().unwrap_or("")
                ),
            ));
        }
        runs.push_str(&r("", &format!(" {}", SENT[(k + 3) % SENT.len()])));
        b10.push_str(&para(
            r#"<w:ind w:firstLine="567"/><w:jc w:val="both"/>"#,
            &runs,
        ));
    }
    v.push((
        "office-10-chu-thich-cuoi-trang-va-van-ban.docx",
        "office",
        "8 chú thích cuối trang và 4 chú thích cuối văn bản xen kẽ, Times New Roman 14 giãn 1,3",
        Doc {
            body: b10,
            sect: sect_a4("", ""),
            styles: Some(styles(
                "Times New Roman",
                14.0,
                &spacing(120, 0, 312, false),
            )),
            footnotes: Some(fns),
            endnotes: Some(ens),
            ..Doc::default()
        },
    ));

    // ---- 11 mixed Times New Roman / Arial 13–14 pt.
    let mut b11 = String::new();
    for k in 0..14 {
        let (f1, f2) = if k % 2 == 0 {
            ("Times New Roman", "Arial")
        } else {
            ("Arial", "Times New Roman")
        };
        let s1 = if k % 3 == 0 { 14.0 } else { 13.0 };
        let s2 = if k % 3 == 1 { 14.0 } else { 13.0 };
        if k % 5 == 0 {
            b11.push_str(&para(
                r#"<w:keepNext/><w:spacing w:before="240"/>"#,
                &r(
                    &format!("{}<w:b/>{}", font("Arial"), sz(14.0)),
                    &format!("{}. NỘI DUNG THỨ {}", k / 5 + 1, k / 5 + 1),
                ),
            ));
        }
        b11.push_str(&para(
            r#"<w:ind w:firstLine="567"/><w:jc w:val="both"/>"#,
            &[
                r(&format!("{}{}", font(f1), sz(s1)), &sents(k, 1)),
                r(&format!("{}<w:b/>{}", font(f2), sz(s2)), " Nhấn mạnh: "),
                r(
                    &format!("{}<w:i/>{}", font(f2), sz(s2)),
                    SENT[(k + 2) % SENT.len()],
                ),
                r(
                    &format!("{}{}", font(f1), sz(s1)),
                    &format!(" {}", SENT[(k + 5) % SENT.len()]),
                ),
            ]
            .concat(),
        ));
    }
    v.push((
        "office-11-phong-hon-hop.docx",
        "office",
        "Phông Times New Roman / Arial 13–14 pt xen kẽ trong đoạn và giữa các đoạn, đậm / nghiêng, đề mục Arial 14",
        Doc {
            body: b11,
            sect: sect_a4("", ""),
            styles: Some(styles("Times New Roman", 13.0, &spacing(60, 60, 276, false))),
            ..Doc::default()
        },
    ));

    // ---- 12 hợp đồng: tab-aligned party block, articles, 1.15 lines + 6 pt after.
    let party = |label: &str, name: &str, rep: &str| {
        let tabs = r#"<w:tabs><w:tab w:val="left" w:pos="2268"/><w:tab w:val="left" w:pos="5670"/></w:tabs><w:spacing w:before="0" w:after="0"/>"#;
        [
            para(
                r#"<w:keepNext/><w:spacing w:before="120" w:after="0"/>"#,
                &r("<w:b/>", &format!("{label}: {name}")),
            ),
            para(
                tabs,
                &[
                    r("", "Địa chỉ:"),
                    tab(),
                    r("", "Số 12 phố Giả Định, quận Ba Đình, Hà Nội"),
                ]
                .concat(),
            ),
            para(
                tabs,
                &[
                    r("", "Điện thoại:"),
                    tab(),
                    r("", "024 3999 0000"),
                    tab(),
                    r("", "Mã số thuế: 0101234567"),
                ]
                .concat(),
            ),
            para(
                tabs,
                &[
                    r("", "Đại diện:"),
                    tab(),
                    r("<w:b/>", rep),
                    tab(),
                    r("", "Chức vụ: Giám đốc"),
                ]
                .concat(),
            ),
        ]
        .concat()
    };
    let mut b12 = [
        center(&format!("<w:b/>{}", sz(13.0)), "CỘNG HÒA XÃ HỘI CHỦ NGHĨA VIỆT NAM"),
        center(&format!("<w:b/>{}", sz(14.0)), "Độc lập - Tự do - Hạnh phúc"),
        para(r#"<w:spacing w:before="360" w:after="0"/><w:jc w:val="center"/>"#, &r(&format!("<w:b/>{}", sz(16.0)), "HỢP ĐỒNG CUNG CẤP DỊCH VỤ")),
        center(&format!("<w:i/>{}", sz(13.0)), "Số: 09/2026/HĐDV"),
        para(r#"<w:spacing w:before="240"/><w:ind w:firstLine="567"/><w:jc w:val="both"/>"#, &r("<w:i/>", "Căn cứ Bộ luật Dân sự ngày 24 tháng 11 năm 2015; căn cứ nhu cầu và khả năng của hai bên,")),
        body(567, "Hôm nay, ngày 20 tháng 11 năm 2026, tại trụ sở Công ty Cổ phần Ánh Sao, chúng tôi gồm:"),
        party("BÊN A (Bên sử dụng dịch vụ)", "CÔNG TY CỔ PHẦN ÁNH SAO", "Ông Trần Minh Khoa"),
        party("BÊN B (Bên cung cấp dịch vụ)", "CÔNG TY TNHH SAO MAI", "Bà Nguyễn Thị Lan"),
        body(567, "Hai bên thống nhất ký kết hợp đồng với các điều khoản sau:"),
    ]
    .concat();
    for n in 1..=8 {
        let clauses: Vec<String> = (0..2 + n % 2)
            .map(|k| format!("{}.{}. {}", n, k + 1, sents(n * 2 + k, 1 + k % 2)))
            .collect();
        b12.push_str(&article(
            n,
            [
                "Nội dung dịch vụ",
                "Giá trị hợp đồng và phương thức thanh toán",
                "Thời gian thực hiện",
                "Quyền và nghĩa vụ của Bên A",
                "Quyền và nghĩa vụ của Bên B",
                "Bảo mật thông tin",
                "Giải quyết tranh chấp",
                "Điều khoản chung",
            ][n - 1],
            &clauses,
        ));
    }
    b12.push_str(&tbl(
        &[4677, 4678],
        false,
        r#"<w:tblLayout w:type="fixed"/>"#,
        &[tr(
            "<w:cantSplit/>",
            &[
                tc(
                    4677,
                    "",
                    &[
                        center("<w:b/>", "ĐẠI DIỆN BÊN A"),
                        center("<w:i/>", "(Ký, đóng dấu)"),
                        center("", ""),
                        center("", ""),
                        center("<w:b/>", "Trần Minh Khoa"),
                    ]
                    .concat(),
                ),
                tc(
                    4678,
                    "",
                    &[
                        center("<w:b/>", "ĐẠI DIỆN BÊN B"),
                        center("<w:i/>", "(Ký, đóng dấu)"),
                        center("", ""),
                        center("", ""),
                        center("<w:b/>", "Nguyễn Thị Lan"),
                    ]
                    .concat(),
                ),
            ],
        )],
    ));
    v.push((
        "office-12-hop-dong.docx",
        "office",
        "Hợp đồng: quốc hiệu căn giữa, thông tin hai bên thẳng cột bằng tab trái, 8 \"Điều\" có khoản 1.1, giãn 1,15 + sau đoạn 6 pt (dòng bội số ở đáy trang), chữ ký hai bên",
        Doc {
            body: b12,
            sect: sect_a4("", ""),
            styles: Some(styles("Times New Roman", 13.0, &spacing(0, 120, 276, false))),
            ..Doc::default()
        },
    ));

    // ---- 13 biên bản: participants table with merged cells, 3-column signatures.
    let g13 = [680, 3118, 2835, 2722];
    let mut rows13 = vec![tr(
        "<w:tblHeader/>",
        &["TT", "Họ và tên", "Chức vụ", "Đơn vị"]
            .iter()
            .zip(g13)
            .map(|(t, w)| tc(w, "", &center("<w:b/>", t)))
            .collect::<Vec<_>>(),
    )];
    let people = [
        "Trần Minh Khoa",
        "Lê Thu Hà",
        "Phạm Quốc Bảo",
        "Nguyễn Thị Lan",
        "Đỗ Văn Hùng",
        "Vũ Ngọc Ánh",
    ];
    for (i, n) in people.iter().enumerate() {
        // Unit cells merged vertically per company (§17.4.85 vMerge).
        let unit = match i {
            0 | 3 => tc(
                2722,
                r#"<w:vMerge w:val="restart"/><w:vAlign w:val="center"/>"#,
                &center(
                    "",
                    if i == 0 {
                        "Công ty Cổ phần Ánh Sao"
                    } else {
                        "Công ty TNHH Sao Mai"
                    },
                ),
            ),
            _ => tc(2722, "<w:vMerge/>", &center("", "")),
        };
        rows13.push(tr(
            "",
            &[
                tc(680, "", &center("", &(i + 1).to_string())),
                tc(3118, "", &para(TIGHT, &r("", n))),
                tc(
                    2835,
                    "",
                    &para(
                        TIGHT,
                        &r(
                            "",
                            [
                                "Giám đốc",
                                "Kế toán trưởng",
                                "Trưởng ban",
                                "Giám đốc",
                                "Kỹ sư",
                                "Chuyên viên",
                            ][i],
                        ),
                    ),
                ),
                unit,
            ],
        ));
    }
    rows13.push(tr(
        "",
        &[tc(
            680 + 3118 + 2835 + 2722,
            r#"<w:gridSpan w:val="4"/>"#,
            &para(TIGHT, &r("<w:i/>", "Tổng số: 06 người tham dự.")),
        )],
    ));
    let sig3 = |t: &str, n: &str| {
        tc(
            3118,
            "",
            &[
                center("<w:b/>", t),
                center("", ""),
                center("", ""),
                center("", ""),
                center("<w:b/>", n),
            ]
            .concat(),
        )
    };
    v.push((
        "office-13-bien-ban.docx",
        "office",
        "Biên bản họp: bảng thành phần có ô gộp dọc (vMerge) và hàng gộp ngang (gridSpan), nội dung đánh số tay, chữ ký 3 cột, Times New Roman 13",
        Doc {
            body: [
                admin_header(13.0, "CÔNG TY CỔ PHẦN ÁNH SAO", "BAN DỰ ÁN", "Số: 05/BB-DA", ""),
                para(r#"<w:spacing w:before="240"/><w:jc w:val="center"/>"#, &r(&format!("<w:b/>{}", sz(14.0)), "BIÊN BẢN NGHIỆM THU")),
                center("<w:b/>", "Hạng mục: Hệ thống quản lý văn bản điện tử"),
                body(567, "Hôm nay, vào lúc 09 giờ 00 ngày 20 tháng 11 năm 2026, tại phòng họp tầng 3, chúng tôi gồm:"),
                tbl(&g13, true, r#"<w:tblLayout w:type="fixed"/>"#, &rows13),
                para(r#"<w:spacing w:before="120"/>"#, &r("<w:b/>", "I. NỘI DUNG NGHIỆM THU")),
                body(567, &format!("1. {}", sents(0, 2))),
                body(567, &format!("2. {}", sents(2, 3))),
                body(567, &format!("3. {}", sents(5, 2))),
                para(r#"<w:spacing w:before="120"/>"#, &r("<w:b/>", "II. KẾT LUẬN")),
                body(567, &sents(3, 3)),
                body(567, "Biên bản được lập thành 04 bản có giá trị pháp lý như nhau, mỗi bên giữ 02 bản./."),
                tbl(&[3118, 3118, 3119], false, r#"<w:tblLayout w:type="fixed"/>"#, &[tr("<w:cantSplit/>", &[sig3("BÊN GIAO", "Nguyễn Thị Lan"), sig3("BÊN NHẬN", "Trần Minh Khoa"), sig3("THƯ KÝ", "Vũ Ngọc Ánh")])]),
            ]
            .concat(),
            sect: sect_a4("", ""),
            styles: Some(styles("Times New Roman", 13.0, &spacing(60, 60, 276, false))),
            ..Doc::default()
        },
    ));

    // ---- 14 giấy mời / phiếu đăng ký: dot-leader form lines and check boxes.
    let line14 = |label: &str| {
        para(
            &format!(
                r#"<w:tabs><w:tab w:val="right" w:leader="dot" w:pos="{TEXT_W}"/></w:tabs><w:spacing w:before="60" w:after="60"/>"#
            ),
            &[r("", label), tab()].concat(),
        )
    };
    let line14b = |a: &str, b: &str| {
        para(
            &format!(
                r#"<w:tabs><w:tab w:val="right" w:leader="dot" w:pos="5103"/><w:tab w:val="right" w:leader="dot" w:pos="{TEXT_W}"/></w:tabs><w:spacing w:before="60" w:after="60"/>"#
            ),
            &[r("", a), tab(), r("", b), tab()].concat(),
        )
    };
    v.push((
        "office-14-phieu-dang-ky.docx",
        "office",
        "Phiếu đăng ký: dòng điền thông tin bằng tab phải dẫn chấm (một và hai cột), ô đánh dấu □, Times New Roman 14",
        Doc {
            body: [
                center(&format!("<w:b/>{}", sz(13.0)), "CỘNG HÒA XÃ HỘI CHỦ NGHĨA VIỆT NAM"),
                center(&format!("<w:b/>{}", sz(14.0)), "Độc lập - Tự do - Hạnh phúc"),
                para(r#"<w:spacing w:before="360" w:after="240"/><w:jc w:val="center"/>"#, &r(&format!("<w:b/>{}", sz(16.0)), "PHIẾU ĐĂNG KÝ THAM DỰ HỘI NGHỊ")),
                line14("Họ và tên: "),
                line14b("Ngày sinh: ", " Giới tính: "),
                line14("Đơn vị công tác: "),
                line14b("Chức vụ: ", " Điện thoại: "),
                line14("Địa chỉ thư điện tử: "),
                para(r#"<w:spacing w:before="120"/>"#, &r("<w:b/>", "Nội dung đăng ký tham dự:")),
                para(r#"<w:tabs><w:tab w:val="left" w:pos="4536"/></w:tabs><w:ind w:left="567"/>"#, &[r("", "□ Phiên toàn thể"), tab(), r("", "□ Phiên chuyên đề 1")].concat()),
                para(r#"<w:tabs><w:tab w:val="left" w:pos="4536"/></w:tabs><w:ind w:left="567"/>"#, &[r("", "□ Phiên chuyên đề 2"), tab(), r("", "□ Tham quan thực tế")].concat()),
                para(r#"<w:spacing w:before="120"/>"#, &r("<w:b/>", "Ý kiến đề xuất:")),
                line14(""),
                line14(""),
                line14(""),
                para(r#"<w:spacing w:before="240"/><w:ind w:left="4536"/><w:jc w:val="center"/>"#, &r("<w:i/>", "…………, ngày …… tháng …… năm 2026")),
                para(r#"<w:ind w:left="4536"/><w:jc w:val="center"/>"#, &r("<w:b/>", "NGƯỜI ĐĂNG KÝ")),
                body(0, &format!("Ghi chú: {}", sents(5, 2))),
            ]
            .concat(),
            sect: sect_a4("", ""),
            styles: Some(styles("Times New Roman", 14.0, &spacing(0, 60, 276, false))),
            ..Doc::default()
        },
    ));

    // ---- 15 kế hoạch: legal multilevel numbering 1. / 1.1. / 1.1.1., Arial 13.
    let mut b15 = para(
        r#"<w:jc w:val="center"/>"#,
        &r(
            &format!("<w:b/>{}", sz(15.0)),
            "KẾ HOẠCH CHUYỂN ĐỔI SỐ GIAI ĐOẠN 2026 – 2030",
        ),
    );
    for i in 0..4 {
        b15.push_str(&num_para(
            1,
            0,
            r#"<w:keepNext/><w:spacing w:before="180"/>"#,
            &r(
                "<w:b/>",
                [
                    "Mục tiêu",
                    "Nhiệm vụ trọng tâm",
                    "Giải pháp thực hiện",
                    "Tổ chức thực hiện",
                ][i],
            ),
        ));
        for k in 0..3 {
            b15.push_str(&num_para(
                1,
                1,
                r#"<w:keepNext/>"#,
                &r("<w:b/><w:i/>", &format!("Nội dung {}.{}", i + 1, k + 1)),
            ));
            b15.push_str(&num_para(
                1,
                2,
                r#"<w:jc w:val="both"/>"#,
                &r("", &sents(i + k, 2)),
            ));
            if (i + k) % 2 == 0 {
                b15.push_str(&num_para(
                    1,
                    2,
                    r#"<w:jc w:val="both"/>"#,
                    &r("", &sents(i + k + 3, 1)),
                ));
            }
        }
    }
    v.push((
        "office-15-ke-hoach-danh-so-phap-ly.docx",
        "office",
        "Kế hoạch: đánh số nhiều cấp kiểu 1. / 1.1. / 1.1.1. (lvlText %1.%2.%3.), Arial 13, giãn 1,2",
        Doc {
            body: b15,
            sect: sect_a4("", ""),
            styles: Some(styles("Arial", 13.0, &spacing(60, 60, 288, false))),
            numbering: Some(numbering(&[&[
                ("decimal", "%1.", 425, 425, true),
                ("decimal", "%1.%2.", 851, 567, false),
                ("decimal", "%1.%2.%3.", 1418, 709, false),
            ]])),
            ..Doc::default()
        },
    ));

    // ---- 16 thông báo: full-width title, two balanced columns (continuous sections).
    let mut cols16 = String::new();
    for k in 0..9 {
        if k % 3 == 0 {
            cols16.push_str(&para(
                r#"<w:keepNext/><w:spacing w:before="120"/>"#,
                &r(
                    "<w:b/>",
                    &format!("{}. Tin hoạt động {}", k / 3 + 1, k / 3 + 1),
                ),
            ));
        }
        cols16.push_str(&body(284, &sents(k, 2 + k % 2)));
    }
    v.push((
        "office-16-ban-tin-hai-cot.docx",
        "office",
        "Bản tin: tiêu đề toàn trang, section liên tục 2 cột có đường kẻ giữa (cols num 2 sep), section liên tục trở lại 1 cột, Times New Roman 12",
        Doc {
            body: [
                para(r#"<w:jc w:val="center"/>"#, &r(&format!("<w:b/>{}", sz(18.0)), "BẢN TIN NỘI BỘ SỐ 11/2026")),
                body(0, &sents(0, 2)),
                sect_para(&sect_a4("", r#"<w:cols w:space="708"/>"#)),
                cols16,
                sect_para(&sect_a4(r#"<w:type w:val="continuous"/>"#, r#"<w:cols w:num="2" w:sep="1" w:space="567"/>"#)),
                para(r#"<w:spacing w:before="240"/><w:jc w:val="center"/>"#, &r("<w:i/>", "Ban biên tập Bản tin – Văn phòng Công ty")),
                body(0, &sents(4, 2)),
            ]
            .concat(),
            sect: sect_a4(r#"<w:type w:val="continuous"/>"#, r#"<w:cols w:space="708"/>"#),
            styles: Some(styles("Times New Roman", 12.0, &spacing(0, 80, 264, false))),
            ..Doc::default()
        },
    ));

    // ---- 17 bảng lương: landscape, Arial 10, 11 columns, two-level header, totals.
    let g17 = [
        454, 2268, 1418, 1134, 1247, 1247, 1134, 1247, 1134, 1247, 1040,
    ];
    let h17 = |t: &str, w: i32, extra: &str| {
        tc(
            w,
            &format!(r#"{extra}<w:vAlign w:val="center"/>"#),
            &center("<w:b/>", t),
        )
    };
    let mut rows17 = vec![
        tr(
            "<w:tblHeader/>",
            &[
                h17("TT", 454, r#"<w:vMerge w:val="restart"/>"#),
                h17("Họ và tên", 2268, r#"<w:vMerge w:val="restart"/>"#),
                h17("Chức danh", 1418, r#"<w:vMerge w:val="restart"/>"#),
                h17("Hệ số", 1134, r#"<w:vMerge w:val="restart"/>"#),
                h17("Thu nhập", 1247 + 1247 + 1134, r#"<w:gridSpan w:val="3"/>"#),
                h17(
                    "Các khoản khấu trừ",
                    1247 + 1134,
                    r#"<w:gridSpan w:val="2"/>"#,
                ),
                h17("Thực lĩnh", 1247, r#"<w:vMerge w:val="restart"/>"#),
                h17("Ký nhận", 1040, r#"<w:vMerge w:val="restart"/>"#),
            ],
        ),
        tr(
            "<w:tblHeader/>",
            &[
                h17("", 454, "<w:vMerge/>"),
                h17("", 2268, "<w:vMerge/>"),
                h17("", 1418, "<w:vMerge/>"),
                h17("", 1134, "<w:vMerge/>"),
                h17("Lương cơ bản", 1247, ""),
                h17("Phụ cấp", 1247, ""),
                h17("Thưởng", 1134, ""),
                h17("BHXH, BHYT", 1247, ""),
                h17("Thuế TNCN", 1134, ""),
                h17("", 1247, "<w:vMerge/>"),
                h17("", 1040, "<w:vMerge/>"),
            ],
        ),
    ];
    let given = [
        "Minh", "Thu", "Quốc", "Lan", "Hùng", "Ánh", "Bảo", "Khoa", "Hà", "Ngọc",
    ];
    let family = ["Trần", "Lê", "Phạm", "Nguyễn", "Đỗ", "Vũ", "Hoàng", "Bùi"];
    let fmt = thousands;
    let mut sum = 0u64;
    for i in 0..28u64 {
        let base = 4_960_000 + (i * 731_000) % 9_000_000;
        let allow = 500_000 + (i * 137_000) % 2_000_000;
        let bonus = (i % 4) * 1_000_000;
        let ins = base * 105 / 1000;
        let tax = (base + allow + bonus).saturating_sub(11_000_000) / 20;
        let net = base + allow + bonus - ins - tax;
        sum += net;
        let rt = |w: i32, v: u64| {
            tc(
                w,
                "",
                &para(&format!(r#"{TIGHT}<w:jc w:val="right"/>"#), &r("", &fmt(v))),
            )
        };
        rows17.push(tr(
            "",
            &[
                tc(454, "", &center("", &(i + 1).to_string())),
                tc(
                    2268,
                    "",
                    &para(
                        TIGHT,
                        &r(
                            "",
                            &format!(
                                "{} Văn {}",
                                family[i as usize % family.len()],
                                given[(i as usize * 3) % given.len()]
                            ),
                        ),
                    ),
                ),
                tc(
                    1418,
                    "",
                    &para(
                        TIGHT,
                        &r(
                            "",
                            ["Chuyên viên", "Kế toán", "Kỹ sư", "Nhân viên"][i as usize % 4],
                        ),
                    ),
                ),
                tc(
                    1134,
                    "",
                    &center("", &format!("{},{:02}", 2 + i % 4, (i * 33) % 100)),
                ),
                rt(1247, base),
                rt(1247, allow),
                rt(1134, bonus),
                rt(1247, ins),
                rt(1134, tax),
                rt(1247, net),
                tc(1040, "", &para(TIGHT, "")),
            ],
        ));
    }
    rows17.push(tr(
        "",
        &[
            tc(
                454 + 2268 + 1418 + 1134 + 1247 + 1247 + 1134 + 1247 + 1134,
                r#"<w:gridSpan w:val="9"/>"#,
                &center("<w:b/>", "Tổng cộng"),
            ),
            tc(
                1247,
                "",
                &para(
                    &format!(r#"{TIGHT}<w:jc w:val="right"/>"#),
                    &r("<w:b/>", &fmt(sum)),
                ),
            ),
            tc(1040, "", &para(TIGHT, "")),
        ],
    ));
    v.push((
        "office-17-bang-luong-ngang.docx",
        "office",
        "Bảng lương khổ ngang: Arial 10, 11 cột, tiêu đề 2 tầng (gridSpan, vMerge) lặp lại, số tiền căn phải, hàng tổng cộng, sang trang",
        Doc {
            body: [
                para(r#"<w:jc w:val="center"/>"#, &r(&format!("<w:b/>{}", sz(14.0)), "BẢNG THANH TOÁN TIỀN LƯƠNG THÁNG 11/2026")),
                center("<w:i/>", "Đơn vị tính: đồng"),
                tbl(&g17, true, r#"<w:tblLayout w:type="fixed"/>"#, &rows17),
                para(r#"<w:spacing w:before="120"/>"#, &r("<w:i/>", &format!("Bằng chữ: {}", "Ba trăm mười hai triệu đồng chẵn (giả định)."))),
            ]
            .concat(),
            sect: sect_a4_landscape(
                r#"<w:footerReference w:type="default" r:id="rIdF1"/>"#,
                "",
            ),
            styles: Some(styles("Arial", 10.0, &spacing(0, 0, 240, false))),
            hf: vec![("rIdF1".into(), "footer", page_x_of_y("center", 9.0))],
            ..Doc::default()
        },
    ));

    // ---- 18 đơn xin việc: borderless form table with a photo frame picture.
    let lab = |t: &str| para(r#"<w:spacing w:before="60" w:after="60"/>"#, &r("", t));
    v.push((
        "office-18-don-xin-viec.docx",
        "office",
        "Đơn xin việc: quốc hiệu, bảng không viền có ảnh 3×4 nội dòng trong ô, dòng điền dẫn chấm, đoạn thụt lề treo, Times New Roman 14",
        Doc {
            body: [
                center(&format!("<w:b/>{}", sz(13.0)), "CỘNG HÒA XÃ HỘI CHỦ NGHĨA VIỆT NAM"),
                center(&format!("<w:b/>{}", sz(14.0)), "Độc lập - Tự do - Hạnh phúc"),
                para(r#"<w:spacing w:before="240" w:after="240"/><w:jc w:val="center"/>"#, &r(&format!("<w:b/>{}", sz(16.0)), "ĐƠN XIN VIỆC")),
                tbl(
                    &[1985, 7370],
                    false,
                    r#"<w:tblLayout w:type="fixed"/>"#,
                    &[tr("", &[
                        tc(1985, "", &para(r#"<w:jc w:val="center"/>"#, &inline_pic(41, "rIdP0", 1_080_000, 1_440_000, "", ""))),
                        tc(7370, "", &[
                            para(r#"<w:spacing w:after="120"/><w:ind w:left="1701" w:hanging="1701"/>"#, &[r("<w:b/>", "Kính gửi:"), tab(), r("", "Ban Giám đốc Công ty Cổ phần Ánh Sao; Phòng Hành chính - Nhân sự.")].concat()),
                            lab("Tôi tên là: Nguyễn Văn An"),
                            lab("Sinh ngày: 12/03/1998     Nơi sinh: Nam Định"),
                            lab("Trình độ: Cử nhân Công nghệ thông tin"),
                        ].concat()),
                    ])],
                ),
                body(567, &format!("Qua thông tin tuyển dụng, tôi được biết Công ty có nhu cầu tuyển nhân sự cho vị trí chuyên viên. {}", sents(1, 2))),
                body(567, &sents(3, 3)),
                para(r#"<w:ind w:left="567" w:hanging="283"/><w:jc w:val="both"/>"#, &r("", &format!("- {}", sents(0, 2)))),
                para(r#"<w:ind w:left="567" w:hanging="283"/><w:jc w:val="both"/>"#, &r("", &format!("- {}", sents(4, 2)))),
                para(&format!(r#"<w:tabs><w:tab w:val="right" w:leader="dot" w:pos="{TEXT_W}"/></w:tabs>"#), &[r("", "Hồ sơ kèm theo: "), tab()].concat()),
                body(567, "Tôi xin cam đoan những thông tin trên là đúng sự thật và xin chân thành cảm ơn./."),
                para(r#"<w:spacing w:before="240"/><w:ind w:left="4536"/><w:jc w:val="center"/>"#, &r("<w:i/>", "Hà Nội, ngày 20 tháng 11 năm 2026")),
                para(r#"<w:ind w:left="4536"/><w:jc w:val="center"/>"#, &r("<w:b/>", "Người làm đơn")),
            ]
            .concat(),
            sect: sect_a4("", ""),
            styles: Some(styles("Times New Roman", 14.0, &spacing(0, 60, 276, false))),
            images: vec![("rIdP0".into(), png(12, 16, [0xBF, 0xC8, 0xD6]))],
            ..Doc::default()
        },
    ));

    // ---- 19 nghị quyết: 1.5 lines, TNR 14, first line 1.27 cm, before 6 pt, several pages.
    let mut b19 = [
        admin_header(
            14.0,
            "ĐẢNG BỘ GIẢ ĐỊNH",
            "CHI BỘ ÁNH SAO",
            "Số: 03-NQ/CB",
            "",
        ),
        para(
            r#"<w:spacing w:before="360"/><w:jc w:val="center"/>"#,
            &r(&format!("<w:b/>{}", sz(15.0)), "NGHỊ QUYẾT"),
        ),
        center("<w:b/>", "Hội nghị chi bộ thường kỳ tháng 11 năm 2026"),
    ]
    .concat();
    for k in 0..16 {
        if k % 4 == 0 {
            b19.push_str(&para(
                r#"<w:keepNext/><w:ind w:firstLine="720"/>"#,
                &r(
                    "<w:b/>",
                    [
                        "I. ĐÁNH GIÁ KẾT QUẢ",
                        "II. NHIỆM VỤ THÁNG TỚI",
                        "III. GIẢI PHÁP",
                        "IV. TỔ CHỨC THỰC HIỆN",
                    ][k / 4],
                ),
            ));
        }
        b19.push_str(&body(720, &sents(k, 2 + k % 3)));
    }
    v.push((
        "office-19-nghi-quyet-gian-1-5.docx",
        "office",
        "Nghị quyết: Times New Roman 14, giãn dòng 1,5, trước đoạn 6 pt, thụt dòng đầu 1,27 cm, nhiều trang (khoảng giãn của dòng bội số ở đáy trang)",
        Doc {
            body: b19,
            sect: sect_a4("", ""),
            styles: Some(styles("Times New Roman", 14.0, &spacing(120, 0, 360, false))),
            ..Doc::default()
        },
    ));

    // ---- 20 phụ lục: restart page numbers, landscape table with notes in cells.
    let g20 = [567, 4536, 2268, 2268, 2268, 2663];
    let mut rows20 = vec![tr(
        "<w:tblHeader/>",
        &[
            "STT",
            "Chỉ tiêu",
            "Kế hoạch",
            "Thực hiện",
            "Tỷ lệ (%)",
            "Ghi chú",
        ]
        .iter()
        .zip(g20)
        .map(|(t, w)| {
            tc(
                w,
                r#"<w:shd w:val="clear" w:color="auto" w:fill="FFF2CC"/>"#,
                &center("<w:b/>", t),
            )
        })
        .collect::<Vec<_>>(),
    )];
    let mut fn20 = String::new();
    for i in 0..14usize {
        let plan = 100 + i * 23;
        let done = plan * (80 + (i * 7) % 30) / 100;
        let mut name = r("", SENT[i % SENT.len()].split(',').next().unwrap_or(""));
        if i % 5 == 1 {
            let id = (i / 5 + 1) as i32;
            name.push_str(&note_ref("footnote", id));
            fn20.push_str(&note(
                "footnote",
                id,
                &format!("Số liệu ước tính đến hết tháng {}/2026.", 9 + id),
            ));
        }
        rows20.push(tr(
            "",
            &[
                tc(567, "", &center("", &(i + 1).to_string())),
                tc(4536, "", &para(r#"<w:jc w:val="both"/>"#, &name)),
                tc(2268, "", &center("", &plan.to_string())),
                tc(2268, "", &center("", &done.to_string())),
                tc(
                    2268,
                    "",
                    &center(
                        "",
                        &format!("{},{}", done * 100 / plan, done * 1000 / plan % 10),
                    ),
                ),
                tc(2663, "", &para(TIGHT, "")),
            ],
        ));
    }
    v.push((
        "office-20-phu-luc-ngang.docx",
        "office",
        "Phụ lục: section ngang đánh số trang lại từ 1 (pgNumType start), đầu trang \"Phụ lục\", bảng có chú thích cuối trang trong ô, Times New Roman 13",
        Doc {
            body: [
                heading(1, "BÁO CÁO TÓM TẮT"),
                body(567, &sents(0, 4)),
                body(567, &sents(4, 4)),
                sect_para(&sect_a4(r#"<w:footerReference w:type="default" r:id="rIdF1"/>"#, "")),
                para(r#"<w:jc w:val="center"/>"#, &r(&format!("<w:b/>{}", sz(14.0)), "PHỤ LỤC I")),
                center("<w:i/>", "(Kèm theo Báo cáo số 52/BC-AS ngày 20/11/2026)"),
                para("", ""),
                tbl(&g20, true, r#"<w:tblLayout w:type="fixed"/>"#, &rows20),
                body(0, &sents(2, 2)),
            ]
            .concat(),
            sect: sect_a4_landscape(
                r#"<w:headerReference w:type="default" r:id="rIdH1"/><w:footerReference w:type="default" r:id="rIdF2"/><w:type w:val="nextPage"/>"#,
                r#"<w:pgNumType w:start="1"/>"#,
            ),
            styles: Some(styles("Times New Roman", 13.0, &spacing(60, 60, 276, false))),
            hf: vec![
                ("rIdF1".into(), "footer", para(r#"<w:pStyle w:val="Footer"/><w:jc w:val="center"/>"#, &field_complex("PAGE", "1"))),
                ("rIdH1".into(), "header", para(r#"<w:pStyle w:val="Header"/><w:jc w:val="right"/>"#, &r("<w:i/>", "Phụ lục I"))),
                ("rIdF2".into(), "footer", page_x_of_y("center", 11.0)),
            ],
            footnotes: Some(fn20),
            ..Doc::default()
        },
    ));
    v
}
