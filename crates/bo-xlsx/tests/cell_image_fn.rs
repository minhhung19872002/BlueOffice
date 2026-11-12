//! `IMAGE` function results saved as web picture rich values ([MS-XLSX]
//! §2.3.6.1.11 Web Image Type `_webimage`): the cell keeps its formula
//! (`_xlfn.IMAGE`, [MS-XLSX] §2.2.2) and the `#VALUE!` fallback, and points through
//! `vm` (ECMA-376 Part 1, §18.3.1.4) at a rich value whose `WebImageIdentifier`
//! indexes the Rich Value Web Image part (`webImagesSrd/webImageSrd/address`: an
//! external hyperlink relationship to the URL). Round trips, byte stability, a
//! changed `IMAGE` argument and the corpus file
//! `tests/corpus/sheet/writer-image-function-1.xlsx` (`BO_WRITE_CORPUS=1`).
#![allow(clippy::unwrap_used)]

use bo_model::sheet::{
    Cell, CellError, CellImage, CellImageSource, CellValue, Formula, ImageSizing, OpaquePart,
    Workbook,
};
use bo_xlsx::{read_xlsx, write_xlsx};
use std::io::Read as _;
use std::path::Path;

fn web(url: &str, sizing: ImageSizing, size: Option<(f64, f64)>, alt: Option<&str>) -> CellImage {
    CellImage {
        source: CellImageSource::Web {
            url: url.into(),
            sizing,
            height: size.map(|s| s.0),
            width: size.map(|s| s.1),
        },
        alt_text: alt.map(str::to_owned),
        calc_origin: 1,
    }
}

fn image_cell(formula: &str, img: CellImage) -> Cell {
    Cell {
        value: CellValue::Error(CellError::Value),
        formula: Some(Formula::normal(formula)),
        image: Some(Box::new(img)),
        ..Cell::default()
    }
}

fn sample() -> Workbook {
    let mut wb = Workbook::new();
    let ws = &mut wb.sheets[0];
    ws.set_a1("A1", CellValue::Text("Logo".into()));
    let logo = "https://example.vn/logo.png";
    ws.set_cell(
        0,
        1,
        image_cell(
            &format!("IMAGE(\"{logo}\",\"Biểu trưng\")"),
            web(logo, ImageSizing::Fit, None, Some("Biểu trưng")),
        ),
    );
    // Same picture: shares the rich value.
    ws.set_cell(
        1,
        1,
        image_cell(
            &format!("IMAGE(\"{logo}\",\"Biểu trưng\")"),
            web(logo, ImageSizing::Fit, None, Some("Biểu trưng")),
        ),
    );
    // Custom size, no alt text, same URL: another rich value, same web image entry.
    ws.set_cell(
        2,
        1,
        image_cell(
            &format!("IMAGE(\"{logo}\",,3,40,60)"),
            web(logo, ImageSizing::Custom, Some((40.0, 60.0)), None),
        ),
    );
    let map = "https://example.vn/ban-do.jpg?a=1&b=2";
    ws.set_cell(
        3,
        1,
        image_cell(
            "IMAGE(\"https://example.vn/ban-do.jpg?a=1&b=2\",,1)",
            web(map, ImageSizing::Fill, None, None),
        ),
    );
    wb
}

fn part<'a>(wb: &'a Workbook, suffix: &str) -> &'a OpaquePart {
    wb.opaque
        .parts
        .iter()
        .find(|p| p.name.to_ascii_lowercase().ends_with(suffix))
        .unwrap_or_else(|| panic!("no part {suffix}"))
}

fn text(wb: &Workbook, suffix: &str) -> String {
    String::from_utf8(part(wb, suffix).data.clone()).unwrap()
}

/// The raw XML of a package part.
fn zip_part(bytes: &[u8], name: &str) -> String {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut f = z.by_name(name).unwrap();
    let mut s = String::new();
    f.read_to_string(&mut s).unwrap();
    s
}

#[test]
fn image_results_are_saved_as_web_image_rich_values() {
    let bytes = write_xlsx(&sample()).unwrap();
    let sheet = zip_part(&bytes, "xl/worksheets/sheet1.xml");
    assert!(
        sheet.contains(
            r#"t="e" vm="1"><f>_xlfn.IMAGE(&quot;https://example.vn/logo.png&quot;,&quot;Biểu trưng&quot;)</f><v>#VALUE!</v>"#
        ),
        "{sheet}"
    );
    let back = read_xlsx(&bytes).unwrap();
    let ws = &back.sheets[0];
    assert_eq!(ws.cell(0, 1).unwrap().vm, Some(1));
    assert_eq!(ws.cell(1, 1).unwrap().vm, Some(1));
    assert_eq!(ws.cell(2, 1).unwrap().vm, Some(2));
    assert_eq!(ws.cell(3, 1).unwrap().vm, Some(3));
    // The pictures come back from the rich values.
    let orig = sample();
    for r in 0..4 {
        assert_eq!(
            ws.cell(r, 1).unwrap().image,
            orig.sheets[0].cell(r, 1).unwrap().image,
            "row {r}"
        );
    }
    // [MS-XLSX] §2.3.6.1.11 keys, one `v` per key in key order.
    let rvs = text(&back, "rdrichvaluestructure.xml");
    assert!(
        rvs.contains(
            r#"<s t="_webimage"><k n="WebImageIdentifier" t="i"/><k n="CalcOrigin" t="i"/><k n="ComputedImage" t="b"/><k n="ImageSizing" t="i"/><k n="Text" t="s"/></s>"#
        ),
        "{rvs}"
    );
    let rv = text(&back, "rdrichvalue.xml");
    assert!(
        rv.contains("<rv s=\"0\"><v>0</v><v>1</v><v>1</v><v>0</v><v>Biểu trưng</v></rv>"),
        "{rv}"
    );
    assert!(
        rv.contains("<rv s=\"1\"><v>0</v><v>1</v><v>1</v><v>3</v><v>40</v><v>60</v></rv>"),
        "{rv}"
    );
    assert!(rv.contains("<v>1</v><v>1</v><v>1</v><v>1</v></rv>"), "{rv}");
    // Two URLs → two web image entries, each an external hyperlink.
    let web = text(&back, "rdrichvaluewebimage.xml");
    assert_eq!(web.matches("<webImageSrd>").count(), 2, "{web}");
    let rels = text(&back, "_rels/rdrichvaluewebimage.xml.rels");
    assert!(rels.contains("TargetMode=\"External\""), "{rels}");
    assert!(
        rels.contains("Target=\"https://example.vn/ban-do.jpg?a=1&amp;b=2\""),
        "{rels}"
    );
    let ct = zip_part(&bytes, "[Content_Types].xml");
    assert!(
        ct.contains("application/vnd.ms-excel.rdrichvaluewebimage+xml"),
        "{ct}"
    );
    let wb_rels = zip_part(&bytes, "xl/_rels/workbook.xml.rels");
    assert!(
        wb_rels.contains(
            "http://schemas.microsoft.com/office/2020/07/relationships/rdrichvaluewebimage"
        ),
        "{wb_rels}"
    );
}

#[test]
fn saving_again_is_byte_stable() {
    let first = write_xlsx(&sample()).unwrap();
    let back = read_xlsx(&first).unwrap();
    let second = write_xlsx(&back).unwrap();
    let a = read_xlsx(&first).unwrap();
    let b = read_xlsx(&second).unwrap();
    assert_eq!(a, b);
    for suffix in [
        "metadata.xml",
        "rdrichvalue.xml",
        "rdrichvaluestructure.xml",
        "rdrichvaluewebimage.xml",
    ] {
        assert_eq!(part(&a, suffix).data, part(&b, suffix).data, "{suffix}");
    }
}

#[test]
fn a_changed_image_gets_a_new_rich_value() {
    let first = write_xlsx(&sample()).unwrap();
    let mut back = read_xlsx(&first).unwrap();
    // The formula now asks for another picture (as a recalculation sets it); `vm`
    // still points at the old rich value.
    let url = "https://example.vn/moi.png";
    let cell = back.sheets[0].cells.get_mut(&(1, 1)).unwrap();
    cell.formula = Some(Formula::normal(&format!("IMAGE(\"{url}\")")));
    cell.image = Some(Box::new(web(url, ImageSizing::Fit, None, None)));
    let again = read_xlsx(&write_xlsx(&back).unwrap()).unwrap();
    let ws = &again.sheets[0];
    assert_eq!(ws.cell(0, 1).unwrap().vm, Some(1));
    assert_eq!(ws.cell(1, 1).unwrap().vm, Some(4));
    assert_eq!(
        ws.cell(1, 1).unwrap().image.as_deref(),
        Some(&web(url, ImageSizing::Fit, None, None))
    );
    let web_part = text(&again, "rdrichvaluewebimage.xml");
    assert_eq!(
        web_part.matches("webImageSrd>").count(),
        3 * 2,
        "{web_part}"
    );
}

#[test]
fn generate_corpus_file() {
    if std::env::var("BO_WRITE_CORPUS").as_deref() != Ok("1") {
        return;
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/sheet");
    std::fs::write(
        dir.join("writer-image-function-1.xlsx"),
        write_xlsx(&sample()).unwrap(),
    )
    .unwrap();
}
