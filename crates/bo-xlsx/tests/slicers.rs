//! Slicers ([MS-XLSX] slicer parts): a table slicer created through the model is written as
//! slicer cache + slicers parts with its drawing frame, filters the table, and reads back.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_model::sheet::{
    CellRange, CellRef, CellValue, SlicerSource, Table, TableColumn, Workbook, Worksheet,
};
use bo_sheet::numfmt::Locale;
use bo_sheet::pivot::PivotLabels;
use bo_xlsx::{read_xlsx, write_xlsx};

fn book() -> Workbook {
    let mut ws = Worksheet::new("Bán hàng");
    let rows = [
        ("Vùng", "Doanh số"),
        ("Bắc", "10"),
        ("Nam", "20"),
        ("Bắc", "30"),
        ("Trung", "40"),
    ];
    for (r, (a, b)) in rows.iter().enumerate() {
        let r = r as u32;
        ws.set_cell(
            r,
            0,
            bo_model::sheet::Cell::new(CellValue::Text((*a).into())),
        );
        let v = b
            .parse::<f64>()
            .map_or_else(|_| CellValue::Text((*b).into()), CellValue::Number);
        ws.set_cell(r, 1, bo_model::sheet::Cell::new(v));
    }
    ws.tables.push(Table {
        id: 1,
        name: "BanHang".into(),
        display_name: "BanHang".into(),
        range: CellRange::parse("A1:B5").unwrap(),
        columns: vec![
            TableColumn {
                id: 1,
                name: "Vùng".into(),
                ..TableColumn::default()
            },
            TableColumn {
                id: 2,
                name: "Doanh số".into(),
                ..TableColumn::default()
            },
        ],
        ..Table::default()
    });
    let mut wb = Workbook::new();
    wb.sheets = vec![ws];
    wb
}

#[test]
fn table_slicer_filters_saves_and_reads_back() {
    let mut wb = book();
    let i = bo_sheet::slicer::insert_table_slicer(&mut wb, 0, 1, 1, CellRef::new(0, 3)).unwrap();
    let locale = Locale::VI_VN;
    let items = bo_sheet::slicer::items(&wb, 0, i, &locale);
    let labels: Vec<&str> = items.iter().map(|x| x.label.as_str()).collect();
    assert_eq!(labels, ["Bắc", "Nam", "Trung"]);
    assert!(items.iter().all(|x| x.selected));
    // Keep "Bắc": rows 3 and 5 (Nam, Trung) are hidden.
    bo_sheet::slicer::select(
        &mut wb,
        0,
        i,
        &["Bắc".into()],
        &locale,
        &PivotLabels::default(),
    )
    .unwrap();
    let hidden = |wb: &Workbook, r: u32| wb.sheets[0].rows.get(&r).is_some_and(|x| x.hidden);
    assert!(!hidden(&wb, 1) && hidden(&wb, 2) && !hidden(&wb, 3) && hidden(&wb, 4));
    let items = bo_sheet::slicer::items(&wb, 0, i, &locale);
    assert_eq!(
        items.iter().map(|x| x.selected).collect::<Vec<_>>(),
        [true, false, false]
    );
    // A second slicer on the same table combines with the first: Bắc and 10 → row 2 only.
    let j = bo_sheet::slicer::insert_table_slicer(&mut wb, 0, 1, 2, CellRef::new(0, 6)).unwrap();
    bo_sheet::slicer::select(
        &mut wb,
        0,
        j,
        &["10".into()],
        &locale,
        &PivotLabels::default(),
    )
    .unwrap();
    assert!(!hidden(&wb, 1) && hidden(&wb, 2) && hidden(&wb, 3) && hidden(&wb, 4));
    let af = wb.sheets[0].tables[0].auto_filter.clone().unwrap();
    assert!(
        af.contains("colId=\"0\"") && af.contains("colId=\"1\""),
        "{af}"
    );
    // Clearing the second slicer leaves the first one's filter.
    bo_sheet::slicer::select(&mut wb, 0, j, &[], &locale, &PivotLabels::default()).unwrap();
    assert!(!hidden(&wb, 1) && hidden(&wb, 2) && !hidden(&wb, 3) && hidden(&wb, 4));
    wb.sheets[0].slicers.remove(j);
    // Written with its parts and read back with the same selection.
    let bytes = write_xlsx(&wb).unwrap();
    if let Ok(dir) = std::env::var("BO_SLICER_OUT") {
        std::fs::write(format!("{dir}/table-slicer.xlsx"), &bytes).unwrap();
    }
    let back = read_xlsx(&bytes).unwrap();
    let s = &back.sheets[0].slicers;
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].caption, "Vùng");
    assert_eq!(s[0].cache, "Slicer_Vùng");
    assert_eq!(s[0].from, CellRef::new(0, 3));
    match &s[0].source {
        SlicerSource::Table {
            table_id,
            column,
            shown,
        } => {
            assert_eq!((*table_id, *column), (1, 1));
            assert_eq!(shown.as_deref(), Some(&["Bắc".to_owned()][..]));
        }
        other @ SlicerSource::Pivot { .. } => panic!("{other:?}"),
    }
    assert!(back.defined_names.iter().any(|d| d.name == "Slicer_Vùng"));
    // Clearing the filter shows every row again.
    let mut back = back;
    bo_sheet::slicer::select(&mut back, 0, 0, &[], &locale, &PivotLabels::default()).unwrap();
    assert!((0..5).all(|r| !hidden(&back, r)));
    // A second write keeps a single slicer.
    let again = read_xlsx(&write_xlsx(&back).unwrap()).unwrap();
    assert_eq!(again.sheets[0].slicers.len(), 1);
}

/// A pivot slicer on field "Vùng" of the corpus pivot table: its items are the cache field's
/// items; choosing one hides the others in the pivot table, and the selection is kept.
#[test]
fn pivot_slicer_hides_items_in_the_pivot() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/sheet/writer-pivots-1.xlsx");
    let mut wb = read_xlsx(&std::fs::read(path).unwrap()).unwrap();
    let (si, pivot) = wb
        .sheets
        .iter()
        .enumerate()
        .find_map(|(i, s)| s.pivots.first().map(|p| (i, p.clone())))
        .unwrap();
    wb.sheets[si].slicers.push(bo_model::sheet::Slicer {
        name: "Vùng".into(),
        caption: "Vùng".into(),
        cache: "Slicer_Vùng".into(),
        source: SlicerSource::Pivot {
            field: "Vùng".into(),
            pivots: vec![pivot.name.clone()],
            cache_id: 99,
            items: Vec::new(),
        },
        columns: 1,
        ..bo_model::sheet::Slicer::default()
    });
    let locale = Locale::VI_VN;
    let items = bo_sheet::slicer::items(&wb, si, 0, &locale);
    assert_eq!(
        items.iter().map(|i| i.label.as_str()).collect::<Vec<_>>(),
        ["Bắc", "Nam", "Trung"]
    );
    bo_sheet::slicer::select(
        &mut wb,
        si,
        0,
        &["Nam".into()],
        &locale,
        &PivotLabels::default(),
    )
    .unwrap();
    let p = &wb.sheets[si].pivots[0];
    let field = wb
        .pivot_caches
        .iter()
        .find(|c| c.cache_id == p.cache_id)
        .unwrap()
        .fields
        .iter()
        .position(|f| f.name == "Vùng")
        .unwrap();
    let hidden: Vec<String> = p.fields[field]
        .hidden
        .iter()
        .map(bo_sheet::pivot::value_text)
        .collect();
    assert_eq!(hidden, ["Bắc", "Trung"]);
    let items = bo_sheet::slicer::items(&wb, si, 0, &locale);
    assert_eq!(
        items.iter().map(|i| i.selected).collect::<Vec<_>>(),
        [false, true, false]
    );
    // The pivot table now shows only Nam: no cell of the sheet reads "Bắc" in the pivot area.
    let area = p.location;
    let shows = |label: &str| {
        wb.sheets[si].cells.iter().any(|((r, c), cell)| {
            (area.first.row..=area.last.row + 10).contains(r)
                && (area.first.col..=area.last.col).contains(c)
                && matches!(&cell.value, CellValue::Text(t) if t == label)
        })
    };
    assert!(shows("Nam"));
    assert!(!shows("Bắc"));
}

fn part_text(bytes: &[u8], name: &str) -> Option<String> {
    let pkg = bo_opc::Package::read(bytes, &bo_opc::Limits::default()).unwrap();
    pkg.part(name)
        .map(|p| String::from_utf8(p.data.clone()).unwrap())
}

fn part_names(bytes: &[u8]) -> Vec<String> {
    let pkg = bo_opc::Package::read(bytes, &bo_opc::Limits::default()).unwrap();
    pkg.parts().iter().map(|p| p.name.clone()).collect()
}

/// A slicer inserted over a field of a pivot table read from a file: the slicer cache names
/// the pivot cache by the `x14:pivotCacheDefinition` id added to the cache definition, lists
/// the pivot table and the items; the choice, the options and the deletion survive saving.
#[test]
fn pivot_slicer_is_created_saved_edited_and_deleted() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/sheet/writer-pivots-1.xlsx");
    let original = std::fs::read(path).unwrap();
    let mut wb = read_xlsx(&original).unwrap();
    let labels = PivotLabels::default();
    let locale = Locale::VI_VN;
    let (si, pivot) = wb
        .sheets
        .iter()
        .enumerate()
        .find_map(|(i, s)| s.pivots.first().map(|p| (i, p.clone())))
        .unwrap();
    let cache = wb
        .pivot_caches
        .iter()
        .find(|c| c.cache_id == pivot.cache_id)
        .unwrap();
    assert_eq!(cache.ext_id, None);
    let cache_part = cache.part.clone();
    let field = cache.fields.iter().position(|f| f.name == "Vùng").unwrap() as u32;
    let i = bo_sheet::slicer::insert_pivot_slicer(
        &mut wb,
        si,
        si,
        0,
        field,
        CellRef::new(1, 6),
        &labels,
    )
    .unwrap();
    // The cache part is kept as read and only gets the extension.
    let bytes = write_xlsx(&wb).unwrap();
    let def = part_text(&bytes, &cache_part).unwrap();
    assert!(
        def.contains(r#"<x14:pivotCacheDefinition pivotCacheId="1"/>"#),
        "{def}"
    );
    assert_eq!(def.matches("<extLst>").count(), 1);
    let names = part_names(&bytes);
    let slicer_cache = names
        .iter()
        .find(|n| n.starts_with("/xl/slicerCaches/"))
        .unwrap();
    let xml = part_text(&bytes, slicer_cache).unwrap();
    let sheet_id = wb.sheets[si].sheet_id;
    assert!(
        xml.contains(&format!(
            r#"<pivotTables><pivotTable tabId="{sheet_id}" name="{}"/></pivotTables>"#,
            pivot.name
        )),
        "{xml}"
    );
    assert!(
        xml.contains(
            r#"<data><tabular pivotCacheId="1"><items count="3"><i x="0" s="1"/><i x="1" s="1"/><i x="2" s="1"/></items></tabular></data>"#
        ),
        "{xml}"
    );
    let workbook = part_text(&bytes, "/xl/workbook.xml").unwrap();
    assert!(workbook.contains("{BBE1A952-AA13-448e-AADC-164F8A28A991}"));
    assert!(workbook.contains("<x14:slicerCaches><x14:slicerCache r:id="));
    let back = read_xlsx(&bytes).unwrap();
    let s = &back.sheets[si].slicers[i];
    assert_eq!((s.name.as_str(), s.cache.as_str()), ("Vùng", "Slicer_Vùng"));
    assert_eq!(s.from, CellRef::new(1, 6));
    assert_eq!(s.style.as_deref(), Some("SlicerStyleLight1"));
    match &s.source {
        SlicerSource::Pivot {
            field,
            pivots,
            cache_id,
            items,
        } => {
            assert_eq!(field, "Vùng");
            assert_eq!(pivots.as_slice(), std::slice::from_ref(&pivot.name));
            assert_eq!(*cache_id, 1);
            assert_eq!(items, &[(0, true), (1, true), (2, true)]);
        }
        other @ SlicerSource::Table { .. } => panic!("{other:?}"),
    }
    let back_cache = back
        .pivot_caches
        .iter()
        .find(|c| c.cache_id == pivot.cache_id)
        .unwrap();
    assert_eq!(back_cache.ext_id, Some(1));
    assert!(back.defined_names.iter().any(|d| d.name == "Slicer_Vùng"));

    // Choosing "Nam" in the file slicer refreshes the cache (now written whole, with the
    // extension) and stores the choice in the slicer cache.
    let mut back = back;
    bo_sheet::slicer::select(&mut back, si, i, &["Nam".into()], &locale, &labels).unwrap();
    let bytes = write_xlsx(&back).unwrap();
    let def = part_text(&bytes, &cache_part).unwrap();
    assert!(def.contains(r#"<x14:pivotCacheDefinition pivotCacheId="1"/>"#));
    let mut again = read_xlsx(&bytes).unwrap();
    let chosen: Vec<(String, bool)> = bo_sheet::slicer::items(&again, si, i, &locale)
        .into_iter()
        .map(|x| (x.label, x.selected))
        .collect();
    assert_eq!(
        chosen,
        [
            ("Bắc".to_owned(), false),
            ("Nam".to_owned(), true),
            ("Trung".to_owned(), false)
        ]
    );

    // Caption, columns and style of the slicer read from the file.
    bo_sheet::slicer::set_options(
        &mut again,
        si,
        i,
        Some("Khu vực & miền".into()),
        Some(2),
        Some("SlicerStyleDark2".into()),
    )
    .unwrap();
    bo_xlsx::slicer::patch_slicer(&mut again, si, i);
    let bytes = write_xlsx(&again).unwrap();
    // `BO_SLICER_OUT=<dir>` keeps the files for the OpenXmlValidator (`bo-cli validate`).
    if let Ok(dir) = std::env::var("BO_SLICER_OUT") {
        std::fs::write(format!("{dir}/pivot-slicer-edited.xlsx"), &bytes).unwrap();
    }
    let mut third = read_xlsx(&bytes).unwrap();
    let s = &third.sheets[si].slicers[i];
    assert_eq!(s.caption, "Khu vực & miền");
    assert_eq!(s.columns, 2);
    assert_eq!(s.style.as_deref(), Some("SlicerStyleDark2"));
    assert_eq!(s.name, "Vùng");

    // Deleting it removes its frame, both parts, their relationships and extension entries.
    assert!(bo_xlsx::slicer::remove_slicer(&mut third, si, i));
    let bytes = write_xlsx(&third).unwrap();
    if let Ok(dir) = std::env::var("BO_SLICER_OUT") {
        std::fs::write(format!("{dir}/pivot-slicer-deleted.xlsx"), &bytes).unwrap();
    }
    let names = part_names(&bytes);
    assert!(
        !names.iter().any(|n| n.to_lowercase().contains("slicer")),
        "{names:?}"
    );
    let workbook = part_text(&bytes, "/xl/workbook.xml").unwrap();
    assert!(!workbook.contains("slicerCache"), "{workbook}");
    assert!(!workbook.contains("Slicer_"), "{workbook}");
    for n in names.iter().filter(|n| n.starts_with("/xl/worksheets/")) {
        let text = part_text(&bytes, n).unwrap();
        assert!(!text.contains("slicerList"), "{n}: {text}");
    }
    for n in names.iter().filter(|n| n.starts_with("/xl/drawings/")) {
        let text = part_text(&bytes, n).unwrap();
        assert!(!text.contains("slicer"), "{n}: {text}");
    }
    let last = read_xlsx(&bytes).unwrap();
    assert!(last.sheets.iter().all(|s| s.slicers.is_empty()));
    // The pivot table itself is untouched by the deletion.
    assert_eq!(last.sheets[si].pivots.len(), 1);
    assert!(read_xlsx(&write_xlsx(&last).unwrap()).is_ok());
}

/// Two slicers on one sheet, one deleted: the slicers part stays with the other one; a second
/// new slicer goes into the list of the sheet's existing extension.
#[test]
fn deleting_one_of_two_file_slicers_keeps_the_other() {
    let mut wb = book();
    bo_sheet::slicer::insert_table_slicer(&mut wb, 0, 1, 1, CellRef::new(0, 3)).unwrap();
    bo_sheet::slicer::insert_table_slicer(&mut wb, 0, 1, 2, CellRef::new(0, 6)).unwrap();
    let mut back = read_xlsx(&write_xlsx(&wb).unwrap()).unwrap();
    assert_eq!(back.sheets[0].slicers.len(), 2);
    assert!(bo_xlsx::slicer::remove_slicer(&mut back, 0, 0));
    let bytes = write_xlsx(&back).unwrap();
    let names = part_names(&bytes);
    assert_eq!(
        names
            .iter()
            .filter(|n| n.starts_with("/xl/slicerCaches/") && n.ends_with(".xml"))
            .count(),
        1
    );
    assert_eq!(
        names
            .iter()
            .filter(|n| n.starts_with("/xl/slicers/") && n.ends_with(".xml"))
            .count(),
        1
    );
    let mut again = read_xlsx(&bytes).unwrap();
    assert_eq!(again.sheets[0].slicers.len(), 1);
    assert_eq!(again.sheets[0].slicers[0].caption, "Doanh số");
    assert!(!again.defined_names.iter().any(|d| d.name == "Slicer_Vùng"));
    // A new slicer next to the file one: one extension of the sheet lists both parts.
    bo_sheet::slicer::insert_table_slicer(&mut again, 0, 1, 1, CellRef::new(0, 9)).unwrap();
    let bytes = write_xlsx(&again).unwrap();
    if let Ok(dir) = std::env::var("BO_SLICER_OUT") {
        std::fs::write(format!("{dir}/table-slicers-file-and-new.xlsx"), &bytes).unwrap();
    }
    let sheet = part_text(&bytes, "/xl/worksheets/sheet1.xml").unwrap();
    assert_eq!(
        sheet
            .matches("{3A4CF648-6AED-40f4-86FF-DC5316D8AED3}")
            .count(),
        1
    );
    assert_eq!(sheet.matches("<x14:slicer r:id=").count(), 2);
    let workbook = part_text(&bytes, "/xl/workbook.xml").unwrap();
    assert_eq!(
        workbook
            .matches("{46F421CA-312F-682f-3DD2-61675219B42D}")
            .count(),
        1
    );
    assert_eq!(workbook.matches("<x14:slicerCache r:id=").count(), 2);
    assert_eq!(read_xlsx(&bytes).unwrap().sheets[0].slicers.len(), 2);
}
