//! Pictures placed in cells ([MS-XLSX] §2.3.6.1.10 `_localImage` rich values)
//! and checkbox cell controls ([MS-XLSX] §2.3.9 feature property bags):
//! write → read round trips, appending to existing rich data without touching
//! rich values of other types, and the corpus file
//! `tests/corpus/sheet/writer-cell-media-1.xlsx` (`BO_WRITE_CORPUS=1`).
#![allow(clippy::unwrap_used)]

use bo_model::sheet::{
    Cell, CellError, CellImage, CellValue, CellXf, OpaquePart, OpaqueRel, Workbook,
};
use bo_xlsx::{read_xlsx, write_xlsx};
use std::path::Path;

/// A 2×1 pixel PNG (red, blue): IHDR width 2, height 1, 8-bit RGB.
const PNG_2X1: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x7b, 0x40, 0xe8,
    0xdd, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0x00, 0x04,
    0xff, 0x01, 0x07, 0x00, 0x01, 0xff, 0xe2, 0x23, 0x9e, 0x59, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

fn picture_cell(data: &[u8], alt: Option<&str>) -> Cell {
    Cell {
        value: CellValue::Error(CellError::Value),
        image: Some(Box::new(CellImage::local(
            data.to_vec(),
            "png",
            alt.map(str::to_owned),
        ))),
        ..Cell::default()
    }
}

fn sample() -> Workbook {
    let mut wb = Workbook::new();
    let checkbox = wb.styles.add_xf(CellXf {
        checkbox: true,
        ..CellXf::default()
    });
    let ws = &mut wb.sheets[0];
    ws.set_a1("A1", CellValue::Text("Ảnh".into()));
    ws.set_cell(0, 1, picture_cell(PNG_2X1, Some("Hai điểm ảnh")));
    // Same picture, same alt text: shares the rich value.
    ws.set_cell(1, 1, picture_cell(PNG_2X1, Some("Hai điểm ảnh")));
    // Same picture without alt text: another structure and rich value, same media.
    ws.set_cell(2, 1, picture_cell(PNG_2X1, None));
    ws.set_cell(0, 2, Cell::new(CellValue::Bool(true)).with_style(checkbox));
    ws.set_cell(1, 2, Cell::new(CellValue::Bool(false)).with_style(checkbox));
    wb
}

fn read_back(wb: &Workbook) -> (Vec<u8>, Workbook) {
    let bytes = write_xlsx(wb).unwrap();
    let back = read_xlsx(&bytes).unwrap();
    (bytes, back)
}

fn part<'a>(wb: &'a Workbook, suffix: &str) -> &'a OpaquePart {
    wb.opaque
        .parts
        .iter()
        .find(|p| p.name.ends_with(suffix))
        .unwrap_or_else(|| panic!("no part {suffix}"))
}

#[test]
fn pictures_and_checkboxes_round_trip() {
    let (_, back) = read_back(&sample());
    let ws = &back.sheets[0];
    let b1 = ws.cell(0, 1).unwrap();
    let img = b1.image.as_deref().unwrap();
    assert_eq!(img.data(), Some(PNG_2X1));
    assert_eq!(img.alt_text.as_deref(), Some("Hai điểm ảnh"));
    assert_eq!(img.calc_origin, 5);
    assert_eq!(b1.value, CellValue::Error(CellError::Value));
    // Two distinct rich values (alt text differs) → vm 1 and 2; B2 shares B1's.
    assert_eq!(b1.vm, Some(1));
    assert_eq!(ws.cell(1, 1).unwrap().vm, Some(1));
    assert_eq!(ws.cell(2, 1).unwrap().vm, Some(2));
    assert_eq!(
        ws.cell(2, 1).unwrap().image.as_ref().unwrap().alt_text,
        None
    );
    // One media part for the shared bytes.
    let media = back
        .opaque
        .parts
        .iter()
        .filter(|p| p.name.starts_with("/xl/media/"))
        .count();
    assert_eq!(media, 1);
    // Checkbox formats.
    let c1 = ws.cell(0, 2).unwrap();
    assert!(back.styles.cell_xfs[c1.style as usize].checkbox);
    assert_eq!(c1.value, CellValue::Bool(true));
    assert!(!back.styles.cell_xfs[0].checkbox);
    // Rich value parts as [MS-XLSX] §2.3.6.1.10 describes them.
    let rv = String::from_utf8(part(&back, "rdrichvalue.xml").data.clone()).unwrap();
    assert!(
        rv.contains("<rv s=\"0\"><v>0</v><v>5</v><v>Hai điểm ảnh</v></rv>"),
        "{rv}"
    );
    assert!(rv.contains("<rv s=\"1\"><v>0</v><v>5</v></rv>"), "{rv}");
}

#[test]
fn saving_again_is_byte_stable() {
    let (first, back) = read_back(&sample());
    let second = write_xlsx(&back).unwrap();
    let a = read_xlsx(&first).unwrap();
    let b = read_xlsx(&second).unwrap();
    assert_eq!(a, b);
    // The rich data parts are kept verbatim once written.
    for suffix in ["metadata.xml", "rdrichvalue.xml", "featurePropertyBag.xml"] {
        assert_eq!(part(&a, suffix).data, part(&b, suffix).data, "{suffix}");
    }
}

#[test]
fn new_pictures_are_appended_after_existing_rich_values() {
    let (_, mut back) = read_back(&sample());
    let other: Vec<u8> = PNG_2X1.iter().rev().copied().collect();
    back.sheets[0].set_cell(5, 0, picture_cell(&other, Some("mới")));
    let (_, again) = read_back(&back);
    let ws = &again.sheets[0];
    assert_eq!(ws.cell(0, 1).unwrap().vm, Some(1));
    assert_eq!(ws.cell(2, 1).unwrap().vm, Some(2));
    let a6 = ws.cell(5, 0).unwrap();
    assert_eq!(a6.vm, Some(3));
    let img = a6.image.as_deref().unwrap();
    assert_eq!(img.data(), Some(other.as_slice()));
    assert_eq!(img.alt_text.as_deref(), Some("mới"));
    assert_eq!(
        ws.cell(0, 1).unwrap().image.as_ref().unwrap().data(),
        Some(PNG_2X1)
    );
}

/// A rich value of a type BlueSheet does not show (`_error`, [MS-XLSX] §2.3.6.1.3)
/// stays in the parts with its `vm` when pictures are added.
#[test]
fn unknown_rich_values_are_kept() {
    let mut wb = Workbook::new();
    let rd = "http://schemas.microsoft.com/office/spreadsheetml/2017/richdata";
    let parts = [
        (
            "/xl/metadata.xml",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheetMetadata+xml",
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/sheetMetadata",
            format!(
                "<metadata xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" \
                 xmlns:xlrd=\"{rd}\"><metadataTypes count=\"1\"><metadataType name=\"XLRICHVALUE\" \
                 minSupportedVersion=\"120000\"/></metadataTypes><futureMetadata name=\"XLRICHVALUE\" \
                 count=\"1\"><bk><extLst><ext uri=\"{{3E2802C4-A4D2-4D8B-9148-E3BE6C30E623}}\">\
                 <xlrd:rvb i=\"0\"/></ext></extLst></bk></futureMetadata><valueMetadata count=\"1\">\
                 <bk><rc t=\"1\" v=\"0\"/></bk></valueMetadata></metadata>"
            ),
        ),
        (
            "/xl/richData/rdrichvalue.xml",
            "application/vnd.ms-excel.rdrichvalue+xml",
            "http://schemas.microsoft.com/office/2017/06/relationships/rdRichValue",
            format!("<rvData xmlns=\"{rd}\" count=\"1\"><rv s=\"0\"><v>9</v></rv></rvData>"),
        ),
        (
            "/xl/richData/rdrichvaluestructure.xml",
            "application/vnd.ms-excel.rdrichvaluestructure+xml",
            "http://schemas.microsoft.com/office/2017/06/relationships/rdRichValueStructure",
            format!(
                "<rvStructures xmlns=\"{rd}\" count=\"1\"><s t=\"_error\"><k n=\"errorType\" t=\"i\"/></s></rvStructures>"
            ),
        ),
    ];
    for (i, (name, ct, rel, xml)) in parts.iter().enumerate() {
        wb.opaque.parts.push(OpaquePart {
            name: (*name).into(),
            content_type: Some((*ct).into()),
            data: xml.as_bytes().to_vec(),
        });
        wb.opaque.workbook_rels.push(OpaqueRel {
            id: format!("rId{}", 90 + i),
            rel_type: (*rel).into(),
            target: (*name).into(),
            external: false,
        });
    }
    wb.sheets[0].set_cell(
        0,
        0,
        Cell {
            value: CellValue::Error(CellError::Other("#CONNECT!".into())),
            vm: Some(1),
            ..Cell::default()
        },
    );
    wb.sheets[0].set_cell(0, 1, picture_cell(PNG_2X1, None));
    let (_, back) = read_back(&wb);
    let ws = &back.sheets[0];
    let a1 = ws.cell(0, 0).unwrap();
    assert_eq!(a1.vm, Some(1));
    assert!(a1.image.is_none());
    let b1 = ws.cell(0, 1).unwrap();
    assert_eq!(b1.vm, Some(2));
    assert_eq!(b1.image.as_ref().unwrap().data(), Some(PNG_2X1));
    let rv = String::from_utf8(part(&back, "rdrichvalue.xml").data.clone()).unwrap();
    assert!(
        rv.starts_with(&format!(
            "<rvData xmlns=\"{rd}\" count=\"2\"><rv s=\"0\"><v>9</v></rv><rv s=\"1\">"
        )),
        "{rv}"
    );
    let rvs = String::from_utf8(part(&back, "rdrichvaluestructure.xml").data.clone()).unwrap();
    assert!(rvs.contains("<s t=\"_error\"><k n=\"errorType\" t=\"i\"/></s><s t=\"_localImage\">"));
}

#[test]
fn removing_the_checkbox_drops_the_complement() {
    let (_, mut back) = read_back(&sample());
    for xf in &mut back.styles.cell_xfs {
        xf.checkbox = false;
    }
    let bytes = write_xlsx(&back).unwrap();
    let again = read_xlsx(&bytes).unwrap();
    assert!(again
        .styles
        .cell_xfs
        .iter()
        .all(|x| !x.checkbox && x.complement.is_none()));
}

#[test]
fn generate_corpus_file() {
    if std::env::var("BO_WRITE_CORPUS").as_deref() != Ok("1") {
        return;
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/sheet");
    std::fs::write(
        dir.join("writer-cell-media-1.xlsx"),
        write_xlsx(&sample()).unwrap(),
    )
    .unwrap();
}
