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
    ("ext", "extLst, mc:AlternateContent, x14ac attributes, sheet autoFilter + sortState, controls, customXml", extensions),
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
    };
    let plot = |kind: m::ChartKind,
                dir: Option<&str>,
                grouping: Option<&str>,
                s: Vec<m::ChartSeries>| m::ChartPlot {
        kind,
        bar_dir: dir.map(str::to_owned),
        grouping: grouping.map(str::to_owned),
        series: s,
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

type WriterFeature = (&'static str, &'static str, fn(usize) -> m::Workbook);

const WRITER_FEATURES: &[WriterFeature] = &[
    ("writer-format", "BlueSheet writer: cell formats (vi-VN number formats, fonts, fills, borders, alignment) and a gallery cell style", writer_format),
    ("writer-sheet", "BlueSheet writer: column width / row height, freeze panes, merge, AutoFilter, conditional formats, list validation, page setup + print titles", writer_sheet),
    ("writer-charts", "BlueSheet writer: inserted charts (column, column+line combo, doughnut, scatter) with axis titles and legend", writer_charts),
];

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
    std::fs::write(dir.join("README.md"), readme).unwrap();
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
