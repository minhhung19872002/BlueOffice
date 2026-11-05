//! Round-trip tests against a hand-written package that exercises most of the
//! SpreadsheetML surface plus content the model does not understand
//! (extLst, mc:AlternateContent, drawings, printer settings, VML, chartsheet).
#![allow(clippy::needless_raw_string_hashes)]

use bo_model::sheet::{
    CellError, CellRange, CellRef, CellValue, FormulaKind, PaneState, SheetKind, SheetState,
};
use bo_opc::{Limits, Package};
use bo_xlsx::{canonicalize_xml, read_xlsx, read_xlsx_with_limits, write_xlsx, XlsxError};

const CT: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="bin" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.printerSettings"/><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="vml" ContentType="application/vnd.openxmlformats-officedocument.vmlDrawing"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/chartsheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.chartsheet+xml"/><Override PartName="/xl/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/><Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/><Override PartName="/xl/drawings/drawing1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawing+xml"/><Override PartName="/xl/tables/table1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml"/><Override PartName="/xl/comments1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.comments+xml"/><Override PartName="/xl/calcChain.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.calcChain+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/></Types>"##;

const PKG_RELS: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"##;

const CORE: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:creator>BlueOffice test</dc:creator></cp:coreProperties>"##;

const WORKBOOK: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="x15" xmlns:x15="http://schemas.microsoft.com/office/spreadsheetml/2010/11/main"><fileVersion appName="xl" lastEdited="7"/><workbookPr defaultThemeVersion="164011"/><mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:Choice Requires="x15"><x15ac:absPath xmlns:x15ac="http://schemas.microsoft.com/office/spreadsheetml/2010/11/ac" url="C:\tmp\"/></mc:Choice></mc:AlternateContent><bookViews><workbookView xWindow="0" yWindow="0" windowWidth="28800" windowHeight="12300" activeTab="0"/></bookViews><sheets><sheet name="Dữ liệu" sheetId="1" r:id="rId1"/><sheet name="Ẩn" sheetId="2" state="hidden" r:id="rId2"/><sheet name="Biểu đồ" sheetId="3" r:id="rId7"/></sheets><definedNames><definedName name="_xlnm.Print_Area" localSheetId="0">'Dữ liệu'!$A$1:$E$20</definedName><definedName name="_xlnm.Print_Titles" localSheetId="0">'Dữ liệu'!$1:$1</definedName><definedName name="TyGia">'Dữ liệu'!$B$2</definedName><definedName name="Local" localSheetId="1" hidden="1">Ẩn!$A$1</definedName></definedNames><calcPr calcId="191029"/><extLst><ext uri="{140A7094-0E35-4892-8432-C4D2E57EDEB5}" xmlns:x15="http://schemas.microsoft.com/office/spreadsheetml/2010/11/main"><x15:workbookPr chartTrackingRefBase="1"/></ext></extLst></workbook>"##;

const WB_RELS: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/><Relationship Id="rId7" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/chartsheet" Target="chartsheets/sheet1.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/><Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/><Relationship Id="rId6" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/calcChain" Target="calcChain.xml"/></Relationships>"##;

const THEME: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="T"><a:themeElements><a:clrScheme name="T"><a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1><a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="1F2A44"/></a:dk2><a:lt2><a:srgbClr val="E8E8E8"/></a:lt2><a:accent1><a:srgbClr val="1565C0"/></a:accent1><a:accent2><a:srgbClr val="EF6C00"/></a:accent2><a:accent3><a:srgbClr val="9E9E9E"/></a:accent3><a:accent4><a:srgbClr val="FBC02D"/></a:accent4><a:accent5><a:srgbClr val="42A5F5"/></a:accent5><a:accent6><a:srgbClr val="43A047"/></a:accent6><a:hlink><a:srgbClr val="0B57D0"/></a:hlink><a:folHlink><a:srgbClr val="7B1FA2"/></a:folHlink></a:clrScheme></a:themeElements></a:theme>"##;

const STYLES: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="x14ac" xmlns:x14ac="http://schemas.microsoft.com/office/spreadsheetml/2009/9/ac"><numFmts count="2"><numFmt numFmtId="164" formatCode="#,##0 [$₫-42A]"/><numFmt numFmtId="165" formatCode="dd/mm/yyyy"/></numFmts><fonts count="2" x14ac:knownFonts="1"><font><sz val="11"/><color theme="1"/><name val="Calibri"/><family val="2"/><scheme val="minor"/></font><font><b/><i/><u val="double"/><sz val="14"/><color rgb="FFC00000"/><name val="Times New Roman"/><family val="1"/></font></fonts><fills count="4"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill><fill><patternFill patternType="solid"><fgColor theme="4" tint="0.79998168889431442"/><bgColor indexed="64"/></patternFill></fill><fill><gradientFill degree="90"><stop position="0"><color rgb="FFFFFFFF"/></stop><stop position="1"><color theme="4"/></stop></gradientFill></fill></fills><borders count="2"><border><left/><right/><top/><bottom/><diagonal/></border><border diagonalUp="1"><left style="thin"><color indexed="64"/></left><right style="medium"><color auto="1"/></right><top style="double"/><bottom style="dashed"><color rgb="FF00FF00"/></bottom><diagonal style="hair"/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="5"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/><xf numFmtId="164" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/><xf numFmtId="165" fontId="1" fillId="2" borderId="1" xfId="0" applyNumberFormat="1" applyFont="1" applyFill="1" applyBorder="1" applyAlignment="1"><alignment horizontal="center" vertical="top" wrapText="1" indent="1" textRotation="45"/></xf><xf numFmtId="10" fontId="0" fillId="3" borderId="0" xfId="0" applyProtection="1"><protection locked="0" hidden="1"/></xf><xf numFmtId="49" fontId="0" fillId="0" borderId="0" xfId="0" quotePrefix="1"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles><dxfs count="1"><dxf><font><b/><color rgb="FF9C0006"/></font><fill><patternFill><bgColor rgb="FFFFC7CE"/></patternFill></fill></dxf></dxfs><tableStyles count="0" defaultTableStyle="TableStyleMedium2" defaultPivotStyle="PivotStyleLight16"/><colors><mruColors><color rgb="FF123456"/></mruColors></colors><extLst><ext uri="{EB79DEF2-80B8-43e5-95BD-54CBDDF9020C}" xmlns:x14="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main"><x14:slicerStyles defaultSlicerStyle="SlicerStyleLight1"/></ext></extLst></styleSheet>"##;

const SST: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="4" uniqueCount="4"><si><t>Tên hàng</t></si><si><r><rPr><b/><sz val="11"/><color rgb="FFFF0000"/><rFont val="Calibri"/></rPr><t>Đậm</t></r><r><t xml:space="preserve"> thường</t></r></si><si><t>a_x000D_b _x005F_x0041_</t></si><si><t xml:space="preserve">  cách  </t><phoneticPr fontId="0"/></si></sst>"##;

const SHEET1: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="x14ac xr" xmlns:x14ac="http://schemas.microsoft.com/office/spreadsheetml/2009/9/ac" xmlns:xr="http://schemas.microsoft.com/office/spreadsheetml/2014/revision" xr:uid="{00000000-0001-0000-0000-000000000000}"><sheetPr codeName="Sheet1"><tabColor rgb="FF00B050"/><outlinePr summaryBelow="0"/><pageSetUpPr fitToPage="1"/></sheetPr><dimension ref="A1:F12"/><sheetViews><sheetView tabSelected="1" zoomScale="120" workbookViewId="0"><pane xSplit="1" ySplit="1" topLeftCell="B2" activePane="bottomRight" state="frozen"/><selection pane="topRight" activeCell="B1" sqref="B1"/><selection pane="bottomRight" activeCell="C5" sqref="C5"/></sheetView></sheetViews><sheetFormatPr defaultRowHeight="15" x14ac:dyDescent="0.25" outlineLevelRow="1"/><cols><col min="1" max="1" width="22.7109375" customWidth="1"/><col min="2" max="3" width="12" style="1" customWidth="1"/><col min="5" max="5" width="9.140625" hidden="1" outlineLevel="1"/></cols><sheetData><row r="1" spans="1:6" s="2" customFormat="1" ht="24" customHeight="1" x14ac:dyDescent="0.25"><c r="A1" s="2" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c><c r="C1" t="inlineStr"><is><t>Nội dung</t></is></c><c r="D1" t="s"><v>2</v></c><c r="E1" t="s"><v>3</v></c></row><row r="2"><c r="A2"><v>1234567.891</v></c><c r="B2" s="1"><v>25000</v></c><c r="C2" t="b"><v>1</v></c><c r="D2" t="e"><v>#DIV/0!</v></c><c r="E2" t="str"><f>"Xin "&amp;"chào"</f><v>Xin chào</v></c><c r="F2" s="2"><v>45000.5</v></c></row><row r="3" hidden="1" outlineLevel="1"><c r="A3"><f t="shared" ref="A3:A5" si="0">A2*2</f><v>2469135.782</v></c><c r="B3"><f t="array" ref="B3:B4">B2:B2*{1;2}</f><v>25000</v></c></row><row r="4" outlineLevel="1" collapsed="1"><c r="A4"><f t="shared" si="0"/><v>4938271.564</v></c><c r="B4"><v>50000</v></c></row><row r="5"><c r="A5"><f t="shared" si="0"/><v>9876543.128</v></c><c r="C5" s="3"><f ca="1">RAND()</f><v>0.5</v></c><c r="D5" s="4" t="s"><v>0</v></c></row><row r="12"><c r="F12" t="d"><v>2024-02-29T00:00:00</v></c></row></sheetData><sheetProtection sheet="1" objects="1" scenarios="1"/><autoFilter ref="A1:C5"/><mergeCells count="1"><mergeCell ref="D3:E4"/></mergeCells><conditionalFormatting sqref="A2:A5"><cfRule type="cellIs" dxfId="0" priority="1" operator="greaterThan"><formula>1000000</formula></cfRule></conditionalFormatting><conditionalFormatting sqref="B2:B10 D2:D10"><cfRule type="colorScale" priority="2"><colorScale><cfvo type="min"/><cfvo type="max"/><color rgb="FFF8696B"/><color rgb="FF63BE7B"/></colorScale></cfRule><cfRule type="dataBar" priority="3"><dataBar><cfvo type="min"/><cfvo type="max"/><color rgb="FF638EC6"/></dataBar><extLst><ext uri="{B025F937-C7B1-47D3-B67F-A62EFF666E3E}" xmlns:x14="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main"><x14:id>{1E1C2A2B-0000-0000-0000-000000000001}</x14:id></ext></extLst></cfRule><cfRule type="iconSet" priority="4"><iconSet iconSet="3Arrows"><cfvo type="percent" val="0"/><cfvo type="percent" val="33"/><cfvo type="percent" val="67"/></iconSet></cfRule><cfRule type="expression" priority="5" stopIfTrue="1"><formula>MOD(ROW(),2)=0</formula></cfRule></conditionalFormatting><dataValidations count="1"><dataValidation type="list" allowBlank="1" showInputMessage="1" showErrorMessage="1" errorTitle="Lỗi" error="Chọn trong danh sách" sqref="C2:C10"><formula1>"Có,Không"</formula1></dataValidation></dataValidations><hyperlinks><hyperlink ref="A1" r:id="rId9" tooltip="Trang chủ"/><hyperlink ref="B5" location="'Ẩn'!A1" display="Sang sheet ẩn"/></hyperlinks><printOptions gridLines="1" horizontalCentered="1"/><pageMargins left="0.7" right="0.7" top="0.75" bottom="0.75" header="0.3" footer="0.3"/><pageSetup paperSize="9" orientation="landscape" fitToHeight="0" r:id="rId3"/><headerFooter><oddHeader>&amp;CBáo cáo</oddHeader><oddFooter>Trang &amp;P / &amp;N</oddFooter></headerFooter><rowBreaks count="1" manualBreakCount="1"><brk id="10" max="16383" man="1"/></rowBreaks><drawing r:id="rId2"/><legacyDrawing r:id="rId5"/><tableParts count="1"><tablePart r:id="rId4"/></tableParts><extLst><ext uri="{78C0D931-6437-407d-A8EE-F0AAD7539E65}" xmlns:x14="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main"><x14:conditionalFormattings><x14:conditionalFormatting xmlns:xm="http://schemas.microsoft.com/office/excel/2006/main"><x14:cfRule type="dataBar" id="{1E1C2A2B-0000-0000-0000-000000000001}"><x14:dataBar minLength="0" maxLength="100"/></x14:cfRule><xm:sqref>B2:B10</xm:sqref></x14:conditionalFormatting></x14:conditionalFormattings></ext></extLst></worksheet>"##;

const SHEET1_RELS: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId9" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.vn/?a=1&amp;b=2" TargetMode="External"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/printerSettings" Target="../printerSettings/printerSettings1.bin"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing" Target="../drawings/drawing1.xml"/><Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/vmlDrawing" Target="../drawings/vmlDrawing1.vml"/><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="../comments1.xml"/><Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/table" Target="../tables/table1.xml"/></Relationships>"##;

const SHEET2: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1"><v>42</v></c></row></sheetData></worksheet>"##;

const COMMENTS: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<comments xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><authors><author>Kế toán</author><author>Kiểm toán</author></authors><commentList><comment ref="B2" authorId="1"><text><r><rPr><b/><sz val="9"/><rFont val="Tahoma"/></rPr><t>Kiểm toán:</t></r><r><t xml:space="preserve">
Đã đối chiếu</t></r></text></comment><comment ref="A1" authorId="0"><text><t>Ghi chú</t></text></comment></commentList></comments>"##;

const TABLE: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<table xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="xr" xmlns:xr="http://schemas.microsoft.com/office/spreadsheetml/2014/revision" xr:uid="{AAAA0000-0000-0000-0000-000000000000}" id="1" name="BangHang" displayName="BangHang" ref="A1:C5" totalsRowShown="0"><autoFilter ref="A1:C5" xr:uid="{BBBB0000-0000-0000-0000-000000000000}"/><tableColumns count="3"><tableColumn id="1" name="Tên hàng"/><tableColumn id="2" name="Đậm thường" dataDxfId="0"/><tableColumn id="3" name="Nội dung"><calculatedColumnFormula>BangHang[[#This Row],[Tên hàng]]</calculatedColumnFormula></tableColumn></tableColumns><tableStyleInfo name="TableStyleMedium2" showFirstColumn="0" showLastColumn="0" showRowStripes="1" showColumnStripes="0"/></table>"##;

const DRAWING: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><xdr:twoCellAnchor><xdr:from><xdr:col>6</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>1</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from><xdr:to><xdr:col>10</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>10</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to><xdr:sp><xdr:nvSpPr><xdr:cNvPr id="2" name="Hình 1"/><xdr:cNvSpPr/></xdr:nvSpPr><xdr:spPr/></xdr:sp><xdr:clientData/></xdr:twoCellAnchor></xdr:wsDr>"##;

const VML: &str = r##"<xml xmlns:v="urn:schemas-microsoft-com:vml"><v:shape id="_x0000_s1025" type="#_x0000_t202"/></xml>"##;

const CHARTSHEET: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<chartsheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetViews><sheetView workbookViewId="0"/></sheetViews></chartsheet>"##;

const CALC_CHAIN: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<calcChain xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><c r="E2" i="1"/></calcChain>"##;

const PRINTER: &[u8] = &[0x00, 0x01, 0x02, 0xFE, 0xFF, 0x10, 0x20];

fn fixture() -> Vec<u8> {
    let mut p = Package::new();
    p.put_part("/[Content_Types].xml", CT.as_bytes().to_vec());
    let parts: &[(&str, &[u8])] = &[
        ("/_rels/.rels", PKG_RELS.as_bytes()),
        ("/docProps/core.xml", CORE.as_bytes()),
        ("/xl/workbook.xml", WORKBOOK.as_bytes()),
        ("/xl/_rels/workbook.xml.rels", WB_RELS.as_bytes()),
        ("/xl/theme/theme1.xml", THEME.as_bytes()),
        ("/xl/styles.xml", STYLES.as_bytes()),
        ("/xl/sharedStrings.xml", SST.as_bytes()),
        ("/xl/worksheets/sheet1.xml", SHEET1.as_bytes()),
        (
            "/xl/worksheets/_rels/sheet1.xml.rels",
            SHEET1_RELS.as_bytes(),
        ),
        ("/xl/worksheets/sheet2.xml", SHEET2.as_bytes()),
        ("/xl/chartsheets/sheet1.xml", CHARTSHEET.as_bytes()),
        ("/xl/comments1.xml", COMMENTS.as_bytes()),
        ("/xl/tables/table1.xml", TABLE.as_bytes()),
        ("/xl/drawings/drawing1.xml", DRAWING.as_bytes()),
        ("/xl/drawings/vmlDrawing1.vml", VML.as_bytes()),
        ("/xl/printerSettings/printerSettings1.bin", PRINTER),
        ("/xl/calcChain.xml", CALC_CHAIN.as_bytes()),
    ];
    for (n, d) in parts {
        p.put_part(n, d.to_vec());
    }
    p.write().unwrap()
}

fn part(bytes: &[u8], name: &str) -> Option<Vec<u8>> {
    Package::read(bytes, &Limits::default())
        .unwrap()
        .part(name)
        .map(|p| p.data.clone())
}

fn rels_of(bytes: &[u8], source: &str) -> Vec<bo_opc::Relationship> {
    Package::read(bytes, &Limits::default())
        .unwrap()
        .relationships(source)
        .unwrap()
}

#[test]
fn reads_every_modelled_feature() {
    let wb = read_xlsx(&fixture()).unwrap();
    assert_eq!(wb.sheets.len(), 3);
    assert!(!wb.date1904);
    assert_eq!(
        wb.theme.as_ref().unwrap().colors[4],
        "1565C0",
        "accent1 from the theme part"
    );
    let ws = &wb.sheets[0];
    assert_eq!(ws.name, "Dữ liệu");
    assert_eq!(wb.sheets[1].state, SheetState::Hidden);
    assert!(matches!(wb.sheets[2].kind, SheetKind::Opaque { .. }));

    // Values.
    let v = |a1: &str| ws.cell_a1(a1).unwrap().value.clone();
    assert_eq!(v("A1"), CellValue::Text("Tên hàng".into()));
    match v("B1") {
        CellValue::RichText(runs) => {
            assert_eq!(runs.len(), 2);
            assert!(runs[0].font.as_ref().unwrap().bold);
            assert_eq!(runs[1].text, " thường");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(v("C1"), CellValue::Text("Nội dung".into()));
    assert_eq!(v("D1"), CellValue::Text("a\rb _x0041_".into()));
    assert_eq!(v("E1"), CellValue::Text("  cách  ".into()));
    assert_eq!(v("A2"), CellValue::Number(1_234_567.891));
    assert_eq!(v("C2"), CellValue::Bool(true));
    assert_eq!(v("D2"), CellValue::Error(CellError::Div0));
    assert_eq!(v("E2"), CellValue::Text("Xin chào".into()));
    assert_eq!(v("F12"), CellValue::Date("2024-02-29T00:00:00".into()));
    assert_eq!(
        ws.cell_a1("E2").unwrap().formula.as_ref().unwrap().text,
        "\"Xin \"&\"chào\""
    );

    // Formulas.
    let f = |a1: &str| ws.cell_a1(a1).unwrap().formula.clone().unwrap();
    assert_eq!(
        f("A3").kind,
        FormulaKind::Shared {
            index: 0,
            range: Some("A3:A5".into())
        }
    );
    assert_eq!(
        f("A4").kind,
        FormulaKind::Shared {
            index: 0,
            range: None
        }
    );
    assert_eq!(f("A4").text, "");
    assert_eq!(
        f("B3").kind,
        FormulaKind::Array {
            range: "B3:B4".into()
        }
    );
    assert!(f("C5").calc_always);

    // Rows / cols / views.
    assert!(ws.rows[&2].hidden);
    assert_eq!(ws.rows[&2].outline_level, 1);
    assert_eq!(ws.rows[&0].height, Some(24.0));
    assert_eq!(ws.rows[&0].style, Some(2));
    assert_eq!(ws.cols.len(), 3);
    assert_eq!((ws.cols[1].min, ws.cols[1].max), (1, 2));
    assert!(ws.cols[2].hidden);
    let pane = ws.frozen().unwrap();
    assert_eq!(
        (pane.x_split, pane.y_split, pane.state),
        (1.0, 1.0, PaneState::Frozen)
    );
    assert_eq!(
        ws.props.tab_color.as_ref().unwrap().rgb.as_deref(),
        Some("FF00B050")
    );
    assert_eq!(ws.props.summary_below, Some(false));

    // Sheet features.
    assert_eq!(ws.merges, vec![CellRange::parse("D3:E4").unwrap()]);
    assert_eq!(ws.conditional_formats.len(), 2);
    let rules = &ws.conditional_formats[1].rules;
    assert_eq!(rules[0].color_scale.as_ref().unwrap().colors.len(), 2);
    assert_eq!(rules[1].data_bar.as_ref().unwrap().cfvos.len(), 2);
    assert_eq!(rules[1].unknown.len(), 1, "cfRule extLst kept");
    assert_eq!(
        rules[2].icon_set.as_ref().unwrap().icon_set.as_deref(),
        Some("3Arrows")
    );
    assert_eq!(
        ws.data_validations[0].formula1.as_deref(),
        Some("\"Có,Không\"")
    );
    assert_eq!(
        ws.hyperlinks[0].target.as_deref(),
        Some("https://example.vn/?a=1&b=2")
    );
    assert_eq!(ws.hyperlinks[1].location.as_deref(), Some("'Ẩn'!A1"));
    assert_eq!(ws.comments.len(), 2);
    assert_eq!(ws.comments[0].author, "Kiểm toán");
    assert_eq!(ws.comments[0].cell, CellRef::new(1, 1));
    assert_eq!(ws.tables[0].name, "BangHang");
    assert_eq!(ws.tables[0].columns.len(), 3);
    assert!(ws.tables[0].columns[2].calculated_formula.is_some());
    assert_eq!(ws.print_area.as_deref(), Some("'Dữ liệu'!$A$1:$E$20"));
    assert_eq!(ws.print_titles.as_deref(), Some("'Dữ liệu'!$1:$1"));
    assert_eq!(ws.page_setup.paper_size, Some(9));
    assert_eq!(ws.page_setup.orientation.as_deref(), Some("landscape"));
    assert_eq!(ws.page_setup.row_breaks, vec![9]);
    assert_eq!(ws.page_setup.odd_header.as_deref(), Some("&CBáo cáo"));
    assert!(ws.page_setup.print_grid_lines);
    // Defined names: the two built-ins moved to the sheet, the rest stay.
    assert_eq!(wb.defined_names.len(), 2);
    assert_eq!(wb.defined_names[1].local_sheet, Some(1));
    assert!(wb.defined_names[1].hidden);

    // Styles.
    let ss = &wb.styles;
    assert_eq!(ss.custom_num_fmt(164), Some("#,##0 [$₫-42A]"));
    assert_eq!(ss.fonts[1].underline.as_deref(), Some("double"));
    assert!(ss.fonts[1].bold && ss.fonts[1].italic);
    assert_eq!(ss.cell_xfs.len(), 5);
    let al = ss.cell_xfs[2].alignment.as_ref().unwrap();
    assert_eq!(al.horizontal.as_deref(), Some("center"));
    assert!(al.wrap_text);
    assert_eq!(
        ss.cell_xfs[3].protection.as_ref().unwrap().locked,
        Some(false)
    );
    assert_eq!(ss.dxfs.len(), 1);
    assert_eq!(ss.unknown.len(), 3, "tableStyles, colors, extLst");

    // Unknown worksheet children are kept in order with their anchors.
    let names: Vec<&str> = ws.unknown.iter().map(|u| u.after.as_str()).collect();
    assert_eq!(
        names,
        [
            "sheetProtection",
            "autoFilter",
            "drawing",
            "legacyDrawing",
            "extLst"
        ]
    );
}

#[test]
fn model_roundtrip_is_lossless() {
    let src = fixture();
    let wb = read_xlsx(&src).unwrap();
    let out = write_xlsx(&wb).unwrap();
    let wb2 = read_xlsx(&out).unwrap();
    assert_wb_eq(&wb, &wb2);
    assert_eq!(wb, wb2);
    // And again: writing is idempotent.
    let out2 = write_xlsx(&wb2).unwrap();
    let a = Package::read(&out, &Limits::default()).unwrap();
    let b = Package::read(&out2, &Limits::default()).unwrap();
    assert_eq!(a.parts().len(), b.parts().len());
    for p in a.parts() {
        let q = b.part(&p.name).unwrap();
        assert_eq!(p.data, q.data, "{} differs between writes", p.name);
    }
}

#[test]
fn untouched_parts_are_byte_identical_and_canonically_equal() {
    let src = fixture();
    let out = write_xlsx(&read_xlsx(&src).unwrap()).unwrap();
    for name in [
        "/docProps/core.xml",
        "/xl/theme/theme1.xml",
        "/xl/drawings/drawing1.xml",
        "/xl/drawings/vmlDrawing1.vml",
        "/xl/printerSettings/printerSettings1.bin",
        "/xl/chartsheets/sheet1.xml",
    ] {
        let a = part(&src, name).unwrap();
        let b = part(&out, name).unwrap_or_else(|| panic!("{name} missing"));
        assert_eq!(a, b, "{name}");
        if name.ends_with(".xml") {
            assert_eq!(canonicalize_xml(&a).unwrap(), canonicalize_xml(&b).unwrap());
        }
    }
    // The calculation chain is intentionally dropped.
    assert!(part(&out, "/xl/calcChain.xml").is_none());
    let ct = String::from_utf8(part(&out, "/[Content_Types].xml").unwrap()).unwrap();
    assert!(!ct.contains("calcChain"));
    assert!(ct.contains("Extension=\"bin\""));
    assert!(ct.contains("/xl/drawings/drawing1.xml"));
}

#[test]
fn unknown_fragments_survive_in_place() {
    let src = fixture();
    let out = write_xlsx(&read_xlsx(&src).unwrap()).unwrap();
    let sheet = String::from_utf8(part(&out, "/xl/worksheets/sheet1.xml").unwrap()).unwrap();
    let canon = canonicalize_xml(sheet.as_bytes()).unwrap();
    for frag in [
        r##"<sheetProtection sheet="1" objects="1" scenarios="1"/>"##,
        r##"<drawing r:id="rId2"/>"##,
        r##"<legacyDrawing r:id="rId5"/>"##,
        r##"<x14:cfRule type="dataBar" id="{1E1C2A2B-0000-0000-0000-000000000001}"><x14:dataBar minLength="0" maxLength="100"/></x14:cfRule>"##,
    ] {
        let c = canonicalize_xml(frag.as_bytes()).unwrap();
        assert!(canon.contains(&c), "missing {frag}");
    }
    // Schema order (§18.3.1.99) is respected.
    let order = [
        "<sheetPr ",
        "<dimension ",
        "<sheetViews>",
        "<sheetFormatPr ",
        "<cols>",
        "<sheetData>",
        "<sheetProtection ",
        "<autoFilter ",
        "<mergeCells ",
        "<conditionalFormatting ",
        "<dataValidations ",
        "<hyperlinks>",
        "<printOptions ",
        "<pageMargins ",
        "<pageSetup ",
        "<headerFooter>",
        "<rowBreaks ",
        "<drawing ",
        "<legacyDrawing ",
        "<tableParts ",
        "<extLst>",
    ];
    let pos: Vec<usize> = order
        .iter()
        .map(|t| {
            if *t == "<extLst>" {
                sheet.rfind(t)
            } else {
                sheet.find(t)
            }
            .unwrap_or_else(|| panic!("{t} missing"))
        })
        .collect();
    assert!(pos.windows(2).all(|w| w[0] < w[1]), "{pos:?}");
    // Root namespace declarations are preserved for prefixed fragments.
    assert!(sheet.contains("xmlns:x14ac="));
    assert!(sheet.contains("mc:Ignorable=\"x14ac xr\""));

    // Workbook: AlternateContent after workbookPr, before bookViews.
    let wbx = String::from_utf8(part(&out, "/xl/workbook.xml").unwrap()).unwrap();
    let i_pr = wbx.find("<workbookPr").unwrap();
    let i_ac = wbx.find("<mc:AlternateContent").unwrap();
    let i_bv = wbx.find("<bookViews").unwrap();
    let i_sh = wbx.find("<sheets").unwrap();
    let i_dn = wbx.find("<definedNames").unwrap();
    let i_calc = wbx.find("<calcPr").unwrap();
    assert!(i_pr < i_ac && i_ac < i_bv && i_bv < i_sh && i_sh < i_dn && i_dn < i_calc);
}

#[test]
fn opaque_relationship_ids_are_preserved() {
    let out = write_xlsx(&read_xlsx(&fixture()).unwrap()).unwrap();
    let sheet_path = "/xl/worksheets/sheet1.xml";
    let rels = rels_of(&out, sheet_path);
    let find = |id: &str| rels.iter().find(|r| r.id == id).unwrap().clone();
    assert!(find("rId2").rel_type.ends_with("/drawing"));
    assert_eq!(find("rId2").target, "../drawings/drawing1.xml");
    assert!(find("rId3").rel_type.ends_with("/printerSettings"));
    assert!(find("rId5").rel_type.ends_with("/vmlDrawing"));
    // Newly allocated ids do not collide.
    let mut ids: Vec<&str> = rels.iter().map(|r| r.id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), rels.len());
    assert!(rels
        .iter()
        .any(|r| r.rel_type.ends_with("/hyperlink") && r.target == "https://example.vn/?a=1&b=2"));
    // Package-level docProps relationship kept.
    assert!(rels_of(&out, "/")
        .iter()
        .any(|r| r.rel_type.ends_with("/core-properties") && r.target == "docProps/core.xml"));
    // Chartsheet still wired from the workbook.
    assert!(rels_of(&out, "/xl/workbook.xml")
        .iter()
        .any(|r| r.rel_type.ends_with("/chartsheet") && r.target == "chartsheets/sheet1.xml"));
}

#[test]
fn rejects_zip_bomb_and_deep_xml() {
    // A highly compressible worksheet trips the compression-ratio limit.
    let mut p = Package::read(&fixture(), &Limits::default()).unwrap();
    let mut big = String::from(
        "<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData>",
    );
    big.push_str(&" ".repeat(8 * 1024 * 1024));
    big.push_str("</sheetData></worksheet>");
    p.put_part("/xl/worksheets/sheet2.xml", big.into_bytes());
    let bytes = p.write().unwrap();
    let limits = Limits {
        max_ratio: 100,
        ..Limits::default()
    };
    assert!(matches!(
        read_xlsx_with_limits(&bytes, &limits),
        Err(XlsxError::Opc(bo_opc::OpcError::LimitExceeded(_)))
    ));

    // Nesting beyond bo-opc's MAX_DEPTH is rejected, not overflowed.
    let mut p = Package::read(&fixture(), &Limits::default()).unwrap();
    let deep = format!(
        "<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">{}",
        "<extLst>".repeat(bo_opc::xml::MAX_DEPTH + 10)
    );
    p.put_part("/xl/worksheets/sheet2.xml", deep.into_bytes());
    assert!(read_xlsx(&p.write().unwrap()).is_err());

    assert!(read_xlsx(b"PK\x03\x04garbage").is_err());
}

#[test]
fn missing_workbook_is_an_error() {
    let p = Package::new();
    assert!(matches!(
        read_xlsx(&p.write().unwrap()),
        Err(XlsxError::NoWorkbook)
    ));
}

/// Compare two workbooks component by component for readable failures.
fn assert_wb_eq(a: &bo_model::sheet::Workbook, b: &bo_model::sheet::Workbook) {
    assert_eq!(a.sheets.len(), b.sheets.len());
    for (x, y) in a.sheets.iter().zip(&b.sheets) {
        for (k, c) in &x.cells {
            assert_eq!(Some(c), y.cells.get(k), "cell {k:?}");
        }
        assert_eq!(x.cells.len(), y.cells.len());
        assert_eq!(x.rows, y.rows);
        assert_eq!(x.cols, y.cols);
        assert_eq!(x.format, y.format);
        assert_eq!(x.props, y.props);
        assert_eq!(x.views, y.views);
        assert_eq!(x.merges, y.merges);
        assert_eq!(x.hyperlinks, y.hyperlinks);
        assert_eq!(x.comments, y.comments);
        assert_eq!(x.data_validations, y.data_validations);
        assert_eq!(x.conditional_formats, y.conditional_formats);
        assert_eq!(x.tables, y.tables);
        assert_eq!(x.page_setup, y.page_setup);
        assert_eq!(x.root_attrs, y.root_attrs);
        assert_eq!(x.unknown, y.unknown);
        assert_eq!(x.rels, y.rels);
        assert_eq!(x, y);
    }
    assert_eq!(a.styles, b.styles);
    assert_eq!(a.defined_names, b.defined_names);
    assert_eq!(a.unknown, b.unknown);
    assert_eq!(a.root_attrs, b.root_attrs);
    assert_eq!(a.opaque.package_rels, b.opaque.package_rels);
    assert_eq!(a.opaque.workbook_rels, b.opaque.workbook_rels);
    assert_eq!(
        a.opaque.content_type_defaults,
        b.opaque.content_type_defaults
    );
    let names = |w: &bo_model::sheet::Workbook| {
        w.opaque
            .parts
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(a), names(b));
}
