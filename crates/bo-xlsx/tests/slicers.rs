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
