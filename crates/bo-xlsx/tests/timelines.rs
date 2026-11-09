//! Timelines ([MS-XLSX] timeline parts) over the corpus pivot table with a date field: the
//! parts are added by hand (x15 timelineCacheDefinition with a selection, a timelines part and
//! the relationships), read into the model, filtered, saved and read back.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_model::sheet::PivotFilterType;
use bo_opc::{Limits, Package, Relationship, TargetMode};
use bo_sheet::pivot::PivotLabels;
use bo_xlsx::{read_xlsx, write_xlsx};

const X15: &str = "http://schemas.microsoft.com/office/spreadsheetml/2010/11/main";

fn with_timeline() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/sheet/writer-pivot-dates-1.xlsx");
    let bytes = std::fs::read(path).unwrap();
    let wb = read_xlsx(&bytes).unwrap();
    let pivot = wb.sheets[1].pivots[0].name.clone();
    let mut pkg = Package::read(&bytes, &Limits::default()).unwrap();
    let cache = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<timelineCacheDefinition xmlns="{X15}" name="NativeTimeline_Ngay" sourceName="Ngày"><pivotTables><pivotTable tabId="2" name="{pivot}"/></pivotTables><state minimalRefreshVersion="6" lastRefreshVersion="6" pivotCacheId="1" filterType="dateBetween"><selection startDate="2023-04-01T00:00:00" endDate="2023-06-30T00:00:00"/><bounds startDate="2023-01-01T00:00:00" endDate="2025-01-01T00:00:00"/></state></timelineCacheDefinition>"#
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
    let mut wrels = pkg.relationships("/xl/workbook.xml").unwrap();
    wrels.push(Relationship {
        id: "rId90".into(),
        rel_type: bo_xlsx::timeline::REL_TIMELINE_CACHE.into(),
        target: "timelineCaches/timelineCache1.xml".into(),
        mode: TargetMode::Internal,
    });
    pkg.set_relationships("/xl/workbook.xml", &wrels);
    let mut srels = pkg.relationships("/xl/worksheets/sheet2.xml").unwrap();
    srels.push(Relationship {
        id: "rId90".into(),
        rel_type: bo_xlsx::timeline::REL_TIMELINE.into(),
        target: "../timelines/timeline1.xml".into(),
        mode: TargetMode::Internal,
    });
    pkg.set_relationships("/xl/worksheets/sheet2.xml", &srels);
    pkg.write().unwrap()
}

#[test]
fn timeline_reads_filters_and_saves() {
    let mut wb = read_xlsx(&with_timeline()).unwrap();
    let t = &wb.sheets[1].timelines;
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].caption, "Ngày");
    assert_eq!(t[0].field, "Ngày");
    assert_eq!(t[0].level, 1);
    assert_eq!(t[0].start.as_deref(), Some("2023-04-01T00:00:00"));
    // Quarters from the first to the last date (45000 = 2023-03-15 … 45667 = 2025-01-10):
    // only Q2 2023 is inside the stored selection.
    let p = bo_sheet::timeline::periods(&wb, 1, 0, true);
    assert_eq!(p.first().unwrap().label, "Quý 1 2023");
    assert_eq!(p.last().unwrap().label, "Quý 1 2025");
    let chosen: Vec<&str> = p
        .iter()
        .filter(|x| x.selected)
        .map(|x| x.label.as_str())
        .collect();
    assert_eq!(chosen, ["Quý 2 2023"]);
    // Filter to 2024: the pivot table gets a dateBetween filter on the field.
    let y2024: Vec<_> = p.iter().filter(|x| x.label.ends_with("2024")).collect();
    let range = (y2024.first().unwrap().start, y2024.last().unwrap().end);
    bo_sheet::timeline::select(&mut wb, 1, 0, Some(range), &PivotLabels::default()).unwrap();
    let f = &wb.sheets[1].pivots[0].field_filters;
    assert!(f.iter().any(|f| f.kind == PivotFilterType::DateBetween));
    assert_eq!(
        wb.sheets[1].timelines[0].start.as_deref(),
        Some("2024-01-01T00:00:00")
    );
    assert_eq!(
        wb.sheets[1].timelines[0].end.as_deref(),
        Some("2024-12-31T00:00:00")
    );
    // Saved: the cache's selection is rewritten and read back.
    let saved = write_xlsx(&wb).unwrap();
    if let Ok(dir) = std::env::var("BO_SLICER_OUT") {
        std::fs::write(format!("{dir}/timeline.xlsx"), &saved).unwrap();
    }
    let back = read_xlsx(&saved).unwrap();
    let t = &back.sheets[1].timelines[0];
    assert_eq!(t.start.as_deref(), Some("2024-01-01T00:00:00"));
    // Clearing removes the filter and the selection.
    let mut back = back;
    bo_sheet::timeline::select(&mut back, 1, 0, None, &PivotLabels::default()).unwrap();
    assert!(back.sheets[1].pivots[0]
        .field_filters
        .iter()
        .all(|f| f.kind != PivotFilterType::DateBetween));
    let again = read_xlsx(&write_xlsx(&back).unwrap()).unwrap();
    assert_eq!(again.sheets[1].timelines[0].start, None);
}
