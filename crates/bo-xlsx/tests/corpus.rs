//! Corpus round trip (`tests/corpus/sheet`, prompt §7).
//!
//! * `generate_corpus` rewrites the corpus files from the hand-written SpreadsheetML
//!   below when `BO_WRITE_CORPUS=1` (`cargo test -p bo-xlsx --test corpus`).
//! * `corpus_roundtrip` opens every `.xlsx` in the corpus, saves it and checks that
//!   nothing was lost: the model re-reads identically, opaque parts are byte-identical,
//!   every part survives (except the calculation chain, dropped on purpose), and every
//!   attribute / text "fact" of each regenerated XML part (element path + attribute +
//!   value, relationship ids replaced by their targets) is still present. It prints the
//!   lossless percentage per feature and requires ≥ 95 % overall.
// Test helpers may unwrap: a panic is the desired failure mode here.
#![allow(clippy::unwrap_used)]

use bo_opc::xml::{unescape, XmlReader};
use bo_opc::{resolve_target, Limits, Package};
use bo_xlsx::{read_xlsx, write_xlsx};
use quick_xml::events::Event;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Package builder
// ---------------------------------------------------------------------------

const NS_MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/";
const CT_SML: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.";
const DECL: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;

#[derive(Default, Clone)]
struct Sheet {
    name: String,
    state: Option<&'static str>,
    xml: String,
    /// (id, relationship type suffix, target relative to the sheet, external)
    rels: Vec<(String, String, String, bool)>,
}

#[derive(Default, Clone)]
struct Book {
    sheets: Vec<Sheet>,
    /// `workbook` children before `sheets` (fileVersion, workbookPr, bookViews…).
    wb_pre: String,
    /// `workbook` children after `sheets` (definedNames, calcPr, extLst…).
    wb_post: String,
    /// Extra root attributes of `workbook`.
    wb_root: String,
    styles: Option<String>,
    /// Shared string items (inner XML of `si`), in order of first use.
    sst: Vec<String>,
    /// Extra parts: (name, content type override, data).
    parts: Vec<(String, Option<String>, Vec<u8>)>,
    defaults: Vec<(String, String)>,
    /// Extra package relationships: (type, target).
    pkg_rels: Vec<(String, String)>,
    /// Extra workbook-part relationships: (type, target relative to `/xl/`).
    wb_rels: Vec<(String, String)>,
}

fn ws(body: &str) -> String {
    format!(r#"{DECL}<worksheet xmlns="{NS_MAIN}" xmlns:r="{NS_R}">{body}</worksheet>"#)
}

fn sheet(name: &str, body: &str) -> Sheet {
    Sheet {
        name: name.into(),
        xml: ws(body),
        ..Sheet::default()
    }
}

const STYLES_MIN: &str = r#"<numFmts count="0"/><fonts count="1"><font><sz val="11"/><color theme="1"/><name val="Calibri"/><family val="2"/><scheme val="minor"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles><dxfs count="0"/>"#;

fn build(b: &Book) -> Vec<u8> {
    let mut p = Package::new();
    let mut overrides: Vec<(String, String)> = Vec::new();
    let mut put = |p: &mut Package, name: &str, ct: Option<&str>, data: Vec<u8>| {
        if let Some(ct) = ct {
            overrides.push((name.to_owned(), ct.to_owned()));
        }
        p.put_part(name, data);
    };
    // Package relationships.
    let mut rels = format!(
        r#"{DECL}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL}officeDocument" Target="xl/workbook.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>"#
    );
    for (i, (t, target)) in b.pkg_rels.iter().enumerate() {
        let _ = write!(
            rels,
            r#"<Relationship Id="rId{}" Type="{t}" Target="{target}"/>"#,
            i + 3
        );
    }
    rels.push_str("</Relationships>");
    put(&mut p, "/_rels/.rels", None, rels.into_bytes());
    put(
        &mut p,
        "/docProps/core.xml",
        Some("application/vnd.openxmlformats-package.core-properties+xml"),
        format!(r#"{DECL}<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:creator>BlueOffice corpus generator</dc:creator></cp:coreProperties>"#).into_bytes(),
    );
    // Workbook + its relationships.
    let mut sheets = String::new();
    let mut wb_rels = format!(
        r#"{DECL}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#
    );
    for (i, s) in b.sheets.iter().enumerate() {
        let n = i + 1;
        let state = s
            .state
            .map(|st| format!(r#" state="{st}""#))
            .unwrap_or_default();
        let _ = write!(
            sheets,
            r#"<sheet name="{}" sheetId="{n}"{state} r:id="rId{n}"/>"#,
            s.name
        );
        let _ = write!(
            wb_rels,
            r#"<Relationship Id="rId{n}" Type="{REL}worksheet" Target="worksheets/sheet{n}.xml"/>"#
        );
        let path = format!("/xl/worksheets/sheet{n}.xml");
        put(
            &mut p,
            &path,
            Some(&format!("{CT_SML}worksheet+xml")),
            s.xml.clone().into_bytes(),
        );
        if !s.rels.is_empty() {
            let mut r = format!(
                r#"{DECL}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#
            );
            for (id, t, target, ext) in &s.rels {
                let t = if t.starts_with("http") {
                    t.clone()
                } else {
                    format!("{REL}{t}")
                };
                let mode = if *ext {
                    r#" TargetMode="External""#
                } else {
                    ""
                };
                let _ = write!(
                    r,
                    r#"<Relationship Id="{id}" Type="{t}" Target="{}"{mode}/>"#,
                    bo_opc::xml::escape(target)
                );
            }
            r.push_str("</Relationships>");
            put(&mut p, &bo_opc::rels_part_name(&path), None, r.into_bytes());
        }
    }
    let k = b.sheets.len();
    let _ = write!(
        wb_rels,
        r#"<Relationship Id="rId{}" Type="{REL}styles" Target="styles.xml"/>"#,
        k + 1
    );
    if !b.sst.is_empty() {
        let _ = write!(
            wb_rels,
            r#"<Relationship Id="rId{}" Type="{REL}sharedStrings" Target="sharedStrings.xml"/>"#,
            k + 2
        );
    }
    let _ = write!(
        wb_rels,
        r#"<Relationship Id="rId{}" Type="{REL}theme" Target="theme/theme1.xml"/>"#,
        k + 3
    );
    for (i, (t, target)) in b.wb_rels.iter().enumerate() {
        let _ = write!(
            wb_rels,
            r#"<Relationship Id="rId{}" Type="{t}" Target="{target}"/>"#,
            k + 4 + i
        );
    }
    wb_rels.push_str("</Relationships>");
    put(
        &mut p,
        "/xl/_rels/workbook.xml.rels",
        None,
        wb_rels.into_bytes(),
    );
    let wb = format!(
        r#"{DECL}<workbook xmlns="{NS_MAIN}" xmlns:r="{NS_R}"{}>{}<sheets>{sheets}</sheets>{}</workbook>"#,
        b.wb_root, b.wb_pre, b.wb_post
    );
    put(
        &mut p,
        "/xl/workbook.xml",
        Some(&format!("{CT_SML}sheet.main+xml")),
        wb.into_bytes(),
    );
    let styles = format!(
        r#"{DECL}<styleSheet xmlns="{NS_MAIN}">{}</styleSheet>"#,
        b.styles.as_deref().unwrap_or(STYLES_MIN)
    );
    put(
        &mut p,
        "/xl/styles.xml",
        Some(&format!("{CT_SML}styles+xml")),
        styles.into_bytes(),
    );
    if !b.sst.is_empty() {
        let mut s = format!(
            r#"{DECL}<sst xmlns="{NS_MAIN}" count="{0}" uniqueCount="{0}">"#,
            b.sst.len()
        );
        for si in &b.sst {
            let _ = write!(s, "<si>{si}</si>");
        }
        s.push_str("</sst>");
        put(
            &mut p,
            "/xl/sharedStrings.xml",
            Some(&format!("{CT_SML}sharedStrings+xml")),
            s.into_bytes(),
        );
    }
    put(
        &mut p,
        "/xl/theme/theme1.xml",
        Some("application/vnd.openxmlformats-officedocument.theme+xml"),
        THEME.as_bytes().to_vec(),
    );
    for (name, ct, data) in &b.parts {
        put(&mut p, name, ct.as_deref(), data.clone());
    }
    let mut ct = format!(
        r#"{DECL}<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>"#
    );
    for (e, t) in &b.defaults {
        let _ = write!(ct, r#"<Default Extension="{e}" ContentType="{t}"/>"#);
    }
    for (n, t) in &overrides {
        let _ = write!(ct, r#"<Override PartName="{n}" ContentType="{t}"/>"#);
    }
    ct.push_str("</Types>");
    p.put_part("/[Content_Types].xml", ct.into_bytes());
    p.write().expect("package")
}

const THEME: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="BlueOffice"><a:themeElements><a:clrScheme name="BlueOffice"><a:dk1><a:srgbClr val="000000"/></a:dk1><a:lt1><a:srgbClr val="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="1F2A44"/></a:dk2><a:lt2><a:srgbClr val="E7E6E6"/></a:lt2><a:accent1><a:srgbClr val="2F6FB5"/></a:accent1><a:accent2><a:srgbClr val="E07B39"/></a:accent2><a:accent3><a:srgbClr val="9E9E9E"/></a:accent3><a:accent4><a:srgbClr val="F2B632"/></a:accent4><a:accent5><a:srgbClr val="4BA3DB"/></a:accent5><a:accent6><a:srgbClr val="5AA454"/></a:accent6><a:hlink><a:srgbClr val="0B57D0"/></a:hlink><a:folHlink><a:srgbClr val="7B1FA2"/></a:folHlink></a:clrScheme><a:fontScheme name="BlueOffice"><a:majorFont><a:latin typeface="Calibri Light"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme><a:fmtScheme name="BlueOffice"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>"#;

fn rows(n: usize, cols: usize, base: f64) -> String {
    let mut s = String::new();
    for r in 1..=n {
        let _ = write!(s, r#"<row r="{r}">"#);
        for c in 0..cols {
            let col = (b'A' + c as u8) as char;
            let _ = write!(
                s,
                r#"<c r="{col}{r}"><v>{}</v></c>"#,
                base + (r * 10 + c) as f64
            );
        }
        s.push_str("</row>");
    }
    s
}

// ---------------------------------------------------------------------------
// Features (three variants each)
// ---------------------------------------------------------------------------

fn values(i: usize) -> Book {
    let (n1, n2) = (1234.5 * i as f64, -0.000_125 * i as f64);
    let date1904 = if i == 3 {
        r#"<workbookPr date1904="1"/>"#
    } else {
        ""
    };
    let body = format!(
        r#"<sheetData><row r="1"><c r="A1"><v>{n1}</v></c><c r="B1"><v>{n2}</v></c><c r="C1" t="b"><v>{}</v></c><c r="D1" t="e"><v>#N/A</v></c><c r="E1" t="s"><v>0</v></c></row><row r="2"><c r="A2"><f>A1*{i}</f><v>{}</v></c><c r="B2" t="str"><f>"Mã "&amp;TEXT(A1,"0")</f><v>Mã {}</v></c><c r="C2"><f t="shared" ref="C2:C4" si="0">A2+1</f><v>1</v></c><c r="D2" t="e"><f>1/0</f><v>#DIV/0!</v></c></row><row r="3"><c r="C3"><f t="shared" si="0"/><v>2</v></c><c r="D3"><f t="array" ref="D3:D4">SUM(A1:A2*{{1;2}})</f><v>3</v></c></row><row r="4"><c r="C4"><f t="shared" si="0"/><v>3</v></c><c r="D4"><v>4</v></c><c r="E4" t="d"><v>2024-0{i}-15T08:30:00</v></c><c r="F4"><f ca="1">NOW()</f><v>45{i}00.25</v></c></row><row r="7"><c r="G7"><v>1E+{}</v></c></row></sheetData>"#,
        i % 2,
        n1 * i as f64,
        n1 as i64,
        10 + i
    );
    Book {
        sheets: vec![sheet("Giá trị", &body)],
        sst: vec![format!("<t>Chuỗi {i}</t>")],
        wb_pre: date1904.into(),
        ..Book::default()
    }
}

fn sst(i: usize) -> Book {
    let runs = match i {
        1 => {
            r#"<r><rPr><b/><sz val="11"/><color rgb="FFFF0000"/><rFont val="Calibri"/><family val="2"/></rPr><t>Đỏ đậm</t></r><r><t xml:space="preserve"> và thường</t></r>"#
        }
        2 => {
            r#"<r><rPr><i/><strike/><u val="double"/><sz val="12"/><color theme="4" tint="-0.249977111117893"/><rFont val="Times New Roman"/><family val="1"/></rPr><t>Nghiêng</t></r><r><rPr><vertAlign val="superscript"/><sz val="11"/><rFont val="Calibri"/></rPr><t>2</t></r>"#
        }
        _ => {
            r#"<r><rPr><b/><i/><sz val="9"/><color indexed="10"/><rFont val="Tahoma"/><charset val="163"/></rPr><t>Tahoma</t></r><r><rPr><sz val="11"/><rFont val="Calibri"/><scheme val="minor"/></rPr><t xml:space="preserve">  hai khoảng  </t></r>"#
        }
    };
    let items = vec![
        runs.to_owned(),
        format!("<t>Dòng 1_x000D_\nDòng {i}</t>"),
        r#"<t xml:space="preserve">  cách đầu cuối  </t>"#.to_owned(),
        format!("<t>_x005F_x0041_ ký tự &lt;&amp;&gt; {i}</t>"),
    ];
    let mut body = String::from("<sheetData>");
    for k in 0..items.len() {
        let _ = write!(
            body,
            r#"<row r="{0}"><c r="A{0}" t="s"><v>{1}</v></c></row>"#,
            k + 1,
            k
        );
    }
    body.push_str("</sheetData>");
    Book {
        sheets: vec![sheet("Chuỗi", &body)],
        sst: items,
        ..Book::default()
    }
}

fn styles(i: usize) -> Book {
    let fmt = [
        "#,##0 [$₫-42A]",
        "dd/mm/yyyy hh:mm",
        "[Red][<0]-#,##0.00;[Blue]#,##0.00",
    ][i - 1];
    let styles = format!(
        r#"<numFmts count="2"><numFmt numFmtId="164" formatCode="{}"/><numFmt numFmtId="165" formatCode="0.0%"/></numFmts><fonts count="3"><font><sz val="11"/><color theme="1"/><name val="Calibri"/><family val="2"/><scheme val="minor"/></font><font><b/><i/><u/><sz val="{}"/><color rgb="FF1F4E79"/><name val="Arial"/><family val="2"/></font><font><strike/><sz val="10"/><color indexed="12"/><name val="Times New Roman"/><family val="1"/><charset val="163"/></font></fonts><fills count="5"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill><fill><patternFill patternType="solid"><fgColor rgb="FFFFFF00"/><bgColor indexed="64"/></patternFill></fill><fill><patternFill patternType="darkTrellis"><fgColor theme="5" tint="0.39997558519241921"/><bgColor auto="1"/></patternFill></fill><fill><gradientFill type="path" left="0.5" right="0.5" top="0.5" bottom="0.5"><stop position="0"><color theme="0"/></stop><stop position="1"><color theme="{}"/></stop></gradientFill></fill></fills><borders count="3"><border><left/><right/><top/><bottom/><diagonal/></border><border><left style="thin"><color auto="1"/></left><right style="thin"><color auto="1"/></right><top style="medium"><color rgb="FFFF0000"/></top><bottom style="double"><color indexed="64"/></bottom><diagonal/></border><border diagonalUp="1" diagonalDown="1"><left style="dashDot"/><right style="mediumDashed"/><top style="hair"/><bottom style="thick"/><diagonal style="dotted"><color theme="3"/></diagonal></border></borders><cellStyleXfs count="2"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/><xf numFmtId="0" fontId="1" fillId="2" borderId="0" applyFont="1" applyFill="1"/></cellStyleXfs><cellXfs count="5"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/><xf numFmtId="164" fontId="1" fillId="2" borderId="1" xfId="1" applyNumberFormat="1" applyFont="1" applyFill="1" applyBorder="1"/><xf numFmtId="165" fontId="2" fillId="3" borderId="2" xfId="0" applyAlignment="1"><alignment horizontal="centerContinuous" vertical="center" wrapText="1" shrinkToFit="1" indent="{}" readingOrder="1"/></xf><xf numFmtId="14" fontId="0" fillId="4" borderId="0" xfId="0" applyProtection="1"><alignment textRotation="255"/><protection locked="0"/></xf><xf numFmtId="49" fontId="0" fillId="0" borderId="0" xfId="0" quotePrefix="1"/></cellXfs><cellStyles count="2"><cellStyle name="Normal" xfId="0" builtinId="0"/><cellStyle name="Tiêu đề {}" xfId="1" customBuiltin="1"/></cellStyles><dxfs count="2"><dxf><font><b/><color rgb="FF9C0006"/></font><fill><patternFill><bgColor rgb="FFFFC7CE"/></patternFill></fill></dxf><dxf><numFmt numFmtId="166" formatCode="0.000"/><border><left style="thin"/></border></dxf></dxfs><tableStyles count="0" defaultTableStyle="TableStyleMedium{}" defaultPivotStyle="PivotStyleLight16"/><colors><mruColors><color rgb="FF00{:02}FF"/></mruColors></colors>"#,
        bo_opc::xml::escape(fmt),
        10 + i,
        3 + i,
        i,
        i,
        i,
        i * 11
    );
    let mut body = String::from("<sheetData>");
    for s in 0..5 {
        let _ = write!(
            body,
            r#"<row r="{0}"><c r="A{0}" s="{s}"><v>{1}</v></c></row>"#,
            s + 1,
            45_000.5 + s as f64
        );
    }
    body.push_str("</sheetData>");
    Book {
        sheets: vec![sheet("Định dạng", &body)],
        styles: Some(styles),
        ..Book::default()
    }
}

fn layout(i: usize) -> Book {
    let body = format!(
        r#"<sheetPr><outlinePr summaryBelow="0" summaryRight="{}"/></sheetPr><sheetFormatPr baseColWidth="10" defaultRowHeight="15" outlineLevelRow="2" outlineLevelCol="1"/><cols><col min="1" max="1" width="{}" customWidth="1"/><col min="2" max="4" width="9.140625" hidden="1" outlineLevel="1"/><col min="5" max="5" width="20" bestFit="1" customWidth="1" collapsed="1"/></cols><sheetData><row r="1" ht="{}" customHeight="1"><c r="A1"><v>1</v></c></row><row r="2" hidden="1" outlineLevel="1"><c r="A2"><v>2</v></c></row><row r="3" outlineLevel="2" hidden="1"><c r="A3"><v>3</v></c></row><row r="4" collapsed="1"><c r="A4"><v>4</v></c></row><row r="6" ht="0.75" customHeight="1" thickBot="1"><c r="B6"><v>6</v></c></row></sheetData><mergeCells count="{}">{}</mergeCells>"#,
        i % 2,
        10.0 + i as f64 * 2.5,
        20 + i * 5,
        i,
        [
            "<mergeCell ref=\"A10:C12\"/>",
            "<mergeCell ref=\"A10:C12\"/><mergeCell ref=\"E1:F1\"/>",
            "<mergeCell ref=\"A10:C12\"/><mergeCell ref=\"E1:F1\"/><mergeCell ref=\"H2:H20\"/>"
        ][i - 1]
    );
    Book {
        sheets: vec![sheet("Bố cục", &body)],
        ..Book::default()
    }
}

fn panes(i: usize) -> Book {
    let view = match i {
        1 => {
            r#"<sheetView tabSelected="1" workbookViewId="0"><pane ySplit="1" topLeftCell="A2" activePane="bottomLeft" state="frozen"/><selection pane="bottomLeft" activeCell="A2" sqref="A2"/></sheetView>"#
        }
        2 => {
            r#"<sheetView showGridLines="0" zoomScale="85" zoomScaleNormal="85" workbookViewId="0"><pane xSplit="2" ySplit="3" topLeftCell="C4" activePane="bottomRight" state="frozen"/><selection pane="topRight" activeCell="C1" sqref="C1"/><selection pane="bottomLeft" activeCell="A4" sqref="A4"/><selection pane="bottomRight" activeCell="D5" sqref="D5:E6"/></sheetView>"#
        }
        _ => {
            r#"<sheetView rightToLeft="1" showRowColHeaders="0" workbookViewId="0" topLeftCell="B2"><pane xSplit="2400" ySplit="1800" topLeftCell="D8" activePane="bottomRight"/><selection pane="topRight"/><selection pane="bottomLeft"/><selection pane="bottomRight" activeCell="D8" sqref="D8"/></sheetView>"#
        }
    };
    let body = format!(
        "<sheetViews>{view}</sheetViews><sheetData>{}</sheetData>",
        rows(5, 3, i as f64)
    );
    Book {
        sheets: vec![sheet("Khung", &body)],
        ..Book::default()
    }
}

fn names(i: usize) -> Book {
    let mut post = String::from("<definedNames>");
    let _ = write!(
        post,
        r#"<definedName name="TyGia">'Sổ 1'!$B${i}</definedName>"#
    );
    let _ = write!(
        post,
        r#"<definedName name="DanhSach" comment="Danh sách {i}">'Sổ 2'!$A$1:$A$10</definedName>"#
    );
    post.push_str(r#"<definedName name="_xlnm._FilterDatabase" localSheetId="0" hidden="1">'Sổ 1'!$A$1:$C$5</definedName>"#);
    post.push_str(r#"<definedName name="NoiBo" localSheetId="1">'Sổ 2'!$C$3</definedName>"#);
    let _ = write!(
        post,
        r#"<definedName name="_xlnm.Print_Area" localSheetId="0">'Sổ 1'!$A$1:$C${}</definedName>"#,
        10 * i
    );
    if i >= 2 {
        post.push_str(
            r#"<definedName name="_xlnm.Print_Titles" localSheetId="0">'Sổ 1'!$1:$2</definedName>"#,
        );
    }
    if i == 3 {
        post.push_str(r#"<definedName name="_xlnm.Print_Titles" localSheetId="1">'Sổ 2'!$A:$A,'Sổ 2'!$1:$1</definedName><definedName name="Hang" function="1" vbProcedure="1">#REF!</definedName>"#);
    }
    post.push_str("</definedNames><calcPr calcId=\"191029\"/>");
    Book {
        sheets: vec![
            sheet(
                "Sổ 1",
                &format!("<sheetData>{}</sheetData>", rows(5, 3, 0.0)),
            ),
            sheet(
                "Sổ 2",
                &format!("<sheetData>{}</sheetData>", rows(10, 1, 100.0)),
            ),
        ],
        wb_post: post,
        ..Book::default()
    }
}

fn page(i: usize) -> Book {
    let hf = match i {
        1 => "<headerFooter><oddHeader>&amp;C&amp;\"Arial,Bold\"Báo cáo</oddHeader><oddFooter>Trang &amp;P / &amp;N</oddFooter></headerFooter>".to_owned(),
        2 => "<headerFooter differentFirst=\"1\"><oddHeader>&amp;LCông ty&amp;R&amp;D</oddHeader><firstHeader>&amp;CTrang bìa</firstHeader></headerFooter>".to_owned(),
        _ => "<headerFooter differentOddEven=\"1\" alignWithMargins=\"0\"><oddFooter>&amp;RLẻ</oddFooter><evenFooter>&amp;LChẵn</evenFooter></headerFooter>".to_owned(),
    };
    let body = format!(
        r#"<sheetPr><pageSetUpPr fitToPage="{}"/></sheetPr><sheetData>{}</sheetData><printOptions headings="1" gridLines="{}" verticalCentered="1"/><pageMargins left="{}" right="0.5" top="0.75" bottom="0.75" header="0.3" footer="0.3"/><pageSetup paperSize="{}" scale="{}" fitToWidth="1" fitToHeight="0" orientation="{}" horizontalDpi="600" verticalDpi="600" firstPageNumber="{i}" useFirstPageNumber="1"/>{hf}<rowBreaks count="2" manualBreakCount="2"><brk id="{}" max="16383" man="1"/><brk id="40" max="16383" man="1"/></rowBreaks><colBreaks count="1" manualBreakCount="1"><brk id="{}" max="1048575" man="1"/></colBreaks>"#,
        i % 2,
        rows(3, 2, 0.0),
        i % 2,
        0.25 * i as f64,
        [9, 1, 8][i - 1],
        70 + i * 10,
        ["portrait", "landscape", "default"][i - 1],
        10 + i,
        3 + i
    );
    Book {
        sheets: vec![sheet("In", &body)],
        ..Book::default()
    }
}

fn links(i: usize) -> Book {
    let mut hl = String::from("<hyperlinks>");
    let mut rels = Vec::new();
    for k in 1..=i {
        let _ = write!(
            hl,
            r#"<hyperlink ref="A{k}" r:id="rId{k}" tooltip="Mở trang {k}"/>"#
        );
        rels.push((
            format!("rId{k}"),
            "hyperlink".to_owned(),
            format!("https://bluestar.vn/tra-cuu?q={k}&lang=vi"),
            true,
        ));
    }
    let _ = write!(
        hl,
        r#"<hyperlink ref="B1:B{i}" location="'Đích'!C{i}" display="Đi tới C{i}"/>"#
    );
    if i == 3 {
        hl.push_str(r#"<hyperlink ref="C1" r:id="rId9"/>"#);
        rels.push((
            "rId9".into(),
            "hyperlink".into(),
            "mailto:hotro@bluestar.vn?subject=Hỏi".into(),
            true,
        ));
    }
    hl.push_str("</hyperlinks>");
    let body = format!("<sheetData>{}</sheetData>{hl}", rows(i, 3, 0.0));
    let mut s = sheet("Liên kết", &body);
    s.rels = rels;
    Book {
        sheets: vec![s, sheet("Đích", "<sheetData/>")],
        ..Book::default()
    }
}

fn comments(i: usize) -> Book {
    let mut list = String::new();
    for k in 1..=i {
        let _ = write!(
            list,
            r#"<comment ref="B{k}" authorId="{}"><text><r><rPr><b/><sz val="9"/><color indexed="81"/><rFont val="Tahoma"/><family val="2"/></rPr><t>Người {k}:</t></r><r><rPr><sz val="9"/><color indexed="81"/><rFont val="Tahoma"/><family val="2"/></rPr><t xml:space="preserve">
Ghi chú số {k}</t></r></text></comment>"#,
            k % 2
        );
    }
    let comments = format!(
        r#"{DECL}<comments xmlns="{NS_MAIN}"><authors><author>Kế toán</author><author>Kiểm soát</author></authors><commentList>{list}</commentList></comments>"#
    );
    let vml = format!(
        r##"<xml xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:x="urn:schemas-microsoft-com:office:excel"><v:shape id="_x0000_s1025" type="#_x0000_t202"><x:ClientData ObjectType="Note"><x:Row>{i}</x:Row><x:Column>1</x:Column></x:ClientData></v:shape></xml>"##
    );
    let body = format!(
        r#"<sheetData>{}</sheetData><legacyDrawing r:id="rId2"/>"#,
        rows(i, 2, 0.0)
    );
    let mut s = sheet("Ghi chú", &body);
    s.rels = vec![
        (
            "rId1".into(),
            "comments".into(),
            "../comments1.xml".into(),
            false,
        ),
        (
            "rId2".into(),
            "vmlDrawing".into(),
            "../drawings/vmlDrawing1.vml".into(),
            false,
        ),
    ];
    Book {
        sheets: vec![s],
        parts: vec![
            (
                "/xl/comments1.xml".into(),
                Some(format!("{CT_SML}comments+xml")),
                comments.into_bytes(),
            ),
            (
                "/xl/drawings/vmlDrawing1.vml".into(),
                None,
                vml.into_bytes(),
            ),
        ],
        defaults: vec![(
            "vml".into(),
            "application/vnd.openxmlformats-officedocument.vmlDrawing".into(),
        )],
        ..Book::default()
    }
}

fn validation(i: usize) -> Book {
    let all = [
        r#"<dataValidation type="list" allowBlank="1" showInputMessage="1" showErrorMessage="1" promptTitle="Chọn" prompt="Chọn tỉnh" sqref="A2:A100"><formula1>"Hà Nội,Huế,TP HCM"</formula1></dataValidation>"#,
        r#"<dataValidation type="whole" operator="between" showErrorMessage="1" errorStyle="warning" errorTitle="Sai" error="Từ 1 đến 10" sqref="B2:B100"><formula1>1</formula1><formula2>10</formula2></dataValidation>"#,
        r#"<dataValidation type="decimal" operator="greaterThan" sqref="C2:C100 E2"><formula1>0.5</formula1></dataValidation>"#,
        r#"<dataValidation type="date" operator="lessThanOrEqual" allowBlank="1" sqref="D2:D50"><formula1>TODAY()</formula1></dataValidation>"#,
        r#"<dataValidation type="textLength" operator="lessThan" showDropDown="1" sqref="F2"><formula1>20</formula1></dataValidation>"#,
        r#"<dataValidation type="custom" allowBlank="1" imeMode="off" sqref="G2:G9"><formula1>ISNUMBER(G2)</formula1></dataValidation>"#,
        r#"<dataValidation type="list" sqref="H2:H9"><formula1>$Z$1:$Z$5</formula1></dataValidation>"#,
    ];
    let pick: Vec<&str> = all.iter().copied().skip(i - 1).take(3 + i).collect();
    let attrs = if i == 3 { r#" disablePrompts="1""# } else { "" };
    let body = format!(
        r#"<sheetData>{}</sheetData><dataValidations count="{}"{attrs}>{}</dataValidations>"#,
        rows(2, 2, 0.0),
        pick.len(),
        pick.concat()
    );
    Book {
        sheets: vec![sheet("Kiểm tra", &body)],
        ..Book::default()
    }
}

fn conditional(i: usize) -> Book {
    let styles = STYLES_MIN.replace(
        r#"<dxfs count="0"/>"#,
        r#"<dxfs count="2"><dxf><font><color rgb="FF9C0006"/></font><fill><patternFill><bgColor rgb="FFFFC7CE"/></patternFill></fill></dxf><dxf><font><b/></font></dxf></dxfs>"#,
    );
    let rules = [
        r#"<conditionalFormatting sqref="A1:A20"><cfRule type="cellIs" dxfId="0" priority="1" operator="between"><formula>10</formula><formula>20</formula></cfRule></conditionalFormatting>"#,
        r#"<conditionalFormatting sqref="B1:B20"><cfRule type="expression" dxfId="1" priority="2" stopIfTrue="1"><formula>AND($B1&gt;0,MOD(ROW(),2)=0)</formula></cfRule></conditionalFormatting>"#,
        r#"<conditionalFormatting sqref="C1:C20"><cfRule type="colorScale" priority="3"><colorScale><cfvo type="min"/><cfvo type="percentile" val="50"/><cfvo type="max"/><color rgb="FFF8696B"/><color rgb="FFFFEB84"/><color rgb="FF63BE7B"/></colorScale></cfRule></conditionalFormatting>"#,
        r#"<conditionalFormatting sqref="D1:D20"><cfRule type="dataBar" priority="4"><dataBar minLength="5" maxLength="95" showValue="0"><cfvo type="num" val="0"/><cfvo type="max"/><color theme="4"/></dataBar></cfRule></conditionalFormatting>"#,
        r#"<conditionalFormatting sqref="E1:E20"><cfRule type="iconSet" priority="5"><iconSet iconSet="5Quarters" reverse="1" showValue="0"><cfvo type="percent" val="0"/><cfvo type="percent" val="20"/><cfvo type="percent" val="40"/><cfvo type="percent" val="60"/><cfvo type="percent" val="80" gte="0"/></iconSet></cfRule></conditionalFormatting>"#,
        r#"<conditionalFormatting sqref="F1:F20"><cfRule type="top10" dxfId="0" priority="6" rank="3" percent="1" bottom="1"/></conditionalFormatting>"#,
        r#"<conditionalFormatting sqref="G1:G20 H1:H5"><cfRule type="containsText" dxfId="1" priority="7" operator="containsText" text="lỗi"><formula>NOT(ISERROR(SEARCH("lỗi",G1)))</formula></cfRule><cfRule type="duplicateValues" dxfId="0" priority="8"/></conditionalFormatting>"#,
    ];
    let pick: String = rules.iter().copied().skip(i - 1).take(4 + i).collect();
    let ext = if i == 3 {
        r#"<extLst><ext uri="{78C0D931-6437-407d-A8EE-F0AAD7539E65}" xmlns:x14="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main"><x14:conditionalFormattings><x14:conditionalFormatting xmlns:xm="http://schemas.microsoft.com/office/excel/2006/main"><x14:cfRule type="iconSet" priority="9" id="{00000000-0000-0000-0000-00000000000A}"><x14:iconSet iconSet="3Stars" custom="1"><x14:cfvo type="percent"><xm:f>0</xm:f></x14:cfvo><x14:cfvo type="percent"><xm:f>33</xm:f></x14:cfvo><x14:cfvo type="percent"><xm:f>67</xm:f></x14:cfvo></x14:iconSet></x14:cfRule><xm:sqref>J1:J9</xm:sqref></x14:conditionalFormatting></x14:conditionalFormattings></ext></extLst>"#
    } else {
        ""
    };
    let body = format!("<sheetData>{}</sheetData>{pick}{ext}", rows(20, 8, -5.0));
    Book {
        sheets: vec![sheet("Định dạng ĐK", &body)],
        styles: Some(styles),
        ..Book::default()
    }
}

fn table_xml(id: usize, name: &str, r: &str, totals: bool, cols: &[&str]) -> String {
    let mut c = String::new();
    for (k, n) in cols.iter().enumerate() {
        if totals && k == 0 {
            let _ = write!(
                c,
                r#"<tableColumn id="{}" name="{n}" totalsRowLabel="Tổng"/>"#,
                k + 1
            );
        } else if totals && k == cols.len() - 1 {
            let _ = write!(
                c,
                r#"<tableColumn id="{}" name="{n}" totalsRowFunction="sum"><calculatedColumnFormula>{name}[[#This Row],[{}]]*2</calculatedColumnFormula></tableColumn>"#,
                k + 1,
                cols[0]
            );
        } else {
            let _ = write!(c, r#"<tableColumn id="{}" name="{n}"/>"#, k + 1);
        }
    }
    let tot = if totals {
        r#" totalsRowCount="1""#
    } else {
        r#" totalsRowShown="0""#
    };
    let af_ref = if totals {
        // The filter excludes the totals row.
        let range = bo_model::sheet::CellRange::parse(r).expect("range");
        format!(
            "{}:{}",
            range.first,
            bo_model::sheet::CellRef::new(range.last.row - 1, range.last.col)
        )
    } else {
        r.to_owned()
    };
    format!(
        r#"{DECL}<table xmlns="{NS_MAIN}" id="{id}" name="{name}" displayName="{name}" ref="{r}"{tot}><autoFilter ref="{af_ref}"/><tableColumns count="{}">{c}</tableColumns><tableStyleInfo name="TableStyleMedium{id}" showFirstColumn="0" showLastColumn="{}" showRowStripes="1" showColumnStripes="0"/></table>"#,
        cols.len(),
        id % 2
    )
}

fn tables(i: usize) -> Book {
    let mut sst = vec![
        "<t>Mã</t>".to_owned(),
        "<t>Tên</t>".to_owned(),
        "<t>Số lượng</t>".to_owned(),
    ];
    let header = r#"<row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c><c r="C1" t="s"><v>2</v></c></row>"#;
    let mut data = String::new();
    for r in 2..=5 {
        let _ = write!(
            data,
            r#"<row r="{r}"><c r="A{r}"><v>{r}</v></c><c r="B{r}"><v>{}</v></c><c r="C{r}"><f>BangHang[[#This Row],[Mã]]*2</f><v>{}</v></c></row>"#,
            r * 3,
            r * 2
        );
    }
    let totals = i >= 2;
    let last = if totals { 6 } else { 5 };
    if totals {
        data.push_str(r#"<row r="6"><c r="A6" t="s"><v>3</v></c><c r="C6"><f>SUBTOTAL(109,BangHang[Số lượng])</f><v>28</v></c></row>"#);
        sst.push("<t>Tổng</t>".into());
    }
    let body = format!(
        r#"<sheetData>{header}{data}</sheetData><tableParts count="1"><tablePart r:id="rId1"/></tableParts>"#
    );
    let mut s1 = sheet("Bảng", &body);
    s1.rels = vec![(
        "rId1".into(),
        "table".into(),
        "../tables/table1.xml".into(),
        false,
    )];
    let mut parts = vec![(
        "/xl/tables/table1.xml".to_owned(),
        Some(format!("{CT_SML}table+xml")),
        table_xml(
            1,
            "BangHang",
            &format!("A1:C{last}"),
            totals,
            &["Mã", "Tên", "Số lượng"],
        )
        .into_bytes(),
    )];
    let mut sheets = vec![s1];
    if i == 3 {
        let body = format!(
            r#"<sheetData>{}</sheetData><tableParts count="1"><tablePart r:id="rId1"/></tableParts>"#,
            rows(4, 2, 0.0)
        );
        let mut s2 = sheet("Bảng 2", &body);
        s2.rels = vec![(
            "rId1".into(),
            "table".into(),
            "../tables/table2.xml".into(),
            false,
        )];
        sheets.push(s2);
        parts.push((
            "/xl/tables/table2.xml".into(),
            Some(format!("{CT_SML}table+xml")),
            table_xml(2, "Phu", "A1:B4", false, &["Cột1", "Cột2"]).into_bytes(),
        ));
    }
    Book {
        sheets,
        sst,
        parts,
        ..Book::default()
    }
}

fn chart_xml(kind: usize) -> String {
    let ser = |idx: usize, col: char| {
        format!(
            r#"<c:ser><c:idx val="{idx}"/><c:order val="{idx}"/><c:tx><c:strRef><c:f>'Biểu đồ'!${col}$1</c:f><c:strCache><c:ptCount val="1"/><c:pt idx="0"><c:v>Năm {idx}</c:v></c:pt></c:strCache></c:strRef></c:tx><c:cat><c:strRef><c:f>'Biểu đồ'!$A$2:$A$4</c:f><c:strCache><c:ptCount val="3"/><c:pt idx="0"><c:v>Bắc</c:v></c:pt><c:pt idx="1"><c:v>Trung</c:v></c:pt><c:pt idx="2"><c:v>Nam</c:v></c:pt></c:strCache></c:strRef></c:cat><c:val><c:numRef><c:f>'Biểu đồ'!${col}$2:${col}$4</c:f><c:numCache><c:formatCode>General</c:formatCode><c:ptCount val="3"/><c:pt idx="0"><c:v>{}</c:v></c:pt><c:pt idx="1"><c:v>20</c:v></c:pt><c:pt idx="2"><c:v>30</c:v></c:pt></c:numCache></c:numRef></c:val></c:ser>"#,
            10 + idx
        )
    };
    let plot = match kind {
        1 => format!(
            r#"<c:barChart><c:barDir val="col"/><c:grouping val="clustered"/><c:varyColors val="0"/>{}{}<c:axId val="1"/><c:axId val="2"/></c:barChart><c:catAx><c:axId val="1"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:axPos val="b"/><c:crossAx val="2"/></c:catAx><c:valAx><c:axId val="2"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:axPos val="l"/><c:crossAx val="1"/></c:valAx>"#,
            ser(0, 'B'),
            ser(1, 'C')
        ),
        2 => format!(
            r#"<c:barChart><c:barDir val="bar"/><c:grouping val="stacked"/>{}<c:overlap val="100"/><c:axId val="1"/><c:axId val="2"/></c:barChart><c:lineChart><c:grouping val="standard"/>{}<c:marker val="1"/><c:axId val="1"/><c:axId val="2"/></c:lineChart>"#,
            ser(0, 'B'),
            ser(1, 'C')
        ),
        _ => format!(
            r#"<c:pieChart><c:varyColors val="1"/>{}<c:firstSliceAng val="0"/></c:pieChart>"#,
            ser(0, 'B')
        ),
    };
    format!(
        r#"{DECL}<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="{NS_R}"><c:roundedCorners val="0"/><c:chart><c:title><c:tx><c:rich><a:bodyPr/><a:p><a:r><a:t>Doanh thu vùng {kind}</a:t></a:r></a:p></c:rich></c:tx><c:overlay val="0"/></c:title><c:autoTitleDeleted val="0"/><c:plotArea><c:layout/>{plot}</c:plotArea><c:legend><c:legendPos val="{}"/></c:legend><c:plotVisOnly val="1"/></c:chart></c:chartSpace>"#,
        ["r", "b", "t"][kind - 1]
    )
}

fn charts(i: usize) -> Book {
    let drawing = format!(
        r#"{DECL}<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><xdr:twoCellAnchor editAs="oneCell"><xdr:from><xdr:col>4</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>{i}</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from><xdr:to><xdr:col>11</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>{}</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to><xdr:graphicFrame macro=""><xdr:nvGraphicFramePr><xdr:cNvPr id="2" name="Biểu đồ {i}"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr><xdr:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/></xdr:xfrm><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:r="{NS_R}" r:id="rId1"/></a:graphicData></a:graphic></xdr:graphicFrame><xdr:clientData/></xdr:twoCellAnchor></xdr:wsDr>"#,
        i + 15
    );
    let drels = format!(
        r#"{DECL}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL}chart" Target="../charts/chart1.xml"/></Relationships>"#
    );
    let mut sst = vec![
        "<t>Vùng</t>".to_owned(),
        "<t>Năm 0</t>".to_owned(),
        "<t>Năm 1</t>".to_owned(),
    ];
    let mut data = String::from(
        r#"<row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c><c r="C1" t="s"><v>2</v></c></row>"#,
    );
    for (k, name) in ["Bắc", "Trung", "Nam"].iter().enumerate() {
        let r = k + 2;
        let _ = write!(
            data,
            r#"<row r="{r}"><c r="A{r}" t="s"><v>{}</v></c><c r="B{r}"><v>{}</v></c><c r="C{r}"><v>{}</v></c></row>"#,
            k + 3,
            10 * (k + 1),
            11 + k
        );
        sst.push(format!("<t>{name}</t>"));
    }
    let body = format!(r#"<sheetData>{data}</sheetData><drawing r:id="rId1"/>"#);
    let mut s = sheet("Biểu đồ", &body);
    s.rels = vec![(
        "rId1".into(),
        "drawing".into(),
        "../drawings/drawing1.xml".into(),
        false,
    )];
    Book {
        sheets: vec![s],
        sst,
        parts: vec![
            (
                "/xl/drawings/drawing1.xml".into(),
                Some("application/vnd.openxmlformats-officedocument.drawing+xml".into()),
                drawing.into_bytes(),
            ),
            (
                "/xl/drawings/_rels/drawing1.xml.rels".into(),
                None,
                drels.into_bytes(),
            ),
            (
                "/xl/charts/chart1.xml".into(),
                Some("application/vnd.openxmlformats-officedocument.drawingml.chart+xml".into()),
                chart_xml(i).into_bytes(),
            ),
        ],
        ..Book::default()
    }
}

/// Charts in one-cell (§20.5.2.24) and absolute (§20.5.2.1) anchors.
fn chart_anchors(i: usize) -> Book {
    let mut b = charts(i);
    let frame = r#"<xdr:graphicFrame macro=""><xdr:nvGraphicFramePr><xdr:cNvPr id="2" name="Biểu đồ neo"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr><xdr:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/></xdr:xfrm><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" r:id="rId1"/></a:graphicData></a:graphic></xdr:graphicFrame><xdr:clientData/>"#;
    let anchor = match i {
        1 => format!(
            r#"<xdr:oneCellAnchor><xdr:from><xdr:col>4</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>1</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from><xdr:ext cx="4572000" cy="2743200"/>{frame}</xdr:oneCellAnchor>"#
        ),
        2 => format!(
            r#"<xdr:absoluteAnchor><xdr:pos x="2438400" y="381000"/><xdr:ext cx="4876800" cy="2895600"/>{frame}</xdr:absoluteAnchor>"#
        ),
        _ => format!(
            r#"<xdr:absoluteAnchor><xdr:pos x="0" y="1143000"/><xdr:ext cx="3657600" cy="2286000"/>{frame}</xdr:absoluteAnchor>"#
        ),
    };
    let drawing = format!(
        r#"{DECL}<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">{anchor}</xdr:wsDr>"#
    );
    b.parts[0].2 = drawing.into_bytes();
    b
}

/// SmartArt in a sheet drawing: `xdr:graphicFrame` with `dgm:relIds`
/// (§20.5.2.16, §21.4.2.6), the four diagram parts and the drawing fallback
/// ([MS-ODRAWXML] `dsp:drawing`) named by `dsp:dataModelExt@relId`.
fn smartart(i: usize) -> Book {
    const DGM: &str = "http://schemas.openxmlformats.org/drawingml/2006/diagram";
    const DSP: &str = "http://schemas.microsoft.com/office/drawing/2008/diagram";
    const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
    const CT_D: &str = "application/vnd.openxmlformats-officedocument.drawingml.";
    let (to_col, to_row) = (5 + 2 * i, 4 + 3 * i);
    let drawing = format!(
        r#"{DECL}<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="{A}"><xdr:twoCellAnchor><xdr:from><xdr:col>1</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>1</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from><xdr:to><xdr:col>{to_col}</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>{to_row}</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to><xdr:graphicFrame macro=""><xdr:nvGraphicFramePr><xdr:cNvPr id="2" name="Sơ đồ {i}"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr><xdr:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/></xdr:xfrm><a:graphic><a:graphicData uri="{DGM}"><dgm:relIds xmlns:dgm="{DGM}" xmlns:r="{NS_R}" r:dm="rId1" r:lo="rId2" r:qs="rId3" r:cs="rId4"/></a:graphicData></a:graphic></xdr:graphicFrame><xdr:clientData/></xdr:twoCellAnchor></xdr:wsDr>"#
    );
    let drels = format!(
        r#"{DECL}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL}diagramData" Target="../diagrams/data1.xml"/><Relationship Id="rId2" Type="{REL}diagramLayout" Target="../diagrams/layout1.xml"/><Relationship Id="rId3" Type="{REL}diagramQuickStyle" Target="../diagrams/quickStyle1.xml"/><Relationship Id="rId4" Type="{REL}diagramColors" Target="../diagrams/colors1.xml"/><Relationship Id="rId5" Type="http://schemas.microsoft.com/office/2007/relationships/diagramDrawing" Target="../diagrams/drawing1.xml"/></Relationships>"#
    );
    let steps = ["Lập dự toán", "Phê duyệt", "Giải ngân", "Quyết toán"];
    let (cx, cy) = (1_400_000i64, 700_000i64);
    let mut shapes = String::new();
    for (k, text) in steps[..=i].iter().enumerate() {
        let x = k as i64 * (cx + 200_000);
        let rgb = ["2F6FB5", "E07B39", "5AA454", "7B1FA2"][k];
        let _ = write!(
            shapes,
            r#"<dsp:sp modelId="{{00000000-0000-0000-0000-0000000000{k:02}}}"><dsp:nvSpPr><dsp:cNvPr id="0" name=""/><dsp:cNvSpPr/></dsp:nvSpPr><dsp:spPr><a:xfrm><a:off x="{x}" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="chevron"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="{rgb}"/></a:solidFill></dsp:spPr><dsp:txBody><a:bodyPr anchor="ctr"/><a:lstStyle/><a:p><a:pPr algn="ctr"/><a:r><a:rPr lang="vi-VN" sz="1100"><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill></a:rPr><a:t>{text}</a:t></a:r></a:p></dsp:txBody></dsp:sp>"#
        );
    }
    let x = |root: &str, inner: &str| {
        format!(r#"{DECL}<{root} xmlns:dgm="{DGM}" xmlns:a="{A}">{inner}</{root}>"#).into_bytes()
    };
    let mut s = sheet(
        "Sơ đồ",
        r#"<sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Quy trình</t></is></c></row></sheetData><drawing r:id="rId1"/>"#,
    );
    s.rels = vec![(
        "rId1".into(),
        "drawing".into(),
        "../drawings/drawing1.xml".into(),
        false,
    )];
    Book {
        sheets: vec![s],
        parts: vec![
            (
                "/xl/drawings/drawing1.xml".into(),
                Some("application/vnd.openxmlformats-officedocument.drawing+xml".into()),
                drawing.into_bytes(),
            ),
            ("/xl/drawings/_rels/drawing1.xml.rels".into(), None, drels.into_bytes()),
            (
                "/xl/diagrams/data1.xml".into(),
                Some(format!("{CT_D}diagramData+xml")),
                x(
                    "dgm:dataModel",
                    &format!(
                        r#"<dgm:ptLst><dgm:pt modelId="{{00000000-0000-0000-0000-000000000001}}" type="doc"/></dgm:ptLst><dgm:extLst><a:ext uri="http://schemas.microsoft.com/office/drawing/2008/diagram"><dsp:dataModelExt xmlns:dsp="{DSP}" relId="rId5" minVer="http://schemas.openxmlformats.org/drawingml/2006/diagram"/></a:ext></dgm:extLst>"#
                    ),
                ),
            ),
            (
                "/xl/diagrams/layout1.xml".into(),
                Some(format!("{CT_D}diagramLayout+xml")),
                x("dgm:layoutDef", "<dgm:layoutNode/>"),
            ),
            (
                "/xl/diagrams/quickStyle1.xml".into(),
                Some(format!("{CT_D}diagramStyle+xml")),
                x("dgm:styleDef", r#"<dgm:styleLbl name="node0"/>"#),
            ),
            (
                "/xl/diagrams/colors1.xml".into(),
                Some(format!("{CT_D}diagramColors+xml")),
                x("dgm:colorsDef", ""),
            ),
            (
                "/xl/diagrams/drawing1.xml".into(),
                Some("application/vnd.ms-office.drawingml.diagramDrawing+xml".into()),
                format!(
                    r#"{DECL}<dsp:drawing xmlns:dgm="{DGM}" xmlns:dsp="{DSP}" xmlns:a="{A}"><dsp:spTree><dsp:nvGrpSpPr><dsp:cNvPr id="0" name=""/><dsp:cNvGrpSpPr/></dsp:nvGrpSpPr><dsp:grpSpPr/>{shapes}</dsp:spTree></dsp:drawing>"#
                )
                .into_bytes(),
            ),
        ],
        ..Book::default()
    }
}

/// Threaded comments ([MS-XLSX] threaded comments part + person part) with the
/// legacy notes (§18.7) Excel keeps next to them; variant `i` has `i` replies
/// and variant 3 a resolved thread.
fn threaded(i: usize) -> Book {
    const NS_TC: &str = "http://schemas.microsoft.com/office/spreadsheetml/2018/threadedcomments";
    let pid = |k: usize| format!("{{6A3E2F10-0000-4000-8000-00000000000{k}}}");
    let cid = |k: usize| format!("{{9C1B7D20-0000-4000-8000-0000000000{k:02}}}");
    let mut tc = String::new();
    let mut legacy = String::new();
    for (t, cell) in ["B2", "D5"].iter().enumerate() {
        let root = cid(10 * t);
        let done = if i == 3 && t == 1 { r#" done="1""# } else { "" };
        let _ = write!(
            tc,
            r#"<threadedComment ref="{cell}" dT="2026-11-0{}T08:30:00.00" personId="{}" id="{root}"{done}><text>Kiểm tra lại ô {cell} &amp; số liệu quý {}</text></threadedComment>"#,
            t + 1,
            pid(1),
            t + 2
        );
        for k in 1..=i {
            let _ = write!(
                tc,
                r#"<threadedComment ref="{cell}" dT="2026-11-0{}T09:0{k}:00.00" personId="{}" id="{}" parentId="{root}"><text>Trả lời {k}: đã sửa</text></threadedComment>"#,
                t + 1,
                pid(1 + k % 2),
                cid(10 * t + k)
            );
        }
        let _ = write!(
            legacy,
            r#"<comment ref="{cell}" authorId="{t}"><text><t>[Threaded comment] Kiểm tra lại ô {cell}</t></text></comment>"#
        );
    }
    let threaded = format!(r#"{DECL}<ThreadedComments xmlns="{NS_TC}">{tc}</ThreadedComments>"#);
    let persons = format!(
        r#"{DECL}<personList xmlns="{NS_TC}" xmlns:x="{NS_MAIN}"><person displayName="Nguyễn Văn An" id="{}" userId="an@example.invalid" providerId="None"/><person displayName="Trần Thị Bình" id="{}" userId="binh@example.invalid" providerId="None"/></personList>"#,
        pid(1),
        pid(2)
    );
    let comments = format!(
        r#"{DECL}<comments xmlns="{NS_MAIN}"><authors><author>tc={}</author><author>tc={}</author></authors><commentList>{legacy}</commentList></comments>"#,
        cid(0),
        cid(10)
    );
    let mut s = sheet(
        "Bình luận",
        r#"<sheetData><row r="2"><c r="B2"><v>120</v></c></row><row r="5"><c r="D5"><v>7</v></c></row></sheetData>"#,
    );
    s.rels = vec![
        (
            "rId1".into(),
            "comments".into(),
            "../comments1.xml".into(),
            false,
        ),
        (
            "rId2".into(),
            "http://schemas.microsoft.com/office/2017/10/relationships/threadedComment".into(),
            "../threadedComments/threadedComment1.xml".into(),
            false,
        ),
    ];
    Book {
        sheets: vec![s],
        parts: vec![
            (
                "/xl/comments1.xml".into(),
                Some(format!("{CT_SML}comments+xml")),
                comments.into_bytes(),
            ),
            (
                "/xl/threadedComments/threadedComment1.xml".into(),
                Some("application/vnd.ms-excel.threadedcomments+xml".into()),
                threaded.into_bytes(),
            ),
            (
                "/xl/persons/person.xml".into(),
                Some("application/vnd.ms-excel.person+xml".into()),
                persons.into_bytes(),
            ),
        ],
        wb_rels: vec![(
            "http://schemas.microsoft.com/office/2017/10/relationships/person".into(),
            "persons/person.xml".into(),
        )],
        ..Book::default()
    }
}

#[test]
fn corpus_threaded_comments_round_trip() {
    for i in 1..=3 {
        let wb = read_xlsx(&build(&threaded(i))).unwrap();
        let tc = &wb.sheets[0].threaded_comments;
        assert_eq!(tc.len(), 2 * (i + 1), "variant {i}");
        assert_eq!(wb.persons.len(), 2);
        assert_eq!(tc[0].cell, m::CellRef::new(1, 1));
        assert_eq!(tc[0].text, "Kiểm tra lại ô B2 & số liệu quý 2");
        assert_eq!(tc[1].parent_id.as_deref(), Some(tc[0].id.as_str()));
        assert_eq!(tc.iter().any(|c| c.done), i == 3);
        let back = read_xlsx(&write_xlsx(&wb).unwrap()).unwrap();
        assert_eq!(back.sheets[0].threaded_comments, *tc);
        assert_eq!(back.persons, wb.persons);
    }
}

#[test]
fn corpus_smartart_is_found() {
    for i in 1..=3 {
        let wb = read_xlsx(&build(&smartart(i))).unwrap();
        let d = &wb.sheets[0].diagrams;
        assert_eq!(d.len(), 1, "variant {i}");
        assert_eq!(d[0].part, "/xl/diagrams/drawing1.xml");
        assert_eq!(d[0].from, m::CellRef::new(1, 1));
        assert_eq!(
            d[0].to,
            Some(m::CellRef::new(4 + 3 * i as u32, 5 + 2 * i as u32))
        );
        assert_eq!(d[0].name, format!("Sơ đồ {i}"));
        // The drawing and diagram parts stay in the package after a save.
        let saved = read_xlsx(&write_xlsx(&wb).unwrap()).unwrap();
        assert_eq!(saved.sheets[0].diagrams, *d);
    }
}

fn extensions(i: usize) -> Book {
    let root = r#" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="x15 xr" xmlns:x15="http://schemas.microsoft.com/office/spreadsheetml/2010/11/main" xmlns:xr="http://schemas.microsoft.com/office/spreadsheetml/2014/revision""#;
    let pre = format!(
        r#"<fileVersion appName="xl" lastEdited="7" lowestEdited="7" rupBuild="2{i}"/><workbookPr defaultThemeVersion="16{i}"/><mc:AlternateContent><mc:Choice Requires="x15"><x15ac:absPath xmlns:x15ac="http://schemas.microsoft.com/office/spreadsheetml/2010/11/ac" url="D:\Tai lieu\{i}\"/></mc:Choice></mc:AlternateContent><xr:revisionPtr revIDLastSave="0" documentId="{i}"/><bookViews><workbookView xWindow="{}" yWindow="0" windowWidth="28800" windowHeight="12300"/></bookViews>"#,
        i * 100
    );
    let post = r#"<calcPr calcId="191029" fullCalcOnLoad="1"/><extLst><ext uri="{140A7094-0E35-4892-8432-C4D2E57EDEB5}"><x15:workbookPr chartTrackingRefBase="1"/></ext></extLst>"#;
    let filter = match i {
        1 => {
            r#"<autoFilter ref="A1:C5"><filterColumn colId="0"><filters blank="1"><filter val="11"/><filter val="21"/></filters></filterColumn></autoFilter>"#
        }
        2 => {
            r#"<autoFilter ref="A1:C5"><filterColumn colId="1"><customFilters and="1"><customFilter operator="greaterThan" val="10"/><customFilter operator="lessThan" val="50"/></customFilters></filterColumn><filterColumn colId="2"><top10 val="3" filterVal="42"/></filterColumn><sortState ref="A2:C5"><sortCondition descending="1" ref="B2:B5"/></sortState></autoFilter>"#
        }
        _ => {
            r#"<autoFilter ref="A1:C5"><filterColumn colId="0"><dynamicFilter type="aboveAverage" val="25"/></filterColumn><filterColumn colId="2" hiddenButton="1"/><extLst><ext uri="{00000000-0000-0000-0000-000000000001}"><x14:filterExt xmlns:x14="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main"/></ext></extLst></autoFilter>"#
        }
    };
    let body = format!(
        r#"<sheetPr codeName="Sheet{i}"/><dimension ref="A1:C5"/><sheetViews><sheetView workbookViewId="0"/></sheetViews><sheetFormatPr defaultRowHeight="15" x14ac:dyDescent="0.25"/><sheetData><row r="1" spans="1:3" x14ac:dyDescent="0.25"><c r="A1"><v>1</v></c></row>{}</sheetData><sheetProtection algorithmName="SHA-512" hashValue="QUJD{i}w==" saltValue="eHl6" spinCount="100000" sheet="1" objects="1" scenarios="1"/>{filter}<mc:AlternateContent><mc:Choice Requires="x14"><controls><mc:AlternateContent><mc:Choice Requires="x14"><control shapeId="1025" r:id="rId9" name="Nút {i}"/></mc:Choice></mc:AlternateContent></controls></mc:Choice></mc:AlternateContent><ignoredErrors><ignoredError sqref="A1:C5" numberStoredAsText="1"/></ignoredErrors><extLst><ext uri="{{CCE6A557-97BC-4b89-ADB6-D9C93CAAB3DF}}" xmlns:x14="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main"><x14:dataValidations count="1" xmlns:xm="http://schemas.microsoft.com/office/excel/2006/main"><x14:dataValidation type="list" allowBlank="1"><x14:formula1><xm:f>Ẩn!$A$1:$A$3</xm:f></x14:formula1><xm:sqref>B1:B5</xm:sqref></x14:dataValidation></x14:dataValidations></ext></extLst>"#,
        rows(5, 3, 0.0).replacen(r#"<row r="1"><c r="A1"><v>10</v></c><c r="B1"><v>11</v></c><c r="C1"><v>12</v></c></row>"#, "", 1)
    );
    let xml = format!(
        r#"{DECL}<worksheet xmlns="{NS_MAIN}" xmlns:r="{NS_R}" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="x14ac xr" xmlns:x14="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main" xmlns:x14ac="http://schemas.microsoft.com/office/spreadsheetml/2009/9/ac" xmlns:xr="http://schemas.microsoft.com/office/spreadsheetml/2014/revision" xr:uid="{{0000000{i}-0000-0000-0000-000000000000}}">{body}</worksheet>"#
    );
    let ctrl = format!(
        r#"{DECL}<formControlPr xmlns="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main" objectType="Button" lockText="1"/>"#
    );
    let s = Sheet {
        name: "Mở rộng".into(),
        state: None,
        xml,
        rels: vec![(
            "rId9".into(),
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/ctrlProp".into(),
            "../ctrlProps/ctrlProp1.xml".into(),
            false,
        )],
    };
    let hidden = Sheet {
        state: Some(if i == 3 { "veryHidden" } else { "hidden" }),
        ..sheet("Ẩn", "<sheetData/>")
    };
    let custom = format!(r#"{DECL}<root xmlns="urn:bluestar:test"><v>{i}</v></root>"#);
    Book {
        sheets: vec![s, hidden],
        wb_root: root.into(),
        wb_pre: pre,
        wb_post: post.into(),
        parts: vec![
            (
                "/xl/ctrlProps/ctrlProp1.xml".into(),
                Some("application/vnd.ms-excel.controlproperties+xml".into()),
                ctrl.into_bytes(),
            ),
            ("/customXml/item1.xml".into(), None, custom.into_bytes()),
        ],
        // ECMA-376 Part 1 §15.2.4: custom XML data is targeted from the workbook part.
        wb_rels: vec![(
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml".into(),
            "../customXml/item1.xml".into(),
        )],
        ..Book::default()
    }
}

/// Sparkline groups in the worksheet `extLst` ([MS-XLSX] `x14:sparklineGroups`).
fn sparklines(i: usize) -> Book {
    const X14: &str = "http://schemas.microsoft.com/office/spreadsheetml/2009/9/main";
    const XM: &str = "http://schemas.microsoft.com/office/excel/2006/main";
    let line = |f: usize| {
        format!(
            r"<x14:sparkline><xm:f>'Dữ liệu'!A{f}:D{f}</xm:f><xm:sqref>F{f}</xm:sqref></x14:sparkline>"
        )
    };
    let groups = match i {
        1 => format!(
            r#"<x14:sparklineGroup displayEmptyCellsAs="gap" markers="1" high="1" low="1"><x14:colorSeries rgb="FF2E6DB4"/><x14:colorNegative rgb="FFD64545"/><x14:colorAxis rgb="FF000000"/><x14:colorMarkers rgb="FF1F4E8C"/><x14:colorFirst theme="5"/><x14:colorLast theme="5" tint="-0.249977111117893"/><x14:colorHigh rgb="FF2FA36B"/><x14:colorLow rgb="FFD64545"/><x14:sparklines>{}{}{}</x14:sparklines></x14:sparklineGroup>"#,
            line(1),
            line(2),
            line(3)
        ),
        2 => format!(
            r#"<x14:sparklineGroup manualMax="50" manualMin="-10" type="column" displayEmptyCellsAs="zero" negative="1" displayXAxis="1" minAxisType="custom" maxAxisType="custom"><x14:colorSeries theme="4"/><x14:colorNegative rgb="FFC00000"/><x14:colorAxis rgb="FF404040"/><x14:sparklines>{}{}</x14:sparklines></x14:sparklineGroup><x14:sparklineGroup type="stacked" displayEmptyCellsAs="gap" first="1" last="1" rightToLeft="1"><x14:colorSeries rgb="FF5AA454"/><x14:colorFirst rgb="FFF28C28"/><x14:colorLast rgb="FFF28C28"/><x14:sparklines>{}</x14:sparklines></x14:sparklineGroup>"#,
            line(1),
            line(2),
            line(3)
        ),
        _ => format!(
            r#"<x14:sparklineGroup lineWeight="1.5" dateAxis="1" displayEmptyCellsAs="span" displayHidden="1" minAxisType="group" maxAxisType="group"><x14:colorSeries rgb="FF7B1FA2"/><x14:colorMarkers rgb="FF7B1FA2"/><xm:f>'Dữ liệu'!A5:D5</xm:f><x14:sparklines>{}{}{}</x14:sparklines></x14:sparklineGroup>"#,
            line(1),
            line(2),
            line(3)
        ),
    };
    // Variant 2 keeps another worksheet ext next to the sparklines.
    let other = if i == 2 {
        format!(
            r#"<ext uri="{{CCE6A557-97BC-4b89-ADB6-D9C93CAAB3DF}}" xmlns:x14="{X14}"><x14:dataValidations count="1" xmlns:xm="{XM}"><x14:dataValidation type="list" allowBlank="1"><x14:formula1><xm:f>'Dữ liệu'!$A$1:$A$3</xm:f></x14:formula1><xm:sqref>G1</xm:sqref></x14:dataValidation></x14:dataValidations></ext>"#
        )
    } else {
        String::new()
    };
    let mut data = rows(3, 4, f64::from(i as u32) - 25.0);
    // Row 5: dates (serials) for the date axis; B2 empty for the empty-cell rules.
    data = data.replace(r#"<c r="B2"><v>"#, r#"<c r="B2" s="0"><v>"#);
    data = data.replacen(
        &format!(
            r#"<c r="B2" s="0"><v>{}</v></c>"#,
            f64::from(i as u32) - 25.0 + 21.0
        ),
        "",
        1,
    );
    data.push_str(r#"<row r="5"><c r="A5"><v>45292</v></c><c r="B5"><v>45299</v></c><c r="C5"><v>45320</v></c><c r="D5"><v>45351</v></c></row>"#);
    let body = format!(
        r#"<sheetData>{data}</sheetData><extLst>{other}<ext uri="{{05C60535-1F16-4fd2-B633-F4F36F0B64E0}}" xmlns:x14="{X14}"><x14:sparklineGroups xmlns:xm="{XM}">{groups}</x14:sparklineGroups></ext></extLst>"#
    );
    Book {
        sheets: vec![sheet("Dữ liệu", &body)],
        ..Book::default()
    }
}

type Feature = (&'static str, &'static str, fn(usize) -> Book);

const FEATURES: &[Feature] = &[
    ("values", "numbers, booleans, errors, dates (t=\"d\", 1904 system), normal/shared/array formulas with cached values", values),
    ("sst", "shared strings with rich-text runs, xml:space, _xHHHH_ escapes", sst),
    ("styles", "numFmts, fonts, pattern/gradient fills, borders, cellXfs, cellStyleXfs, cellStyles, dxfs", styles),
    ("layout", "column widths, row heights, hidden rows/cols, outline levels, merges", layout),
    ("panes", "frozen rows, frozen rows+cols, split panes, selections, zoom, hidden gridlines/headers", panes),
    ("names", "defined names (workbook and sheet scope), print area, print titles", names),
    ("page", "printOptions, pageMargins, pageSetup, headerFooter variants, row/col breaks", page),
    ("links", "external (relationship) and internal hyperlinks, tooltips", links),
    ("comments", "comments part with rich text + VML legacy drawing", comments),
    ("validation", "data validation: list, whole, decimal, date, textLength, custom", validation),
    ("cf", "conditional formatting: cellIs, expression, colorScale, dataBar, iconSet, top10, text, x14 ext", conditional),
    ("tables", "tables (ListObject) with autoFilter, totals row, calculated columns, style info", tables),
    ("charts", "drawing + chartSpace parts (bar, bar+line combo, pie)", charts),
    ("chart-anchors", "charts in a oneCellAnchor (from + ext) and absoluteAnchor (pos + ext)", chart_anchors),
    ("ext", "extLst, mc:AlternateContent, x14ac attributes, sheet autoFilter + sortState, controls, customXml", extensions),
    ("smartart", "SmartArt: dgm:relIds graphic frame, diagram parts and dsp:drawing fallback", smartart),
    ("threaded", "threaded comments + replies + resolved thread ([MS-XLSX] threadedComments, persons) with legacy notes", threaded),
    ("sparklines", "sparklines ([MS-XLSX] x14:sparklineGroups): line with markers/high/low and theme colours; column with custom axis + negative and win/loss right-to-left next to a data validation ext; line on a date axis with group axes, span and hidden data", sparklines),
];

// ---------------------------------------------------------------------------
// Files produced by the BlueSheet writer itself (editing features)
// ---------------------------------------------------------------------------

use bo_model::sheet as m;

/// Cell formats as the Format Cells dialog / ribbon writes them: deduplicated
/// fonts, solid fills, borders, alignment and vi-VN number formats.
fn writer_format(i: usize) -> m::Workbook {
    let mut wb = m::Workbook::new();
    let st = &mut wb.styles;
    let codes = ["#,##0 [$₫-42A]", "dd/mm/yyyy", "0.00%"];
    let num = st.add_num_fmt(codes[i - 1]);
    let bold = st.add_font(m::Font {
        bold: true,
        italic: i == 2,
        underline: (i == 3).then(|| "single".into()),
        color: Some(m::SheetColor::rgb("1F3A5F")),
        ..st.fonts[0].clone()
    });
    let fill = st.add_fill(m::Fill::solid(["FFEE00", "D8F3E3", "FADBD8"][i - 1]));
    let edge = m::BorderEdge {
        style: Some(["thin", "medium", "dashed"][i - 1].into()),
        color: None,
    };
    let border = st.add_border(m::Border {
        top: edge.clone(),
        bottom: edge.clone(),
        left: edge.clone(),
        right: edge,
        ..m::Border::default()
    });
    let xf = st.add_xf(m::CellXf {
        num_fmt_id: num,
        font_id: bold,
        fill_id: fill,
        border_id: border,
        xf_id: Some(0),
        apply_number_format: Some(true),
        apply_font: Some(true),
        apply_fill: Some(true),
        apply_border: Some(true),
        apply_alignment: Some(true),
        alignment: Some(m::CellAlignment {
            horizontal: Some("center".into()),
            vertical: Some("center".into()),
            wrap_text: i == 2,
            indent: (i == 3).then_some(1),
            ..m::CellAlignment::default()
        }),
        ..m::CellXf::default()
    });
    // A named style of the gallery (§18.8.7 builtinId 26 = Good).
    let good_font = st.add_font(m::Font {
        color: Some(m::SheetColor::rgb("1E6B3C")),
        ..st.fonts[0].clone()
    });
    let good_fill = st.add_fill(m::Fill::solid("D8F3E3"));
    st.cell_style_xfs.push(m::CellXf {
        font_id: good_font,
        fill_id: good_fill,
        ..m::CellXf::default()
    });
    st.cell_styles.push(m::CellStyle {
        name: "Good".into(),
        xf_id: 1,
        builtin_id: Some(26),
        ..m::CellStyle::default()
    });
    let good = st.add_xf(m::CellXf {
        font_id: good_font,
        fill_id: good_fill,
        xf_id: Some(1),
        ..m::CellXf::default()
    });
    let ws = &mut wb.sheets[0];
    for r in 0..5u32 {
        ws.set_cell(
            r,
            0,
            m::Cell::new(m::CellValue::Number(45_658.0 + f64::from(r) * 31.5)).with_style(xf),
        );
        ws.set_cell(
            r,
            1,
            m::Cell::new(m::CellValue::Text(format!("Mục {r}"))).with_style(good),
        );
    }
    wb
}

/// Rows/columns, freeze panes, merges, AutoFilter, CF, validation and page setup
/// as written after editing.
fn writer_sheet(i: usize) -> m::Workbook {
    let mut wb = m::Workbook::new();
    wb.styles.dxfs.push(m::Dxf {
        fill: Some(m::Fill::Pattern(m::PatternFill {
            pattern: Some("solid".into()),
            fg: Some(m::SheetColor::rgb("FADBD8")),
            bg: Some(m::SheetColor::rgb("FADBD8")),
        })),
        ..m::Dxf::default()
    });
    let ws = &mut wb.sheets[0];
    ws.set_cell(0, 0, m::Cell::new(m::CellValue::Text("Tên".into())));
    ws.set_cell(0, 1, m::Cell::new(m::CellValue::Text("Điểm".into())));
    for r in 1..=10u32 {
        ws.set_cell(r, 0, m::Cell::new(m::CellValue::Text(format!("HS {r}"))));
        ws.set_cell(
            r,
            1,
            m::Cell::new(m::CellValue::Number(f64::from(r % 4 + 6))),
        );
    }
    ws.cols.push(m::ColInfo {
        min: 0,
        max: 0,
        width: Some(18.5),
        custom_width: true,
        ..m::ColInfo::default()
    });
    ws.rows.insert(
        0,
        m::RowInfo {
            height: Some(24.0),
            custom_height: true,
            ..m::RowInfo::default()
        },
    );
    ws.views.push(m::SheetView {
        tab_selected: true,
        pane: Some(m::Pane {
            x_split: if i == 2 { 1.0 } else { 0.0 },
            y_split: 1.0,
            top_left_cell: Some(if i == 2 { "B2" } else { "A2" }.into()),
            active_pane: Some(if i == 2 { "bottomRight" } else { "bottomLeft" }.into()),
            state: m::PaneState::Frozen,
        }),
        ..m::SheetView::default()
    });
    ws.merges.push(m::CellRange::parse("D1:F1").unwrap());
    ws.set_cell(0, 3, m::Cell::new(m::CellValue::Text("Bảng điểm".into())));
    let range = m::CellRange::parse("A1:B11").unwrap();
    ws.auto_filter = Some(m::AutoFilter {
        range,
        columns: if i == 3 {
            vec![m::FilterColumn {
                col_id: 1,
                hidden_button: false,
                show_button: None,
                filter: m::ColumnFilter::Custom {
                    and: false,
                    conditions: vec![m::CustomFilter {
                        operator: Some("greaterThan".into()),
                        val: "7".into(),
                    }],
                },
            }]
        } else {
            Vec::new()
        },
        ..m::AutoFilter::default()
    });
    let cfvo = |k: &str, v: Option<&str>| m::Cfvo {
        kind: k.into(),
        val: v.map(str::to_owned),
        gte: None,
    };
    ws.conditional_formats.push(m::ConditionalFormat {
        sqref: vec![m::CellRange::parse("B2:B11").unwrap()],
        pivot: false,
        rules: vec![
            m::CfRule {
                kind: "cellIs".into(),
                priority: 1,
                dxf_id: Some(0),
                operator: Some("lessThan".into()),
                formulas: vec!["7".into()],
                ..m::CfRule::default()
            },
            match i {
                1 => m::CfRule {
                    kind: "colorScale".into(),
                    priority: 2,
                    color_scale: Some(m::ColorScale {
                        cfvos: vec![cfvo("min", None), cfvo("max", None)],
                        colors: vec![m::SheetColor::rgb("FFFFFF"), m::SheetColor::rgb("2E6DB4")],
                    }),
                    ..m::CfRule::default()
                },
                2 => m::CfRule {
                    kind: "dataBar".into(),
                    priority: 2,
                    data_bar: Some(m::DataBar {
                        cfvos: vec![cfvo("min", None), cfvo("max", None)],
                        color: Some(m::SheetColor::rgb("4A7BB7")),
                        ..m::DataBar::default()
                    }),
                    ..m::CfRule::default()
                },
                _ => m::CfRule {
                    kind: "iconSet".into(),
                    priority: 2,
                    icon_set: Some(m::IconSet {
                        icon_set: Some("3Arrows".into()),
                        cfvos: vec![
                            cfvo("percent", Some("0")),
                            cfvo("percent", Some("33")),
                            cfvo("percent", Some("66")),
                        ],
                        ..m::IconSet::default()
                    }),
                    ..m::CfRule::default()
                },
            },
        ],
    });
    ws.data_validations.push(m::DataValidation {
        sqref: vec![m::CellRange::parse("C2:C11").unwrap()],
        kind: Some("list".into()),
        allow_blank: true,
        show_error_message: true,
        error: Some("Chọn Đạt hoặc Chưa đạt".into()),
        formula1: Some("\"Đạt,Chưa đạt\"".into()),
        ..m::DataValidation::default()
    });
    ws.print_titles = Some("Sheet1!$1:$1".into());
    ws.print_area = (i == 2).then(|| "Sheet1!$A$1:$B$11".into());
    ws.page_setup.orientation = Some(if i == 1 { "landscape" } else { "portrait" }.into());
    ws.page_setup.paper_size = Some(9);
    ws.page_setup.margins = Some(m::PageMargins::default());
    if i == 3 {
        ws.props.present = true;
        ws.props.fit_to_page = Some(true);
        ws.page_setup.fit_to_width = Some(1);
        ws.page_setup.fit_to_height = Some(0);
    } else {
        ws.page_setup.scale = Some(90);
    }
    wb
}

/// Charts inserted with "Insert chart" (new drawing + `c:chartSpace` parts).
fn writer_charts(i: usize) -> m::Workbook {
    let mut wb = m::Workbook::new();
    let ws = &mut wb.sheets[0];
    ws.set_cell(0, 0, m::Cell::new(m::CellValue::Text("Tỉnh".into())));
    ws.set_cell(0, 1, m::Cell::new(m::CellValue::Text("2024".into())));
    ws.set_cell(0, 2, m::Cell::new(m::CellValue::Text("2025".into())));
    let names = ["Hà Nội", "Huế", "Đà Nẵng"];
    for (r, n) in names.iter().enumerate() {
        let r = r as u32 + 1;
        ws.set_cell(r, 0, m::Cell::new(m::CellValue::Text((*n).into())));
        ws.set_cell(
            r,
            1,
            m::Cell::new(m::CellValue::Number(f64::from(r) * 10.0)),
        );
        ws.set_cell(
            r,
            2,
            m::Cell::new(m::CellValue::Number(f64::from(r) * 12.5)),
        );
    }
    let series = |c: u32, name: &str| m::ChartSeries {
        name: Some(name.into()),
        name_ref: Some(format!("Sheet1!${}$1", m::col_to_letters(c))),
        cat_ref: Some("Sheet1!$A$2:$A$4".into()),
        val_ref: Some(format!("Sheet1!${0}$2:${0}$4", m::col_to_letters(c))),
        categories: names.iter().map(|s| (*s).to_owned()).collect(),
        values: (1..=3)
            .map(|r| Some(f64::from(r) * if c == 1 { 10.0 } else { 12.5 }))
            .collect(),
        ..m::ChartSeries::default()
    };
    let plot = |kind: m::ChartKind,
                dir: Option<&str>,
                grouping: Option<&str>,
                s: Vec<m::ChartSeries>| m::ChartPlot {
        kind,
        bar_dir: dir.map(str::to_owned),
        grouping: grouping.map(str::to_owned),
        series: s,
        ..m::ChartPlot::default()
    };
    let plots = match i {
        1 => vec![plot(
            m::ChartKind::Bar,
            Some("col"),
            Some("clustered"),
            vec![series(1, "2024"), series(2, "2025")],
        )],
        2 => vec![
            plot(
                m::ChartKind::Bar,
                Some("col"),
                Some("clustered"),
                vec![series(1, "2024")],
            ),
            plot(
                m::ChartKind::Line,
                None,
                Some("standard"),
                vec![series(2, "2025")],
            ),
        ],
        _ => vec![plot(
            m::ChartKind::Doughnut,
            None,
            None,
            vec![series(1, "2024")],
        )],
    };
    ws.charts.push(m::Chart {
        from: m::CellRef::new(0, 4),
        to: Some(m::CellRef::new(15, 11)),
        title: Some("Doanh thu theo tỉnh".into()),
        x_title: (i != 3).then(|| "Tỉnh".into()),
        y_title: (i != 3).then(|| "Tỷ đồng".into()),
        legend: Some(if i == 2 { "b" } else { "r" }.into()),
        plots,
        dirty: true,
        ..m::Chart::default()
    });
    if i == 1 {
        let mut scatter = ws.charts[0].clone();
        scatter.from = m::CellRef::new(17, 4);
        scatter.to = Some(m::CellRef::new(32, 11));
        scatter.plots = vec![plot(
            m::ChartKind::Scatter,
            None,
            None,
            vec![series(1, "2024")],
        )];
        ws.charts.push(scatter);
    }
    wb
}

/// Charts of the CORE-CHART-002 types inserted with `bo_sheet::chart::new_chart` (each file one variant).
fn writer_chart_kind(group: &str, i: usize) -> m::Workbook {
    let mut wb = m::Workbook::new();
    let text = |s: &str| m::Cell::new(m::CellValue::Text(s.into()));
    let num = |v: f64| m::Cell::new(m::CellValue::Number(v));
    let (kind, rows): (&str, Vec<Vec<m::Cell>>) = match group {
        "bubble" => {
            // X, then (Y, size) pairs; the third file has negative values.
            let head: Vec<&str> = if i == 2 {
                vec![
                    "Giá",
                    "Doanh số A",
                    "Thị phần A",
                    "Doanh số B",
                    "Thị phần B",
                ]
            } else {
                vec!["Giá", "Doanh số", "Thị phần"]
            };
            let mut rows = vec![head.iter().map(|h| text(h)).collect::<Vec<_>>()];
            for r in 1..=5u32 {
                let x = f64::from(r) * 10.0;
                let sign = if i == 3 && r % 2 == 0 { -1.0 } else { 1.0 };
                let mut row = vec![
                    num(x),
                    num(sign * f64::from(r * r) * 3.0),
                    num(f64::from(r) * 4.0),
                ];
                if i == 2 {
                    row.push(num(f64::from(6 - r) * 7.0));
                    row.push(num(f64::from(r) * 2.5));
                }
                rows.push(row);
            }
            ("bubble", rows)
        }
        "stock" => {
            // HLC (1), OHLC (2), OHLC with up/down bars (3).
            let ohlc = i >= 2;
            let mut head = vec![text("Phiên")];
            if ohlc {
                head.push(text("Mở cửa"));
            }
            head.extend(["Cao nhất", "Thấp nhất", "Đóng cửa"].map(text));
            let mut rows = vec![head];
            let closes = [21.5, 22.8, 22.1, 23.9, 24.4, 23.6];
            for (k, c) in closes.iter().enumerate() {
                let open = if k == 0 { 21.0 } else { closes[k - 1] };
                let mut row = vec![text(&format!("Ngày {}", k + 1))];
                if ohlc {
                    row.push(num(open));
                }
                row.extend([num(c.max(open) + 0.8), num(c.min(open) - 0.7), num(*c)]);
                rows.push(row);
            }
            (if i == 3 { "stockUpDown" } else { "stock" }, rows)
        }
        _ => {
            let kind = match (group, i) {
                ("percent", 1) => "percentColumn",
                ("percent", 2) => "percentBar",
                ("percent", _) => "percentArea",
                ("radar", 1) => "radar",
                ("radar", 2) => "radarMarker",
                ("radar", _) => "radarFilled",
                ("surface", 2) => "surfaceWireframe",
                ("surface", _) => "surface",
                ("ofpie", 1) => "pieOfPie",
                ("ofpie", 2) => "barOfPie",
                ("3dbar", 1) => "column3D",
                ("3dbar", 2) => "cylinder3D",
                ("3dbar", _) => "stackedBar3D",
                ("3dline", 1) => "line3D",
                ("3dline", 2) => "area3D",
                ("3dline", _) => "percentArea3D",
                ("3dpie", 1) => "pie3D",
                ("3dpie", 2) => "explodedPie3D",
                ("3dpie", _) => "surface3D",
                _ => "explodedPie",
            };
            let cats = ["Hà Nội", "Hải Phòng", "Huế", "Đà Nẵng", "Cần Thơ", "TP.HCM"];
            let series = if group == "ofpie" || (group == "3dpie" && i < 3) {
                1
            } else if group == "surface" && i == 3 {
                5
            } else {
                3
            };
            let mut head = vec![text("Tỉnh")];
            for k in 0..series {
                head.push(text(&format!("Năm {}", 2023 + k)));
            }
            let mut rows = vec![head];
            for (r, c) in cats.iter().enumerate() {
                let mut row = vec![text(c)];
                for k in 0..series {
                    let v = ((r as f64 + 1.0) * 7.0 + k as f64 * 5.0) % 37.0 + 3.0;
                    row.push(num(v));
                }
                rows.push(row);
            }
            (kind, rows)
        }
    };
    let ws = &mut wb.sheets[0];
    let (nr, nc) = (rows.len() as u32, rows[0].len() as u32);
    for (r, row) in rows.into_iter().enumerate() {
        for (c, cell) in row.into_iter().enumerate() {
            ws.set_cell(r as u32, c as u32, cell);
        }
    }
    let range = m::CellRange::new(m::CellRef::new(0, 0), m::CellRef::new(nr - 1, nc - 1));
    let mut chart = bo_sheet::chart::new_chart(&wb, 0, range, kind).expect("chart");
    chart.title = Some(format!("Biểu đồ {kind}"));
    wb.sheets[0].charts.push(chart);
    wb
}

/// Chartex charts ([MS-ODRAWXML] 2.24) inserted with `bo_sheet::chart::new_chart` (CORE-CHART-004).
fn writer_chartex(kind: &str, i: usize) -> m::Workbook {
    let mut wb = m::Workbook::new();
    let text = |s: &str| m::Cell::new(m::CellValue::Text(s.into()));
    let num = |v: f64| m::Cell::new(m::CellValue::Number(v));
    let rows: Vec<Vec<m::Cell>> = match kind {
        "waterfall" => {
            let items: &[(&str, f64)] = match i {
                1 => &[
                    ("Đầu kỳ", 120.0),
                    ("Doanh thu", 80.0),
                    ("Chi phí", -45.0),
                    ("Thuế", -15.0),
                    ("Cuối kỳ", 140.0),
                ],
                2 => &[
                    ("T1", 30.0),
                    ("T2", -12.0),
                    ("T3", 25.0),
                    ("T4", -8.0),
                    ("T5", 18.0),
                    ("T6", -20.0),
                ],
                _ => &[
                    ("Vốn", 500.0),
                    ("Lãi", 60.0),
                    ("Lỗ", -90.0),
                    ("Tổng", 470.0),
                ],
            };
            let mut r = vec![vec![text("Khoản mục"), text("Giá trị")]];
            r.extend(items.iter().map(|(n, v)| vec![text(n), num(*v)]));
            r
        }
        "treemap" | "sunburst" => {
            let items: &[(&str, &str, &str, f64)] = &[
                ("Miền Bắc", "Hà Nội", "Ba Đình", 34.0),
                ("Miền Bắc", "Hà Nội", "Cầu Giấy", 28.0),
                ("Miền Bắc", "Hải Phòng", "Lê Chân", 19.0),
                ("Miền Trung", "Đà Nẵng", "Hải Châu", 22.0),
                ("Miền Trung", "Huế", "Phú Hội", 11.0),
                ("Miền Nam", "TP.HCM", "Quận 1", 41.0),
                ("Miền Nam", "TP.HCM", "Thủ Đức", 37.0),
                ("Miền Nam", "Cần Thơ", "Ninh Kiều", 16.0),
            ];
            // Variants: two levels, three levels, two levels of a subset.
            let three = i == 2;
            let take = if i == 3 { 5 } else { items.len() };
            let mut head = vec![text("Vùng"), text("Tỉnh")];
            if three {
                head.push(text("Quận"));
            }
            head.push(text("Doanh số"));
            let mut r = vec![head];
            for (a, b, c, v) in &items[..take] {
                let mut row = vec![text(a), text(b)];
                if three {
                    row.push(text(c));
                }
                row.push(num(*v));
                r.push(row);
            }
            r
        }
        "histogram" => {
            // Deterministic scores 40..100 (a linear congruential sequence).
            let n = [30, 60, 15][i - 1];
            let mut r = vec![vec![text("Điểm")]];
            let mut x: u64 = 7 + i as u64;
            for _ in 0..n {
                x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345) % 2_147_483_648;
                r.push(vec![num(40.0 + (x % 61) as f64)]);
            }
            r
        }
        "pareto" => {
            let items: &[(&str, f64)] = match i {
                1 => &[
                    ("Trễ hạn", 42.0),
                    ("Sai số liệu", 18.0),
                    ("Thiếu chữ ký", 9.0),
                    ("Khác", 5.0),
                ],
                2 => &[
                    ("A", 3.0),
                    ("B", 27.0),
                    ("C", 14.0),
                    ("D", 8.0),
                    ("E", 21.0),
                ],
                _ => &[("Lỗi in", 12.0), ("Lỗi máy", 12.0), ("Lỗi người", 30.0)],
            };
            let mut r = vec![vec![text("Nguyên nhân"), text("Số lần")]];
            r.extend(items.iter().map(|(n, v)| vec![text(n), num(*v)]));
            r
        }
        "boxWhisker" => {
            let series = if i == 2 { 2 } else { 1 };
            let mut head = vec![text("Lớp")];
            for k in 0..series {
                head.push(text(&format!("Môn {}", k + 1)));
            }
            let mut r = vec![head];
            for (g, base) in [("10A", 6.0), ("10B", 7.0), ("10C", 5.5)] {
                for k in 0..8 {
                    let mut row = vec![text(g)];
                    for sidx in 0..series {
                        let v = base + f64::from((k * 7 + sidx * 3) % 9) * 0.5 - 2.0;
                        // An outlier in the third file.
                        let v = if i == 3 && k == 7 { v + 6.0 } else { v };
                        row.push(num(v));
                    }
                    r.push(row);
                }
            }
            r
        }
        _ => {
            let items: &[(&str, f64)] = match i {
                1 => &[
                    ("Truy cập", 5000.0),
                    ("Đăng ký", 1800.0),
                    ("Dùng thử", 900.0),
                    ("Mua", 240.0),
                ],
                2 => &[("Hồ sơ", 320.0), ("Phỏng vấn", 96.0), ("Đề nghị", 21.0)],
                _ => &[
                    ("Bước 1", 100.0),
                    ("Bước 2", 80.0),
                    ("Bước 3", 60.0),
                    ("Bước 4", 40.0),
                    ("Bước 5", 20.0),
                ],
            };
            let mut r = vec![vec![text("Giai đoạn"), text("Số lượng")]];
            r.extend(items.iter().map(|(n, v)| vec![text(n), num(*v)]));
            r
        }
    };
    let ws = &mut wb.sheets[0];
    let (nr, nc) = (rows.len() as u32, rows[0].len() as u32);
    for (r, row) in rows.into_iter().enumerate() {
        for (c, cell) in row.into_iter().enumerate() {
            ws.set_cell(r as u32, c as u32, cell);
        }
    }
    let range = m::CellRange::new(m::CellRef::new(0, 0), m::CellRef::new(nr - 1, nc - 1));
    let mut chart = bo_sheet::chart::new_chart(&wb, 0, range, kind).expect("chart");
    chart.title = Some(format!("Biểu đồ {kind} {i}"));
    if kind == "waterfall" && i != 2 {
        // The last row is a total (cx:subtotals).
        let s = &mut chart.plots[0].series[0];
        s.subtotals = vec![s.values.len() - 1];
    }
    if kind == "histogram" && i == 2 {
        chart.plots[0].style = Some("count:6".into());
    }
    wb.sheets[0].charts.push(chart);
    wb
}

fn writer_chartex_waterfall(i: usize) -> m::Workbook {
    writer_chartex("waterfall", i)
}
fn writer_chartex_treemap(i: usize) -> m::Workbook {
    writer_chartex("treemap", i)
}
fn writer_chartex_sunburst(i: usize) -> m::Workbook {
    writer_chartex("sunburst", i)
}
fn writer_chartex_histogram(i: usize) -> m::Workbook {
    writer_chartex("histogram", i)
}
fn writer_chartex_pareto(i: usize) -> m::Workbook {
    writer_chartex("pareto", i)
}
fn writer_chartex_box(i: usize) -> m::Workbook {
    writer_chartex("boxWhisker", i)
}
fn writer_chartex_funnel(i: usize) -> m::Workbook {
    writer_chartex("funnel", i)
}

/// Chart components (CORE-CHART-005): trend lines with equation / R², error bars, drop lines, high-low
/// lines, up/down bars, a data table, a logarithmic value axis and a date axis.
fn writer_chart_components(i: usize) -> m::Workbook {
    let mut wb = m::Workbook::new();
    let ws = &mut wb.sheets[0];
    let text = |s: &str| m::Cell::new(m::CellValue::Text(s.into()));
    let num = |v: f64| m::Cell::new(m::CellValue::Number(v));
    ws.set_cell(0, 0, text("Ngày"));
    ws.set_cell(0, 1, text("Kế hoạch"));
    ws.set_cell(0, 2, text("Thực hiện"));
    for r in 1..=6u32 {
        let x = f64::from(r);
        // Dates as serial numbers for the date axis (1 March 2026 = 46082).
        ws.set_cell(
            r,
            0,
            if i == 3 {
                num(46_081.0 + x)
            } else {
                text(&format!("Kỳ {r}"))
            },
        );
        ws.set_cell(
            r,
            1,
            num(if i == 2 {
                3.0 * 2f64.powf(x)
            } else {
                10.0 + 2.0 * x
            }),
        );
        ws.set_cell(
            r,
            2,
            num(if i == 2 {
                2.0 * 3f64.powf(x * 0.7)
            } else {
                9.0 + 2.5 * x - (x % 2.0)
            }),
        );
    }
    let kind = if i == 2 { "column" } else { "line" };
    // The date column holds numbers: it is given as the categories explicitly.
    let range = if i == 3 { "B1:C7" } else { "A1:C7" };
    let mut chart = bo_sheet::chart::new_chart(&wb, 0, m::CellRange::parse(range).unwrap(), kind)
        .expect("chart");
    if i == 3 {
        for s in &mut chart.plots[0].series {
            s.cat_ref = Some("Sheet1!$A$2:$A$7".into());
            s.categories = (1..=6).map(|r| (46_081 + r).to_string()).collect();
        }
    }
    chart.title = Some(format!("Thành phần biểu đồ {i}"));
    let tl = |kind: &str, order: u32| m::Trendline {
        kind: kind.into(),
        order,
        disp_eq: true,
        disp_r2: true,
    };
    let eb = |value_type: &str, value: f64| m::ErrorBars {
        kind: "both".into(),
        value_type: value_type.into(),
        value,
    };
    let p = &mut chart.plots[0];
    match i {
        1 => {
            p.series[0].trendlines.push(tl("linear", 0));
            p.series[1].err_bars = Some(eb("fixedVal", 1.0));
            p.drop_lines = true;
        }
        2 => {
            p.series[0].trendlines.push(tl("exp", 0));
            p.series[1].trendlines.push(tl("poly", 2));
            p.series[1].err_bars = Some(eb("percentage", 10.0));
            chart.axes.log_base = Some(10.0);
            chart.axes.data_table = true;
        }
        _ => {
            p.series[0].trendlines.push(tl("movingAvg", 2));
            p.series[1].err_bars = Some(eb("stdDev", 1.0));
            p.hi_low_lines = true;
            p.flag = true;
            chart.axes.date = true;
        }
    }
    chart.dirty = true;
    wb.sheets[0].charts.push(chart);
    wb
}

fn writer_charts_percent(i: usize) -> m::Workbook {
    writer_chart_kind("percent", i)
}
fn writer_charts_bubble(i: usize) -> m::Workbook {
    writer_chart_kind("bubble", i)
}
fn writer_charts_radar(i: usize) -> m::Workbook {
    writer_chart_kind("radar", i)
}
fn writer_charts_stock(i: usize) -> m::Workbook {
    writer_chart_kind("stock", i)
}
fn writer_charts_surface(i: usize) -> m::Workbook {
    writer_chart_kind("surface", i)
}
fn writer_charts_ofpie(i: usize) -> m::Workbook {
    writer_chart_kind("ofpie", i)
}
fn writer_charts_3dbar(i: usize) -> m::Workbook {
    writer_chart_kind("3dbar", i)
}
fn writer_charts_3dline(i: usize) -> m::Workbook {
    writer_chart_kind("3dline", i)
}
fn writer_charts_3dpie(i: usize) -> m::Workbook {
    writer_chart_kind("3dpie", i)
}

/// PivotTables created with "Insert → Pivot table" (`bo_sheet::pivot`): cache
/// definition + records + pivot table definition parts (§18.10).
fn writer_pivots(i: usize) -> m::Workbook {
    use bo_sheet::pivot::{create_pivot, PivotLabels, PivotSpec};
    let mut wb = m::Workbook::new();
    wb.sheets[0].name = "Dữ liệu".into();
    let ws = &mut wb.sheets[0];
    for (c, h) in ["Vùng", "Sản phẩm", "Năm", "Doanh thu", "Số lượng"]
        .iter()
        .enumerate()
    {
        ws.set_cell(0, c as u32, m::Cell::new(m::CellValue::Text((*h).into())));
    }
    let regions = ["Bắc", "Trung", "Nam"];
    let products = ["Gạo", "Cà phê", "Chè", "Tiêu"];
    for r in 0..24u32 {
        let row = r + 1;
        let t = |s: &str| m::Cell::new(m::CellValue::Text(s.into()));
        let n = |v: f64| m::Cell::new(m::CellValue::Number(v));
        ws.set_cell(row, 0, t(regions[(r % 3) as usize]));
        ws.set_cell(row, 1, t(products[(r % 4) as usize]));
        ws.set_cell(row, 2, n(f64::from(2023 + r % 2)));
        ws.set_cell(row, 3, n(f64::from(r * 37 % 100) * 1.5));
        ws.set_cell(row, 4, n(f64::from(r % 7 + 1)));
    }
    let source = m::CellRange::parse("A1:E25").unwrap();
    let labels = PivotLabels {
        grand_total: "Tổng cộng".into(),
        values: "Giá trị".into(),
        all: "(Tất cả)".into(),
        multiple: "(Nhiều mục)".into(),
        blank: "(trống)".into(),
        column: "Cột".into(),
        name: "BangTongHop".into(),
        ..PivotLabels::default()
    };
    let value = |field: u32, function: m::PivotFunction, name: &str| m::PivotDataField {
        field,
        function,
        name: Some(name.into()),
        ..m::PivotDataField::default()
    };
    let (dest_sheet, dest, spec) = match i {
        1 => (
            0,
            m::CellRef::new(0, 7),
            PivotSpec {
                rows: vec![0],
                values: vec![value(3, m::PivotFunction::Sum, "Tổng Doanh thu")],
                ..PivotSpec::default()
            },
        ),
        2 => {
            wb.sheets.push(m::Worksheet::new("Tổng hợp"));
            (
                1,
                m::CellRef::new(0, 0),
                PivotSpec {
                    rows: vec![0],
                    cols: vec![1],
                    values: vec![value(3, m::PivotFunction::Sum, "Tổng Doanh thu")],
                    filters: vec![m::PivotFilter {
                        field: 2,
                        selected: vec![m::CellValue::Number(2024.0)],
                    }],
                    ..PivotSpec::default()
                },
            )
        }
        _ => {
            wb.sheets.push(m::Worksheet::new("Báo cáo"));
            (
                1,
                m::CellRef::new(2, 1),
                PivotSpec {
                    rows: vec![1, 0],
                    values: vec![
                        value(3, m::PivotFunction::Sum, "Tổng Doanh thu"),
                        value(4, m::PivotFunction::Average, "TB Số lượng"),
                        value(4, m::PivotFunction::Count, "Số dòng"),
                        value(3, m::PivotFunction::Max, "Lớn nhất"),
                    ],
                    filters: vec![m::PivotFilter {
                        field: 0,
                        selected: vec![
                            m::CellValue::Text("Bắc".into()),
                            m::CellValue::Text("Nam".into()),
                        ],
                    }],
                    ..PivotSpec::default()
                },
            )
        }
    };
    create_pivot(&mut wb, 0, source, dest_sheet, dest, &spec, &labels).unwrap();
    wb
}

/// Source data of the report-feature pivots: 30 sales (region, product, date,
/// revenue, quantity) from March 2023 to January 2025.
fn pivot_book() -> m::Workbook {
    let mut wb = m::Workbook::new();
    wb.sheets[0].name = "Dữ liệu".into();
    let date_xf = wb.styles.add_xf(m::CellXf {
        num_fmt_id: 14,
        xf_id: Some(0),
        apply_number_format: Some(true),
        ..m::CellXf::default()
    });
    let ws = &mut wb.sheets[0];
    for (c, h) in ["Vùng", "Sản phẩm", "Ngày", "Doanh thu", "Số lượng"]
        .iter()
        .enumerate()
    {
        ws.set_cell(0, c as u32, m::Cell::new(m::CellValue::Text((*h).into())));
    }
    let regions = ["Bắc", "Trung", "Nam"];
    let products = ["Gạo", "Cà phê", "Chè", "Tiêu"];
    for r in 0..30u32 {
        let row = r + 1;
        let t = |s: &str| m::Cell::new(m::CellValue::Text(s.into()));
        let n = |v: f64| m::Cell::new(m::CellValue::Number(v));
        ws.set_cell(row, 0, t(regions[(r % 3) as usize]));
        ws.set_cell(row, 1, t(products[(r * 7 % 4) as usize]));
        // 45000 = 2023-03-15 (1900 date system).
        ws.set_cell(row, 2, n(f64::from(45_000 + r * 23)).with_style(date_xf));
        ws.set_cell(row, 3, n(f64::from(r * 37 % 100) * 10.0 + 50.0));
        ws.set_cell(row, 4, n(f64::from(r % 5 + 1)));
    }
    wb.sheets.push(m::Worksheet::new("Báo cáo"));
    wb
}

fn pivot_labels() -> bo_sheet::pivot::PivotLabels {
    bo_sheet::pivot::PivotLabels {
        grand_total: "Tổng cộng".into(),
        values: "Giá trị".into(),
        blank: "(trống)".into(),
        name: "BangTongHop".into(),
        subtotal: "Tổng {item}".into(),
        row_labels: "Nhãn hàng".into(),
        col_labels: "Nhãn cột".into(),
        months: (1..=12).map(|i| format!("Thg {i}")).collect(),
        quarter: "Quý {n}".into(),
        years_field: "Năm".into(),
        quarters_field: "Quý".into(),
        months_field: "Tháng".into(),
        ..bo_sheet::pivot::PivotLabels::default()
    }
}

fn pivot_value(field: u32, function: m::PivotFunction, name: &str) -> m::PivotDataField {
    m::PivotDataField {
        field,
        function,
        name: Some(name.into()),
        ..m::PivotDataField::default()
    }
}

fn field_opts(f: &dyn Fn(&mut m::PivotFieldOptions)) -> m::PivotFieldOptions {
    let mut o = m::PivotFieldOptions::default();
    f(&mut o);
    o
}

fn pivot_style(name: &str, rows: bool, cols: bool) -> m::PivotStyleInfo {
    m::PivotStyleInfo {
        name: name.into(),
        show_row_headers: true,
        show_col_headers: true,
        show_row_stripes: rows,
        show_col_stripes: cols,
        show_last_column: true,
    }
}

/// Pivot report settings (§18.10.1.73): subtotals, grand totals, compact /
/// outline / tabular layouts, repeated labels, blank rows, sorting, label /
/// value / top-N filters, "show values as", number formats and styles.
fn writer_pivot_totals(i: usize) -> m::Workbook {
    use bo_sheet::pivot::{create_pivot, PivotSpec};
    let mut wb = pivot_book();
    let labels = pivot_labels();
    let source = m::CellRange::parse("A1:E31").unwrap();
    let revenue = pivot_value(3, m::PivotFunction::Sum, "Tổng Doanh thu");
    let specs: Vec<(m::CellRef, PivotSpec)> = match i {
        1 => vec![
            (
                m::CellRef::new(0, 0),
                PivotSpec {
                    rows: vec![0, 1],
                    values: vec![revenue.clone()],
                    fields: vec![field_opts(&|o| {
                        o.repeat_labels = true;
                        o.blank_row = true;
                    })],
                    ..PivotSpec::default()
                },
            ),
            (
                m::CellRef::new(0, 5),
                PivotSpec {
                    cols: vec![0, 1],
                    values: vec![revenue.clone()],
                    row_grand_totals: false,
                    ..PivotSpec::default()
                },
            ),
        ],
        2 => vec![(
            m::CellRef::new(1, 1),
            PivotSpec {
                rows: vec![0, 1],
                values: vec![revenue.clone()],
                layout: m::PivotLayout::Outline,
                fields: vec![
                    field_opts(&|o| o.subtotal_top = false),
                    field_opts(&|o| {
                        o.sort = m::PivotSort::Descending;
                        o.sort_by = Some(0);
                    }),
                ],
                field_filters: vec![
                    m::PivotFieldFilter {
                        field: 1,
                        kind: m::PivotFilterType::Count,
                        top: true,
                        count: 2.0,
                        ..m::PivotFieldFilter::default()
                    },
                    m::PivotFieldFilter {
                        field: 0,
                        kind: m::PivotFilterType::CaptionNotEqual,
                        value1: "Trung".into(),
                        top: true,
                        ..m::PivotFieldFilter::default()
                    },
                ],
                style: Some(pivot_style("PivotStyleMedium9", true, false)),
                ..PivotSpec::default()
            },
        )],
        _ => vec![(
            m::CellRef::new(0, 0),
            PivotSpec {
                rows: vec![0, 1],
                values: vec![
                    m::PivotDataField {
                        num_fmt_id: Some(3),
                        ..revenue.clone()
                    },
                    m::PivotDataField {
                        show_as: m::PivotShowAs::PercentOfTotal,
                        ..pivot_value(3, m::PivotFunction::Sum, "% tổng")
                    },
                    m::PivotDataField {
                        show_as: m::PivotShowAs::PercentOfParentRow,
                        ..pivot_value(3, m::PivotFunction::Sum, "% nhóm")
                    },
                    m::PivotDataField {
                        show_as: m::PivotShowAs::RankDescending,
                        base_field: 1,
                        ..pivot_value(4, m::PivotFunction::Sum, "Hạng SL")
                    },
                ],
                layout: m::PivotLayout::Compact,
                fields: vec![
                    m::PivotFieldOptions::default(),
                    field_opts(&|o| o.hidden = vec![m::CellValue::Text("Tiêu".into())]),
                ],
                field_filters: vec![m::PivotFieldFilter {
                    field: 0,
                    kind: m::PivotFilterType::ValueGreaterThan,
                    measure: 0,
                    value1: "1000".into(),
                    top: true,
                    ..m::PivotFieldFilter::default()
                }],
                style: Some(pivot_style("PivotStyleDark2", false, true)),
                ..PivotSpec::default()
            },
        )],
    };
    for (at, spec) in specs {
        create_pivot(&mut wb, 0, source, 1, at, &spec, &labels).unwrap();
    }
    wb
}

/// Pivot grouping and calculations (§18.10.1.27 fieldGroup, §18.10.1.3
/// calculated fields, §18.18.70 showDataAs): dates by years / quarters /
/// months, numeric ranges, a calculated field, difference and running total.
fn writer_pivot_groups(i: usize) -> m::Workbook {
    use bo_sheet::pivot::{
        create_pivot, group_field, set_calculated_field, GroupRequest, PivotSpec,
    };
    let mut wb = pivot_book();
    let labels = pivot_labels();
    let source = m::CellRange::parse("A1:E31").unwrap();
    let revenue = pivot_value(3, m::PivotFunction::Sum, "Tổng Doanh thu");
    let at = m::CellRef::new(0, 0);
    match i {
        1 => {
            let spec = PivotSpec {
                rows: vec![2],
                values: vec![revenue],
                layout: m::PivotLayout::Outline,
                ..PivotSpec::default()
            };
            create_pivot(&mut wb, 0, source, 1, at, &spec, &labels).unwrap();
            let dates = GroupRequest::Dates {
                years: true,
                quarters: true,
                months: true,
                days: false,
            };
            group_field(&mut wb, 1, 0, 2, &dates, &labels).unwrap();
        }
        2 => {
            let spec = PivotSpec {
                rows: vec![3],
                cols: vec![0],
                values: vec![pivot_value(4, m::PivotFunction::Count, "Số giao dịch")],
                ..PivotSpec::default()
            };
            create_pivot(&mut wb, 0, source, 1, at, &spec, &labels).unwrap();
            let ranges = GroupRequest::Numbers {
                start: Some(0.0),
                end: Some(1000.0),
                interval: 250.0,
            };
            group_field(&mut wb, 1, 0, 3, &ranges, &labels).unwrap();
        }
        _ => {
            let spec = PivotSpec {
                rows: vec![2],
                values: vec![
                    revenue.clone(),
                    m::PivotDataField {
                        show_as: m::PivotShowAs::Difference,
                        base_field: 2,
                        base_item: m::PIVOT_BASE_PREVIOUS,
                        ..pivot_value(3, m::PivotFunction::Sum, "Chênh lệch")
                    },
                    m::PivotDataField {
                        show_as: m::PivotShowAs::RunTotal,
                        base_field: 2,
                        ..pivot_value(3, m::PivotFunction::Sum, "Lũy kế")
                    },
                ],
                layout: m::PivotLayout::Compact,
                ..PivotSpec::default()
            };
            create_pivot(&mut wb, 0, source, 1, at, &spec, &labels).unwrap();
            let dates = GroupRequest::Dates {
                years: true,
                quarters: false,
                months: true,
                days: false,
            };
            group_field(&mut wb, 1, 0, 2, &dates, &labels).unwrap();
            set_calculated_field(
                &mut wb,
                1,
                0,
                None,
                "Đơn giá",
                "='Doanh thu'/'Số lượng'",
                &labels,
            )
            .unwrap();
        }
    }
    wb
}

/// Calculated items (§18.10.1.8) on row and column fields.
fn writer_pivot_items(i: usize) -> m::Workbook {
    use bo_sheet::pivot::{create_pivot, set_calculated_item, PivotSpec};
    let mut wb = pivot_book();
    let labels = pivot_labels();
    let source = m::CellRange::parse("A1:E31").unwrap();
    let revenue = pivot_value(3, m::PivotFunction::Sum, "Tổng Doanh thu");
    let at = m::CellRef::new(0, 0);
    let (spec, items): (PivotSpec, Vec<(u32, &str, &str)>) = match i {
        1 => (
            PivotSpec {
                rows: vec![0],
                values: vec![revenue],
                ..PivotSpec::default()
            },
            vec![(0, "Bắc + Nam", "='Bắc'+'Nam'")],
        ),
        2 => (
            PivotSpec {
                rows: vec![0],
                cols: vec![1],
                values: vec![revenue],
                layout: m::PivotLayout::Outline,
                ..PivotSpec::default()
            },
            vec![(1, "Đồ uống", "='Cà phê'+'Chè'")],
        ),
        _ => (
            PivotSpec {
                rows: vec![0, 1],
                values: vec![revenue],
                layout: m::PivotLayout::Compact,
                ..PivotSpec::default()
            },
            vec![
                (0, "Bắc − Nam", "='Bắc'-'Nam'"),
                (0, "Trung ×2", "=Trung*2"),
            ],
        ),
    };
    create_pivot(&mut wb, 0, source, 1, at, &spec, &labels).unwrap();
    for (f, name, formula) in items {
        set_calculated_item(&mut wb, 1, 0, f, None, name, formula, &labels).unwrap();
    }
    wb
}

/// Manual grouping of items (§18.10.1.26 discretePr, §18.10.1.27 fieldGroup).
fn writer_pivot_manual(i: usize) -> m::Workbook {
    use bo_sheet::pivot::{create_pivot, group_items, ungroup_items, PivotSpec};
    let mut wb = pivot_book();
    let labels = bo_sheet::pivot::PivotLabels {
        group: "Nhóm {n}".into(),
        ..pivot_labels()
    };
    let source = m::CellRange::parse("A1:E31").unwrap();
    let revenue = pivot_value(3, m::PivotFunction::Sum, "Tổng Doanh thu");
    let at = m::CellRef::new(0, 0);
    match i {
        1 => {
            let spec = PivotSpec {
                rows: vec![0],
                values: vec![revenue],
                ..PivotSpec::default()
            };
            create_pivot(&mut wb, 0, source, 1, at, &spec, &labels).unwrap();
            // Regions: Bắc (0), Nam (1), Trung (2).
            group_items(&mut wb, 1, 0, 0, &[0, 2], &labels).unwrap();
        }
        2 => {
            let spec = PivotSpec {
                rows: vec![0],
                cols: vec![1],
                values: vec![revenue],
                ..PivotSpec::default()
            };
            create_pivot(&mut wb, 0, source, 1, at, &spec, &labels).unwrap();
            // Products (cache order): Chè (0), Cà phê (1), Gạo (2), Tiêu (3).
            let g = group_items(&mut wb, 1, 0, 1, &[0, 1], &labels).unwrap();
            group_items(&mut wb, 1, 0, g, &[1, 2], &labels).unwrap();
        }
        _ => {
            let spec = PivotSpec {
                rows: vec![1],
                values: vec![revenue],
                layout: m::PivotLayout::Outline,
                ..PivotSpec::default()
            };
            create_pivot(&mut wb, 0, source, 1, at, &spec, &labels).unwrap();
            let g = group_items(&mut wb, 1, 0, 1, &[0, 1, 3], &labels).unwrap();
            // Take Tiêu out of the group again.
            ungroup_items(&mut wb, 1, 0, 1, &[3], &labels).unwrap();
            let _ = g;
        }
    }
    wb
}

/// Date filters (§18.18.60) on a date row field; a 1904-system workbook
/// grouped by years (dates written as xsd:dateTime, §18.17.4.1).
fn writer_pivot_dates(i: usize) -> m::Workbook {
    use bo_sheet::pivot::{create_pivot, group_field, GroupRequest, PivotSpec};
    let mut wb = pivot_book();
    let labels = pivot_labels();
    let source = m::CellRange::parse("A1:E31").unwrap();
    let revenue = pivot_value(3, m::PivotFunction::Sum, "Tổng Doanh thu");
    let filter = |kind: m::PivotFilterType, a: &str, b: &str| m::PivotFieldFilter {
        field: 2,
        kind,
        value1: a.into(),
        value2: b.into(),
        top: true,
        ..m::PivotFieldFilter::default()
    };
    let flt = match i {
        1 => vec![filter(m::PivotFilterType::DateBetween, "45078", "45291")],
        2 => vec![filter(m::PivotFilterType::Q2, "", "")],
        _ => vec![filter(
            m::PivotFilterType::DateNewerThanOrEqual,
            "45292",
            "",
        )],
    };
    if i == 3 {
        // Same calendar dates in the 1904 system (serial − 1462).
        wb.date1904 = true;
        for (&(r, c), cell) in &mut wb.sheets[0].cells {
            if let (true, m::CellValue::Number(n)) = (r > 0 && c == 2, &mut cell.value) {
                *n -= 1462.0;
            }
        }
    }
    let flt: Vec<m::PivotFieldFilter> = flt
        .into_iter()
        .map(|mut f| {
            if i == 3 {
                f.value1 = "43830".into();
            }
            f
        })
        .collect();
    let spec = PivotSpec {
        rows: vec![2],
        values: vec![revenue.clone()],
        field_filters: flt,
        ..PivotSpec::default()
    };
    create_pivot(&mut wb, 0, source, 1, m::CellRef::new(0, 0), &spec, &labels).unwrap();
    if i == 3 {
        let spec = PivotSpec {
            rows: vec![2],
            values: vec![revenue],
            ..PivotSpec::default()
        };
        create_pivot(&mut wb, 0, source, 1, m::CellRef::new(0, 4), &spec, &labels).unwrap();
        let years = GroupRequest::Dates {
            years: true,
            quarters: false,
            months: false,
            days: false,
        };
        group_field(&mut wb, 1, 1, 2, &years, &labels).unwrap();
    }
    wb
}

/// Several value fields on the row axis (§18.10.1.73 dataOnRows).
fn writer_pivot_datarows(i: usize) -> m::Workbook {
    use bo_sheet::pivot::{create_pivot, PivotSpec};
    let mut wb = pivot_book();
    let labels = pivot_labels();
    let source = m::CellRange::parse("A1:E31").unwrap();
    let mut values = vec![
        pivot_value(3, m::PivotFunction::Sum, "Tổng Doanh thu"),
        pivot_value(4, m::PivotFunction::Sum, "Tổng Số lượng"),
    ];
    let (cols, layout) = match i {
        1 => (vec![], m::PivotLayout::Tabular),
        2 => (vec![1], m::PivotLayout::Compact),
        _ => {
            values.push(pivot_value(3, m::PivotFunction::Average, "TB Doanh thu"));
            (vec![], m::PivotLayout::Outline)
        }
    };
    let spec = PivotSpec {
        rows: vec![0],
        cols,
        values,
        layout,
        data_on_rows: true,
        ..PivotSpec::default()
    };
    create_pivot(&mut wb, 0, source, 1, m::CellRef::new(0, 0), &spec, &labels).unwrap();
    wb
}

/// Pivot charts (§21.2.2.142 pivotSource, §21.2.2.144 pivotFmts) plotting a
/// pivot table and following it.
fn writer_pivot_charts(i: usize) -> m::Workbook {
    use bo_sheet::pivot::{create_pivot, update_pivot, PivotSpec};
    let mut wb = pivot_book();
    let labels = pivot_labels();
    let source = m::CellRange::parse("A1:E31").unwrap();
    let revenue = pivot_value(3, m::PivotFunction::Sum, "Tổng Doanh thu");
    let (spec, kind) = match i {
        1 => (
            PivotSpec {
                rows: vec![0],
                values: vec![revenue],
                ..PivotSpec::default()
            },
            "column",
        ),
        2 => (
            PivotSpec {
                rows: vec![1],
                cols: vec![0],
                values: vec![revenue],
                ..PivotSpec::default()
            },
            "line",
        ),
        _ => (
            PivotSpec {
                rows: vec![0],
                values: vec![revenue, pivot_value(4, m::PivotFunction::Sum, "Tổng SL")],
                ..PivotSpec::default()
            },
            "combo",
        ),
    };
    create_pivot(&mut wb, 0, source, 1, m::CellRef::new(0, 0), &spec, &labels).unwrap();
    let mut chart = bo_sheet::chart::new_pivot_chart(&wb, 1, 0, kind).unwrap();
    chart.title = Some(format!("Biểu đồ tổng hợp {i}"));
    chart.pivot_buttons = i == 3;
    wb.sheets[1].charts.push(chart);
    if i == 2 {
        // The chart follows a filter on the pivot (hidden product).
        let mut spec = PivotSpec::of(&wb.sheets[1].pivots[0]);
        spec.fields = vec![
            m::PivotFieldOptions::default(),
            field_opts(&|o| o.hidden = vec![m::CellValue::Text("Tiêu".into())]),
        ];
        update_pivot(&mut wb, 1, 0, &spec, &labels).unwrap();
    }
    wb
}

type WriterFeature = (&'static str, &'static str, fn(usize) -> m::Workbook);

const WRITER_FEATURES: &[WriterFeature] = &[
    ("writer-format", "BlueSheet writer: cell formats (vi-VN number formats, fonts, fills, borders, alignment) and a gallery cell style", writer_format),
    ("writer-sheet", "BlueSheet writer: column width / row height, freeze panes, merge, AutoFilter, conditional formats, list validation, page setup + print titles", writer_sheet),
    ("writer-charts", "BlueSheet writer: inserted charts (column, column+line combo, doughnut, scatter) with axis titles and legend", writer_charts),
    ("writer-charts-components", "BlueSheet writer: chart components (linear trend line, fixed error bars, drop lines; exponential and polynomial trend lines, percentage error bars, data table, log axis; moving average, standard-deviation error bars, high-low lines, up/down bars, date axis)", writer_chart_components),
    ("writer-charts-percent", "BlueSheet writer: 100 % stacked column, bar and area charts", writer_charts_percent),
    ("writer-charts-bubble", "BlueSheet writer: bubble charts (one and two series of X, Y and size; negative values)", writer_charts_bubble),
    ("writer-charts-radar", "BlueSheet writer: radar charts (standard, with markers, filled)", writer_charts_radar),
    ("writer-charts-stock", "BlueSheet writer: stock charts (high-low-close; open-high-low-close; with up/down bars)", writer_charts_stock),
    ("writer-charts-surface", "BlueSheet writer: surface charts (3 series; wireframe; 5 series)", writer_charts_surface),
    ("writer-charts-ofpie", "BlueSheet writer: pie of pie, bar of pie and an exploded pie", writer_charts_ofpie),
    ("writer-charts-3dbar", "BlueSheet writer: 3-D columns (series one behind the other), 3-D cylinders, stacked 3-D bars", writer_charts_3dbar),
    ("writer-charts-3dline", "BlueSheet writer: 3-D line, 3-D area, 100 % stacked 3-D area", writer_charts_3dline),
    ("writer-charts-3dpie", "BlueSheet writer: 3-D pie, exploded 3-D pie, 3-D surface", writer_charts_3dpie),
    ("writer-chartex-waterfall", "BlueSheet writer: chartex waterfall (with a total; ups and downs; with a total)", writer_chartex_waterfall),
    ("writer-chartex-treemap", "BlueSheet writer: chartex treemap (2 levels; 3 levels; 2 levels, 5 points)", writer_chartex_treemap),
    ("writer-chartex-sunburst", "BlueSheet writer: chartex sunburst (2 levels; 3 levels; 2 levels, 5 points)", writer_chartex_sunburst),
    ("writer-chartex-histogram", "BlueSheet writer: chartex histogram (automatic bins; 6 bins; 15 values)", writer_chartex_histogram),
    ("writer-chartex-pareto", "BlueSheet writer: chartex pareto (categories sorted by count, cumulative line)", writer_chartex_pareto),
    ("writer-chartex-box", "BlueSheet writer: chartex box & whisker (one series; two series; an outlier)", writer_chartex_box),
    ("writer-chartex-funnel", "BlueSheet writer: chartex funnel (4, 3 and 5 stages)", writer_chartex_funnel),
    ("writer-pivots", "BlueSheet writer: pivot tables (rows + Sum; rows × columns with a report filter on another sheet; two row fields, four value fields, multi-item filter)", writer_pivots),
    ("writer-pivot-totals", "BlueSheet writer: pivot report settings (tabular with repeated labels + blank rows and column subtotals; outline with bottom subtotals, sort by value, top-N + label filter, medium style; compact with number format, % of total / parent, rank, value filter, hidden item, dark style)", writer_pivot_totals),
    ("writer-pivot-groups", "BlueSheet writer: pivot grouping and calculations (dates by years / quarters / months; numeric ranges × columns; years + months with a calculated field, difference from previous and running total)", writer_pivot_groups),
    ("writer-pivot-items", "BlueSheet writer: calculated items (row field; column field in outline form; two items in compact form)", writer_pivot_items),
    ("writer-pivot-manual", "BlueSheet writer: manual item groups (rows; two groups on a column field; group then ungroup one item)", writer_pivot_manual),
    ("writer-pivot-dates", "BlueSheet writer: pivot date filters (dateBetween; Q2; dateNewerThanOrEqual in a 1904 workbook with a year grouping)", writer_pivot_dates),
    ("writer-pivot-datarows", "BlueSheet writer: value fields on rows (tabular; compact × columns; outline with three values)", writer_pivot_datarows),
    ("writer-pivot-charts", "BlueSheet writer: pivot charts (column; line following a hidden item; combo with field buttons)", writer_pivot_charts),
    ("writer-slicers", "BlueSheet writer: table slicers (one slicer; filtered to one item; two slicers on two columns, both filtered)", writer_slicers),
    ("writer-sparklines", "BlueSheet writer: inserted sparklines (line group with markers; column group with custom axis and a win/loss group; a line group ungrouped, then two of its sparklines regrouped as columns)", writer_sparklines),
];

/// Sparklines inserted and edited with `bo_sheet::sparkline` (each file one variant).
fn writer_sparklines(i: usize) -> m::Workbook {
    use bo_sheet::sparkline::{self as sp, SparklinePatch};
    let mut wb = m::Workbook::new();
    wb.sheets[0].name = "Doanh số".into();
    let vals = [
        [5.0, 8.0, 3.0, 9.0],
        [-2.0, 4.0, -1.0, 6.0],
        [7.0, 7.0, 2.0, -3.0],
    ];
    for (r, row) in vals.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            wb.sheets[0].set_cell(r as u32, c as u32, m::Cell::new(m::CellValue::Number(*v)));
        }
    }
    let r = |s: &str| m::CellRange::parse(s).unwrap();
    match i {
        1 => {
            let g = sp::insert(&mut wb, 0, m::SparklineType::Line, "A1:D3", r("F1:F3")).unwrap();
            let p = SparklinePatch {
                markers: Some(true),
                high: Some(true),
                low: Some(true),
                ..SparklinePatch::default()
            };
            sp::edit(&mut wb, 0, g, &p).unwrap();
        }
        2 => {
            let g = sp::insert(&mut wb, 0, m::SparklineType::Column, "A1:D2", r("F1:F2")).unwrap();
            let p = SparklinePatch {
                negative: Some(true),
                display_x_axis: Some(true),
                min_axis: Some(m::SparklineAxis::Custom),
                max_axis: Some(m::SparklineAxis::Custom),
                manual_min: Some(-5.0),
                manual_max: Some(10.0),
                ..SparklinePatch::default()
            };
            sp::edit(&mut wb, 0, g, &p).unwrap();
            sp::insert(
                &mut wb,
                0,
                m::SparklineType::Stacked,
                "'Doanh số'!A3:D3",
                r("F3"),
            )
            .unwrap();
        }
        _ => {
            sp::insert(&mut wb, 0, m::SparklineType::Line, "A1:D3", r("F1:F3")).unwrap();
            sp::ungroup(&mut wb, 0, r("F2")).unwrap();
            let g = sp::group(&mut wb, 0, r("F2:F3")).unwrap();
            let p = SparklinePatch {
                kind: Some(m::SparklineType::Column),
                ..SparklinePatch::default()
            };
            sp::edit(&mut wb, 0, g, &p).unwrap();
        }
    }
    wb
}

/// Table slicers inserted with `bo_sheet::slicer` (each file one variant).
fn writer_slicers(i: usize) -> m::Workbook {
    let mut ws = m::Worksheet::new("Bán hàng");
    let rows = [
        ("Vùng", "Kênh", "Doanh số"),
        ("Bắc", "Online", "10"),
        ("Nam", "Cửa hàng", "20"),
        ("Bắc", "Cửa hàng", "30"),
        ("Trung", "Online", "40"),
        ("Nam", "Online", "50"),
    ];
    for (r, (a, b, c)) in rows.iter().enumerate() {
        let r = r as u32;
        ws.set_cell(r, 0, m::Cell::new(m::CellValue::Text((*a).into())));
        ws.set_cell(r, 1, m::Cell::new(m::CellValue::Text((*b).into())));
        let v = c
            .parse::<f64>()
            .map_or_else(|_| m::CellValue::Text((*c).into()), m::CellValue::Number);
        ws.set_cell(r, 2, m::Cell::new(v));
    }
    ws.tables.push(m::Table {
        id: 1,
        name: "BanHang".into(),
        display_name: "BanHang".into(),
        range: m::CellRange::parse("A1:C6").unwrap(),
        columns: ["Vùng", "Kênh", "Doanh số"]
            .iter()
            .enumerate()
            .map(|(k, n)| m::TableColumn {
                id: k as u32 + 1,
                name: (*n).into(),
                ..m::TableColumn::default()
            })
            .collect(),
        style: Some(m::TableStyleInfo {
            name: Some("TableStyleMedium2".into()),
            show_row_stripes: true,
            ..m::TableStyleInfo::default()
        }),
        ..m::Table::default()
    });
    let mut wb = m::Workbook::new();
    wb.sheets = vec![ws];
    let locale = bo_sheet::numfmt::Locale::VI_VN;
    let labels = bo_sheet::pivot::PivotLabels::default();
    let a = bo_sheet::slicer::insert_table_slicer(&mut wb, 0, 1, 1, m::CellRef::new(0, 4)).unwrap();
    if i >= 2 {
        bo_sheet::slicer::select(&mut wb, 0, a, &["Bắc".into()], &locale, &labels).unwrap();
    }
    if i == 3 {
        let b =
            bo_sheet::slicer::insert_table_slicer(&mut wb, 0, 1, 2, m::CellRef::new(0, 8)).unwrap();
        bo_sheet::slicer::select(&mut wb, 0, b, &["Online".into()], &locale, &labels).unwrap();
    }
    wb
}

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/sheet")
}

#[test]
fn generate_corpus() {
    if std::env::var("BO_WRITE_CORPUS").as_deref() != Ok("1") {
        return;
    }
    let dir = corpus_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let mut readme = String::from(
        "# Corpus BlueSheet (`tests/corpus/sheet`)\n\n\
         Toàn bộ file trong thư mục này do BlueOffice **tự sinh** từ SpreadsheetML viết tay\n\
         (theo ECMA-376 Part 1, §18) trong `crates/bo-xlsx/tests/corpus.rs` — không chứa dữ liệu\n\
         khách hàng, không tạo bằng phần mềm của bên thứ ba. Sinh lại:\n\n\
         ```bash\nBO_WRITE_CORPUS=1 cargo test -p bo-xlsx --test corpus generate_corpus\n```\n\n\
         Kiểm thử round-trip: `cargo test -p bo-xlsx --test corpus corpus_roundtrip -- --nocapture`.\n\n\
         | File | Tính năng | Nguồn gốc |\n|---|---|---|\n",
    );
    for (name, desc, f) in FEATURES {
        for i in 1..=3 {
            let file = format!("{name}-{i}.xlsx");
            std::fs::write(dir.join(&file), build(&f(i))).unwrap();
            let _ = writeln!(
                readme,
                "| `{file}` | {desc} (biến thể {i}) | tự sinh, `corpus.rs::{name}` |"
            );
        }
    }
    for (name, desc, f) in WRITER_FEATURES {
        for i in 1..=3 {
            let file = format!("{name}-{i}.xlsx");
            std::fs::write(dir.join(&file), write_xlsx(&f(i)).unwrap()).unwrap();
            let fname = name.replace('-', "_");
            let _ = writeln!(
                readme,
                "| `{file}` | {desc} (biến thể {i}) | tự sinh bằng `write_xlsx`, `corpus.rs::{fname}` |"
            );
        }
    }
    // A timeline over the pivot table with dates ([MS-XLSX] timeline parts added by hand).
    std::fs::write(dir.join("timeline-1.xlsx"), timeline_fixture()).unwrap();
    let _ = writeln!(
        readme,
        "| `timeline-1.xlsx` | Timeline (x15 timelineCacheDefinition + timelines part) over a pivot table's date field, filtered to Q2 2023 | tự sinh, `corpus.rs::timeline_fixture` |"
    );
    std::fs::write(dir.join("README.md"), readme).unwrap();
}

/// `writer-pivot-dates` variant 1 with a timeline on its date field "Ngày".
fn timeline_fixture() -> Vec<u8> {
    const X15: &str = "http://schemas.microsoft.com/office/spreadsheetml/2010/11/main";
    let wb = writer_pivot_dates(1);
    let bytes = write_xlsx(&wb).unwrap();
    let (si, pivot) = wb
        .sheets
        .iter()
        .enumerate()
        .find_map(|(i, s)| s.pivots.first().map(|p| (i, p.name.clone())))
        .unwrap();
    let mut pkg = Package::read(&bytes, &Limits::default()).unwrap();
    let wb_part = "/xl/workbook.xml";
    let sheet_rel = pkg
        .relationships(wb_part)
        .unwrap()
        .into_iter()
        .filter(|r| r.rel_type.ends_with("/worksheet"))
        .nth(si)
        .unwrap();
    let sheet_part = resolve_target(wb_part, &sheet_rel.target);
    let cache = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<timelineCacheDefinition xmlns="{X15}" name="NativeTimeline_Ngay" sourceName="Ngày"><pivotTables><pivotTable tabId="{}" name="{pivot}"/></pivotTables><state minimalRefreshVersion="6" lastRefreshVersion="6" pivotCacheId="1" filterType="dateBetween"><selection startDate="2023-04-01T00:00:00" endDate="2023-06-30T00:00:00"/><bounds startDate="2023-01-01T00:00:00" endDate="2025-01-01T00:00:00"/></state></timelineCacheDefinition>"#,
        si + 1
    );
    pkg.add_part(
        "/xl/timelineCaches/timelineCache1.xml",
        "application/vnd.ms-excel.timelineCache+xml",
        cache.into_bytes(),
    )
    .unwrap();
    let timelines = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<timelines xmlns="{X15}"><timeline name="Ngày" cache="NativeTimeline_Ngay" caption="Ngày" level="1" selectionLevel="1"/></timelines>"#
    );
    pkg.add_part(
        "/xl/timelines/timeline1.xml",
        "application/vnd.ms-excel.timeline+xml",
        timelines.into_bytes(),
    )
    .unwrap();
    let mut wrels = pkg.relationships(wb_part).unwrap();
    wrels.push(bo_opc::Relationship {
        id: "rId90".into(),
        rel_type: bo_xlsx::timeline::REL_TIMELINE_CACHE.into(),
        target: "timelineCaches/timelineCache1.xml".into(),
        mode: bo_opc::TargetMode::Internal,
    });
    pkg.set_relationships(wb_part, &wrels);
    let mut srels = pkg.relationships(&sheet_part).unwrap();
    srels.push(bo_opc::Relationship {
        id: "rId90".into(),
        rel_type: bo_xlsx::timeline::REL_TIMELINE.into(),
        target: "../timelines/timeline1.xml".into(),
        mode: bo_opc::TargetMode::Internal,
    });
    pkg.set_relationships(&sheet_part, &srels);
    pkg.write().unwrap()
}

// ---------------------------------------------------------------------------
// Lossless comparison
// ---------------------------------------------------------------------------

/// Attributes that are recomputed by design (counts, `spans` hints, `dimension`).
fn ignored_attr(path: &str, attr: &str) -> bool {
    matches!(attr, "count" | "uniqueCount" | "spans") || path.ends_with("/dimension")
}

/// Attributes written with their schema default value carry no information.
fn is_default(path: &str, attr: &str, value: &str) -> bool {
    let el = path.rsplit('/').next().unwrap_or(path);
    match (el, attr) {
        // §18.3.1.4 c/@s (default 0); §18.3.1.70 printOptions booleans (default false).
        ("c", "s") => value == "0",
        ("printOptions", _) => value == "0" || value == "false",
        _ => false,
    }
}

fn rel_map(pkg: &Package, part: &str) -> BTreeMap<String, String> {
    pkg.relationships(part)
        .unwrap_or_default()
        .into_iter()
        .map(|r| {
            let target = if r.mode == bo_opc::TargetMode::External {
                r.target.clone()
            } else {
                resolve_target(part, &r.target)
            };
            (r.id, format!("{}→{}", r.rel_type, target))
        })
        .collect()
}

/// Numbers compare by value (`1E+11` = `100000000000`).
fn norm(v: &str) -> String {
    match v.trim().parse::<f64>() {
        Ok(n) if n.is_finite() && !v.trim().is_empty() => format!("{n}"),
        // A one-cell range `B1:B1` is the reference `B1`.
        _ => match v.split_once(':') {
            Some((a, b)) if a == b && !a.is_empty() => a.to_owned(),
            _ => v.to_owned(),
        },
    }
}

/// Facts of an XML part: `path@attr=value` and `path#text=…`.
fn facts(pkg: &Package, part: &str, data: &[u8]) -> BTreeSet<String> {
    let rels = rel_map(pkg, part);
    let mut out = BTreeSet::new();
    let mut r = XmlReader::new(data, part);
    let mut stack: Vec<String> = Vec::new();
    let mut text = String::new();
    let is_rels = part.ends_with(".rels");
    while let Ok(ev) = r.read() {
        let open = |e: &quick_xml::events::BytesStart<'_>,
                    stack: &mut Vec<String>,
                    out: &mut BTreeSet<String>| {
            let n = e.name();
            let name: &str = n.as_ref();
            stack.push(name.to_owned());
            let path = stack.join("/");
            let mut attrs: Vec<(String, String)> = e
                .attributes()
                .flatten()
                .map(|a| {
                    let k: &str = a.key.as_ref();
                    (k.to_owned(), unescape(&a.value))
                })
                .collect();
            if is_rels {
                // A relationship is identified by type + target, not by its id.
                let get = |k: &str| attrs.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
                if let (Some(t), Some(target)) = (get("Type"), get("Target")) {
                    let ext = get("TargetMode").as_deref() == Some("External");
                    let source = part.trim_end_matches(".rels").replace("/_rels/", "/");
                    let target = if ext {
                        target
                    } else {
                        resolve_target(&source, &target)
                    };
                    out.insert(format!("rel {t}→{target}"));
                }
                attrs.clear();
            }
            for (k, v) in attrs {
                let local = k.rsplit(':').next().unwrap_or(&k).to_owned();
                if ignored_attr(&path, &local) || is_default(&path, &local, &v) {
                    continue;
                }
                let v = if local == "id" && k.contains(':') {
                    rels.get(&v).cloned().unwrap_or(v)
                } else {
                    v
                };
                out.insert(format!("{path}@{k}={}", norm(&v)));
            }
        };
        match ev {
            Event::Start(e) => {
                text.clear();
                open(&e, &mut stack, &mut out);
            }
            Event::Empty(e) => {
                open(&e, &mut stack, &mut out);
                stack.pop();
            }
            Event::End(_) => {
                let t = text.trim();
                if !t.is_empty() {
                    out.insert(format!("{}#text={}", stack.join("/"), norm(t)));
                }
                text.clear();
                stack.pop();
            }
            Event::Text(t) => text.push_str(&unescape(&t)),
            Event::GeneralRef(g) => {
                let t: &str = &g;
                text.push_str(&unescape(&format!("&{t};")));
            }
            Event::CData(c) => text.push_str(&c),
            Event::Eof => break,
            _ => {}
        }
    }
    out
}

/// Check one file; returns the list of problems (empty = lossless).
fn check(src: &[u8]) -> Vec<String> {
    let mut problems = Vec::new();
    let wb = match read_xlsx(src) {
        Ok(wb) => wb,
        Err(e) => return vec![format!("read failed: {e}")],
    };
    let out = match write_xlsx(&wb) {
        Ok(o) => o,
        Err(e) => return vec![format!("write failed: {e}")],
    };
    match read_xlsx(&out) {
        Ok(wb2) if wb2 == wb => {}
        Ok(_) => problems.push("model differs after re-reading the saved file".into()),
        Err(e) => problems.push(format!("re-read failed: {e}")),
    }
    let a = Package::read(src, &Limits::default()).expect("source package");
    let b = Package::read(&out, &Limits::default()).expect("output package");
    let opaque: BTreeSet<String> = wb
        .opaque
        .parts
        .iter()
        .map(|p| p.name.to_ascii_lowercase())
        .collect();
    for p in a.parts() {
        if p.name.to_ascii_lowercase().ends_with("/calcchain.xml") {
            continue;
        }
        let Some(q) = b.part(&p.name) else {
            problems.push(format!("part {} missing", p.name));
            continue;
        };
        if opaque.contains(&p.name.to_ascii_lowercase()) {
            if p.data != q.data {
                problems.push(format!("opaque part {} changed", p.name));
            }
            continue;
        }
        let is_xml = p.name.ends_with(".xml") || p.name.ends_with(".rels");
        if !is_xml {
            continue;
        }
        let fa = facts(&a, &p.name, &p.data);
        let fb = facts(&b, &p.name, &q.data);
        let missing: Vec<&String> = fa.difference(&fb).collect();
        if !missing.is_empty() {
            problems.push(format!(
                "{}: {} fact(s) lost, e.g. {}",
                p.name,
                missing.len(),
                missing
                    .iter()
                    .take(3)
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(" | ")
            ));
        }
    }
    problems
}

#[test]
fn corpus_roundtrip() {
    let dir = corpus_dir();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("xlsx"))
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    // Fall back to in-memory generation when the corpus is not checked out.
    let inputs: Vec<(String, Vec<u8>)> = if files.is_empty() {
        FEATURES
            .iter()
            .flat_map(|(n, _, f)| (1..=3).map(move |i| (format!("{n}-{i}.xlsx"), build(&f(i)))))
            .chain(WRITER_FEATURES.iter().flat_map(|(n, _, f)| {
                (1..=3).map(move |i| (format!("{n}-{i}.xlsx"), write_xlsx(&f(i)).unwrap()))
            }))
            .collect()
    } else {
        files
            .iter()
            .map(|p| {
                (
                    p.file_name().unwrap().to_string_lossy().into_owned(),
                    std::fs::read(p).unwrap(),
                )
            })
            .collect()
    };
    let mut per_feature: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut failures = Vec::new();
    for (name, data) in &inputs {
        let feature = name
            .rsplit_once('-')
            .map_or(name.as_str(), |(f, _)| f)
            .to_owned();
        let problems = check(data);
        let e = per_feature.entry(feature).or_default();
        e.1 += 1;
        if problems.is_empty() {
            e.0 += 1;
        } else {
            failures.push(format!("{name}:\n  {}", problems.join("\n  ")));
        }
    }
    let total = inputs.len();
    let ok: usize = per_feature.values().map(|v| v.0).sum();
    println!(
        "BlueSheet corpus round trip: {ok}/{total} lossless ({:.1} %)",
        100.0 * ok as f64 / total as f64
    );
    for (f, (o, n)) in &per_feature {
        println!("  {f:<12} {o}/{n}");
    }
    for f in &failures {
        println!("{f}");
    }
    assert!(total >= 3 * FEATURES.len(), "corpus has {total} files");
    assert!(
        ok * 100 >= total * 95,
        "lossless {ok}/{total} < 95 %:\n{}",
        failures.join("\n")
    );
}

#[test]
fn corpus_pivots_are_summarized() {
    for i in 1..=3 {
        let bytes = write_xlsx(&writer_pivots(i)).unwrap();
        let wb = read_xlsx(&bytes).unwrap();
        assert_eq!(wb.pivot_caches.len(), 1, "variant {i}");
        let cache = &wb.pivot_caches[0];
        assert_eq!(cache.source_sheet.as_deref(), Some("Dữ liệu"));
        assert_eq!(cache.source_ref.as_deref(), Some("A1:E25"));
        assert_eq!(cache.fields.len(), 5);
        assert_eq!(cache.fields[0].items.len(), 3, "regions");
        assert!(cache.records_part.is_some());
        let ws = wb.sheets.iter().find(|s| !s.pivots.is_empty()).unwrap();
        let pt = &ws.pivots[0];
        assert_eq!(pt.cache_id, cache.cache_id);
        assert_ne!(pt.part, "");
        match i {
            1 => {
                assert_eq!((pt.rows.clone(), pt.values.len()), (vec![0], 1));
                assert_eq!(pt.location.to_string(), "H1:I5");
            }
            2 => {
                assert_eq!(pt.cols, vec![1]);
                assert_eq!(pt.filters.len(), 1);
                assert_eq!(pt.filters[0].selected, vec![m::CellValue::Number(2024.0)]);
            }
            _ => {
                assert_eq!(pt.rows, vec![1, 0]);
                assert_eq!(pt.values.len(), 4);
                assert_eq!(pt.values[1].function, m::PivotFunction::Average);
                assert_eq!(pt.filters[0].selected.len(), 2);
            }
        }
        // A saved pivot reopens unchanged.
        let again = read_xlsx(&write_xlsx(&wb).unwrap()).unwrap();
        assert_eq!(again, wb);
    }
}

/// The report settings of the generated pivots survive write → read: layout,
/// totals, field settings, data fields ("show values as", number formats),
/// filters, style, `rowItems` / `colItems`, groups and calculated fields.
#[test]
fn corpus_pivot_features_round_trip() {
    // The pivot features (not the format / sheet / chart ones).
    for (name, _, f) in WRITER_FEATURES
        .iter()
        .filter(|(n, _, _)| n.starts_with("writer-pivot"))
    {
        for i in 1..=3 {
            let wb = f(i);
            let back = read_xlsx(&write_xlsx(&wb).unwrap()).unwrap();
            assert_eq!(back.pivot_caches.len(), wb.pivot_caches.len());
            for (c, rc) in wb.pivot_caches.iter().zip(&back.pivot_caches) {
                assert_eq!(rc.fields.len(), c.fields.len(), "{name}-{i}");
                for (fl, rf) in c.fields.iter().zip(&rc.fields) {
                    let ctx = format!("{name}-{i} field {}", fl.name);
                    assert_eq!(rf.name, fl.name, "{ctx}");
                    assert_eq!(rf.group, fl.group, "{ctx}");
                    assert_eq!(rf.formula, fl.formula, "{ctx}");
                    assert_eq!(rf.derived, fl.derived, "{ctx}");
                    assert_eq!(rf.num_fmt_id, fl.num_fmt_id, "{ctx}");
                }
                assert_eq!(rc.calculated_items, c.calculated_items, "{name}-{i}");
            }
            let mut pivots = 0;
            for (s, ws) in wb.sheets.iter().enumerate() {
                for (k, pt) in ws.pivots.iter().enumerate() {
                    pivots += 1;
                    let rd = &back.sheets[s].pivots[k];
                    let ctx = format!("{name}-{i} pivot {k}");
                    assert_eq!(rd.layout, pt.layout, "{ctx}");
                    assert_eq!(
                        (rd.row_grand_totals, rd.col_grand_totals),
                        (pt.row_grand_totals, pt.col_grand_totals),
                        "{ctx}"
                    );
                    assert_eq!((&rd.rows, &rd.cols), (&pt.rows, &pt.cols), "{ctx}");
                    assert_eq!(rd.values, pt.values, "{ctx}");
                    assert_eq!(rd.field_filters, pt.field_filters, "{ctx}");
                    assert_eq!(rd.style, pt.style, "{ctx}");
                    assert_eq!(rd.row_items, pt.row_items, "{ctx}");
                    assert_eq!(rd.col_items, pt.col_items, "{ctx}");
                    assert_eq!(rd.location, pt.location, "{ctx}");
                    assert_eq!(rd.data_on_rows, pt.data_on_rows, "{ctx}");
                    let nf = wb.pivot_caches[0].fields.len() as u32 + 1;
                    for x in 0..nf {
                        assert_eq!(rd.field(x), pt.field(x), "{ctx} field {x}");
                    }
                }
            }
            assert!(pivots > 0, "{name}-{i}");
            // A saved pivot reopens unchanged.
            let again = read_xlsx(&write_xlsx(&back).unwrap()).unwrap();
            assert_eq!(again, back, "{name}-{i}");
        }
    }
}

#[test]
fn corpus_charts_are_summarized() {
    for i in 1..=3 {
        let wb = read_xlsx(&build(&charts(i))).unwrap();
        let c = &wb.sheets[0].charts;
        assert_eq!(c.len(), 1, "variant {i}");
        assert_eq!(
            c[0].title.as_deref(),
            Some(format!("Doanh thu vùng {i}").as_str())
        );
        assert_eq!(c[0].from, bo_model::sheet::CellRef::new(i as u32, 4));
        assert_eq!(c[0].plots.len(), if i == 2 { 2 } else { 1 });
        assert_eq!(
            c[0].plots[0].series[0].values,
            vec![Some(10.0), Some(20.0), Some(30.0)]
        );
        assert_eq!(
            c[0].plots[0].series[0].categories,
            vec!["Bắc", "Trung", "Nam"]
        );
    }
}

/// Pivot charts keep their `pivotSource` and series through write → read, and
/// follow the pivot table (a hidden item drops a series point).
#[test]
fn corpus_pivot_charts_round_trip() {
    for i in 1..=3 {
        let wb = writer_pivot_charts(i);
        let bytes = write_xlsx(&wb).unwrap();
        let back = read_xlsx(&bytes).unwrap();
        let c = &back.sheets[1].charts[0];
        assert_eq!(
            c.pivot_source.as_deref(),
            Some("'Báo cáo'!BangTongHop1"),
            "{i}"
        );
        assert_eq!(c.pivot_buttons, i == 3);
        assert_eq!(
            c.title.as_deref(),
            Some(format!("Biểu đồ tổng hợp {i}").as_str())
        );
        let series: Vec<&m::ChartSeries> = c.plots.iter().flat_map(|p| &p.series).collect();
        match i {
            1 => {
                assert_eq!(series.len(), 1);
                assert_eq!(series[0].categories, vec!["Bắc", "Nam", "Trung"]);
                assert!(series[0].val_ref.is_some());
            }
            2 => {
                // Three regions as series; Tiêu hidden: three products.
                assert_eq!(series.len(), 3);
                assert_eq!(series[0].categories, vec!["Chè", "Cà phê", "Gạo"]);
                assert_eq!(series[0].name.as_deref(), Some("Bắc"));
            }
            _ => {
                assert_eq!(c.plots.len(), 2);
                assert_eq!(series[1].name.as_deref(), Some("Tổng SL"));
            }
        }
        let pkg = bo_opc::Package::read(&bytes, &bo_opc::Limits::default()).unwrap();
        let xml =
            String::from_utf8(pkg.part("/xl/charts/chart1.xml").unwrap().data.clone()).unwrap();
        assert!(
            xml.contains("BangTongHop1</c:name><c:fmtId val=\"0\"/></c:pivotSource>"),
            "{xml}"
        );
        assert!(xml.contains("<c:pivotFmts>"));
        let again = read_xlsx(&write_xlsx(&back).unwrap()).unwrap();
        assert_eq!(again.sheets[1].charts, back.sheets[1].charts);
    }
}

#[test]
fn corpus_chart_anchors_are_summarized() {
    for i in 1..=3 {
        let wb = read_xlsx(&build(&chart_anchors(i))).unwrap();
        let c = &wb.sheets[0].charts[0];
        if i == 1 {
            assert_eq!(c.anchor, m::ChartAnchor::OneCell);
            assert_eq!(c.from, m::CellRef::new(1, 4));
            assert_eq!(c.ext, Some((4_572_000, 2_743_200)));
        } else {
            assert_eq!(c.anchor, m::ChartAnchor::Absolute);
            assert!(c.pos.is_some() && c.ext.is_some());
        }
    }
}

/// A 1904 workbook writes group bounds as the same calendar dates
/// (§18.17.4.1, xsd:dateTime) and reads them back as 1904 serials.
#[test]
fn pivot_dates_in_1904_workbooks() {
    let wb = writer_pivot_dates(3);
    let bytes = write_xlsx(&wb).unwrap();
    let pkg = bo_opc::Package::read(&bytes, &bo_opc::Limits::default()).unwrap();
    let defs: Vec<String> = pkg
        .parts()
        .iter()
        .filter(|p| p.name.contains("pivotCacheDefinition"))
        .map(|p| String::from_utf8(p.data.clone()).unwrap())
        .collect();
    // 45000 (1900) = 43538 (1904) = 2023-03-15: the first date of the source.
    assert!(
        defs.iter()
            .any(|d| d.contains("startDate=\"2023-03-15T00:00:00\"")),
        "{defs:?}"
    );
    assert!(
        defs.iter().all(|d| !d.contains("1919-")),
        "1904 serials read as 1900"
    );
    let back = read_xlsx(&bytes).unwrap();
    assert!(back.date1904);
    for (c, rc) in wb.pivot_caches.iter().zip(&back.pivot_caches) {
        for (f, rf) in c.fields.iter().zip(&rc.fields) {
            assert_eq!(f.group, rf.group, "{}", f.name);
        }
    }
}

// source: ECMA-376 Part 1, §21.2.2 (radarChart, bubbleChart/bubbleSize, stockChart/hiLowLines/upDownBars,
// surfaceChart/wireframe, ofPieChart/ofPieType, explosion, percentStacked) — CORE-CHART-002.
#[test]
fn new_chart_types_round_trip() {
    for (name, _, f) in WRITER_FEATURES
        .iter()
        .filter(|(n, _, _)| n.starts_with("writer-charts-") || n.starts_with("writer-chartex-"))
    {
        for i in 1..=3 {
            let wb = f(i);
            let back = read_xlsx(&write_xlsx(&wb).unwrap()).unwrap();
            let (a, b) = (&wb.sheets[0].charts[0], &back.sheets[0].charts[0]);
            let ctx = format!("{name}-{i}");
            assert_eq!(b.plots.len(), a.plots.len(), "{ctx}");
            assert_eq!(b.axes, a.axes, "{ctx}");
            for (pa, pb) in a.plots.iter().zip(&b.plots) {
                assert_eq!(pb.kind, pa.kind, "{ctx}");
                assert_eq!(pb.grouping, pa.grouping, "{ctx}");
                assert_eq!(pb.style, pa.style, "{ctx}");
                assert_eq!(pb.flag, pa.flag, "{ctx}");
                assert_eq!(pb.three_d, pa.three_d, "{ctx}");
                assert_eq!(
                    (pb.drop_lines, pb.hi_low_lines),
                    (pa.drop_lines, pa.hi_low_lines),
                    "{ctx}"
                );
                assert_eq!(pb.series.len(), pa.series.len(), "{ctx}");
                for (sa, sb) in pa.series.iter().zip(&pb.series) {
                    assert_eq!(sb.val_ref, sa.val_ref, "{ctx}");
                    assert_eq!(sb.size_ref, sa.size_ref, "{ctx}");
                    assert_eq!(sb.explosion, sa.explosion, "{ctx}");
                    assert_eq!(sb.trendlines, sa.trendlines, "{ctx}");
                    assert_eq!(sb.err_bars, sa.err_bars, "{ctx}");
                    assert_eq!(sb.values.len(), sa.values.len(), "{ctx}");
                }
            }
            match *name {
                "writer-charts-bubble" => assert!(
                    a.plots[0].series[0].sizes.iter().all(Option::is_some),
                    "{ctx}"
                ),
                "writer-charts-stock" => {
                    assert_eq!(a.plots[0].series.len(), if i == 1 { 3 } else { 4 }, "{ctx}");
                }
                _ => {}
            }
        }
    }
}
