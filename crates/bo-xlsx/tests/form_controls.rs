//! Form controls (§18.3.1.21 `controls`, [MS-XLSX] `controlPr` / `formControlPr`, legacy VML
//! `x:ClientData`) and ActiveX controls: read every control type, keep the parts byte-for-byte
//! when untouched, patch changed controls, drop removed ones and write new ones.
//!
//! The fixture below is hand-written SpreadsheetML in the layout described by the specs.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use std::fmt::Write as _;

use bo_model::sheet::{
    CellRef, CheckState, ControlAnchor, FormControl, FormControlKind as K, Workbook,
};
use bo_opc::{Limits, Package};
use bo_xlsx::{read_xlsx, write_xlsx};

const DECL: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;
const NS_MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/";
const X14: &str = "http://schemas.microsoft.com/office/spreadsheetml/2009/9/main";
const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

/// (shape id, VML ObjectType, caption, extra client data, control properties or `None`).
type Spec = (
    u32,
    &'static str,
    &'static str,
    &'static str,
    Option<&'static str>,
);

const SPECS: &[Spec] = &[
    (
        1025,
        "Button",
        "Tính toán",
        "<x:FmlaMacro>[0]!TinhToan</x:FmlaMacro>",
        Some(r#"objectType="Button" lockText="1""#),
    ),
    (
        1026,
        "Checkbox",
        "Đã duyệt",
        "<x:Checked>1</x:Checked><x:FmlaLink>$C$1</x:FmlaLink>",
        Some(
            r#"objectType="CheckBox" checked="Checked" fmlaLink="$C$1" lockText="1" noThreeD="1""#,
        ),
    ),
    (
        1027,
        "Drop",
        "",
        "<x:FmlaRange>$A$1:$A$3</x:FmlaRange><x:FmlaLink>$C$2</x:FmlaLink><x:Sel>2</x:Sel>",
        Some(
            r#"objectType="Drop" dropStyle="combo" dx="22" fmlaLink="$C$2" fmlaRange="$A$1:$A$3" sel="2" dropLines="5""#,
        ),
    ),
    (
        1028,
        "List",
        "",
        "",
        Some(
            r#"objectType="List" dx="22" fmlaLink="$C$3" fmlaRange="'Dữ liệu'!$A$1:$A$3" sel="0""#,
        ),
    ),
    (
        1029,
        "GBox",
        "Giới tính",
        "",
        Some(r#"objectType="GBox" noThreeD="1""#),
    ),
    (
        1030,
        "Radio",
        "Nam",
        "<x:Checked>1</x:Checked><x:FirstButton/>",
        Some(
            r#"objectType="Radio" checked="Checked" firstButton="1" fmlaLink="$C$4" lockText="1""#,
        ),
    ),
    (
        1031,
        "Radio",
        "Nữ",
        "<x:FmlaLink>$C$4</x:FmlaLink>",
        Some(r#"objectType="Radio" fmlaLink="$C$4" lockText="1""#),
    ),
    (
        1032,
        "Scroll",
        "",
        "",
        Some(
            r#"objectType="Scroll" dx="22" fmlaLink="$C$5" horiz="1" max="100" inc="5" page="20" val="30""#,
        ),
    ),
    (
        1033,
        "Spin",
        "",
        "",
        Some(r#"objectType="Spin" dx="22" fmlaLink="$C$6" max="10" min="1" val="2""#),
    ),
    // Legacy-only control: VML client data, no control properties part.
    (
        1034,
        "Label",
        "Ghi chú",
        "<x:AutoFill>False</x:AutoFill>",
        None,
    ),
];

fn anchor(i: usize) -> String {
    format!("4, 5, {r}, 2, 6, 10, {r}, 18", r = i * 2 + 1)
}

/// The hand-written fixture workbook.
fn fixture() -> Vec<u8> {
    let mut p = Package::new();
    let mut ct = format!(
        r#"{DECL}<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="vml" ContentType="application/vnd.openxmlformats-officedocument.vmlDrawing"/><Default Extension="png" ContentType="image/png"/><Default Extension="bin" ContentType="application/vnd.ms-office.activeX"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/activeX/activeX1.xml" ContentType="application/vnd.ms-office.activeX+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>"#
    );
    p.put_part(
        "/xl/styles.xml",
        format!(r#"{DECL}<styleSheet xmlns="{NS_MAIN}"><fonts count="1"><font><sz val="11"/><name val="Calibri"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>"#).into_bytes(),
    );
    p.put_part(
        "/_rels/.rels",
        format!(r#"{DECL}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL}officeDocument" Target="xl/workbook.xml"/></Relationships>"#).into_bytes(),
    );
    p.put_part(
        "/xl/workbook.xml",
        format!(r#"{DECL}<workbook xmlns="{NS_MAIN}" xmlns:r="{NS_R}"><sheets><sheet name="Dữ liệu" sheetId="1" r:id="rId1"/></sheets></workbook>"#).into_bytes(),
    );
    p.put_part(
        "/xl/_rels/workbook.xml.rels",
        format!(r#"{DECL}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL}worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="{REL}styles" Target="styles.xml"/></Relationships>"#).into_bytes(),
    );
    let mut rels = format!(
        r#"{DECL}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL}vmlDrawing" Target="../drawings/vmlDrawing1.vml"/>"#
    );
    let mut shapes = String::new();
    let mut entries = String::new();
    let mut n = 1;
    for (i, (id, ot, text, extra, prop)) in SPECS.iter().enumerate() {
        let textbox = if text.is_empty() {
            String::new()
        } else {
            format!("<v:textbox style='mso-direction-alt:auto'><div style='text-align:left'><font face=\"Tahoma\" size=\"160\" color=\"auto\">{text}</font></div></v:textbox>")
        };
        let _ = write!(
            shapes,
            "\n <v:shape id=\"_x0000_s{id}\" type=\"#_x0000_t201\" style='position:absolute;width:96pt;height:18pt;z-index:{}' filled=\"f\" stroked=\"f\">{textbox}\n  <x:ClientData ObjectType=\"{ot}\">\n   <x:Anchor>\n    {}</x:Anchor>{extra}</x:ClientData>\n </v:shape>",
            i + 1,
            anchor(i)
        );
        if let Some(prop) = prop {
            n += 1;
            let _ = write!(
                rels,
                r#"<Relationship Id="rId{n}" Type="{REL}ctrlProp" Target="../ctrlProps/ctrlProp{n}.xml"/>"#
            );
            let macro_attr = if *id == 1025 {
                r#" macro="[0]!TinhToan""#
            } else {
                ""
            };
            let r = (i * 2 + 1) as u32;
            let _ = write!(
                entries,
                r#"<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:Choice Requires="x14"><control shapeId="{id}" r:id="rId{n}" name="{ot} {id}"><controlPr defaultSize="0" autoFill="0" autoLine="0" autoPict="0"{macro_attr}><anchor moveWithCells="1"><from><xdr:col>4</xdr:col><xdr:colOff>47625</xdr:colOff><xdr:row>{r}</xdr:row><xdr:rowOff>19050</xdr:rowOff></from><to><xdr:col>6</xdr:col><xdr:colOff>95250</xdr:colOff><xdr:row>{r}</xdr:row><xdr:rowOff>171450</xdr:rowOff></to></anchor></controlPr></control></mc:Choice></mc:AlternateContent>"#
            );
            let _ = write!(
                ct,
                r#"<Override PartName="/xl/ctrlProps/ctrlProp{n}.xml" ContentType="application/vnd.ms-excel.controlproperties+xml"/>"#
            );
            p.put_part(
                &format!("/xl/ctrlProps/ctrlProp{n}.xml"),
                format!(r#"{DECL}<formControlPr xmlns="{X14}" {prop}/>"#).into_bytes(),
            );
        }
    }
    // A note (comment) shape lives in the same drawing and is not a control.
    shapes.push_str("<v:shape id=\"_x0000_s1040\" type=\"#_x0000_t202\"><x:ClientData ObjectType=\"Note\"><x:Row>0</x:Row><x:Column>1</x:Column></x:ClientData></v:shape>");
    // ActiveX command button with a fallback picture.
    shapes.push_str("<v:shape id=\"CommandButton1\" o:spid=\"_x0000_s1050\" type=\"#_x0000_t201\"><v:imagedata o:relid=\"rId1\" o:title=\"\"/><x:ClientData ObjectType=\"Pict\"><x:Anchor>1, 0, 20, 0, 3, 0, 22, 0</x:Anchor><x:CF>Pict</x:CF></x:ClientData></v:shape>");
    let _ = write!(
        rels,
        r#"<Relationship Id="rId20" Type="{REL}control" Target="../activeX/activeX1.xml"/><Relationship Id="rId21" Type="{REL}image" Target="../media/image1.png"/></Relationships>"#
    );
    entries.push_str(r#"<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:Choice Requires="x14"><control shapeId="1050" r:id="rId20" name="CommandButton1"><controlPr defaultSize="0" autoLine="0" r:id="rId21"><anchor moveWithCells="1"><from><xdr:col>1</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>20</xdr:row><xdr:rowOff>0</xdr:rowOff></from><to><xdr:col>3</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>22</xdr:row><xdr:rowOff>0</xdr:rowOff></to></anchor></controlPr></control></mc:Choice><mc:Fallback><control shapeId="1050" r:id="rId20" name="CommandButton1"/></mc:Fallback></mc:AlternateContent>"#);
    let vml = format!(
        "<xml xmlns:v=\"urn:schemas-microsoft-com:vml\"\n xmlns:o=\"urn:schemas-microsoft-com:office:office\"\n xmlns:x=\"urn:schemas-microsoft-com:office:excel\">\n <o:shapelayout v:ext=\"edit\">\n  <o:idmap v:ext=\"edit\" data=\"1\"/>\n </o:shapelayout><v:shapetype id=\"_x0000_t201\" coordsize=\"21600,21600\" o:spt=\"201\" path=\"m,l,21600r21600,l21600,xe\"/>{shapes}</xml>"
    );
    p.put_part("/xl/drawings/vmlDrawing1.vml", vml.into_bytes());
    p.put_part("/xl/worksheets/_rels/sheet1.xml.rels", rels.into_bytes());
    let mut cells = String::new();
    for (r, t) in ["Hà Nội", "Huế", "Sài Gòn"].iter().enumerate() {
        let _ = write!(
            cells,
            r#"<row r="{0}"><c r="A{0}" t="inlineStr"><is><t>{t}</t></is></c></row>"#,
            r + 1
        );
    }
    let sheet = format!(
        r#"{DECL}<worksheet xmlns="{NS_MAIN}" xmlns:r="{NS_R}" xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:x14="{X14}" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="x14ac" xmlns:x14ac="http://schemas.microsoft.com/office/spreadsheetml/2009/9/ac"><sheetData>{cells}</sheetData><pageMargins left="0.7" right="0.7" top="0.75" bottom="0.75" header="0.3" footer="0.3"/><legacyDrawing r:id="rId1"/><mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:Choice Requires="x14"><controls>{entries}</controls></mc:Choice><mc:Fallback><controls><control shapeId="1050" r:id="rId20" name="CommandButton1"/></controls></mc:Fallback></mc:AlternateContent></worksheet>"#
    );
    p.put_part("/xl/worksheets/sheet1.xml", sheet.into_bytes());
    p.put_part(
        "/xl/activeX/activeX1.xml",
        format!(r#"{DECL}<ax:ocx xmlns:ax="http://schemas.microsoft.com/office/2006/activeX" xmlns:r="{NS_R}" ax:classid="{{00000000-0000-0000-0000-0000000000AB}}" ax:persistence="persistStreamInit" r:id="rId1"/>"#).into_bytes(),
    );
    p.put_part(
        "/xl/activeX/_rels/activeX1.xml.rels",
        format!(r#"{DECL}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.microsoft.com/office/2006/relationships/activeXControlBinary" Target="activeX1.bin"/></Relationships>"#).into_bytes(),
    );
    p.put_part("/xl/activeX/activeX1.bin", vec![1, 2, 3, 4, 5, 0, 0xFF]);
    p.put_part("/xl/media/image1.png", PNG_1X1.to_vec());
    ct.push_str("</Types>");
    p.put_part("/[Content_Types].xml", ct.into_bytes());
    p.write().unwrap()
}

fn parts(bytes: &[u8]) -> Package {
    Package::read(bytes, &Limits::default()).unwrap()
}

fn part(p: &Package, name: &str) -> Option<Vec<u8>> {
    p.part(name).map(|x| x.data.clone())
}

fn by_id(wb: &Workbook, id: u32) -> &FormControl {
    wb.sheets[0]
        .form_controls
        .iter()
        .find(|c| c.shape_id == id)
        .unwrap()
}

#[test]
fn reads_every_control_type() {
    let wb = read_xlsx(&fixture()).unwrap();
    let list = &wb.sheets[0].form_controls;
    let kinds: Vec<K> = list.iter().map(|c| c.kind).collect();
    assert_eq!(
        kinds,
        [
            K::Button,
            K::CheckBox,
            K::ComboBox,
            K::ListBox,
            K::GroupBox,
            K::OptionButton,
            K::OptionButton,
            K::ScrollBar,
            K::Spinner,
            K::Label,
            K::ActiveX
        ]
    );
    let b = by_id(&wb, 1025);
    assert_eq!(b.text, "Tính toán");
    assert_eq!(b.macro_name.as_deref(), Some("[0]!TinhToan"));
    assert_eq!(b.name, "Button 1025");
    // controlPr anchor (EMU) wins over the VML anchor (pixels).
    assert_eq!(
        b.anchor,
        ControlAnchor {
            from: CellRef::new(1, 4),
            from_off: (47_625, 19_050),
            to: CellRef::new(1, 6),
            to_off: (95_250, 171_450),
        }
    );
    let cb = by_id(&wb, 1026);
    assert_eq!(
        (cb.checked, cb.link.as_deref(), cb.no_three_d),
        (CheckState::Checked, Some("$C$1"), true)
    );
    let drop = by_id(&wb, 1027);
    assert_eq!(
        (drop.input_range.as_deref(), drop.sel, drop.drop_lines),
        (Some("$A$1:$A$3"), 2, 5)
    );
    assert_eq!(
        by_id(&wb, 1028).input_range.as_deref(),
        Some("'Dữ liệu'!$A$1:$A$3")
    );
    assert!(by_id(&wb, 1030).first_button);
    assert_eq!(by_id(&wb, 1031).checked, CheckState::Unchecked);
    let sc = by_id(&wb, 1032);
    assert_eq!(
        (sc.val, sc.min, sc.max, sc.inc, sc.page, sc.horiz),
        (30, 0, 100, 5, 20, true)
    );
    let sp = by_id(&wb, 1033);
    assert_eq!((sp.val, sp.min, sp.max, sp.inc), (2, 1, 10, 1));
    let label = by_id(&wb, 1034);
    assert_eq!(label.text, "Ghi chú");
    assert_eq!(label.ctrl_prop_part, "");
    assert_eq!(label.vml_part, "/xl/drawings/vmlDrawing1.vml");
    // VML anchor in pixels.
    assert_eq!(label.anchor.from, CellRef::new(19, 4));
    assert_eq!(label.anchor.from_off, (5 * 9525, 2 * 9525));
    let ax = by_id(&wb, 1050);
    assert_eq!(ax.name, "CommandButton1");
    assert_eq!(ax.activex_part, "/xl/activeX/activeX1.xml");
    assert_eq!(ax.image_part.as_deref(), Some("/xl/media/image1.png"));
    assert_eq!(ax.anchor.from, CellRef::new(20, 1));
}

#[test]
fn untouched_controls_stay_byte_for_byte() {
    let src = fixture();
    let wb = read_xlsx(&src).unwrap();
    let out = write_xlsx(&wb).unwrap();
    let (a, b) = (parts(&src), parts(&out));
    for p in a.parts() {
        let n = &p.name;
        if n.contains("ctrlProps")
            || n.contains("vmlDrawing")
            || n.contains("activeX")
            || n.contains("media")
        {
            assert_eq!(part(&b, n).as_deref(), Some(p.data.as_slice()), "{n}");
        }
    }
    let sheet = String::from_utf8(part(&b, "/xl/worksheets/sheet1.xml").unwrap()).unwrap();
    let orig = String::from_utf8(part(&a, "/xl/worksheets/sheet1.xml").unwrap()).unwrap();
    let block = |s: &str| {
        let i = s.find("<mc:AlternateContent").unwrap();
        s[i..s.rfind("</mc:AlternateContent>").unwrap()].to_owned()
    };
    assert_eq!(block(&sheet), block(&orig));
    assert!(sheet.contains("<legacyDrawing r:id=\"rId1\"/>"));
    let again = read_xlsx(&out).unwrap();
    assert_eq!(again.sheets[0].form_controls, wb.sheets[0].form_controls);
}

#[test]
fn changed_controls_are_patched() {
    let src = fixture();
    let mut wb = read_xlsx(&src).unwrap();
    for c in &mut wb.sheets[0].form_controls {
        match c.shape_id {
            1026 => c.checked = CheckState::Unchecked,
            1032 => c.val = 55,
            1034 => c.text = "Ghi chú mới".into(),
            _ => continue,
        }
        c.dirty = true;
    }
    let out = write_xlsx(&wb).unwrap();
    if let Ok(dir) = std::env::var("BO_FORMCTL_OUT") {
        std::fs::write(format!("{dir}/formctl-changed.xlsx"), &out).unwrap();
    }
    let b = parts(&out);
    let cb = String::from_utf8(part(&b, "/xl/ctrlProps/ctrlProp3.xml").unwrap()).unwrap();
    assert!(!cb.contains("checked="), "{cb}");
    assert!(cb.contains(r#"fmlaLink="$C$1""#) && cb.contains(r#"noThreeD="1""#));
    let sc = String::from_utf8(part(&b, "/xl/ctrlProps/ctrlProp9.xml").unwrap()).unwrap();
    assert!(
        sc.contains(r#"val="55""#) && sc.contains(r#"dx="22""#),
        "{sc}"
    );
    // Untouched controls keep their parts.
    let a = parts(&src);
    assert_eq!(
        part(&b, "/xl/ctrlProps/ctrlProp4.xml"),
        part(&a, "/xl/ctrlProps/ctrlProp4.xml")
    );
    let vml = String::from_utf8(part(&b, "/xl/drawings/vmlDrawing1.vml").unwrap()).unwrap();
    assert!(vml.contains("<x:Val>55</x:Val>"));
    assert!(vml.contains("Ghi chú mới"));
    assert!(vml.contains("ObjectType=\"Note\""));
    let again = read_xlsx(&out).unwrap();
    assert_eq!(by_id(&again, 1026).checked, CheckState::Unchecked);
    assert_eq!(by_id(&again, 1032).val, 55);
    assert_eq!(by_id(&again, 1034).text, "Ghi chú mới");
    assert_eq!(by_id(&again, 1030).checked, CheckState::Checked);
}

// source: [MS-XLSX] controlPr/anchor (CT_ObjectAnchor, xdr:from / xdr:to); VML x:Anchor
#[test]
fn moved_controls_get_their_anchors_patched() {
    let src = fixture();
    let mut wb = read_xlsx(&src).unwrap();
    let moved = ControlAnchor {
        from: CellRef::new(7, 5),
        from_off: (47_625, 19_050),
        to: CellRef::new(7, 7),
        to_off: (95_250, 171_450),
    };
    let ax = ControlAnchor {
        from: CellRef::new(23, 1),
        to: CellRef::new(25, 3),
        ..ControlAnchor::default()
    };
    for c in &mut wb.sheets[0].form_controls {
        match c.shape_id {
            // Not marked dirty: rows / columns inserted only move the anchor.
            1026 => c.anchor = moved,
            1050 => c.anchor = ax,
            _ => {}
        }
    }
    let out = write_xlsx(&wb).unwrap();
    let b = parts(&out);
    let sheet = String::from_utf8(part(&b, "/xl/worksheets/sheet1.xml").unwrap()).unwrap();
    assert!(sheet
        .contains("<from><xdr:col>5</xdr:col><xdr:colOff>47625</xdr:colOff><xdr:row>7</xdr:row>"));
    let vml = String::from_utf8(part(&b, "/xl/drawings/vmlDrawing1.vml").unwrap()).unwrap();
    assert!(
        vml.contains("<x:Anchor>5, 5, 7, 2, 7, 10, 7, 18</x:Anchor>"),
        "{vml}"
    );
    assert!(vml.contains("<x:Anchor>1, 0, 23, 0, 3, 0, 25, 0</x:Anchor>"));
    // Untouched controls keep their anchors and parts.
    assert!(vml.contains(&anchor(2)));
    let a = parts(&src);
    assert_eq!(
        part(&b, "/xl/ctrlProps/ctrlProp3.xml"),
        part(&a, "/xl/ctrlProps/ctrlProp3.xml")
    );
    let again = read_xlsx(&out).unwrap();
    assert_eq!(by_id(&again, 1026).anchor, moved);
    assert_eq!(by_id(&again, 1050).anchor, ax);
    assert_eq!(by_id(&again, 1027).anchor, by_id(&wb, 1027).anchor);
}

#[test]
fn removed_controls_leave_the_package() {
    let src = fixture();
    let mut wb = read_xlsx(&src).unwrap();
    wb.sheets[0]
        .form_controls
        .retain(|c| c.shape_id != 1033 && c.shape_id != 1034);
    let out = write_xlsx(&wb).unwrap();
    if let Ok(dir) = std::env::var("BO_FORMCTL_OUT") {
        std::fs::write(format!("{dir}/formctl-removed.xlsx"), &out).unwrap();
    }
    let b = parts(&out);
    assert!(part(&b, "/xl/ctrlProps/ctrlProp10.xml").is_none());
    let rels =
        String::from_utf8(part(&b, "/xl/worksheets/_rels/sheet1.xml.rels").unwrap()).unwrap();
    assert!(!rels.contains("ctrlProp10.xml"));
    let sheet = String::from_utf8(part(&b, "/xl/worksheets/sheet1.xml").unwrap()).unwrap();
    assert!(!sheet.contains("shapeId=\"1033\""));
    let vml = String::from_utf8(part(&b, "/xl/drawings/vmlDrawing1.vml").unwrap()).unwrap();
    assert!(!vml.contains("_x0000_s1033") && !vml.contains("_x0000_s1034"));
    let again = read_xlsx(&out).unwrap();
    let ids: Vec<u32> = again.sheets[0]
        .form_controls
        .iter()
        .map(|c| c.shape_id)
        .collect();
    assert_eq!(ids, [1025, 1026, 1027, 1028, 1029, 1030, 1031, 1032, 1050]);
    // ActiveX parts and the fallback picture are kept.
    assert!(part(&b, "/xl/activeX/activeX1.bin").is_some());
    assert!(part(&b, "/xl/media/image1.png").is_some());
}

/// One new control of each kind, in a row each.
pub fn new_controls() -> Vec<FormControl> {
    let kinds = [
        (K::Button, "Lưu"),
        (K::CheckBox, "Đồng ý"),
        (K::ComboBox, ""),
        (K::ListBox, ""),
        (K::GroupBox, "Chọn"),
        (K::OptionButton, "Có"),
        (K::OptionButton, "Không"),
        (K::ScrollBar, ""),
        (K::Spinner, ""),
        (K::Label, "Nhãn"),
    ];
    kinds
        .iter()
        .enumerate()
        .map(|(i, (k, t))| {
            let r = 30 + i as u32 * 2;
            FormControl {
                kind: *k,
                name: format!("Mới {i}"),
                text: (*t).into(),
                link: (!matches!(k, K::Button | K::GroupBox | K::Label))
                    .then(|| format!("$H${}", i + 1)),
                input_range: matches!(k, K::ComboBox | K::ListBox).then(|| "$A$1:$A$3".into()),
                sel: u32::from(*k == K::ComboBox),
                val: if *k == K::ScrollBar { 40 } else { 0 },
                checked: if *k == K::CheckBox {
                    CheckState::Checked
                } else {
                    CheckState::Unchecked
                },
                macro_name: (*k == K::Button).then(|| "LuuDuLieu".into()),
                anchor: ControlAnchor {
                    from: CellRef::new(r, 1),
                    from_off: (9525, 9525),
                    to: CellRef::new(r + 1, 3),
                    to_off: (0, 0),
                },
                ..FormControl::default()
            }
        })
        .collect()
}

#[test]
fn new_controls_are_written_and_read_back() {
    // Into a sheet that has no legacy drawing yet.
    let mut wb = Workbook::new();
    wb.sheets[0].set_cell(
        0,
        0,
        bo_model::sheet::Cell::new(bo_model::sheet::CellValue::Text("A".into())),
    );
    wb.sheets[0].form_controls = new_controls();
    let out = write_xlsx(&wb).unwrap();
    let again = read_xlsx(&out).unwrap();
    let got = &again.sheets[0].form_controls;
    assert_eq!(got.len(), 10);
    for (a, b) in got.iter().zip(&wb.sheets[0].form_controls) {
        assert_eq!(
            (
                a.kind,
                &a.name,
                &a.text,
                &a.link,
                &a.input_range,
                a.sel,
                a.val,
                a.checked
            ),
            (
                b.kind,
                &b.name,
                &b.text,
                &b.link,
                &b.input_range,
                b.sel,
                b.val,
                b.checked
            )
        );
        assert_eq!(a.anchor, b.anchor);
        assert_eq!(a.macro_name, b.macro_name);
        assert!(!a.ctrl_prop_part.is_empty() && !a.vml_part.is_empty());
        assert!(a.shape_id > 1024);
    }
    let sheet =
        String::from_utf8(part(&parts(&out), "/xl/worksheets/sheet1.xml").unwrap()).unwrap();
    let ld = sheet.find("<legacyDrawing").unwrap();
    assert!(ld < sheet.find("<controls>").unwrap());
    if let Ok(dir) = std::env::var("BO_FORMCTL_OUT") {
        std::fs::write(format!("{dir}/formctl-new.xlsx"), &out).unwrap();
    }

    // Into the fixture: appended to its legacy drawing and `controls` element.
    let mut wb = read_xlsx(&fixture()).unwrap();
    wb.sheets[0].form_controls.extend(new_controls());
    let out = write_xlsx(&wb).unwrap();
    let again = read_xlsx(&out).unwrap();
    let got = &again.sheets[0].form_controls;
    assert_eq!(got.len(), 21);
    let ids: std::collections::BTreeSet<u32> = got.iter().map(|c| c.shape_id).collect();
    assert_eq!(ids.len(), 21);
    assert!(got
        .iter()
        .all(|c| c.vml_part == "/xl/drawings/vmlDrawing1.vml" || c.kind == K::ActiveX));
    if let Ok(dir) = std::env::var("BO_FORMCTL_OUT") {
        std::fs::write(format!("{dir}/formctl-fixture.xlsx"), &out).unwrap();
        std::fs::write(format!("{dir}/formctl-src.xlsx"), fixture()).unwrap();
    }
}
